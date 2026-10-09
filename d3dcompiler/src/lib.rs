//! Cross-platform wrapper for d3dcompiler_47.dll
//!
//! This crate provides a compatibility layer that allows using the Windows
//! D3D shader compiler on Linux by loading the DLL and implementing the
//! necessary Windows API imports.

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(dead_code)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::missing_transmute_annotations)]
#![allow(unsafe_op_in_unsafe_fn)]
#![recursion_limit = "256"]

mod imports;

macro_rules! debug_log {
    ($($arg:tt)*) => {
        #[cfg(feature = "debug-logs")]
        eprintln!($($arg)*)
    };
}

macro_rules! debug_log_return {
    ($tag:literal, $fmt:literal, $expr:expr) => {{
        #[cfg(feature = "debug-logs")]
        {
            let result = $expr;
            eprintln!(concat!($tag, " -> ", $fmt), result);
            result
        }
        #[cfg(not(feature = "debug-logs"))]
        {
            $expr
        }
    }};
}

use d3dcompiler_proc::com_wrapper;
use std::ffi::c_void;
use std::sync::OnceLock;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum D3DCompilerError {
    #[error("Failed to load DLL: {0}")]
    LoadError(String),
    #[error("Function not found: {0}")]
    FunctionNotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("PE parse error: {0}")]
    ParseError(String),
}

pub type Result<T> = std::result::Result<T, D3DCompilerError>;

// D3D Compiler types
pub type HRESULT = i32;
pub type UINT = u32;
pub type SIZE_T = usize;
pub type LPCSTR = *const i8;
pub type LPCWSTR = *const u16;
pub type LPVOID = *mut c_void;

pub const S_OK: HRESULT = 0;
pub const E_FAIL: HRESULT = 0x80004005u32 as i32;

// D3D11 Shader Reflection descriptor types
#[repr(C)]
#[derive(Default)]
pub struct D3D11_SHADER_DESC {
    pub Version: u32,
    pub Creator: LPCSTR,
    pub Flags: u32,
    pub ConstantBuffers: u32,
    pub BoundResources: u32,
    pub InputParameters: u32,
    pub OutputParameters: u32,
    pub InstructionCount: u32,
    pub TempRegisterCount: u32,
    pub TempArrayCount: u32,
    pub DefCount: u32,
    pub DclCount: u32,
    pub TextureNormalInstructions: u32,
    pub TextureLoadInstructions: u32,
    pub TextureCompInstructions: u32,
    pub TextureBiasInstructions: u32,
    pub TextureGradientInstructions: u32,
    pub FloatInstructionCount: u32,
    pub IntInstructionCount: u32,
    pub UintInstructionCount: u32,
    pub StaticFlowControlCount: u32,
    pub DynamicFlowControlCount: u32,
    pub MacroInstructionCount: u32,
    pub ArrayInstructionCount: u32,
    pub CutInstructionCount: u32,
    pub EmitInstructionCount: u32,
    pub GSOutputTopology: u32,
    pub GSMaxOutputVertexCount: u32,
    pub InputPrimitive: u32,
    pub PatchConstantParameters: u32,
    pub cGSInstanceCount: u32,
    pub cControlPoints: u32,
    pub HSOutputPrimitive: u32,
    pub HSPartitioning: u32,
    pub TessellatorDomain: u32,
    pub cBarrierInstructions: u32,
    pub cInterlockedInstructions: u32,
    pub cTextureStoreInstructions: u32,
}

#[repr(C)]
#[derive(Default)]
pub struct D3D11_SHADER_BUFFER_DESC {
    pub Name: LPCSTR,
    pub Type: u32,
    pub Variables: u32,
    pub Size: u32,
    pub uFlags: u32,
}

#[repr(C)]
#[derive(Default)]
pub struct D3D11_SHADER_INPUT_BIND_DESC {
    pub Name: LPCSTR,
    pub Type: u32,
    pub BindPoint: u32,
    pub BindCount: u32,
    pub uFlags: u32,
    pub ReturnType: u32,
    pub Dimension: u32,
    pub NumSamples: u32,
}

#[repr(C)]
#[derive(Default)]
pub struct D3D11_SIGNATURE_PARAMETER_DESC {
    pub SemanticName: LPCSTR,
    pub SemanticIndex: u32,
    pub Register: u32,
    pub SystemValueType: u32,
    pub ComponentType: u32,
    pub Mask: u8,
    pub ReadWriteMask: u8,
    pub Stream: u32,
}

#[repr(C)]
#[derive(Default)]
pub struct D3D11_SHADER_VARIABLE_DESC {
    pub Name: LPCSTR,
    pub StartOffset: u32,
    pub Size: u32,
    pub uFlags: u32,
    pub DefaultValue: *const c_void,
    pub StartTexture: u32,
    pub TextureSize: u32,
    pub StartSampler: u32,
    pub SamplerSize: u32,
}

#[repr(C)]
#[derive(Default)]
pub struct D3D11_SHADER_TYPE_DESC {
    pub Class: u32,
    pub Type: u32,
    pub Rows: u32,
    pub Columns: u32,
    pub Elements: u32,
    pub Members: u32,
    pub Offset: u32,
    pub Name: LPCSTR,
}

com_wrapper! {
    BlobWrapper wraps Win64Blob as ID3DBlob {
        vtable: BLOB_VTABLE: ID3DBlobVtbl,
        fn QueryInterface(riid: *const c_void, ppv: *mut *mut c_void) -> HRESULT;
        fn AddRef() -> u32;
        fn Release() -> u32 => release;
        fn GetBufferPointer() -> *mut c_void;
        fn GetBufferSize() -> SIZE_T;
    }
}

com_wrapper! {
    ReflectionWrapper wraps Win64Reflection as ID3D11ShaderReflection {
        vtable: REFLECTION_VTABLE: ID3D11ShaderReflectionVtbl,
        fn QueryInterface(riid: *const c_void, ppv: *mut *mut c_void) -> HRESULT;
        fn AddRef() -> u32;
        fn Release() -> u32 => release;
        fn GetDesc(desc: *mut D3D11_SHADER_DESC) -> HRESULT => cast;
        fn GetConstantBufferByIndex(index: UINT) -> *mut ID3D11ShaderReflectionConstantBuffer => wrap(wrap_constant_buffer);
        fn GetConstantBufferByName(name: LPCSTR) -> *mut ID3D11ShaderReflectionConstantBuffer => wrap(wrap_constant_buffer);
        fn GetResourceBindingDesc(index: UINT, desc: *mut D3D11_SHADER_INPUT_BIND_DESC) -> HRESULT => cast;
        fn GetInputParameterDesc(index: UINT, desc: *mut D3D11_SIGNATURE_PARAMETER_DESC) -> HRESULT => cast;
        fn GetOutputParameterDesc(index: UINT, desc: *mut D3D11_SIGNATURE_PARAMETER_DESC) -> HRESULT => cast;
        fn GetPatchConstantParameterDesc(index: UINT, desc: *mut D3D11_SIGNATURE_PARAMETER_DESC) -> HRESULT => cast;
        fn GetVariableByName(name: LPCSTR) -> *mut ID3D11ShaderReflectionVariable => wrap(wrap_variable);
        fn GetResourceBindingDescByName(name: LPCSTR, desc: *mut D3D11_SHADER_INPUT_BIND_DESC) -> HRESULT => cast;
        fn GetMovInstructionCount() -> UINT;
        fn GetMovcInstructionCount() -> UINT;
        fn GetConversionInstructionCount() -> UINT;
        fn GetBitwiseInstructionCount() -> UINT;
        fn GetGSInputPrimitive() -> UINT;
        fn IsSampleFrequencyShader() -> i32;
        fn GetNumInterfaceSlots() -> UINT;
        fn GetMinFeatureLevel(level: *mut UINT) -> HRESULT;
        fn GetThreadGroupSize(x: *mut UINT, y: *mut UINT, z: *mut UINT) -> UINT;
        fn GetRequiresFlags() -> u64;
    }
}

com_wrapper! {
    ConstantBufferWrapper wraps Win64ConstantBuffer as ID3D11ShaderReflectionConstantBuffer {
        vtable: CONSTANT_BUFFER_VTABLE: ID3D11ShaderReflectionConstantBufferVtbl,
        fn GetDesc(desc: *mut D3D11_SHADER_BUFFER_DESC) -> HRESULT => cast;
        fn GetVariableByIndex(index: UINT) -> *mut ID3D11ShaderReflectionVariable => wrap(wrap_variable);
        fn GetVariableByName(name: LPCSTR) -> *mut ID3D11ShaderReflectionVariable => wrap(wrap_variable);
    }
}

