//! The narrow printf family, formatted as Microsoft's C runtime does. Every argument takes one 8-byte
//! slot of the Windows x64 va_list. What a conversion reads from its slot follows Windows' sizes,
//! where `long` is 32 bits: `%u`, `%lu` and `%I32u` read the low 32 bits, and only `ll`, `I64`, `I`,
//! `z`, `j` and `t` read all 64, so the upper half of a slot holding an `int` is never printed.
//! Integers and finite floating-point numbers are then written by glibc's snprintf, which follows
//! the same rules for them; infinities and NaNs are spelt as Microsoft's runtime spells them.

use std::ffi::{CStr, c_char};

#[derive(Clone, Copy, PartialEq)]
enum Size {
    Char,
    Short,
    Int,
    Int64,
    // `%ls`, `%lc`, `%ws`, `%wc`, `%S` and `%C`: UTF-16 text
    Wide,
    // `%hs` and `%hc`: narrow text, whatever the function
    Narrow,
    Default,
}

struct Spec {
    flags: Vec<u8>,
    width: Option<usize>,
    precision: Option<usize>,
}

impl Spec {
    fn left(&self) -> bool {
        self.flags.contains(&b'-')
    }

    // The specification glibc's snprintf is given: the flags, width and precision as parsed, the
    // length the value is passed at, and the conversion
    fn c_format(&self, length: &str, conversion: u8) -> Vec<u8> {
        let mut spec = vec![b'%'];
        spec.extend_from_slice(&self.flags);
        if let Some(width) = self.width {
            spec.extend_from_slice(width.to_string().as_bytes());
        }
        if let Some(precision) = self.precision {
            spec.push(b'.');
            spec.extend_from_slice(precision.to_string().as_bytes());
        }
        spec.extend_from_slice(length.as_bytes());
        spec.push(conversion);
        spec.push(0);
        spec
    }

    // Pads text to the width with spaces, on the left unless the `-` flag is set
    fn pad(&self, text: &[u8], out: &mut Vec<u8>) {
        let fill = self.width.unwrap_or(0).saturating_sub(text.len());
        if !self.left() {
            out.resize(out.len() + fill, b' ');
        }
        out.extend_from_slice(text);
        if self.left() {
            out.resize(out.len() + fill, b' ');
        }
    }
}

// Runs glibc's snprintf with one argument, into a buffer that is grown until the text fits
fn c_snprintf(spec: &[u8], write: impl Fn(*mut c_char, usize, *const c_char) -> i32) -> Vec<u8> {
    let mut buffer = vec![0u8; 128];
    loop {
        let n = write(
            buffer.as_mut_ptr() as *mut c_char,
            buffer.len(),
            spec.as_ptr() as *const c_char,
        );
        if n < 0 {
            return Vec::new();
        }
        if (n as usize) < buffer.len() {
            buffer.truncate(n as usize);
            return buffer;
        }
        buffer.resize(n as usize + 1, 0);
    }
}

// Microsoft's spelling of an infinity or NaN: `inf`, `nan`, `-nan(ind)` for the indefinite NaN that
// invalid operations produce, and `nan(snan)` for a signalling NaN; uppercase for E, F, G and A
fn non_finite(value: f64, spec: &Spec, conversion: u8) -> Vec<u8> {
    let bits = value.to_bits();
    let negative = bits >> 63 != 0;
    let mut text = Vec::new();
    if negative {
        text.push(b'-');
    } else if spec.flags.contains(&b'+') {
        text.push(b'+');
    } else if spec.flags.contains(&b' ') {
        text.push(b' ');
    }
    if value.is_infinite() {
        text.extend_from_slice(b"inf");
    } else if bits & (1 << 51) == 0 {
        text.extend_from_slice(b"nan(snan)");
    } else if negative && bits & ((1 << 51) - 1) == 0 {
        text.extend_from_slice(b"nan(ind)");
    } else {
        text.extend_from_slice(b"nan");
    }
    if conversion.is_ascii_uppercase() {
        text.make_ascii_uppercase();
    }
    text
}

// A UTF-16 string or character in narrow text: the C locale has only ASCII, and anything else
// becomes `?`
fn narrow(units: impl Iterator<Item = u16>) -> Vec<u8> {
    units
        .map(|u| if u < 0x80 { u as u8 } else { b'?' })
        .collect()
}

