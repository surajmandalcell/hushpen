//! The one list of error codes. Messages and logs carry a code; the UI maps a code to text and
//! never matches English error text. New codes go here and nowhere else.

pub const ENGINE_UNAVAILABLE: &str = "ENGINE_UNAVAILABLE";
pub const ENGINE_CRASHED: &str = "ENGINE_CRASHED";
pub const ENGINE_NO_SPEECH: &str = "ENGINE_NO_SPEECH";
pub const ENGINE_LOAD_FAILED: &str = "ENGINE_LOAD_FAILED";
pub const ENGINE_BAD_LANGUAGE: &str = "ENGINE_BAD_LANGUAGE";
/// A request the engine child could not read or does not know.
pub const ENGINE_PROTOCOL: &str = "ENGINE_PROTOCOL";
/// A job arrived before any model was loaded.
pub const ENGINE_NO_MODEL: &str = "ENGINE_NO_MODEL";
/// The WAV file of a job could not be read or has an unsupported format.
pub const ENGINE_BAD_AUDIO: &str = "ENGINE_BAD_AUDIO";
pub const IMPORT_UNSUPPORTED_CODEC: &str = "IMPORT_UNSUPPORTED_CODEC";
pub const IMPORT_DECODE_FAILED: &str = "IMPORT_DECODE_FAILED";
pub const CAPTURE_FAILED: &str = "CAPTURE_FAILED";
pub const CAPTURE_RECOVERED: &str = "CAPTURE_RECOVERED";
pub const MIC_PERMISSION: &str = "MIC_PERMISSION";
pub const MIC_UNAVAILABLE: &str = "MIC_UNAVAILABLE";
pub const MODEL_IN_USE: &str = "MODEL_IN_USE";
pub const MODEL_HASH_MISMATCH: &str = "MODEL_HASH_MISMATCH";
pub const MODEL_HOST_BLOCKED: &str = "MODEL_HOST_BLOCKED";
pub const DOWNLOAD_FAILED: &str = "DOWNLOAD_FAILED";
pub const INSERT_SECURE_FIELD: &str = "INSERT_SECURE_FIELD";
pub const INSERT_KEYBOARD_GRABBED: &str = "INSERT_KEYBOARD_GRABBED";
pub const INSERT_NO_PERMISSION: &str = "INSERT_NO_PERMISSION";
pub const INSERT_NO_RECEIPT: &str = "INSERT_NO_RECEIPT";
pub const INSERT_WAYLAND: &str = "INSERT_WAYLAND";
/// "Paste last transcript" was used before any dictation had text.
pub const INSERT_NO_TRANSCRIPT: &str = "INSERT_NO_TRANSCRIPT";
pub const SHORTCUT_IN_USE: &str = "SHORTCUT_IN_USE";
pub const SHORTCUT_RESERVED: &str = "SHORTCUT_RESERVED";
/// The recorded keys are not a shortcut, such as a letter with no modifier.
pub const SHORTCUT_INVALID: &str = "SHORTCUT_INVALID";
pub const LLM_TIMEOUT: &str = "LLM_TIMEOUT";
pub const LLM_GUARD_REJECTED: &str = "LLM_GUARD_REJECTED";
pub const LLM_UNAVAILABLE: &str = "LLM_UNAVAILABLE";
pub const ENDPOINT_FAILED: &str = "ENDPOINT_FAILED";
pub const COMMAND_NO_SELECTION: &str = "COMMAND_NO_SELECTION";
pub const HISTORY_NEWER_SCHEMA: &str = "HISTORY_NEWER_SCHEMA";
pub const HISTORY_WRITE_FAILED: &str = "HISTORY_WRITE_FAILED";
pub const SETTINGS_CORRUPT: &str = "SETTINGS_CORRUPT";
pub const DATA_FOLDER_MOVE_FAILED: &str = "DATA_FOLDER_MOVE_FAILED";
pub const UPDATE_SIGNATURE_INVALID: &str = "UPDATE_SIGNATURE_INVALID";
pub const UPDATE_HASH_MISMATCH: &str = "UPDATE_HASH_MISMATCH";
pub const UPDATE_SWAP_FAILED: &str = "UPDATE_SWAP_FAILED";
