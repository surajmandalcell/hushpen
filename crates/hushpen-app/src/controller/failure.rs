//! The words for a failed run: one place maps an error code to the code and the message
//! Home shows. Every failure after the recording keeps it.

use super::command::edit_failure_code;
use hushpen_core::error::{
    self, CAPTURE_FAILED, ENGINE_CRASHED, ENGINE_NO_SPEECH, INSERT_KEYBOARD_GRABBED,
    INSERT_NO_PERMISSION, INSERT_NO_RECEIPT, INSERT_NO_TRANSCRIPT, INSERT_SECURE_FIELD,
};

/// The code and the message of a failed run. Every failure after the recording keeps it.
pub(crate) fn failure_for(code: &str) -> (&'static str, &'static str) {
    match code {
        error::ENGINE_UNAVAILABLE => (
            error::ENGINE_UNAVAILABLE,
            "The speech engine is not available. Try again in a moment. The recording was kept.",
        ),
        error::ENGINE_NO_MODEL => (
            error::ENGINE_NO_MODEL,
            "The speech model is not loaded yet. Try again in a moment. The recording was kept.",
        ),
        error::ENGINE_BAD_AUDIO => (
            error::ENGINE_BAD_AUDIO,
            "The recording could not be read. It was kept.",
        ),
        error::ENGINE_BAD_LANGUAGE => (
            error::ENGINE_BAD_LANGUAGE,
            "That language is not supported. Choose another one.",
        ),
        error::ENGINE_NO_SPEECH => (
            ENGINE_NO_SPEECH,
            "No speech was heard. Try again and speak a little closer to the microphone.",
        ),
        error::CAPTURE_FAILED => (CAPTURE_FAILED, "Recording stopped because of an error."),
        INSERT_NO_RECEIPT => (
            INSERT_NO_RECEIPT,
            "The app did not take the text. Use Paste last transcript to try again.",
        ),
        INSERT_NO_PERMISSION => (
            INSERT_NO_PERMISSION,
            "Hushpen is not allowed to press keys, so it could not paste. The text is on your clipboard. Use Paste last transcript once you allow it.",
        ),
        INSERT_SECURE_FIELD => (
            INSERT_SECURE_FIELD,
            "Secure input is on, so Hushpen did not paste. Your clipboard is unchanged. Use Paste last transcript in another field.",
        ),
        INSERT_KEYBOARD_GRABBED => (
            INSERT_KEYBOARD_GRABBED,
            "Another app holds the keyboard, so Hushpen did not paste. Your clipboard is unchanged. Use Paste last transcript when the keyboard is free.",
        ),
        INSERT_NO_TRANSCRIPT => (
            INSERT_NO_TRANSCRIPT,
            "No transcript is available yet. Dictate something first.",
        ),
        error::COMMAND_NO_SELECTION => (error::COMMAND_NO_SELECTION, "Select text first"),
        error::READ_NO_SELECTION => (error::READ_NO_SELECTION, "Select text first"),
        error::COMMAND_NEEDS_AI => (
            error::COMMAND_NEEDS_AI,
            "Command mode needs AI cleanup or the endpoint",
        ),
        error::LLM_TIMEOUT => (
            error::LLM_TIMEOUT,
            "The AI edit took too long. Your text was not changed.",
        ),
        error::LLM_GUARD_REJECTED => (
            error::LLM_GUARD_REJECTED,
            "The AI answer could not be used. Your text was not changed.",
        ),
        error::LLM_UNAVAILABLE
        | error::LLM_LOAD_FAILED
        | error::LLM_NO_MODEL
        | error::LLM_PROTOCOL
        | error::LLM_CRASHED
        | error::LLM_PROMPT_TOO_LONG
        | error::LLM_GENERATE_FAILED
        | error::ENDPOINT_FAILED => (
            edit_failure_code(code),
            "The AI edit failed. Your text was not changed.",
        ),
        _ => (
            ENGINE_CRASHED,
            "The speech engine stopped while transcribing. The recording was kept.",
        ),
    }
}
