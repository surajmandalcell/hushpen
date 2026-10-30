//! ONNX Runtime for the Chatterbox models. The worker opens the library at run time, the same
//! file sherpa-onnx links, and keeps one environment for the whole process.

use ort::environment::Environment;
use ort::session::Session;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

static ENVIRONMENT: OnceLock<Environment> = OnceLock::new();
static OPENING: Mutex<()> = Mutex::new(());

/// The environment of the process. The first call opens `library`; a failed open is tried
/// again on the next call.
pub fn environment(library: &Path) -> Result<&'static Environment, String> {
    if let Some(environment) = ENVIRONMENT.get() {
        return Ok(environment);
    }
    let _opening = OPENING
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(environment) = ENVIRONMENT.get() {
        return Ok(environment);
    }
    let built = ort::init_from(library)
        .map_err(|problem| format!("cannot open ONNX Runtime: {problem}"))?
        .with_name("hushpen-tts")
        .with_telemetry(false)
        .build()
        .map_err(|problem| format!("cannot start ONNX Runtime: {}", problem.message()))?;
    Ok(ENVIRONMENT.get_or_init(|| built))
}

/// The version of the ONNX Runtime library at `library`, such as `1.28.2`. Its build string
/// does not hold it, so it is read from the C API.
#[allow(unsafe_code)]
pub fn version(library: &Path) -> Result<String, String> {
    #[repr(C)]
    struct ApiBase {
        _get_api: unsafe extern "C" fn(u32) -> *const std::ffi::c_void,
        get_version_string: unsafe extern "C" fn() -> *const std::ffi::c_char,
    }
    let failed = |problem: String| format!("cannot read the ONNX Runtime version: {problem}");
    // SAFETY: OrtGetApiBase takes nothing and returns a pointer to a static OrtApiBase, whose
    // fields are GetApi and GetVersionString in this order (onnxruntime_c_api.h). The version
    // is a static NUL-terminated string, copied before the library handle is dropped.
    unsafe {
        let opened =
            libloading::Library::new(library).map_err(|error| failed(error.to_string()))?;
        let api_base: libloading::Symbol<unsafe extern "C" fn() -> *const ApiBase> = opened
            .get(b"OrtGetApiBase\0")
            .map_err(|error| failed(error.to_string()))?;
        let base = api_base();
        if base.is_null() {
            return Err(failed("the library has no API".into()));
        }
        let text = ((*base).get_version_string)();
        if text.is_null() {
            return Err(failed("the library gave none".into()));
        }
        Ok(std::ffi::CStr::from_ptr(text)
            .to_string_lossy()
            .into_owned())
    }
}

/// Opens one graph on the CPU with `threads` threads for each operator.
pub fn session(environment: &Environment, graph: &Path, threads: usize) -> Result<Session, String> {
    let failed = |message: &str| format!("cannot open {}: {message}", graph.display());
    Session::builder(environment)
        .map_err(|problem| failed(problem.message()))?
        .with_intra_threads(threads.max(1))
        .map_err(|problem| failed(problem.message()))?
        .with_inter_threads(1)
        .map_err(|problem| failed(problem.message()))?
        .commit_from_file(graph)
        .map_err(|problem| failed(problem.message()))
}