com_wrapper! {
    VariableWrapper wraps Win64Variable as ID3D11ShaderReflectionVariable {
        vtable: VARIABLE_VTABLE: ID3D11ShaderReflectionVariableVtbl,
        fn GetDesc(desc: *mut D3D11_SHADER_VARIABLE_DESC) -> HRESULT => cast;
        fn GetType() -> *mut ID3D11ShaderReflectionType => wrap(wrap_type);
        fn GetBuffer() -> *mut ID3D11ShaderReflectionConstantBuffer => wrap(wrap_constant_buffer);
        fn GetInterfaceSlot(index: UINT) -> UINT;
    }
}

com_wrapper! {
    TypeWrapper wraps Win64Type as ID3D11ShaderReflectionType {
        vtable: TYPE_VTABLE: ID3D11ShaderReflectionTypeVtbl,
        fn GetDesc(desc: *mut D3D11_SHADER_TYPE_DESC) -> HRESULT => cast;
        fn GetMemberTypeByIndex(index: UINT) -> *mut ID3D11ShaderReflectionType => wrap(wrap_type);
        fn GetMemberTypeByName(name: LPCSTR) -> *mut ID3D11ShaderReflectionType => wrap(wrap_type);
        fn GetMemberTypeName(index: UINT) -> LPCSTR;
        fn IsEqual(other: *mut ID3D11ShaderReflectionType) -> HRESULT => unwrap(TypeWrapper, other);
        fn GetSubType() -> *mut ID3D11ShaderReflectionType => wrap(wrap_type);
        fn GetBaseClass() -> *mut ID3D11ShaderReflectionType => wrap(wrap_type);
        fn GetNumInterfaces() -> UINT;
        fn GetInterfaceByIndex(index: UINT) -> *mut ID3D11ShaderReflectionType => wrap(wrap_type);
        fn IsOfType(other: *mut ID3D11ShaderReflectionType) -> HRESULT => unwrap(TypeWrapper, other);
        fn ImplementsInterface(other: *mut ID3D11ShaderReflectionType) -> HRESULT => unwrap(TypeWrapper, other);
    }
}

// D3D_SHADER_MACRO
#[repr(C)]
pub struct D3D_SHADER_MACRO {
    pub Name: LPCSTR,
    pub Definition: LPCSTR,
}

// ============================================================================
// ID3DInclude wrapper (C ABI -> win64 ABI thunking)
// ============================================================================

// Internal include type expected by Windows DLL (win64 ABI)
#[repr(C)]
struct Win64Include {
    vtable: *const Win64IncludeVtbl,
}

#[repr(C)]
struct Win64IncludeVtbl {
    pub Open: unsafe extern "win64" fn(
        *mut Win64Include,
        u32,
        LPCSTR,
        *const c_void,
        *mut *const c_void,
        *mut UINT,
    ) -> HRESULT,
    pub Close: unsafe extern "win64" fn(*mut Win64Include, *const c_void) -> HRESULT,
}

// Public include type with C ABI for use with standard D3D headers
#[repr(C)]
pub struct ID3DInclude {
    pub vtable: *const ID3DIncludeVtbl,
}

#[repr(C)]
pub struct ID3DIncludeVtbl {
    pub Open: unsafe extern "C" fn(
        *mut ID3DInclude,
        u32,
        LPCSTR,
        *const c_void,
        *mut *const c_void,
        *mut UINT,
    ) -> HRESULT,
    pub Close: unsafe extern "C" fn(*mut ID3DInclude, *const c_void) -> HRESULT,
}

// Wrapper that thunks win64 ABI calls (from DLL) to C ABI calls (to user code)
#[repr(C)]
struct IncludeWrapper {
    vtable: *const Win64IncludeVtbl,
    inner: *mut ID3DInclude,
}

// Thunk: receives win64 call from DLL, forwards to user's C ABI callback
unsafe extern "win64" fn include_open_thunk(
    this: *mut Win64Include,
    include_type: u32,
    filename: LPCSTR,
    parent_data: *const c_void,
    data_out: *mut *const c_void,
    bytes_out: *mut UINT,
) -> HRESULT {
    debug_log!(
        "[INCLUDE] Open(this={:?}, type={}, filename={:?})",
        this,
        include_type,
        filename
    );
    let wrapper = this as *mut IncludeWrapper;
    let inner = (*wrapper).inner;
    debug_log_return!(
        "[INCLUDE] Open",
        "0x{:x}",
        ((*(*inner).vtable).Open)(
            inner,
            include_type,
            filename,
            parent_data,
            data_out,
            bytes_out
        )
    )
}

unsafe extern "win64" fn include_close_thunk(
    this: *mut Win64Include,
    data: *const c_void,
) -> HRESULT {
    debug_log!("[INCLUDE] Close(this={:?}, data={:?})", this, data);
    let wrapper = this as *mut IncludeWrapper;
    let inner = (*wrapper).inner;
    debug_log_return!(
        "[INCLUDE] Close",
        "0x{:x}",
        ((*(*inner).vtable).Close)(inner, data)
    )
}

// Static vtable for include wrappers
static INCLUDE_WRAPPER_VTABLE: Win64IncludeVtbl = Win64IncludeVtbl {
    Open: include_open_thunk,
    Close: include_close_thunk,
};

// Wrap a user's C ABI include in a win64 ABI wrapper for the DLL
unsafe fn wrap_include(inner: *mut ID3DInclude) -> *mut Win64Include {
    if inner.is_null() {
        return std::ptr::null_mut();
    }
    let wrapper = Box::new(IncludeWrapper {
        vtable: &INCLUDE_WRAPPER_VTABLE,
        inner,
    });
    Box::into_raw(wrapper) as *mut Win64Include
}

// Free the include wrapper (call after DLL function returns)
unsafe fn free_include_wrapper(wrapper: *mut Win64Include) {
    if !wrapper.is_null() {
        drop(Box::from_raw(wrapper as *mut IncludeWrapper));
    }
}

// Function pointer types for all exports - use win64 ABI for Windows DLL calls
// These use Win64Blob internally since they receive blobs from the Windows DLL
#[allow(non_camel_case_types)]
type PFN_D3DCompile = unsafe extern "win64" fn(
    pSrcData: *const c_void,
    SrcDataSize: SIZE_T,
    pSourceName: LPCSTR,
    pDefines: *const D3D_SHADER_MACRO,
    pInclude: *mut Win64Include,
    pEntrypoint: LPCSTR,
    pTarget: LPCSTR,
    Flags1: UINT,
    Flags2: UINT,
    ppCode: *mut *mut Win64Blob,
    ppErrorMsgs: *mut *mut Win64Blob,
) -> HRESULT;

#[allow(non_camel_case_types)]
type PFN_D3DCompile2 = unsafe extern "win64" fn(
    pSrcData: *const c_void,
    SrcDataSize: SIZE_T,
    pSourceName: LPCSTR,
    pDefines: *const D3D_SHADER_MACRO,
    pInclude: *mut Win64Include,
    pEntrypoint: LPCSTR,
    pTarget: LPCSTR,
    Flags1: UINT,
    Flags2: UINT,
    SecondaryDataFlags: UINT,
    pSecondaryData: *const c_void,
    SecondaryDataSize: SIZE_T,
    ppCode: *mut *mut Win64Blob,
    ppErrorMsgs: *mut *mut Win64Blob,
) -> HRESULT;

#[allow(non_camel_case_types)]
type PFN_D3DCompileFromFile = unsafe extern "win64" fn(
    pFileName: LPCWSTR,
    pDefines: *const D3D_SHADER_MACRO,
    pInclude: *mut Win64Include,
    pEntrypoint: LPCSTR,
    pTarget: LPCSTR,
    Flags1: UINT,
    Flags2: UINT,
    ppCode: *mut *mut Win64Blob,
    ppErrorMsgs: *mut *mut Win64Blob,
) -> HRESULT;

#[allow(non_camel_case_types)]
type PFN_D3DPreprocess = unsafe extern "win64" fn(
    pSrcData: *const c_void,
    SrcDataSize: SIZE_T,
    pSourceName: LPCSTR,
    pDefines: *const D3D_SHADER_MACRO,
    pInclude: *mut Win64Include,
    ppCodeText: *mut *mut Win64Blob,
    ppErrorMsgs: *mut *mut Win64Blob,
) -> HRESULT;

#[allow(non_camel_case_types)]
type PFN_D3DDisassemble = unsafe extern "win64" fn(
    pSrcData: *const c_void,
    SrcDataSize: SIZE_T,
    Flags: UINT,
    szComments: LPCSTR,
    ppDisassembly: *mut *mut Win64Blob,
) -> HRESULT;

#[allow(non_camel_case_types)]
type PFN_D3DCreateBlob =
    unsafe extern "win64" fn(Size: SIZE_T, ppBlob: *mut *mut Win64Blob) -> HRESULT;

#[allow(non_camel_case_types)]
type PFN_D3DReflect = unsafe extern "win64" fn(
    pSrcData: *const c_void,
    SrcDataSize: SIZE_T,
    pInterface: *const c_void, // REFIID
    ppReflector: *mut *mut c_void,
) -> HRESULT;

