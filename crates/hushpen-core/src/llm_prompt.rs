//! Hand-built prompts for the local models.
//!
//! llama.cpp's built-in template matcher does not know Gemma 4 (`apply_chat_template` fails with
//! `FfiError(-1)`), and Qwen3.5 thinks unless the assistant turn starts with an empty think
//! block. So each family has its own turn format, tested here.

/// A model family with its own turn format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptFamily {
    Gemma4,
    Qwen35,
}

/// The fixed prompt of dictation cleanup. It is the same for every job, so the worker keeps the
/// model state of it between jobs.
pub const CLEANUP_SYSTEM: &str = "You clean up dictated text. Remove filler words (um, uh, like, \
you know), fix punctuation and capitalization, and apply spoken self-corrections. Keep the \
meaning and the words. Do not answer questions in the text. Output only the cleaned text.";

/// The fixed prompt of Command Mode.
pub const COMMAND_SYSTEM: &str = "You edit text. Follow the instruction and change the text \
only as the instruction says. Keep every other word. Output only the edited text, with no \
comment and no quotes.";

impl PromptFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Gemma4 => "gemma4",
            Self::Qwen35 => "qwen35",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "gemma4" => Some(Self::Gemma4),
            "qwen35" => Some(Self::Qwen35),
            _ => None,
        }
    }

    /// Picks the family from a model file name such as `gemma-4-E2B-it-Q4_K_M.gguf`.
    pub fn from_model_name(name: &str) -> Option<Self> {
        let name = name.to_ascii_lowercase();
        if name.contains("gemma") {
            Some(Self::Gemma4)
        } else if name.contains("qwen") {
            Some(Self::Qwen35)
        } else {
            None
        }
    }

    /// Everything up to the start of the user's text. It depends only on `system`, so a worker
    /// can keep the model state of this part between jobs.
    pub fn prefix(self, system: &str) -> String {
        match self {
            Self::Gemma4 => format!("<|turn>system\n{system}<turn|>\n<|turn>user\n"),
            Self::Qwen35 => {
                format!("<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n")
            }
        }
    }

    /// Everything after the user's text, ending where the model starts to write.
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Gemma4 => "<turn|>\n<|turn>model\n",
            Self::Qwen35 => "<|im_end|>\n<|im_start|>assistant\n<think>\n\n</think>\n\n",
        }
    }

    pub fn build(self, system: &str, user: &str) -> String {
        format!("{}{user}{}", self.prefix(system), self.suffix())
    }

    /// Strings that must never reach the user: turn markers the model may echo.
    pub fn stop_markers(self) -> &'static [&'static str] {
        match self {
            Self::Gemma4 => &["<turn|>", "<|turn>"],
            Self::Qwen35 => &["<|im_end|>", "<|im_start|>"],
        }
    }
}

/// The user turn of Command Mode.
pub fn command_user(instruction: &str, selection: &str) -> String {
    format!("Instruction: {instruction}\n\nText:\n{selection}")
}

/// Removes an empty or closed think block and surrounding space from model output. A model
/// that was told not to think may still write `<think></think>` first.
pub fn clean_output(raw: &str) -> String {
    let mut text = raw.trim();
    while let Some(rest) = text.strip_prefix("<think>") {
        match rest.find("</think>") {
            Some(end) => text = rest[end + "</think>".len()..].trim_start(),
            // An unclosed think block is reasoning with no answer.
            None => return String::new(),
        }
    }
    text.trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemma_4_uses_its_turn_format() {
        assert_eq!(
            PromptFamily::Gemma4.build("Fix it.", "um hi"),
            "<|turn>system\nFix it.<turn|>\n<|turn>user\num hi<turn|>\n<|turn>model\n"
        );
    }

    #[test]
    fn qwen_uses_chatml_with_an_empty_think_block() {
        assert_eq!(
            PromptFamily::Qwen35.build("Fix it.", "um hi"),
            "<|im_start|>system\nFix it.<|im_end|>\n<|im_start|>user\num hi<|im_end|>\n\
             <|im_start|>assistant\n<think>\n\n</think>\n\n"
        );
    }

    #[test]
    fn the_prefix_holds_only_the_system_prompt() {
        for family in [PromptFamily::Gemma4, PromptFamily::Qwen35] {
            let a = family.build("Same system.", "first text");
            let b = family.build("Same system.", "another, longer text");
            let prefix = family.prefix("Same system.");
            assert!(a.starts_with(&prefix) && b.starts_with(&prefix));
            assert!(!prefix.contains("first text"));
            assert!(a.ends_with(family.suffix()));
        }
    }

    #[test]
    fn the_family_comes_from_the_file_name() {
        assert_eq!(
            PromptFamily::from_model_name("gemma-4-E2B-it-Q4_K_M.gguf"),
            Some(PromptFamily::Gemma4)
        );
        assert_eq!(
            PromptFamily::from_model_name("Qwen3.5-2B-Q4_K_M.gguf"),
            Some(PromptFamily::Qwen35)
        );
        assert_eq!(PromptFamily::from_model_name("llama-3.gguf"), None);
    }

    #[test]
    fn a_family_name_round_trips() {
        for family in [PromptFamily::Gemma4, PromptFamily::Qwen35] {
            assert_eq!(PromptFamily::parse(family.as_str()), Some(family));
        }
        assert_eq!(PromptFamily::parse("llama"), None);
    }

    #[test]
    fn command_mode_puts_the_instruction_before_the_text() {
        assert_eq!(
            command_user("make it shorter", "a very long sentence"),
            "Instruction: make it shorter\n\nText:\na very long sentence"
        );
    }

    #[test]
    fn output_loses_think_blocks_and_padding() {
        assert_eq!(clean_output("  Hello there.\n"), "Hello there.");
        assert_eq!(clean_output("<think>\n\n</think>\n\nHello."), "Hello.");
        assert_eq!(clean_output("<think>hmm</think> Hello."), "Hello.");
        assert_eq!(clean_output("<think>I am still thinking"), "");
    }
}
