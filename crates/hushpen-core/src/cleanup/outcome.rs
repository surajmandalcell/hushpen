//! How the AI step of a dictation ended. The key is what the history row stores in
//! `llm_outcome`; every outcome but `Used` means the rule text was inserted.

use crate::error::{LLM_GUARD_REJECTED, LLM_TIMEOUT, LLM_UNAVAILABLE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// AI cleanup is off, or the provider is not the local model.
    Off,
    /// The model's text was inserted.
    Used,
    Timeout,
    /// The worker failed or crashed.
    Error,
    /// The output failed a guard (see [`super::guard`]).
    GuardRejected,
    /// AI cleanup is on, but no worker could take the job (no model file yet, or the worker is
    /// down).
    Unavailable,
}

impl Outcome {
    pub fn key(self) -> &'static str {
        match self {
            Outcome::Off => "off",
            Outcome::Used => "used",
            Outcome::Timeout => "timeout",
            Outcome::Error => "error",
            Outcome::GuardRejected => "guard_rejected",
            Outcome::Unavailable => "unavailable",
        }
    }

    /// The outcome of a job that the worker or its supervisor failed with `code`.
    pub fn of_failure(code: &str) -> Self {
        match code {
            LLM_TIMEOUT => Outcome::Timeout,
            LLM_UNAVAILABLE | crate::error::LLM_NO_MODEL => Outcome::Unavailable,
            _ => Outcome::Error,
        }
    }

    /// The error code the hook shows next to the outcome, or `OK` for a used text.
    pub fn code(self, failure: Option<&str>) -> String {
        match self {
            Outcome::Used => "OK".to_owned(),
            Outcome::Off => "OFF".to_owned(),
            Outcome::GuardRejected => LLM_GUARD_REJECTED.to_owned(),
            Outcome::Timeout => LLM_TIMEOUT.to_owned(),
            Outcome::Unavailable => failure.unwrap_or(LLM_UNAVAILABLE).to_owned(),
            Outcome::Error => failure.unwrap_or(crate::error::LLM_CRASHED).to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{LLM_CRASHED, LLM_GENERATE_FAILED, LLM_LOAD_FAILED, LLM_NO_MODEL};

    #[test]
    fn the_keys_are_what_the_history_row_stores() {
        let keys: Vec<_> = [
            Outcome::Off,
            Outcome::Used,
            Outcome::Timeout,
            Outcome::Error,
            Outcome::GuardRejected,
            Outcome::Unavailable,
        ]
        .map(Outcome::key)
        .to_vec();
        assert_eq!(
            keys,
            [
                "off",
                "used",
                "timeout",
                "error",
                "guard_rejected",
                "unavailable"
            ]
        );
    }

    #[test]
    fn a_failure_code_gives_its_outcome() {
        assert_eq!(Outcome::of_failure(LLM_TIMEOUT), Outcome::Timeout);
        assert_eq!(Outcome::of_failure(LLM_UNAVAILABLE), Outcome::Unavailable);
        assert_eq!(Outcome::of_failure(LLM_NO_MODEL), Outcome::Unavailable);
        for code in [LLM_CRASHED, LLM_LOAD_FAILED, LLM_GENERATE_FAILED] {
            assert_eq!(Outcome::of_failure(code), Outcome::Error, "{code}");
        }
    }

    #[test]
    fn the_hook_code_names_the_failure() {
        assert_eq!(Outcome::Used.code(None), "OK");
        assert_eq!(Outcome::Unavailable.code(None), "LLM_UNAVAILABLE");
        assert_eq!(Outcome::Error.code(Some(LLM_CRASHED)), "LLM_CRASHED");
        assert_eq!(Outcome::GuardRejected.code(None), "LLM_GUARD_REJECTED");
        assert_eq!(Outcome::Timeout.code(None), "LLM_TIMEOUT");
    }
}