#[allow(non_camel_case_types)]
type PFN_D3DStripShader = unsafe extern "win64" fn(
    pShaderBytecode: *const c_void,
    BytecodeLength: SIZE_T,
    uStripFlags: UINT,
    ppStrippedBlob: *mut *mut Win64Blob,
) -> HRESULT;

#[allow(non_camel_case_types)]
type PFN_D3DGetBlobPart = unsafe extern "win64" fn(
    pSrcData: *const c_void,
    SrcDataSize: SIZE_T,
    Part: u32,
    Flags: UINT,
    ppPart: *mut *mut Win64Blob,
) -> HRESULT;

#[allow(non_camel_case_types)]
type PFN_D3DSetBlobPart = unsafe extern "win64" fn(
    pSrcData: *const c_void,
    SrcDataSize: SIZE_T,
    Part: u32,
    Flags: UINT,
    pPart: *const c_void,
    PartSize: SIZE_T,
    ppNewShader: *mut *mut Win64Blob,
) -> HRESULT;

// Global state for loaded DLL
struct D3DCompilerState {
    #[cfg(unix)]
    _mmap: *mut u8,
    #[cfg(unix)]
    _mmap_size: usize,

    // Function pointers
    d3d_compile: PFN_D3DCompile,
    d3d_compile2: PFN_D3DCompile2,
    d3d_compile_from_file: PFN_D3DCompileFromFile,
    d3d_preprocess: PFN_D3DPreprocess,
    d3d_disassemble: PFN_D3DDisassemble,
    d3d_create_blob: PFN_D3DCreateBlob,
    d3d_reflect: PFN_D3DReflect,
    d3d_strip_shader: PFN_D3DStripShader,
    d3d_get_blob_part: PFN_D3DGetBlobPart,
    d3d_set_blob_part: PFN_D3DSetBlobPart,
}

unsafe impl Send for D3DCompilerState {}
unsafe impl Sync for D3DCompilerState {}

static STATE: OnceLock<Result<D3DCompilerState>> = OnceLock::new();

// Expected hash for verification (update with actual hash)
static DLL_NAME: &str = "d3dcompiler_47.dll";

fn get_dll_path() -> std::path::PathBuf {
    // Look for DLL next to executable, or in current directory
    if let Ok(exe) = std::env::current_exe() {
        let path = exe.with_file_name(DLL_NAME);
        if path.exists() {
            return path;
        }
    }
    std::path::PathBuf::from(DLL_NAME)
}

// Initialize the compiler - call this before using any functions
unsafe fn init() -> &'static Result<D3DCompilerState> {
    use std::sync::Mutex;
    static INIT_ERROR: Mutex<Option<String>> = Mutex::new(None);

    unsafe { linux_loader::setup_tib() }

    STATE.get_or_init(linux_loader::load_dll)
}

// Public API functions that forward to the loaded DLL and wrap returned blobs

