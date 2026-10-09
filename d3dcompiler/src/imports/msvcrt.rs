use super::*;

// The C runtime's character classes for the "C" locale, as Microsoft's runtime returns them:
// _UPPER 0x1, _LOWER 0x2, _DIGIT 0x4, _SPACE 0x8, _PUNCT 0x10, _CONTROL 0x20, _BLANK 0x40, _HEX 0x80,
// _ALPHA 0x100. The is* functions return the class bits they test, not just non-zero, and callers
// can keep only the low byte, so glibc's values, which use other bits, read as false there.
fn ctype(c: i32) -> i32 {
    match c {
        0x41..=0x46 => 0x181,
        0x47..=0x5A => 0x101,
        0x61..=0x66 => 0x182,
        0x67..=0x7A => 0x102,
        0x30..=0x39 => 0x84,
        0x20 => 0x48,
        0x09 => 0x68,
        0x0A..=0x0D => 0x28,
        0x00..=0x08 | 0x0E..=0x1F | 0x7F => 0x20,
        0x21..=0x2F | 0x3A..=0x40 | 0x5B..=0x60 | 0x7B..=0x7E => 0x10,
        _ => 0,
    }
}

// ============ msvcrt - memory ============

static ERRNO_VAL: AtomicU32 = AtomicU32::new(0);

type Win64Comparator = unsafe extern "win64" fn(*const c_void, *const c_void) -> i32;

unsafe fn swap_elements(a: *mut u8, b: *mut u8, width: usize) {
    if a != b {
        std::ptr::swap_nonoverlapping(a, b, width);
    }
}

// The Microsoft C runtime's qsort: a quicksort on the median of three, with a selection sort below
// nine elements. It is not stable, and the order it leaves equal elements in is its own, which the
// output of a compiler that sorts with it can depend on.
unsafe fn ms_qsort(base: *mut u8, num: usize, width: usize, comp: Win64Comparator) {
    const CUTOFF: usize = 8;
    if num < 2 || width == 0 {
        return;
    }
    let cmp = |a: *mut u8, b: *mut u8| comp(a as *const c_void, b as *const c_void);
    let mut stack: Vec<(*mut u8, *mut u8)> = Vec::new();
    let (mut lo, mut hi) = (base, base.add(width * (num - 1)));
    loop {
        let size = (hi as usize - lo as usize) / width + 1;
        if size <= CUTOFF {
            // repeatedly move the largest remaining element to the end
            let mut end = hi;
            while end > lo {
                let mut max = lo;
                let mut p = lo.add(width);
                while p <= end {
                    if cmp(p, max) > 0 {
                        max = p;
                    }
                    p = p.add(width);
                }
                swap_elements(max, end, width);
                end = end.sub(width);
            }
        } else {
            let mut mid = lo.add((size / 2) * width);
            if cmp(lo, mid) > 0 {
                swap_elements(lo, mid, width);
            }
            if cmp(lo, hi) > 0 {
                swap_elements(lo, hi, width);
            }
            if cmp(mid, hi) > 0 {
                swap_elements(mid, hi, width);
            }
            let (mut loguy, mut higuy) = (lo, hi);
            loop {
                if mid > loguy {
                    loop {
                        loguy = loguy.add(width);
                        if !(loguy < mid && cmp(loguy, mid) <= 0) {
                            break;
                        }
                    }
                }
                if mid <= loguy {
                    loop {
                        loguy = loguy.add(width);
                        if !(loguy <= hi && cmp(loguy, mid) <= 0) {
                            break;
                        }
                    }
                }
                loop {
                    higuy = higuy.sub(width);
                    if !(higuy > mid && cmp(higuy, mid) > 0) {
                        break;
                    }
                }
                if higuy < loguy {
                    break;
                }
                swap_elements(loguy, higuy, width);
                if mid == higuy {
                    mid = loguy;
                }
            }
            higuy = higuy.add(width);
            if mid < higuy {
                loop {
                    higuy = higuy.sub(width);
                    if !(higuy > mid && cmp(higuy, mid) == 0) {
                        break;
                    }
                }
            }
            if mid >= higuy {
                loop {
                    higuy = higuy.sub(width);
                    if !(higuy > lo && cmp(higuy, mid) == 0) {
                        break;
                    }
                }
            }
            // sort the smaller part next and keep the larger for later
            if higuy as usize - lo as usize >= hi as usize - loguy as usize {
                if lo < higuy {
                    stack.push((lo, higuy));
                }
                if loguy < hi {
                    lo = loguy;
                    continue;
                }
            } else {
                if loguy < hi {
                    stack.push((loguy, hi));
                }
                if lo < higuy {
                    hi = higuy;
                    continue;
                }
            }
        }
        match stack.pop() {
            Some((l, h)) => (lo, hi) = (l, h),
            None => return,
        }
    }
}

