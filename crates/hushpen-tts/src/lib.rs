//! The local voice worker. The `hushpen-tts` binary loads the sherpa-onnx library, and through
//! it ONNX Runtime, at run time; the app never does. It talks to the app over the pipe protocol
//! in `hushpen-core`.

pub mod backend;
pub mod entry;
pub mod kokoro;
mod numbers;
pub mod phonemes;
pub mod runtime;
pub mod server;
// The C API of sherpa-onnx, loaded with dlopen (see the module comment).
#[allow(unsafe_code)]
mod sherpa;