#[unsafe(no_mangle)]
pub unsafe extern "C" fn D3DCompile(
    pSrcData: *const c_void,
    SrcDataSize: SIZE_T,
    pSourceName: LPCSTR,
    pDefines: *const D3D_SHADER_MACRO,
    pInclude: *mut ID3DInclude,
    pEntrypoint: LPCSTR,
    pTarget: LPCSTR,
    Flags1: UINT,
    Flags2: UINT,
    ppCode: *mut *mut ID3DBlob,
    ppErrorMsgs: *mut *mut ID3DBlob,
) -> HRESULT {
    // eprintln!("[EXPORT ENTER] D3DCompile");

    // let src = slice::from_raw_parts(pSrcData.cast(), SrcDataSize);
    // let src_str = String::from_utf8_lossy(&src);

    // let name = std::ffi::CStr::from_ptr(pSourceName).to_string_lossy();
    // let entry = std::ffi::CStr::from_ptr(pEntrypoint).to_string_lossy();

    // eprintln!("COMPILING {name:?} {entry:?}");
    // let path = format!("/tmp/shaders/{}.hlsl", name.replace('/', "_"));
    // std::fs::write(&path, src).unwrap();
    // println!("{} bytes => {path}", src.len());

    let mut code: *mut Win64Blob = std::ptr::null_mut();
    let mut errors: *mut Win64Blob = std::ptr::null_mut();
    let wrapped_include = wrap_include(pInclude);
    let result = match init() {
        Ok(s) => (s.d3d_compile)(
            pSrcData,
            SrcDataSize,
            pSourceName,
            pDefines,
            wrapped_include,
            pEntrypoint,
            pTarget,
            Flags1,
            Flags2,
            &mut code,
            &mut errors,
        ),
        Err(_) => E_FAIL,
    };
    free_include_wrapper(wrapped_include);
    if !ppCode.is_null() {
        *ppCode = wrap_blob(code);
    }
    if !ppErrorMsgs.is_null() {
        *ppErrorMsgs = wrap_blob(errors);
    }
    // eprintln!("[EXPORT EXIT] D3DCompile = 0x{result:x} {ppCode:?}");
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn D3DCompile2(
    pSrcData: *const c_void,
    SrcDataSize: SIZE_T,
    pSourceName: LPCSTR,
    pDefines: *const D3D_SHADER_MACRO,
    pInclude: *mut ID3DInclude,
    pEntrypoint: LPCSTR,
    pTarget: LPCSTR,
    Flags1: UINT,
    Flags2: UINT,
    SecondaryDataFlags: UINT,
    pSecondaryData: *const c_void,
    SecondaryDataSize: SIZE_T,
    ppCode: *mut *mut ID3DBlob,
    ppErrorMsgs: *mut *mut ID3DBlob,
) -> HRESULT {
    // eprintln!("[EXPORT ENTER] D3DCompile2");
    let mut code: *mut Win64Blob = std::ptr::null_mut();
    let mut errors: *mut Win64Blob = std::ptr::null_mut();
    let wrapped_include = wrap_include(pInclude);
    let result = match init() {
        Ok(s) => (s.d3d_compile2)(
            pSrcData,
            SrcDataSize,
            pSourceName,
            pDefines,
            wrapped_include,
            pEntrypoint,
            pTarget,
            Flags1,
            Flags2,
            SecondaryDataFlags,
            pSecondaryData,
            SecondaryDataSize,
            &mut code,
            &mut errors,
        ),
        Err(_) => E_FAIL,
    };
    free_include_wrapper(wrapped_include);
    if !ppCode.is_null() {
        *ppCode = wrap_blob(code);
    }
    if !ppErrorMsgs.is_null() {
        *ppErrorMsgs = wrap_blob(errors);
    }
    // eprintln!("[EXPORT EXIT] D3DCompile2 = 0x{result:x}");
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn D3DCompileFromFile(
    pFileName: LPCWSTR,
    pDefines: *const D3D_SHADER_MACRO,
    pInclude: *mut ID3DInclude,
    pEntrypoint: LPCSTR,
    pTarget: LPCSTR,
    Flags1: UINT,
    Flags2: UINT,
    ppCode: *mut *mut ID3DBlob,
    ppErrorMsgs: *mut *mut ID3DBlob,
) -> HRESULT {
    // eprintln!("[EXPORT ENTER] D3DCompileFromFile");
    let mut code: *mut Win64Blob = std::ptr::null_mut();
    let mut errors: *mut Win64Blob = std::ptr::null_mut();
    let wrapped_include = wrap_include(pInclude);
    let result = match init() {
        Ok(s) => (s.d3d_compile_from_file)(
            pFileName,
            pDefines,
            wrapped_include,
            pEntrypoint,
            pTarget,
            Flags1,
            Flags2,
            &mut code,
            &mut errors,
        ),
        Err(_) => E_FAIL,
    };
    free_include_wrapper(wrapped_include);
    if !ppCode.is_null() {
        *ppCode = wrap_blob(code);
    }
    if !ppErrorMsgs.is_null() {
        *ppErrorMsgs = wrap_blob(errors);
    }
    // eprintln!("[EXPORT EXIT] D3DCompileFromFile = 0x{result:x}");
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn D3DPreprocess(
    pSrcData: *const c_void,
    SrcDataSize: SIZE_T,
    pSourceName: LPCSTR,
    pDefines: *const D3D_SHADER_MACRO,
    pInclude: *mut ID3DInclude,
    ppCodeText: *mut *mut ID3DBlob,
    ppErrorMsgs: *mut *mut ID3DBlob,
) -> HRESULT {
    // eprintln!("[EXPORT ENTER] D3DPreprocess");
    let mut code: *mut Win64Blob = std::ptr::null_mut();
    let mut errors: *mut Win64Blob = std::ptr::null_mut();
    let wrapped_include = wrap_include(pInclude);
    let result = match init() {
        Ok(s) => (s.d3d_preprocess)(
            pSrcData,
            SrcDataSize,
            pSourceName,
            pDefines,
            wrapped_include,
            &mut code,
            &mut errors,
        ),
        Err(_) => E_FAIL,
    };
    free_include_wrapper(wrapped_include);
    if !ppCodeText.is_null() {
        *ppCodeText = wrap_blob(code);
    }
    if !ppErrorMsgs.is_null() {
        *ppErrorMsgs = wrap_blob(errors);
    }
    // eprintln!("[EXPORT EXIT] D3DPreprocess = 0x{result:x}");
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn D3DDisassemble(
    pSrcData: *const c_void,
    SrcDataSize: SIZE_T,
    Flags: UINT,
    szComments: LPCSTR,
    ppDisassembly: *mut *mut ID3DBlob,
) -> HRESULT {
    // eprintln!("[EXPORT ENTER] D3DDisassemble");
    let mut disasm: *mut Win64Blob = std::ptr::null_mut();
    let result = match init() {
        Ok(s) => (s.d3d_disassemble)(pSrcData, SrcDataSize, Flags, szComments, &mut disasm),
        Err(_) => E_FAIL,
    };
    if !ppDisassembly.is_null() {
        *ppDisassembly = wrap_blob(disasm);
    }
    // eprintln!("[EXPORT EXIT] D3DDisassemble = 0x{result:x}");
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn D3DCreateBlob(Size: SIZE_T, ppBlob: *mut *mut ID3DBlob) -> HRESULT {
    // eprintln!("[EXPORT ENTER] D3DCreateBlob");
    let mut blob: *mut Win64Blob = std::ptr::null_mut();
    let result = match init() {
        Ok(s) => (s.d3d_create_blob)(Size, &mut blob),
        Err(_) => E_FAIL,
    };
    if !ppBlob.is_null() {
        *ppBlob = wrap_blob(blob);
    }
    // eprintln!("[EXPORT ENTER] D3DCreateBlob = 0x{result:x}");
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn D3DReflect(
    pSrcData: *const c_void,
    SrcDataSize: SIZE_T,
    pInterface: *const c_void,
    ppReflector: *mut *mut c_void,
) -> HRESULT {
    let mut reflector: *mut Win64Reflection = std::ptr::null_mut();
    let result = match init() {
        Ok(s) => (s.d3d_reflect)(
            pSrcData,
            SrcDataSize,
            pInterface,
            &mut reflector as *mut _ as *mut *mut c_void,
        ),
        Err(_) => E_FAIL,
    };
    if !ppReflector.is_null() {
        *ppReflector = wrap_reflection(reflector) as *mut c_void;
    }
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn D3DStripShader(
    pShaderBytecode: *const c_void,
    BytecodeLength: SIZE_T,
    uStripFlags: UINT,
    ppStrippedBlob: *mut *mut ID3DBlob,
) -> HRESULT {
    // eprintln!("[EXPORT ENTER] D3DStripShader");
    let mut blob: *mut Win64Blob = std::ptr::null_mut();
    let result = match init() {
        Ok(s) => (s.d3d_strip_shader)(pShaderBytecode, BytecodeLength, uStripFlags, &mut blob),
        Err(_) => E_FAIL,
    };
    if !ppStrippedBlob.is_null() {
        *ppStrippedBlob = wrap_blob(blob);
    }
    // eprintln!("[EXPORT EXIT] D3DStripShader = 0x{result:x}");
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn D3DGetBlobPart(
    pSrcData: *const c_void,
    SrcDataSize: SIZE_T,
    Part: u32,
    Flags: UINT,
    ppPart: *mut *mut ID3DBlob,
) -> HRESULT {
    // eprintln!("[EXPORT ENTER] D3DGetBlobPart");
    let mut blob: *mut Win64Blob = std::ptr::null_mut();
    let result = match init() {
        Ok(s) => (s.d3d_get_blob_part)(pSrcData, SrcDataSize, Part, Flags, &mut blob),
        Err(_) => E_FAIL,
    };
    if !ppPart.is_null() {
        *ppPart = wrap_blob(blob);
    }
    // eprintln!("[EXPORT EXIT] D3DGetBlobPart = 0x{result:x}");
    result
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn D3DSetBlobPart(
    pSrcData: *const c_void,
    SrcDataSize: SIZE_T,
    Part: u32,
    Flags: UINT,
    pPart: *const c_void,
    PartSize: SIZE_T,
    ppNewShader: *mut *mut ID3DBlob,
) -> HRESULT {
    // eprintln!("[EXPORT ENTER] D3DSetBlobPart");
    let mut blob: *mut Win64Blob = std::ptr::null_mut();
    let result = match init() {
        Ok(s) => (s.d3d_set_blob_part)(
            pSrcData,
            SrcDataSize,
            Part,
            Flags,
            pPart,
            PartSize,
            &mut blob,
        ),
        Err(_) => E_FAIL,
    };
    if !ppNewShader.is_null() {
        *ppNewShader = wrap_blob(blob);
    }
    // eprintln!("[EXPORT EXIT] D3DSetBlobPart = 0x{result:x}");
    result
}

// Linux loader - manual PE loading with import hooking
#[cfg(unix)]
mod linux_loader {
    use super::*;
    use object::pe::{
        IMAGE_REL_BASED_DIR64, IMAGE_SCN_MEM_EXECUTE, IMAGE_SCN_MEM_READ, IMAGE_SCN_MEM_WRITE,
        ImageNtHeaders64,
    };
    use object::read::pe::{ImageOptionalHeader, ImageThunkData, PeFile64};
    use object::{LittleEndian as LE, Object, ObjectSection};
    use std::collections::HashMap;

    // Thread Information Block for Windows ABI compatibility
    // Windows x64 TEB layout (relevant fields):
    //   0x00: ExceptionList (NT_TIB.ExceptionList)
    //   0x08: StackBase (NT_TIB.StackBase)
    //   0x10: StackLimit (NT_TIB.StackLimit)
    //   0x18: SubSystemTib
    //   0x20: FiberData / Version
    //   0x28: ArbitraryUserPointer
    //   0x30: Self (pointer to TEB itself - NT_TIB.Self)
    #[repr(C)]
    struct ThreadInformationBlock {
        exception_list: usize,         // 0x00
        stack_base: usize,             // 0x08
        stack_limit: usize,            // 0x10
        sub_system_tib: usize,         // 0x18
        fiber_data: usize,             // 0x20
        arbitrary_user_pointer: usize, // 0x28
        teb_self: usize,               // 0x30 - MUST point to this struct itself!
        environment_pointer: usize,    // 0x38
        process_id: usize,             // 0x40
        thread_id: usize,              // 0x48
    }

    // Thread-local TIB - each thread gets its own
    thread_local! {
        static TIB: std::cell::UnsafeCell<ThreadInformationBlock> = const {
            std::cell::UnsafeCell::new(ThreadInformationBlock {
                exception_list: 0,
                stack_base: 0,
                stack_limit: 0,
                sub_system_tib: 0,
                fiber_data: 0,
                arbitrary_user_pointer: 0,
                teb_self: 0,
                environment_pointer: 0,
                process_id: 0,
                thread_id: 0,
            })
        };
        static TIB_INITIALIZED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    // Set up GS register for Windows TIB access (once per thread)
    pub unsafe fn setup_tib() {
        TIB_INITIALIZED.with(|initialized| {
            if initialized.get() {
                return;
            }

            TIB.with(|tib| {
                let tib_ptr = tib.get();

                // Get current stack info
                let mut stack_var: usize = 0;
                let stack_ptr = (&raw mut stack_var) as usize;

                // Estimate stack bounds (stack grows down on x86-64)
                // Use 8MB stack size estimate for maximum compatibility
                let stack_base = (stack_ptr + 0x800000) & !0xFFF;
                let stack_limit = (stack_ptr - 0x800000) & !0xFFF;

                // Initialize TIB fields
                (*tib_ptr).stack_base = stack_base;
                (*tib_ptr).stack_limit = stack_limit;
                (*tib_ptr).teb_self = tib_ptr as usize;
                (*tib_ptr).process_id = std::process::id() as usize;
                (*tib_ptr).thread_id = libc::syscall(libc::SYS_gettid) as usize;

                // Set GS base to point to our TIB using arch_prctl
                const ARCH_SET_GS: i32 = 0x1001;
                libc::syscall(libc::SYS_arch_prctl, ARCH_SET_GS, tib_ptr as usize);
            });

            initialized.set(true);
        });
    }

    // // Global state for crash debugging
    // static mut DLL_MAP_BASE: usize = 0;
    // static mut DLL_MAP_SIZE: usize = 0;
    // static mut DLL_IMAGE_BASE: usize = 0;

    // unsafe extern "C" fn crash_handler(
    //     sig: i32,
    //     _info: *mut libc::siginfo_t,
    //     context: *mut c_void,
    // ) {
    //     let uc = context as *mut libc::ucontext_t;
    //     let rip = (*uc).uc_mcontext.gregs[libc::REG_RIP as usize] as u64;
    //     let rsp = (*uc).uc_mcontext.gregs[libc::REG_RSP as usize] as u64;
    //     let rax = (*uc).uc_mcontext.gregs[libc::REG_RAX as usize] as u64;
    //     let rbx = (*uc).uc_mcontext.gregs[libc::REG_RBX as usize] as u64;
    //     let rcx = (*uc).uc_mcontext.gregs[libc::REG_RCX as usize] as u64;
    //     let rdx = (*uc).uc_mcontext.gregs[libc::REG_RDX as usize] as u64;
    //     let rsi = (*uc).uc_mcontext.gregs[libc::REG_RSI as usize] as u64;
    //     let rdi = (*uc).uc_mcontext.gregs[libc::REG_RDI as usize] as u64;
    //     let r8 = (*uc).uc_mcontext.gregs[libc::REG_R8 as usize] as u64;
    //     let r9 = (*uc).uc_mcontext.gregs[libc::REG_R9 as usize] as u64;

    //     eprintln!("\n============================================================");
    //     eprintln!("CRASH: Signal {} at RIP=0x{:016x}", sig, rip);
    //     eprintln!("============================================================");

    //     // Check if crash is in DLL
    //     let map_base = DLL_MAP_BASE;
    //     let map_size = DLL_MAP_SIZE;
    //     let image_base = DLL_IMAGE_BASE;

    //     if map_base != 0 && (rip as usize) >= map_base && (rip as usize) < map_base + map_size {
    //         let dll_offset = rip as usize - map_base;
    //         let dll_rva = dll_offset; // RVA from start of image
    //         let original_va = image_base + dll_offset; // VA in original DLL

    //         eprintln!("CRASH IN DLL:");
    //         eprintln!("  Map base:        0x{:016x}", map_base);
    //         eprintln!("  Crash RIP:       0x{:016x}", rip);
    //         eprintln!("  DLL offset/RVA:  0x{:08x}", dll_rva);
    //         eprintln!("  Original VA:     0x{:016x}", original_va);
    //         eprintln!("");
    //         eprintln!("To debug in IDA/Ghidra, go to address: 0x{:x}", original_va);
    //         eprintln!(
    //             "Or use RVA: 0x{:x} from image base 0x{:x}",
    //             dll_rva, image_base
    //         );
    //     } else {
    //         eprintln!(
    //             "Crash outside DLL (map_base=0x{:x}, size=0x{:x})",
    //             map_base, map_size
    //         );
    //     }

    //     eprintln!("\nRegisters:");
    //     eprintln!("  RAX=0x{:016x}  RBX=0x{:016x}", rax, rbx);
    //     eprintln!("  RCX=0x{:016x}  RDX=0x{:016x}", rcx, rdx);
    //     eprintln!("  RSI=0x{:016x}  RDI=0x{:016x}", rsi, rdi);
    //     eprintln!("  R8 =0x{:016x}  R9 =0x{:016x}", r8, r9);
    //     eprintln!("  RSP=0x{:016x}  RIP=0x{:016x}", rsp, rip);

    //     // Dump stack
    //     eprintln!("\nStack (top 16 qwords):");
    //     let stack = rsp as *const u64;
    //     for i in 0..16 {
    //         let addr = stack.add(i);
    //         let val = *addr;
    //         let in_dll = (val as usize) >= map_base && (val as usize) < map_base + map_size;
    //         if in_dll {
    //             let rva = val as usize - map_base;
    //             eprintln!(
    //                 "  [RSP+0x{:02x}] 0x{:016x}  <- DLL RVA 0x{:x}",
    //                 i * 8,
    //                 val,
    //                 rva
    //             );
    //         } else {
    //             eprintln!("  [RSP+0x{:02x}] 0x{:016x}", i * 8, val);
    //         }
    //     }

    //     // Dump bytes at RIP
    //     eprintln!("\nCode at RIP:");
    //     let code = rip as *const u8;
    //     eprint!("  ");
    //     for i in 0..32 {
    //         eprint!("{:02x} ", *code.add(i));
    //     }
    //     eprintln!();

    //     eprintln!("============================================================\n");

    //     std::process::abort();
    // }

    // pub fn install_crash_handler() {
    //     unsafe {
    //         let mut sa: libc::sigaction = std::mem::zeroed();
    //         sa.sa_sigaction = crash_handler as usize;
    //         sa.sa_flags = libc::SA_SIGINFO;
    //         libc::sigaction(libc::SIGSEGV, &sa, std::ptr::null_mut());
    //         libc::sigaction(libc::SIGBUS, &sa, std::ptr::null_mut());
    //         libc::sigaction(libc::SIGILL, &sa, std::ptr::null_mut());
    //     }
    // }

    #[cfg(feature = "embed-dll")]
    static EMBEDDED_DLL: &[u8] =
        include_bytes_aligned::include_bytes_aligned!(8, "../../d3dcompiler_47.dll");

    pub fn load_dll() -> Result<D3DCompilerState> {
        #[cfg(feature = "embed-dll")]
        let dll: &[u8] = EMBEDDED_DLL;

        #[cfg(not(feature = "embed-dll"))]
        let dll_vec = std::fs::read(get_dll_path())?;
        #[cfg(not(feature = "embed-dll"))]
        let dll: &[u8] = &dll_vec;

        let obj_file =
            PeFile64::parse(dll).map_err(|e| D3DCompilerError::ParseError(e.to_string()))?;

        let size = obj_file.nt_headers().optional_header.size_of_image() as usize;
        let header_size = obj_file.nt_headers().optional_header.size_of_headers() as usize;
        let image_base = obj_file.relative_address_base() as usize;

        // Allocate memory for the image
        let mmap = unsafe {
            let ptr = libc::mmap(
                std::ptr::null_mut(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            );
            if ptr == libc::MAP_FAILED {
                return Err(D3DCompilerError::LoadError("mmap failed".into()));
            }
            std::slice::from_raw_parts_mut(ptr as *mut u8, size)
        };

        let map_base = mmap.as_ptr();

        // Store globals for crash handler and import tracing
        imports::DLL_MAP_BASE.store(map_base as usize, std::sync::atomic::Ordering::Relaxed);
        imports::DLL_MAP_SIZE.store(size, std::sync::atomic::Ordering::Relaxed);
        imports::DLL_IMAGE_BASE.store(image_base, std::sync::atomic::Ordering::Relaxed);

        // // Install crash handler
        // install_crash_handler();
        // eprintln!("[d3dcompiler] Crash handler installed");
        // eprintln!(
        //     "[d3dcompiler] DLL mapped at 0x{:x}, size 0x{:x}, image base 0x{:x}",
        //     map_base as usize, size, image_base
        // );

        // Copy header
        mmap[0..header_size].copy_from_slice(&dll[0..header_size]);
        unsafe {
            libc::mprotect(
                mmap.as_mut_ptr() as *mut c_void,
                header_size,
                libc::PROT_READ,
            );
        }

        // Copy sections
        for section in obj_file.sections() {
            let address = section.address() as usize;
            if let Ok(data) = section.data() {
                let offset = address - image_base;
                if offset + data.len() <= mmap.len() {
                    mmap[offset..offset + data.len()].copy_from_slice(data);
                }
            }
        }

        // Apply relocations
        let sections = obj_file.section_table();
        if let Ok(Some(mut blocks)) = obj_file
            .data_directories()
            .relocation_blocks(dll, &sections)
        {
            while let Ok(Some(block)) = blocks.next() {
                let block_address = block.virtual_address();
                let block_data = sections.pe_data_at(dll, block_address).map(object::Bytes);
                for reloc in block {
                    let offset = (reloc.virtual_address - block_address) as usize;
                    if reloc.typ == IMAGE_REL_BASED_DIR64
                        && let Some(addend) = block_data
                            .and_then(|data| data.read_at::<object::U64Bytes<LE>>(offset).ok())
                            .map(|addend| addend.get(LE))
                    {
                        let target = reloc.virtual_address as usize;
                        if target + 8 <= mmap.len() {
                            let new_addr = addend - image_base as u64 + map_base as u64;
                            mmap[target..target + 8].copy_from_slice(&new_addr.to_le_bytes());
                        }
                    }
                }
            }
        }

        // Fix up imports
        let mut unresolved = Vec::new();
        if let Ok(Some(import_table)) = obj_file.import_table()
            && let Ok(mut import_descs) = import_table.descriptors()
        {
            while let Ok(Some(import_desc)) = import_descs.next() {
                // Get DLL name for this import descriptor
                let dll_name = import_table
                    .name(import_desc.name.get(LE))
                    .ok()
                    .map(|n| String::from_utf8_lossy(n).to_lowercase())
                    .unwrap_or_default();

                if let Ok(mut thunks) =
                    import_table.thunks(import_desc.original_first_thunk.get(LE))
                {
                    let mut address = import_desc.first_thunk.get(LE) as usize;
                    while let Ok(Some(thunk)) = thunks.next::<ImageNtHeaders64>() {
                        if let Ok((_hint, name)) = import_table.hint_name(thunk.address()) {
                            let name = String::from_utf8_lossy(name).to_string();
                            let Some(fn_addr) = resolve_import(&dll_name, &name) else {
                                unresolved.push(format!("{dll_name}!{name}"));
                                address += 8;
                                continue;
                            };
                            if address + 8 <= mmap.len() {
                                mmap[address..address + 8].copy_from_slice(&fn_addr.to_le_bytes());
                            }
                        }
                        address += 8;
                    }
                }
            }
        }
        if !unresolved.is_empty() {
            panic!(
                "d3dcompiler_47.dll has {} unimplemented imports: {}",
                unresolved.len(),
                unresolved.join(", ")
            );
        }

        // Build export table
        let mut exports = HashMap::new();
        if let Ok(export_list) = obj_file.exports() {
            for export in export_list {
                let name = String::from_utf8_lossy(export.name());
                let address = export.address() - image_base as u64 + map_base as u64;
                exports.insert(name.to_string(), address as *const c_void);
            }
        }

        // Fix section permissions
        for section in obj_file.sections() {
            let address = section.address() as usize;
            if let Ok(data) = section.data() {
                let size = data.len();
                let mut permissions = 0;

                let flags = match section.flags() {
                    object::SectionFlags::Coff { characteristics } => characteristics,
                    _ => continue,
                };

                if flags & IMAGE_SCN_MEM_READ != 0 {
                    permissions |= libc::PROT_READ;
                }
                if flags & IMAGE_SCN_MEM_WRITE != 0 {
                    permissions |= libc::PROT_WRITE;
                }
                if flags & IMAGE_SCN_MEM_EXECUTE != 0 {
                    permissions |= libc::PROT_EXEC;
                }

                unsafe {
                    libc::mprotect(
                        mmap.as_mut_ptr().add(address - image_base) as *mut c_void,
                        size,
                        permissions,
                    );
                }
            }
        }

        // eprintln!("[d3dcompiler] Found {} exports", exports.len());
        // for (name, addr) in &exports {
        //     eprintln!("[d3dcompiler]   {} @ {:p}", name, *addr);
        // }

        // Get function pointers from exports
        let get_fn = |name: &str| -> Result<*const c_void> {
            exports
                .get(name)
                .copied()
                .ok_or_else(|| D3DCompilerError::FunctionNotFound(name.into()))
        };

        // Set up TIB before calling into DLL
        unsafe {
            setup_tib();
        }

        // Call DllMain via PE entry point (DLL_PROCESS_ATTACH = 1)
        let entry_rva = obj_file
            .nt_headers()
            .optional_header
            .address_of_entry_point();
        if entry_rva != 0 {
            let entry_addr = map_base as usize + entry_rva as usize;
            // eprintln!(
            //     "[d3dcompiler] Calling DllMain at entry point 0x{:x} (RVA 0x{:x})...",
            //     entry_addr, entry_rva
            // );

            // Call with win64 ABI: DllMain(hModule, DLL_PROCESS_ATTACH, lpReserved)
            let _result = unsafe {
                type DllMain = unsafe extern "win64" fn(
                    hinst: *const (),
                    fdw_reason: u32,
                    lpv_reserved: *mut (),
                ) -> bool;

                let dll_main = std::mem::transmute::<usize, DllMain>(entry_addr);
                dll_main(
                    map_base.cast(),
                    1,                    // DLL_PROCESS_ATTACH
                    std::ptr::null_mut(), // NULL
                )
            };
            // eprintln!("[d3dcompiler] DllMain returned: {}", result);
        } else {
            // eprintln!("[d3dcompiler] No entry point found (this is unusual for a DLL)");
        }

        // eprintln!("[d3dcompiler] Resolving D3D exports...");

        unsafe {
            let state = D3DCompilerState {
                _mmap: mmap.as_mut_ptr(),
                _mmap_size: size,
                d3d_compile: std::mem::transmute(get_fn("D3DCompile")?),
                d3d_compile2: std::mem::transmute(get_fn("D3DCompile2")?),
                d3d_compile_from_file: std::mem::transmute(get_fn("D3DCompileFromFile")?),
                d3d_preprocess: std::mem::transmute(get_fn("D3DPreprocess")?),
                d3d_disassemble: std::mem::transmute(get_fn("D3DDisassemble")?),
                d3d_create_blob: std::mem::transmute(get_fn("D3DCreateBlob")?),
                d3d_reflect: std::mem::transmute(get_fn("D3DReflect")?),
                d3d_strip_shader: std::mem::transmute(get_fn("D3DStripShader")?),
                d3d_get_blob_part: std::mem::transmute(get_fn("D3DGetBlobPart")?),
                d3d_set_blob_part: std::mem::transmute(get_fn("D3DSetBlobPart")?),
            };
            // eprintln!("[d3dcompiler] DLL loaded successfully!");
            Ok(state)
        }
    }

    // Import resolver - resolves by DLL name and import name, None if no shim exists
    fn resolve_import(dll: &str, name: &str) -> Option<usize> {
        // Normalize DLL name (remove .dll extension if present)
        let dll_base = dll.trim_end_matches(".dll");

        // Per-DLL resolvers return 0 for unknown names
        let addr = match dll_base {
            "msvcrt"
            | "msvcr100"
            | "msvcr110"
            | "msvcr120"
            | "vcruntime140"
            | "ucrtbase"
            | "api-ms-win-crt-runtime-l1-1-0"
            | "api-ms-win-crt-heap-l1-1-0"
            | "api-ms-win-crt-string-l1-1-0"
            | "api-ms-win-crt-stdio-l1-1-0"
            | "api-ms-win-crt-math-l1-1-0"
            | "api-ms-win-crt-convert-l1-1-0"
            | "api-ms-win-crt-utility-l1-1-0"
            | "api-ms-win-crt-time-l1-1-0"
            | "api-ms-win-crt-locale-l1-1-0"
            | "api-ms-win-crt-environment-l1-1-0"
            | "api-ms-win-crt-filesystem-l1-1-0"
            | "api-ms-win-crt-private-l1-1-0" => resolve_msvcrt(name),
            "kernel32"
            | "api-ms-win-core-heap-l1-1-0"
            | "api-ms-win-core-synch-l1-1-0"
            | "api-ms-win-core-synch-l1-2-0"
            | "api-ms-win-core-file-l1-1-0"
            | "api-ms-win-core-file-l1-2-0"
            | "api-ms-win-core-file-l2-1-0"
            | "api-ms-win-core-processthreads-l1-1-0"
            | "api-ms-win-core-processthreads-l1-1-1"
            | "api-ms-win-core-libraryloader-l1-1-0"
            | "api-ms-win-core-libraryloader-l1-2-0"
            | "api-ms-win-core-memory-l1-1-0"
            | "api-ms-win-core-localization-l1-2-0"
            | "api-ms-win-core-sysinfo-l1-1-0"
            | "api-ms-win-core-errorhandling-l1-1-0"
            | "api-ms-win-core-profile-l1-1-0"
            | "api-ms-win-core-string-l1-1-0"
            | "api-ms-win-core-debug-l1-1-0"
            | "api-ms-win-core-handle-l1-1-0"
            | "api-ms-win-core-fibers-l1-1-0"
            | "api-ms-win-core-fibers-l1-1-1" => resolve_kernel32(name),
            "advapi32" | "api-ms-win-core-registry-l1-1-0" | "api-ms-win-security-base-l1-1-0" => {
                resolve_advapi32(name)
            }
            "ntdll" => resolve_ntdll(name),
            "rpcrt4" => resolve_rpcrt4(name),
            _ => 0,
        };
        (addr != 0).then_some(addr)
    }

    fn resolve_msvcrt(name: &str) -> usize {
        match name {
            // memory
            "malloc" => imports::msvcrt::malloc as *const () as usize,
            "free" => imports::msvcrt::free as *const () as usize,
            "??2@YAPEAX_K@Z" => imports::msvcrt::op_new as *const () as usize,
            "??3@YAXPEAX@Z" => imports::msvcrt::op_delete as *const () as usize,
            "??_U@YAPEAX_K@Z" => imports::msvcrt::op_new_array as *const () as usize,
            "??_V@YAXPEAX@Z" => imports::msvcrt::op_delete_array as *const () as usize,
            "memcpy" => imports::msvcrt::memcpy as *const () as usize,
            "memcpy_s" => imports::msvcrt::memcpy_s as *const () as usize,
            "memmove" => imports::msvcrt::memmove as *const () as usize,
            "memset" => imports::msvcrt::memset as *const () as usize,
            "memcmp" => imports::msvcrt::memcmp as *const () as usize,
            "_memicmp" => imports::msvcrt::_memicmp as *const () as usize,

            // string
            "strlen" => imports::msvcrt::strlen as *const () as usize,
            "strcmp" => imports::msvcrt::strcmp as *const () as usize,
            "strncmp" => imports::msvcrt::strncmp as *const () as usize,
            "strcpy_s" => imports::msvcrt::strcpy_s as *const () as usize,
            "strncpy_s" => imports::msvcrt::strncpy_s as *const () as usize,
            "strcat_s" => imports::msvcrt::strcat_s as *const () as usize,
            "strchr" => imports::msvcrt::strchr as *const () as usize,
            "strrchr" => imports::msvcrt::strrchr as *const () as usize,
            "strstr" => imports::msvcrt::strstr as *const () as usize,
            "strnlen" => imports::msvcrt::strnlen as *const () as usize,
            "_strdup" => imports::msvcrt::_strdup as *const () as usize,
            "_stricmp" => imports::msvcrt::_stricmp as *const () as usize,
            "_strnicmp" => imports::msvcrt::_strnicmp as *const () as usize,
            "tolower" => imports::msvcrt::tolower as *const () as usize,
            "toupper" => imports::msvcrt::toupper as *const () as usize,
            "towlower" => imports::msvcrt::towlower as *const () as usize,
            "isalnum" => imports::msvcrt::isalnum as *const () as usize,
            "isalpha" => imports::msvcrt::isalpha as *const () as usize,
            "isdigit" => imports::msvcrt::isdigit as *const () as usize,
            "iswdigit" => imports::msvcrt::iswdigit as *const () as usize,
            "isspace" => imports::msvcrt::isspace as *const () as usize,
            "isxdigit" => imports::msvcrt::isxdigit as *const () as usize,
            "__isascii" => imports::msvcrt::__isascii as *const () as usize,

            // wide string
            "wcsncmp" => imports::msvcrt::wcsncmp as *const () as usize,
            "wcsncpy_s" => imports::msvcrt::wcsncpy_s as *const () as usize,
            "wcsncat_s" => imports::msvcrt::wcsncat_s as *const () as usize,
            "wcscat_s" => imports::msvcrt::wcscat_s as *const () as usize,
            "wcscpy_s" => imports::msvcrt::wcscpy_s as *const () as usize,
            "wcschr" => imports::msvcrt::wcschr as *const () as usize,
            "wcsrchr" => imports::msvcrt::wcsrchr as *const () as usize,
            "_wcsdup" => imports::msvcrt::_wcsdup as *const () as usize,
            "_wcsicmp" => imports::msvcrt::_wcsicmp as *const () as usize,
            "_wcsnicmp" => imports::msvcrt::_wcsnicmp as *const () as usize,
            "_mbscmp" => imports::msvcrt::_mbscmp as *const () as usize,
            "_mbstrlen" => imports::msvcrt::_mbstrlen as *const () as usize,

            // printf/scanf
            "sprintf_s" => imports::msvcrt::sprintf_s as *const () as usize,
            "sscanf" => imports::msvcrt::sscanf as *const () as usize,
            "vsprintf_s" => imports::msvcrt::vsprintf_s as *const () as usize,
            "sscanf_s" => imports::msvcrt::sscanf_s as *const () as usize,
            "swprintf_s" => imports::msvcrt::swprintf_s as *const () as usize,
            "_vsnprintf" => imports::msvcrt::_vsnprintf as *const () as usize,
            "_vsnwprintf" => imports::msvcrt::_vsnwprintf as *const () as usize,
            "_snwprintf_s" => imports::msvcrt::_snwprintf_s as *const () as usize,

            // file I/O
            "fclose" => imports::msvcrt::fclose as *const () as usize,
            "fread" => imports::msvcrt::fread as *const () as usize,
            "fseek" => imports::msvcrt::fseek as *const () as usize,
            "ftell" => imports::msvcrt::ftell as *const () as usize,
            "_wfsopen" => imports::msvcrt::_wfsopen as *const () as usize,
            "_fileno" => imports::msvcrt::_fileno as *const () as usize,
            "_filelengthi64" => imports::msvcrt::_filelengthi64 as *const () as usize,
            "_read" => imports::msvcrt::_read as *const () as usize,
            "_write" => imports::msvcrt::_write as *const () as usize,
            "_close" => imports::msvcrt::_close as *const () as usize,
            "_lseeki64" => imports::msvcrt::_lseeki64 as *const () as usize,
            "_chsize_s" => imports::msvcrt::_chsize_s as *const () as usize,
            "_chsize" => imports::msvcrt::_chsize as *const () as usize,
            "_wsopen" => imports::msvcrt::_wsopen as *const () as usize,
            "_get_osfhandle" => imports::msvcrt::_get_osfhandle as *const () as usize,
            "_open_osfhandle" => imports::msvcrt::_open_osfhandle as *const () as usize,

            // math
            "acos" => imports::msvcrt::acos as *const () as usize,
            "asin" => imports::msvcrt::asin as *const () as usize,
            "atan" => imports::msvcrt::atan as *const () as usize,
            "atan2" => imports::msvcrt::atan2 as *const () as usize,
            "ceil" => imports::msvcrt::ceil as *const () as usize,
            "cos" => imports::msvcrt::cos as *const () as usize,
            "cosh" => imports::msvcrt::cosh as *const () as usize,
            "exp" => imports::msvcrt::exp as *const () as usize,
            "floor" => imports::msvcrt::floor as *const () as usize,
            "floorf" => imports::msvcrt::floorf as *const () as usize,
            "fmod" => imports::msvcrt::fmod as *const () as usize,
            "log" => imports::msvcrt::log as *const () as usize,
            "modf" => imports::msvcrt::modf as *const () as usize,
            "pow" => imports::msvcrt::pow as *const () as usize,
            "sin" => imports::msvcrt::sin as *const () as usize,
            "sinh" => imports::msvcrt::sinh as *const () as usize,
            "sqrt" => imports::msvcrt::sqrt as *const () as usize,
            "tan" => imports::msvcrt::tan as *const () as usize,
            "tanh" => imports::msvcrt::tanh as *const () as usize,
            "_isnan" => imports::msvcrt::_isnan as *const () as usize,
            "_finite" => imports::msvcrt::_finite as *const () as usize,
            "_fpclass" => imports::msvcrt::_fpclass as *const () as usize,
            "_clearfp" => imports::msvcrt::_clearfp as *const () as usize,
            "_controlfp" => imports::msvcrt::_controlfp as *const () as usize,

            // conversion
            "atoi" => imports::msvcrt::atoi as *const () as usize,
            "atof" => imports::msvcrt::atof as *const () as usize,
            "_atoi64" => imports::msvcrt::_atoi64 as *const () as usize,
            "strtod" => imports::msvcrt::strtod as *const () as usize,
            "strtoul" => imports::msvcrt::strtoul as *const () as usize,
            "wcstoul" => imports::msvcrt::wcstoul as *const () as usize,
            "wcstol" => imports::msvcrt::wcstol as *const () as usize,
            "_strtoui64" => imports::msvcrt::_strtoui64 as *const () as usize,

            // other
            "qsort" => imports::msvcrt::qsort as *const () as usize,
            "bsearch" => imports::msvcrt::bsearch as *const () as usize,
            "getenv" => imports::msvcrt::getenv as *const () as usize,
            "_wgetenv" => imports::msvcrt::_wgetenv as *const () as usize,
            "setlocale" => imports::msvcrt::setlocale as *const () as usize,
            "_time64" => imports::msvcrt::_time64 as *const () as usize,
            "_errno" => imports::msvcrt::_errno as *const () as usize,

            // CRT init
            "_initterm" => imports::msvcrt::_initterm as *const () as usize,
            "_amsg_exit" => imports::msvcrt::_amsg_exit as *const () as usize,
            "_purecall" => imports::msvcrt::_purecall as *const () as usize,
            "_onexit" => imports::msvcrt::_onexit as *const () as usize,
            "__dllonexit" => imports::msvcrt::__dllonexit as *const () as usize,
            "_lock" => imports::msvcrt::_lock as *const () as usize,
            "_unlock" => imports::msvcrt::_unlock as *const () as usize,
            "_callnewh" => imports::msvcrt::_callnewh as *const () as usize,

            // exceptions
            "__C_specific_handler" => imports::msvcrt::__C_specific_handler as *const () as usize,
            "__CxxFrameHandler3" => imports::msvcrt::__CxxFrameHandler3 as *const () as usize,
            "_CxxThrowException" => imports::msvcrt::_CxxThrowException as *const () as usize,
            "?terminate@@YAXXZ" => imports::msvcrt::terminate as *const () as usize,
            "??1type_info@@UEAA@XZ" => imports::msvcrt::type_info_dtor as *const () as usize,
            "__unDName" => imports::msvcrt::__unDName as *const () as usize,
            "_XcptFilter" => imports::msvcrt::_XcptFilter as *const () as usize,

            // path
            "_wfullpath" => imports::msvcrt::_wfullpath as *const () as usize,
            "_wmakepath_s" => imports::msvcrt::_wmakepath_s as *const () as usize,
            "_wsplitpath_s" => imports::msvcrt::_wsplitpath_s as *const () as usize,
            _ => 0,
        }
    }

    fn resolve_kernel32(name: &str) -> usize {
        match name {
            // memory
            "VirtualAlloc" => imports::kernel32::VirtualAlloc as *const () as usize,
            "VirtualFree" => imports::kernel32::VirtualFree as *const () as usize,
            "GetProcessHeap" => imports::kernel32::GetProcessHeap as *const () as usize,
            "HeapCreate" => imports::kernel32::HeapCreate as *const () as usize,
            "HeapDestroy" => imports::kernel32::HeapDestroy as *const () as usize,
            "HeapAlloc" => imports::kernel32::HeapAlloc as *const () as usize,
            "HeapFree" => imports::kernel32::HeapFree as *const () as usize,
            "LocalAlloc" => imports::kernel32::LocalAlloc as *const () as usize,
            "LocalFree" => imports::kernel32::LocalFree as *const () as usize,

            // file
            "CreateFileW" => imports::kernel32::CreateFileW as *const () as usize,
            "CreateFileA" => imports::kernel32::CreateFileA as *const () as usize,
            "ReadFile" => imports::kernel32::ReadFile as *const () as usize,
            "WriteFile" => imports::kernel32::WriteFile as *const () as usize,
            "CloseHandle" => imports::kernel32::CloseHandle as *const () as usize,
            "GetFileSize" => imports::kernel32::GetFileSize as *const () as usize,
            "GetFileSizeEx" => imports::kernel32::GetFileSizeEx as *const () as usize,
            "GetFileType" => imports::kernel32::GetFileType as *const () as usize,
            "SetFilePointer" => imports::kernel32::SetFilePointer as *const () as usize,
            "SetFilePointerEx" => imports::kernel32::SetFilePointerEx as *const () as usize,
            "SetEndOfFile" => imports::kernel32::SetEndOfFile as *const () as usize,
            "CopyFileExW" => imports::kernel32::CopyFileExW as *const () as usize,
            "DeleteFileW" => imports::kernel32::DeleteFileW as *const () as usize,
            "GetFileAttributesW" => imports::kernel32::GetFileAttributesW as *const () as usize,
            "SetFileAttributesW" => imports::kernel32::SetFileAttributesW as *const () as usize,
            "GetFullPathNameW" => imports::kernel32::GetFullPathNameW as *const () as usize,
            "GetFullPathNameA" => imports::kernel32::GetFullPathNameA as *const () as usize,
            "CreateFileMappingW" => imports::kernel32::CreateFileMappingW as *const () as usize,
            "MapViewOfFile" => imports::kernel32::MapViewOfFile as *const () as usize,
            "MapViewOfFileEx" => imports::kernel32::MapViewOfFileEx as *const () as usize,
            "UnmapViewOfFile" => imports::kernel32::UnmapViewOfFile as *const () as usize,
            "FlushViewOfFile" => imports::kernel32::FlushViewOfFile as *const () as usize,
            "DeviceIoControl" => imports::kernel32::DeviceIoControl as *const () as usize,

            // sync
            "InitializeCriticalSection" => {
                imports::kernel32::InitializeCriticalSection as *const () as usize
            }
            "InitializeCriticalSectionAndSpinCount" => {
                imports::kernel32::InitializeCriticalSectionAndSpinCount as *const () as usize
            }
            "DeleteCriticalSection" => {
                imports::kernel32::DeleteCriticalSection as *const () as usize
            }
            "EnterCriticalSection" => imports::kernel32::EnterCriticalSection as *const () as usize,
            "LeaveCriticalSection" => imports::kernel32::LeaveCriticalSection as *const () as usize,
            "Sleep" => imports::kernel32::Sleep as *const () as usize,

            // TLS
            "TlsAlloc" => imports::kernel32::TlsAlloc as *const () as usize,
            "TlsFree" => imports::kernel32::TlsFree as *const () as usize,
            "TlsGetValue" => imports::kernel32::TlsGetValue as *const () as usize,
            "TlsSetValue" => imports::kernel32::TlsSetValue as *const () as usize,

            // misc
            "GetLastError" => imports::kernel32::GetLastError as *const () as usize,
            "SetLastError" => imports::kernel32::SetLastError as *const () as usize,
            "GetCurrentProcessId" => imports::kernel32::GetCurrentProcessId as *const () as usize,
            "GetCurrentThreadId" => imports::kernel32::GetCurrentThreadId as *const () as usize,
            "GetCurrentProcess" => imports::kernel32::GetCurrentProcess as *const () as usize,
            "GetTickCount" => imports::kernel32::GetTickCount as *const () as usize,
            "QueryPerformanceCounter" => {
                imports::kernel32::QueryPerformanceCounter as *const () as usize
            }
            "GetSystemTimeAsFileTime" => {
                imports::kernel32::GetSystemTimeAsFileTime as *const () as usize
            }
            "GetSystemInfo" => imports::kernel32::GetSystemInfo as *const () as usize,
            "OutputDebugStringA" => imports::kernel32::OutputDebugStringA as *const () as usize,
            "DisableThreadLibraryCalls" => {
                imports::kernel32::DisableThreadLibraryCalls as *const () as usize
            }
            "FreeLibrary" => imports::kernel32::FreeLibrary as *const () as usize,
            "LoadLibraryExW" => imports::kernel32::LoadLibraryExW as *const () as usize,
            "GetProcAddress" => imports::kernel32::GetProcAddress as *const () as usize,
            "GetModuleFileNameA" => imports::kernel32::GetModuleFileNameA as *const () as usize,
            "GetEnvironmentVariableA" => {
                imports::kernel32::GetEnvironmentVariableA as *const () as usize
            }
            "ExpandEnvironmentStringsW" => {
                imports::kernel32::ExpandEnvironmentStringsW as *const () as usize
            }
            "MultiByteToWideChar" => imports::kernel32::MultiByteToWideChar as *const () as usize,
            "WideCharToMultiByte" => imports::kernel32::WideCharToMultiByte as *const () as usize,
            "LCMapStringW" => imports::kernel32::LCMapStringW as *const () as usize,
            "lstrcmpiA" => imports::kernel32::lstrcmpiA as *const () as usize,
            "TerminateProcess" => imports::kernel32::TerminateProcess as *const () as usize,
            "UnhandledExceptionFilter" => {
                imports::kernel32::UnhandledExceptionFilter as *const () as usize
            }
            "SetUnhandledExceptionFilter" => {
                imports::kernel32::SetUnhandledExceptionFilter as *const () as usize
            }
            "GetVersion" => imports::kernel32::GetVersion as *const () as usize,
            "IsDebuggerPresent" => imports::kernel32::IsDebuggerPresent as *const () as usize,
            "IsProcessorFeaturePresent" => {
                imports::kernel32::IsProcessorFeaturePresent as *const () as usize
            }

            "RtlCaptureContext" | "RtlLookupFunctionEntry" | "RtlVirtualUnwind" => {
                resolve_ntdll(name)
            }
            _ => 0,
        }
    }

    fn resolve_advapi32(name: &str) -> usize {
        match name {
            // registry
            "RegOpenKeyExA" => imports::advapi32::RegOpenKeyExA as *const () as usize,
            "RegOpenKeyExW" => imports::advapi32::RegOpenKeyExW as *const () as usize,
            "RegQueryValueExA" => imports::advapi32::RegQueryValueExA as *const () as usize,
            "RegQueryValueExW" => imports::advapi32::RegQueryValueExW as *const () as usize,
            "RegEnumKeyExA" => imports::advapi32::RegEnumKeyExA as *const () as usize,
            "RegCloseKey" => imports::advapi32::RegCloseKey as *const () as usize,

            // crypto
            "CryptAcquireContextW" => imports::advapi32::CryptAcquireContextW as *const () as usize,
            "CryptReleaseContext" => imports::advapi32::CryptReleaseContext as *const () as usize,
            "CryptCreateHash" => imports::advapi32::CryptCreateHash as *const () as usize,
            "CryptDestroyHash" => imports::advapi32::CryptDestroyHash as *const () as usize,
            "CryptHashData" => imports::advapi32::CryptHashData as *const () as usize,
            "CryptGetHashParam" => imports::advapi32::CryptGetHashParam as *const () as usize,
            _ => 0,
        }
    }

    fn resolve_ntdll(name: &str) -> usize {
        match name {
            "RtlCaptureContext" => imports::ntdll::RtlCaptureContext as *const () as usize,
            "RtlLookupFunctionEntry" => {
                imports::ntdll::RtlLookupFunctionEntry as *const () as usize
            }
            "RtlVirtualUnwind" => imports::ntdll::RtlVirtualUnwind as *const () as usize,
            "RtlUnwindEx" => imports::ntdll::RtlUnwindEx as *const () as usize,
            _ => 0,
        }
    }

    fn resolve_rpcrt4(name: &str) -> usize {
        match name {
            "UuidCreate" => imports::rpcrt4::UuidCreate as *const () as usize,
            _ => 0,
        }
    }
}