// The Microsoft C runtime's bsearch, which finds the same one of several equal elements it does
unsafe fn ms_bsearch(
    key: *const c_void,
    base: *const u8,
    mut num: usize,
    width: usize,
    comp: Win64Comparator,
) -> *mut c_void {
    if num == 0 {
        return std::ptr::null_mut();
    }
    let mut lo = base;
    let mut hi = base.add((num - 1) * width);
    while lo <= hi {
        let half = num / 2;
        if half != 0 {
            let mid = lo.add(if num & 1 != 0 { half } else { half - 1 } * width);
            let result = comp(key, mid as *const c_void);
            if result == 0 {
                return mid as *mut c_void;
            } else if result < 0 {
                if mid == base {
                    break;
                }
                hi = mid.sub(width);
                num = if num & 1 != 0 { half } else { half - 1 };
            } else {
                lo = mid.add(width);
                num = half;
            }
        } else if num != 0 {
            return if comp(key, lo as *const c_void) != 0 {
                std::ptr::null_mut()
            } else {
                lo as *mut c_void
            };
        } else {
            break;
        }
    }
    std::ptr::null_mut()
}

import_fn! {
    fn malloc(size: usize) -> *mut c_void {
        trace_call!("msvcrt!malloc", "size={}", size);
        libc::malloc(size)
    }

    fn free(ptr: *mut c_void) {
        trace_call!("msvcrt!free", "ptr={:p}", ptr);
        libc::free(ptr)
    }

    fn op_new(size: usize) -> *mut c_void {
        trace_call!("msvcrt!operator new", "size={}", size);
        libc::malloc(size.max(1))
    }

    fn op_delete(ptr: *mut c_void) {
        trace_call!("msvcrt!operator delete", "ptr={:p}", ptr);
        libc::free(ptr)
    }

    fn op_new_array(size: usize) -> *mut c_void {
        trace_call!("msvcrt!operator new[]", "size={}", size);
        libc::malloc(size.max(1))
    }

    fn op_delete_array(ptr: *mut c_void) {
        trace_call!("msvcrt!operator delete[]", "ptr={:p}", ptr);
        libc::free(ptr)
    }

    fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
        trace_call!("msvcrt!memcpy", "dst={:p}, src={:p}, n={}", dst, src, n);
        libc::memcpy(dst, src, n)
    }

    fn memcpy_s(dst: *mut c_void, dst_size: usize, src: *const c_void, count: usize) -> i32 {
        trace_call!("msvcrt!memcpy_s", "dst={:p}, dst_size={}, src={:p}, count={}", dst, dst_size, src, count);
        if dst.is_null() || src.is_null() || dst_size < count {
            return 22; // EINVAL
        }
        libc::memcpy(dst, src, count);
        0
    }

    fn memmove(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void {
        trace_call!("msvcrt!memmove", "dst={:p}, src={:p}, n={}", dst, src, n);
        libc::memmove(dst, src, n)
    }

    fn memset(ptr: *mut c_void, value: i32, num: usize) -> *mut c_void {
        trace_call!("msvcrt!memset", "ptr={:p}, value={}, num={}", ptr, value, num);
        libc::memset(ptr, value, num)
    }

    fn memcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 {
        trace_call!("msvcrt!memcmp", "s1={:p}, s2={:p}, n={}", s1, s2, n);
        libc::memcmp(s1, s2, n)
    }

    fn _memicmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 {
        trace_call!("msvcrt!_memicmp", "s1={:p}, s2={:p}, n={}", s1, s2, n);
        let s1 = std::slice::from_raw_parts(s1 as *const u8, n);
        let s2 = std::slice::from_raw_parts(s2 as *const u8, n);
        for i in 0..n {
            let c1 = s1[i].to_ascii_lowercase();
            let c2 = s2[i].to_ascii_lowercase();
            if c1 != c2 {
                return c1 as i32 - c2 as i32;
            }
        }
        0
    }

    // ============ msvcrt - string ============

    fn strcmp(s1: *const i8, s2: *const i8) -> i32 {
        trace_call!("msvcrt!strcmp");
        libc::strcmp(s1, s2)
    }

    fn strncmp(s1: *const i8, s2: *const i8, n: usize) -> i32 {
        trace_call!("msvcrt!strncmp");
        libc::strncmp(s1, s2, n)
    }

    fn strcpy_s(dst: *mut i8, dst_size: usize, src: *const i8) -> i32 {
        trace_call!("msvcrt!strcpy_s");
        if dst.is_null() || src.is_null() {
            return 22;
        }
        let len = libc::strlen(src);
        if len >= dst_size {
            return 34; // ERANGE
        }
        libc::strcpy(dst, src);
        0
    }

    fn strncpy_s(
        dst: *mut i8,
        dst_size: usize,
        src: *const i8,
        count: usize,
    ) -> i32 {
        trace_call!("msvcrt!strncpy_s");
        if dst.is_null() || src.is_null() {
            return 22;
        }
        let len = libc::strlen(src).min(count);
        if len >= dst_size {
            return 34;
        }
        libc::strncpy(dst, src, len);
        *dst.add(len) = 0;
        0
    }

    fn strcat_s(dst: *mut i8, dst_size: usize, src: *const i8) -> i32 {
        trace_call!("msvcrt!strcat_s");
        if dst.is_null() || src.is_null() {
            return 22;
        }
        let dst_len = libc::strlen(dst);
        let src_len = libc::strlen(src);
        if dst_len + src_len >= dst_size {
            return 34;
        }
        libc::strcat(dst, src);
        0
    }

    fn strchr(s: *const i8, c: i32) -> *mut i8 {
        trace_call!("msvcrt!strchr");
        libc::strchr(s, c)
    }

    fn strrchr(s: *const i8, c: i32) -> *mut i8 {
        trace_call!("msvcrt!strrchr");
        libc::strrchr(s, c)
    }

    fn strstr(haystack: *const i8, needle: *const i8) -> *mut i8 {
        trace_call!("msvcrt!strstr");
        libc::strstr(haystack, needle)
    }

    fn strlen(s: *const i8) -> usize {
        trace_call!("msvcrt!strlen");
        libc::strlen(s)
    }

    fn strnlen(s: *const i8, max_len: usize) -> usize {
        trace_call!("msvcrt!strnlen");
        libc::strnlen(s, max_len)
    }

    fn _strdup(s: *const i8) -> *mut i8 {
        trace_call!("msvcrt!_strdup");
        libc::strdup(s)
    }

    fn _stricmp(s1: *const i8, s2: *const i8) -> i32 {
        trace_call!("msvcrt!_stricmp");
        libc::strcasecmp(s1, s2)
    }

    fn _strnicmp(s1: *const i8, s2: *const i8, n: usize) -> i32 {
        trace_call!("msvcrt!_strnicmp");
        libc::strncasecmp(s1, s2, n)
    }

    fn tolower(c: i32) -> i32 {
        trace_call!("msvcrt!tolower");
        libc::tolower(c)
    }

    fn toupper(c: i32) -> i32 {
        trace_call!("msvcrt!toupper");
        libc::toupper(c)
    }

    fn towlower(c: u32) -> u32 {
        trace_call!("msvcrt!towlower");
        // Simple ASCII-only lowercase
        if c >= 'A' as u32 && c <= 'Z' as u32 {
            c + 32
        } else {
            c
        }
    }

    fn isalnum(c: i32) -> i32 {
        trace_call!("msvcrt!isalnum");
        ctype(c) & 0x107
    }

    fn isalpha(c: i32) -> i32 {
        let r = ctype(c) & 0x103;
        trace_call!("msvcrt!isalpha", "c=0x{:x} -> 0x{:x}", c, r);
        r
    }

    fn isdigit(c: i32) -> i32 {
        let r = ctype(c) & 0x4;
        trace_call!("msvcrt!isdigit", "c=0x{:x} -> 0x{:x}", c, r);
        r
    }

    fn iswdigit(c: u16) -> i32 {
        trace_call!("msvcrt!iswdigit");
        matches!(c, 0x30..=0x39) as i32
    }

    fn isspace(c: i32) -> i32 {
        trace_call!("msvcrt!isspace");
        ctype(c) & 0x8
    }

    fn isxdigit(c: i32) -> i32 {
        trace_call!("msvcrt!isxdigit");
        ctype(c) & 0x80
    }

    fn __isascii(c: i32) -> i32 {
        trace_call!("msvcrt!__isascii");
        if (0..=127).contains(&c) {
            1
        } else {
            0
        }
    }

    // ============ msvcrt - wide string ============

    fn wcsncmp(s1: *const u16, s2: *const u16, n: usize) -> i32 {
        trace_call!("msvcrt!wcsncmp");
        for i in 0..n {
            let c1 = *s1.add(i);
            let c2 = *s2.add(i);
            if c1 != c2 {
                return c1 as i32 - c2 as i32;
            }
            if c1 == 0 {
                return 0;
            }
        }
        0
    }

    fn wcsncpy_s(
        dst: *mut u16,
        dst_size: usize,
        src: *const u16,
        count: usize,
    ) -> i32 {
        trace_call!("msvcrt!wcsncpy_s");
        if dst.is_null() || src.is_null() {
            return 22;
        }
        let mut len = 0;
        while len < count && *src.add(len) != 0 {
            len += 1;
        }
        if len >= dst_size {
            return 34;
        }
        for i in 0..len {
            *dst.add(i) = *src.add(i);
        }
        *dst.add(len) = 0;
        0
    }

    fn wcsncat_s(
        dst: *mut u16,
        dst_size: usize,
        src: *const u16,
        count: usize,
    ) -> i32 {
        trace_call!("msvcrt!wcsncat_s");
        if dst.is_null() || src.is_null() {
            return 22;
        }
        let mut dst_len = 0;
        while *dst.add(dst_len) != 0 {
            dst_len += 1;
        }
        let mut src_len = 0;
        while src_len < count && *src.add(src_len) != 0 {
            src_len += 1;
        }
        if dst_len + src_len >= dst_size {
            return 34;
        }
        for i in 0..src_len {
            *dst.add(dst_len + i) = *src.add(i);
        }
        *dst.add(dst_len + src_len) = 0;
        0
    }

    fn wcscat_s(dst: *mut u16, dst_size: usize, src: *const u16) -> i32 {
        trace_call!("msvcrt!wcscat_s");
        wcsncat_s(dst, dst_size, src, usize::MAX)
    }

    fn wcscpy_s(dst: *mut u16, dst_size: usize, src: *const u16) -> i32 {
        trace_call!("msvcrt!wcscpy_s");
        if dst.is_null() || src.is_null() {
            return 22;
        }
        let mut len = 0;
        while *src.add(len) != 0 {
            len += 1;
        }
        if len >= dst_size {
            return 34;
        }
        for i in 0..=len {
            *dst.add(i) = *src.add(i);
        }
        0
    }

    fn wcsrchr(s: *const u16, c: u16) -> *mut u16 {
        trace_call!("msvcrt!wcsrchr");
        let mut last = std::ptr::null_mut();
        let mut p = s;
        while *p != 0 {
            if *p == c {
                last = p as *mut u16;
            }
            p = p.add(1);
        }
        last
    }

    fn wcschr(s: *const u16, c: u16) -> *mut u16 {
        trace_call!("msvcrt!wcschr");
        let mut p = s;
        loop {
            if *p == c {
                return p as *mut u16;
            }
            if *p == 0 {
                return std::ptr::null_mut();
            }
            p = p.add(1);
        }
    }

    fn _wcsdup(s: *const u16) -> *mut u16 {
        trace_call!("msvcrt!_wcsdup");
        let mut len = 0;
        while *s.add(len) != 0 {
            len += 1;
        }
        let size = (len + 1) * 2;
        let dst = libc::malloc(size) as *mut u16;
        if !dst.is_null() {
            for i in 0..=len {
                *dst.add(i) = *s.add(i);
            }
        }
        dst
    }

    fn _wcsicmp(s1: *const u16, s2: *const u16) -> i32 {
        trace_call!("msvcrt!_wcsicmp");
        let mut i = 0;
        loop {
            let c1 = ascii_lower(*s1.add(i) as u32);
            let c2 = ascii_lower(*s2.add(i) as u32);
            if c1 != c2 {
                return c1 as i32 - c2 as i32;
            }
            if c1 == 0 {
                return 0;
            }
            i += 1;
        }
    }

    fn _wcsnicmp(s1: *const u16, s2: *const u16, n: usize) -> i32 {
        trace_call!("msvcrt!_wcsnicmp");
        for i in 0..n {
            let c1 = ascii_lower(*s1.add(i) as u32);
            let c2 = ascii_lower(*s2.add(i) as u32);
            if c1 != c2 {
                return c1 as i32 - c2 as i32;
            }
            if c1 == 0 {
                return 0;
            }
        }
        0
    }

    fn _mbscmp(s1: *const u8, s2: *const u8) -> i32 {
        trace_call!("msvcrt!_mbscmp");
        libc::strcmp(s1 as *const i8, s2 as *const i8)
    }

    fn _mbstrlen(s: *const u8) -> usize {
        trace_call!("msvcrt!_mbstrlen");
        libc::strlen(s as *const i8)
    }

    // ============ msvcrt - printf/scanf ============

    fn sscanf_s(
        _buffer: *const i8,
        _format: *const i8,
        _arg1: u64,
        _arg2: u64,
        _arg3: u64,
        _arg4: u64,
    ) -> i32 {
        trace_call!("msvcrt!sscanf_s");
        panic!("msvcrt!sscanf_s not implemented");
    }

    fn swprintf_s(
        _buffer: *mut u16,
        _size: usize,
        _format: *const u16,
        _arg1: u64,
        _arg2: u64,
        _arg3: u64,
        _arg4: u64,
    ) -> i32 {
        trace_call!("msvcrt!swprintf_s");
        panic!("msvcrt!swprintf_s not implemented");
    }

    fn _vsnprintf(
        buffer: *mut i8,
        count: usize,
        format: *const i8,
        argptr: *mut c_void,
    ) -> i32 {
        trace_call!("msvcrt!_vsnprintf");
        super::printf::vsnprintf_core(buffer, count, format, argptr as *const u64)
    }

    fn vsprintf_s(
        buffer: *mut i8,
        size: usize,
        format: *const i8,
        argptr: *mut c_void,
    ) -> i32 {
        trace_call!("msvcrt!vsprintf_s");
        super::printf::vsnprintf_core(buffer, size, format, argptr as *const u64)
    }

    fn _vsnwprintf(
        _buffer: *mut u16,
        _count: usize,
        _format: *const u16,
        _argptr: *mut c_void,
    ) -> i32 {
        trace_call!("msvcrt!_vsnwprintf");
        panic!("msvcrt!_vsnwprintf not implemented");
    }

    fn _snwprintf_s(
        _buffer: *mut u16,
        _size_in_words: usize,
        _count: usize,
        _format: *const u16,
        _arg1: u64,
        _arg2: u64,
        _arg3: u64,
        _arg4: u64,
    ) -> i32 {
        trace_call!("msvcrt!_snwprintf_s");
        panic!("msvcrt!_snwprintf_s not implemented");
    }

    // ============ msvcrt - file I/O ============

    fn fclose(stream: *mut c_void) -> i32 {
        trace_call!("msvcrt!fclose", "stream={:p}", stream);
        libc::fclose(stream as *mut libc::FILE)
    }

    fn fread(
        ptr: *mut c_void,
        size: usize,
        count: usize,
        stream: *mut c_void,
    ) -> usize {
        trace_call!("msvcrt!fread", "size={}, count={}", size, count);
        libc::fread(ptr, size, count, stream as *mut libc::FILE)
    }

    fn fseek(stream: *mut c_void, offset: i64, origin: i32) -> i32 {
        trace_call!("msvcrt!fseek", "offset={}, origin={}", offset, origin);
        libc::fseek(stream as *mut libc::FILE, offset as libc::c_long, origin)
    }

    fn ftell(stream: *mut c_void) -> i64 {
        trace_call!("msvcrt!ftell");
        libc::ftell(stream as *mut libc::FILE) as i64
    }

    fn _wfsopen(
        filename: *const u16,
        mode: *const u16,
        _shflag: i32,
    ) -> *mut c_void {
        trace_call!("msvcrt!_wfsopen");
        let filename = wstr_to_string(filename);
        let mode = wstr_to_string(mode);
        libc::fopen(filename.as_ptr() as *const i8, mode.as_ptr() as *const i8) as *mut c_void
    }

    fn _fileno(stream: *mut c_void) -> i32 {
        trace_call!("msvcrt!_fileno");
        libc::fileno(stream as *mut libc::FILE)
    }

    fn _filelengthi64(fd: i32) -> i64 {
        trace_call!("msvcrt!_filelengthi64", "fd={}", fd);
        let mut stat: libc::stat = std::mem::zeroed();
        if libc::fstat(fd, &mut stat) == 0 {
            stat.st_size
        } else {
            -1
        }
    }

    fn _read(fd: i32, buf: *mut c_void, count: u32) -> i32 {
        trace_call!("msvcrt!_read", "fd={}, count={}", fd, count);
        libc::read(fd, buf, count as usize) as i32
    }

    fn _write(fd: i32, buf: *const c_void, count: u32) -> i32 {
        trace_call!("msvcrt!_write", "fd={}, count={}", fd, count);
        libc::write(fd, buf, count as usize) as i32
    }

    fn _close(fd: i32) -> i32 {
        trace_call!("msvcrt!_close", "fd={}", fd);
        libc::close(fd)
    }

    fn _lseeki64(fd: i32, offset: i64, origin: i32) -> i64 {
        trace_call!(
            "msvcrt!_lseeki64",
            "fd={}, offset={}, origin={}",
            fd,
            offset,
            origin
        );
        libc::lseek(fd, offset, origin)
    }

    fn _chsize_s(fd: i32, size: i64) -> i32 {
        trace_call!("msvcrt!_chsize_s", "fd={}, size={}", fd, size);
        libc::ftruncate(fd, size)
    }

    fn _chsize(fd: i32, size: i32) -> i32 {
        trace_call!("msvcrt!_chsize", "fd={}, size={}", fd, size);
        libc::ftruncate(fd, size as i64)
    }

    // pmode is variadic but only read with _O_CREAT, so taking it as a 4th arg is fine
    fn _wsopen(filename: *const u16, oflag: i32, _shflag: i32, pmode: i32) -> i32 {
        trace_call!("msvcrt!_wsopen", "oflag={:#x}, pmode={:#x}", oflag, pmode);
        let path = wstr_to_string(filename);

        let mut flags = match oflag & 3 {
            1 => libc::O_WRONLY,
            2 => libc::O_RDWR,
            _ => libc::O_RDONLY,
        };
        if oflag & 0x0008 != 0 {
            flags |= libc::O_APPEND;
        }
        if oflag & 0x0080 != 0 {
            flags |= libc::O_CLOEXEC; // _O_NOINHERIT
        }
        if oflag & 0x0100 != 0 {
            flags |= libc::O_CREAT;
        }
        if oflag & 0x0200 != 0 {
            flags |= libc::O_TRUNC;
        }
        if oflag & 0x0400 != 0 {
            flags |= libc::O_EXCL;
        }

        // _S_IWRITE (0x80) controls whether the file is writable
        let mode = if pmode & 0x80 != 0 { 0o666 } else { 0o444 };
        let fd = libc::open(path.as_ptr() as *const i8, flags, mode);

        // _O_TEMPORARY: delete when closed, which unlinking now gives us
        if fd >= 0 && oflag & 0x0040 != 0 {
            libc::unlink(path.as_ptr() as *const i8);
        }
        fd
    }

    fn _get_osfhandle(fd: i32) -> isize {
        trace_call!("msvcrt!_get_osfhandle", "fd={}", fd);
        fd as isize
    }

    fn _open_osfhandle(osfhandle: isize, _flags: i32) -> i32 {
        trace_call!(
            "msvcrt!_open_osfhandle",
            "osfhandle={}, flags={}",
            osfhandle,
            _flags
        );
        osfhandle as i32
    }

    // ============ msvcrt - math ============

    fn acos(x: f64) -> f64 {
        trace_call!("msvcrt!acos");
        x.acos()
    }
    fn asin(x: f64) -> f64 {
        trace_call!("msvcrt!asin");
        x.asin()
    }
    fn atan(x: f64) -> f64 {
        trace_call!("msvcrt!atan");
        x.atan()
    }
    fn atan2(y: f64, x: f64) -> f64 {
        trace_call!("msvcrt!atan2");
        y.atan2(x)
    }
    fn ceil(x: f64) -> f64 {
        trace_call!("msvcrt!ceil");
        x.ceil()
    }
    fn cos(x: f64) -> f64 {
        trace_call!("msvcrt!cos");
        x.cos()
    }
    fn cosh(x: f64) -> f64 {
        trace_call!("msvcrt!cosh");
        x.cosh()
    }
    fn exp(x: f64) -> f64 {
        trace_call!("msvcrt!exp");
        x.exp()
    }
    fn floor(x: f64) -> f64 {
        trace_call!("msvcrt!floor");
        x.floor()
    }
    fn floorf(x: f32) -> f32 {
        trace_call!("msvcrt!floorf");
        x.floor()
    }
    fn fmod(x: f64, y: f64) -> f64 {
        trace_call!("msvcrt!fmod");
        x % y
    }
    fn log(x: f64) -> f64 {
        trace_call!("msvcrt!log");
        x.ln()
    }
    fn modf(x: f64, iptr: *mut f64) -> f64 {
        trace_call!("msvcrt!modf");
        *iptr = x.trunc();
        x.fract()
    }
    fn pow(x: f64, y: f64) -> f64 {
        trace_call!("msvcrt!pow");
        x.powf(y)
    }
    fn sin(x: f64) -> f64 {
        trace_call!("msvcrt!sin");
        x.sin()
    }
    fn sinh(x: f64) -> f64 {
        trace_call!("msvcrt!sinh");
        x.sinh()
    }
    fn sqrt(x: f64) -> f64 {
        trace_call!("msvcrt!sqrt");
        x.sqrt()
    }
    fn tan(x: f64) -> f64 {
        trace_call!("msvcrt!tan");
        x.tan()
    }
    fn tanh(x: f64) -> f64 {
        trace_call!("msvcrt!tanh");
        x.tanh()
    }

    fn _isnan(x: f64) -> i32 {
        trace_call!("msvcrt!_isnan");
        if x.is_nan() {
            1
        } else {
            0
        }
    }

    fn _finite(x: f64) -> i32 {
        trace_call!("msvcrt!_finite");
        if x.is_finite() {
            1
        } else {
            0
        }
    }

    // _FPCLASS_SNAN 0x1, QNAN 0x2, NINF 0x4, NN 0x8, ND 0x10, NZ 0x20, PZ 0x40, PD 0x80, PN 0x100,
    // PINF 0x200: sign, zero and denormal all count
    fn _fpclass(x: f64) -> i32 {
        trace_call!("msvcrt!_fpclass");
        let negative = x.is_sign_negative();
        if x.is_nan() {
            // a quiet NaN has the top bit of the mantissa set
            if x.to_bits() & (1 << 51) != 0 { 0x0002 } else { 0x0001 }
        } else if x.is_infinite() {
            if negative { 0x0004 } else { 0x0200 }
        } else if x == 0.0 {
            if negative { 0x0020 } else { 0x0040 }
        } else if x.is_subnormal() {
            if negative { 0x0010 } else { 0x0080 }
        } else if negative {
            0x0008
        } else {
            0x0100
        }
    }

    fn _clearfp() -> u32 {
        trace_call!("msvcrt!_clearfp");
        0
    }

    fn _controlfp(_new: u32, _mask: u32) -> u32 {
        trace_call!("msvcrt!_controlfp");
        0
    }

    // ============ msvcrt - conversion ============

    fn atoi(s: *const i8) -> i32 {
        trace_call!("msvcrt!atoi");
        libc::atoi(s)
    }

    fn atof(s: *const i8) -> f64 {
        trace_call!("msvcrt!atof");
        libc::atof(s)
    }

    fn _atoi64(s: *const i8) -> i64 {
        trace_call!("msvcrt!_atoi64");
        libc::strtoll(s, std::ptr::null_mut(), 10)
    }

    fn strtod(s: *const i8, endptr: *mut *mut i8) -> f64 {
        trace_call!("msvcrt!strtod");
        libc::strtod(s, endptr)
    }

    fn strtoul(s: *const i8, endptr: *mut *mut i8, base: i32) -> u64 {
        trace_call!("msvcrt!strtoul");
        libc::strtoul(s, endptr, base) as u64
    }

    fn wcstoul(s: *const u16, _endptr: *mut *mut u16, base: i32) -> u64 {
        trace_call!("msvcrt!wcstoul");
        let narrow = wstr_to_string(s);
        libc::strtoul(narrow.as_ptr() as *const i8, std::ptr::null_mut(), base) as u64
    }

    fn wcstol(s: *const u16, endptr: *mut *mut u16, base: i32) -> i32 {
        trace_call!("msvcrt!wcstol");
        let narrow = wstr_to_string(s);
        let mut end: *mut i8 = std::ptr::null_mut();
        let v = libc::strtol(narrow.as_ptr() as *const i8, &mut end, base);
        if !endptr.is_null() {
            *endptr = s.add(end.offset_from(narrow.as_ptr() as *const i8) as usize) as *mut u16;
        }
        v.clamp(i32::MIN as i64, i32::MAX as i64) as i32
    }

    fn _strtoui64(s: *const i8, endptr: *mut *mut i8, base: i32) -> u64 {
        trace_call!("msvcrt!_strtoui64");
        libc::strtoull(s, endptr, base)
    }

    // ============ msvcrt - other ============

    fn qsort(
        base: *mut c_void,
        num: usize,
        size: usize,
        compar: *const c_void,
    ) {
        trace_call!("msvcrt!qsort", "num={}, size={}", num, size);
        ms_qsort(base as *mut u8, num, size, std::mem::transmute::<*const c_void, Win64Comparator>(compar));
    }

    fn bsearch(
        key: *const c_void,
        base: *const c_void,
        num: usize,
        size: usize,
        compar: *const c_void,
    ) -> *mut c_void {
        trace_call!("msvcrt!bsearch", "num={}, size={}", num, size);
        ms_bsearch(key, base as *const u8, num, size, std::mem::transmute::<*const c_void, Win64Comparator>(compar))
    }

    fn getenv(name: *const i8) -> *mut i8 {
        trace_call!("msvcrt!getenv");
        libc::getenv(name)
    }

    fn _wgetenv(_name: *const u16) -> *mut u16 {
        trace_call!("msvcrt!_wgetenv");
        panic!("msvcrt!_wgetenv not implemented");
    }

    fn setlocale(category: i32, locale: *const i8) -> *mut i8 {
        trace_call!("msvcrt!setlocale");
        libc::setlocale(category, locale)
    }

    fn _time64(timer: *mut i64) -> i64 {
        trace_call!("msvcrt!_time64");
        let t = libc::time(std::ptr::null_mut());
        if !timer.is_null() {
            *timer = t;
        }
        t
    }

    fn _errno() -> *mut i32 {
        trace_call!("msvcrt!_errno");
        libc::__errno_location()
    }

    // ============ msvcrt - CRT init ============

    fn _initterm(start: *const *const c_void, end: *const *const c_void) {
        trace_call!("msvcrt!_initterm");
        let mut p = start;
        while p < end {
            if !(*p).is_null() {
                let f: extern "win64" fn() = std::mem::transmute(*p);
                f();
            }
            p = p.add(1);
        }
    }

    fn _amsg_exit(code: i32) {
        trace_call!("msvcrt!_amsg_exit", "code={}", code);
        std::process::exit(code);
    }

    fn _purecall() {
        trace_call!("msvcrt!_purecall");
        panic!("pure virtual function call");
    }

    fn _onexit(func: *const c_void) -> *const c_void {
        trace_call!("msvcrt!_onexit");
        func
    }

    fn __dllonexit(
        func: *const c_void,
        _pbegin: *mut *const c_void,
        _pend: *mut *const c_void,
    ) -> *const c_void {
        trace_call!("msvcrt!__dllonexit");
        func
    }

    fn _lock(_locknum: i32) {
        trace_call!("msvcrt!_lock");
        // No-op: CRT lock for thread safety
    }

    fn _unlock(_locknum: i32) {
        trace_call!("msvcrt!_unlock");
        // No-op: CRT unlock for thread safety
    }

    fn _callnewh(_size: usize) -> i32 {
        trace_call!("msvcrt!_callnewh");
        panic!("msvcrt!_callnewh not implemented");
    }

    // ============ msvcrt - exceptions ============

    fn __C_specific_handler() {
        trace_call!("msvcrt!__C_specific_handler");
        panic!("msvcrt!__C_specific_handler not implemented");
    }

    fn __CxxFrameHandler3() {
        trace_call!("msvcrt!__CxxFrameHandler3");
        panic!("msvcrt!__CxxFrameHandler3 not implemented");
    }

    fn _CxxThrowException(_obj: *mut c_void, _info: *mut c_void) {
        trace_call!("msvcrt!_CxxThrowException");
        panic!("C++ exception thrown");
    }

    fn terminate() {
        trace_call!("msvcrt!terminate");
        std::process::abort();
    }

    fn type_info_dtor() {
        trace_call!("msvcrt!type_info_dtor");
        panic!("msvcrt!type_info_dtor not implemented");
    }

    fn __unDName(
        buffer: *mut i8,
        name: *const i8,
        buflen: i32,
        _malloc: *const c_void,
        _free: *const c_void,
        _flags: u16,
    ) -> *mut i8 {
        trace_call!("msvcrt!__unDName");
        if buffer.is_null() {
            _strdup(name)
        } else {
            strcpy_s(buffer, buflen as usize, name);
            buffer
        }
    }

    fn _XcptFilter(_code: u32, _info: *mut c_void) -> i32 {
        trace_call!("msvcrt!_XcptFilter");
        panic!("msvcrt!_XcptFilter not implemented");
    }

    // ============ msvcrt - path ============

    fn _wfullpath(
        absPath: *mut u16,
        relPath: *const u16,
        maxLength: usize,
    ) -> *mut u16 {
        trace_call!("msvcrt!_wfullpath");
        wcscpy_s(absPath, maxLength, relPath);
        absPath
    }

    fn _wmakepath_s(
        path: *mut u16,
        size: usize,
        _drive: *const u16,
        dir: *const u16,
        fname: *const u16,
        ext: *const u16,
    ) -> i32 {
        trace_call!("msvcrt!_wmakepath_s");
        *path = 0;
        if !dir.is_null() {
            wcscat_s(path, size, dir);
        }
        if !fname.is_null() {
            wcscat_s(path, size, fname);
        }
        if !ext.is_null() {
            wcscat_s(path, size, ext);
        }
        0
    }

    fn _wsplitpath_s(
        _path: *const u16,
        drive: *mut u16,
        drive_size: usize,
        dir: *mut u16,
        dir_size: usize,
        fname: *mut u16,
        fname_size: usize,
        ext: *mut u16,
        ext_size: usize,
    ) -> i32 {
        trace_call!("msvcrt!_wsplitpath_s");
        if !drive.is_null() && drive_size > 0 {
            *drive = 0;
        }
        if !dir.is_null() && dir_size > 0 {
            *dir = 0;
        }
        if !fname.is_null() && fname_size > 0 {
            *fname = 0;
        }
        if !ext.is_null() && ext_size > 0 {
            *ext = 0;
        }
        0
    }
}