/// Formats a printf format string and its Windows x64 va_list into the full text it describes.
///
/// # Safety
/// `format` must be a null-terminated string, and `args` must point to the 8-byte slots of the
/// arguments its conversions read, with any strings they point to valid.
pub unsafe fn format(format: *const c_char, args: *const u64) -> Vec<u8> {
    let format = unsafe { CStr::from_ptr(format) }.to_bytes();
    let mut args = args;
    let mut next = || {
        let value = unsafe { *args };
        args = unsafe { args.add(1) };
        value
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < format.len() {
        if format[i] != b'%' {
            out.push(format[i]);
            i += 1;
            continue;
        }
        i += 1;
        let mut spec = Spec {
            flags: Vec::new(),
            width: None,
            precision: None,
        };
        while let Some(&flag @ (b'-' | b'+' | b' ' | b'#' | b'0')) = format.get(i) {
            if !spec.flags.contains(&flag) {
                spec.flags.push(flag);
            }
            i += 1;
        }
        if format.get(i) == Some(&b'*') {
            // a negative width is the `-` flag and its absolute value
            let width = next() as i32;
            if width < 0 && !spec.left() {
                spec.flags.push(b'-');
            }
            spec.width = Some(width.unsigned_abs() as usize);
            i += 1;
        } else {
            let start = i;
            while format.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
            if i > start {
                spec.width = std::str::from_utf8(&format[start..i])
                    .ok()
                    .and_then(|w| w.parse().ok());
            }
        }
        if format.get(i) == Some(&b'.') {
            i += 1;
            if format.get(i) == Some(&b'*') {
                // a negative precision is as if none were given
                let precision = next() as i32;
                spec.precision = (precision >= 0).then_some(precision as usize);
                i += 1;
            } else {
                let start = i;
                while format.get(i).is_some_and(u8::is_ascii_digit) {
                    i += 1;
                }
                spec.precision = Some(
                    std::str::from_utf8(&format[start..i])
                        .ok()
                        .and_then(|p| p.parse().ok())
                        .unwrap_or(0),
                );
            }
        }
        let rest = &format[i..];
        let (size, skip) = match rest {
            [b'h', b'h', ..] => (Size::Char, 2),
            [b'h', ..] => (Size::Short, 1),
            [b'l', b'l', ..] => (Size::Int64, 2),
            [b'l', ..] => (Size::Int, 1),
            [b'I', b'6', b'4', ..] => (Size::Int64, 3),
            [b'I', b'3', b'2', ..] => (Size::Int, 3),
            [b'I', ..] | [b'z', ..] | [b'j', ..] | [b't', ..] => (Size::Int64, 1),
            [b'w', ..] => (Size::Wide, 1),
            // long double is double on Windows
            [b'L', ..] => (Size::Default, 1),
            _ => (Size::Default, 0),
        };
        i += skip;
        let Some(&conversion) = format.get(i) else {
            break;
        };
        i += 1;
        // `l` before c or s means wide text, and `h` narrow, rather than a size
        let size = match (rest.first(), conversion) {
            (Some(b'l'), b'c' | b's') if skip == 1 => Size::Wide,
            (Some(b'h'), b'c' | b's' | b'C' | b'S') if skip == 1 => Size::Narrow,
            _ => size,
        };
        match conversion {
            b'%' => out.push(b'%'),
            b'd' | b'i' => {
                let slot = next();
                let value = match size {
                    Size::Char => slot as i8 as i64,
                    Size::Short => slot as i16 as i64,
                    Size::Int64 => slot as i64,
                    _ => slot as i32 as i64,
                };
                let spec = spec.c_format("ll", conversion);
                out.extend(c_snprintf(&spec, |b, n, f| unsafe {
                    libc::snprintf(b, n, f, value)
                }));
            }
            b'u' | b'o' | b'x' | b'X' => {
                let slot = next();
                let value = match size {
                    Size::Char => slot as u8 as u64,
                    Size::Short => slot as u16 as u64,
                    Size::Int64 => slot,
                    _ => slot as u32 as u64,
                };
                let spec = spec.c_format("ll", conversion);
                out.extend(c_snprintf(&spec, |b, n, f| unsafe {
                    libc::snprintf(b, n, f, value)
                }));
            }
            b'e' | b'E' | b'f' | b'F' | b'g' | b'G' | b'a' | b'A' => {
                let value = f64::from_bits(next());
                if value.is_finite() {
                    // Microsoft's %a writes every hexadecimal digit of the fraction unless asked for fewer
                    if matches!(conversion, b'a' | b'A') && spec.precision.is_none() {
                        spec.precision = Some(13);
                    }
                    let spec = spec.c_format("", conversion);
                    out.extend(c_snprintf(&spec, |b, n, f| unsafe {
                        libc::snprintf(b, n, f, value)
                    }));
                } else {
                    let text = non_finite(value, &spec, conversion);
                    spec.pad(&text, &mut out);
                }
            }
            b'c' | b'C' => {
                let slot = next();
                let wide = size == Size::Wide || (conversion == b'C' && size != Size::Narrow);
                let text = if wide {
                    narrow(std::iter::once(slot as u16))
                } else {
                    vec![slot as u8]
                };
                spec.pad(&text, &mut out);
            }
            b's' | b'S' => {
                let pointer = next() as usize;
                let wide = size == Size::Wide || (conversion == b'S' && size != Size::Narrow);
                let limit = spec.precision.unwrap_or(usize::MAX);
                let text = if pointer == 0 {
                    b"(null)"[..limit.min(6)].to_vec()
                } else if wide {
                    let units = (0..).map(|k| unsafe { *(pointer as *const u16).add(k) });
                    narrow(units.take_while(|&u| u != 0).take(limit))
                } else {
                    let bytes = (0..).map(|k| unsafe { *(pointer as *const u8).add(k) });
                    bytes.take_while(|&b| b != 0).take(limit).collect()
                };
                spec.pad(&text, &mut out);
            }
            b'p' => {
                let text = format!("{:016X}", next());
                spec.pad(text.as_bytes(), &mut out);
            }
            // %n is disabled in Microsoft's runtime; its argument is passed over
            b'n' => {
                next();
            }
            _ => {}
        }
    }
    out
}

/// `_vsnprintf`: text that fits is written with its terminator and its length returned; text that
/// fills the buffer exactly is written without a terminator and its length returned; longer text
/// fills the buffer, without a terminator, and -1 is returned.
///
/// # Safety
/// As `format`, and `buffer` must hold `count` bytes.
pub unsafe fn vsnprintf_core(
    buffer: *mut c_char,
    count: usize,
    format: *const c_char,
    args: *const u64,
) -> i32 {
    let text = unsafe { self::format(format, args) };
    if buffer.is_null() {
        return -1;
    }
    let n = text.len().min(count);
    unsafe { std::ptr::copy_nonoverlapping(text.as_ptr(), buffer as *mut u8, n) };
    if text.len() < count {
        unsafe { *buffer.add(text.len()) = 0 };
    }
    if text.len() <= count {
        text.len() as i32
    } else {
        -1
    }
}

#[cfg(test)]
mod tests {
    use super::format;

    fn run(f: &str, args: &[u64]) -> String {
        let f = std::ffi::CString::new(f).unwrap();
        String::from_utf8(unsafe { format(f.as_ptr(), args.as_ptr()) }).unwrap()
    }

    // An int in a slot whose upper half holds whatever was on the stack
    const DIRTY: u64 = 0x7ffc_0000_0000_0000;

    #[test]
    fn fxc_message() {
        assert_eq!(
            run(
                "%s(%u,%u-%u): warning X%04u: %s",
                &[
                    c"a.usf".as_ptr() as u64,
                    DIRTY | 32,
                    DIRTY | 9,
                    DIRTY | 14,
                    DIRTY | 3557,
                    c"loop".as_ptr() as u64,
                ]
            ),
            "a.usf(32,9-14): warning X3557: loop"
        );
    }

    #[test]
    fn integer_sizes() {
        assert_eq!(
            run("%d %ld %I32d", &[DIRTY | 0xffff_ffff, DIRTY | 5, DIRTY | 6]),
            "-1 5 6"
        );
        assert_eq!(
            run("%lld %I64u %zu", &[u64::MAX, 1 << 40, 7]),
            "-1 1099511627776 7"
        );
        assert_eq!(
            run("%hd %hhu %hx", &[0x1_ffff, 0x1ff, 0xabcd_1234]),
            "-1 255 1234"
        );
    }

    #[test]
    fn flags_width_precision() {
        assert_eq!(
            run("[%5d|%-5d|%05d|%+d|% d]", &[42, 42, 42, 42, 42]),
            "[   42|42   |00042|+42| 42]"
        );
        assert_eq!(
            run(
                "[%#x|%#o|%.3d|%*d|%-*d]",
                &[255, 8, 7, 4, 1, (-3i32) as u64, 2]
            ),
            "[0xff|010|007|   1|2  ]"
        );
        assert_eq!(
            run(
                "[%.2s|%5s|%-5s]",
                &[
                    c"abc".as_ptr() as u64,
                    c"ab".as_ptr() as u64,
                    c"ab".as_ptr() as u64
                ]
            ),
            "[ab|   ab|ab   ]"
        );
    }

    #[test]
    fn floats() {
        let f = |x: f64| x.to_bits();
        assert_eq!(
            run(
                "%f %.2f %e %g %G",
                &[f(1.5), f(2.0), f(1234.5), f(0.0001), f(1e20)]
            ),
            "1.500000 2.00 1.234500e+03 0.0001 1E+20"
        );
        assert_eq!(
            run("%a %.1a", &[f(1.0), f(1.0)]),
            "0x1.0000000000000p+0 0x1.0p+0"
        );
        assert_eq!(
            run(
                "%f %f %F %f %f %5f",
                &[
                    f(f64::INFINITY),
                    f(f64::NEG_INFINITY),
                    f(f64::INFINITY),
                    0xfff8_0000_0000_0000,
                    0x7ff8_0000_0000_0000,
                    f(f64::INFINITY)
                ]
            ),
            "inf -inf INF -nan(ind) nan   inf"
        );
    }

    #[test]
    fn text_and_pointers() {
        let wide: Vec<u16> = "hi\u{e9}\0".encode_utf16().collect();
        assert_eq!(
            run(
                "%c%3c|%lc|%ls|%S|%s",
                &[
                    b'a' as u64,
                    b'b' as u64,
                    b'w' as u64,
                    wide.as_ptr() as u64,
                    wide.as_ptr() as u64,
                    0
                ]
            ),
            "a  b|w|hi?|hi?|(null)"
        );
        assert_eq!(run("%p %%", &[0xabc]), "0000000000000ABC %");
    }
}