// ============ Variadic functions with proper naked thunks ============

/// sprintf_s - variadic printf to buffer
/// Win64 ABI: RCX=buffer, RDX=size, R8=format, R9=first_vararg, stack has rest
#[unsafe(naked)]
pub unsafe extern "win64" fn sprintf_s() -> i32 {
    std::arch::naked_asm!(
        // Store R9 (first vararg) into shadow space to make args contiguous
        "mov [rsp+0x20], r9",
        // R9 becomes pointer to varargs (va_list)
        "lea r9, [rsp+0x20]",
        "jmp {impl_fn}",
        impl_fn = sym sprintf_s_impl,
    )
}

unsafe extern "win64" fn sprintf_s_impl(
    buffer: *mut i8,
    size: usize,
    format: *const i8,
    argptr: *const u64,
) -> i32 {
    trace_call!("msvcrt!sprintf_s");
    // Text that does not fit is an invalid parameter: the buffer is emptied and -1 returned
    let text = super::printf::format(format, argptr);
    if buffer.is_null() || size == 0 {
        return -1;
    }
    if text.len() >= size {
        *buffer = 0;
        return -1;
    }
    std::ptr::copy_nonoverlapping(text.as_ptr(), buffer as *mut u8, text.len());
    *buffer.add(text.len()) = 0;
    text.len() as i32
}

#[unsafe(naked)]
pub unsafe extern "win64" fn sscanf() -> i32 {
    std::arch::naked_asm!(
        "mov [rsp+0x18], r8",
        "mov [rsp+0x20], r9",
        "lea r8, [rsp+0x18]",
        "jmp {impl_fn}",
        impl_fn = sym sscanf_impl,
    )
}

unsafe extern "win64" fn sscanf_impl(
    buffer: *const i8,
    format: *const i8,
    argptr: *const u64,
) -> i32 {
    trace_call!("msvcrt!sscanf");
    // scanf args are all pointers, so forward a fixed number and let the format pick how many get used
    // MS size prefixes (%I64, 32-bit %l) are not translated.
    let a = |i| unsafe { *argptr.add(i) as *mut c_void };
    unsafe {
        libc::sscanf(
            buffer,
            format,
            a(0),
            a(1),
            a(2),
            a(3),
            a(4),
            a(5),
            a(6),
            a(7),
        )
    }
}
