//! When "Undo cleanup" may be offered, and for how long.
//!
//! After an AI-cleaned text went into an app, the user has a few seconds to take it back for
//! the rule text. The offer needs proof that the text really arrived and that the app is the
//! same one, because the undo is a key press into whatever has the focus. Nothing here touches
//! a display or a pasteboard.

use super::method::is_terminal;
use super::report::{Outcome, Report};

/// How long "Undo cleanup" stays on offer after the text went in.
pub const UNDO_WINDOW_MS: u64 = 5_000;

/// Why no undo is offered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Declined {
    /// The text that went in is not the model's: the AI step is off, timed out, or failed.
    NotAiText,
    /// The model changed nothing, so there is nothing to take back.
    SameText,
    /// The target never read the text (a keyboard grab, a secure field, a lost paste, or a
    /// copy with no key press), so the app holds no edit to undo.
    NoReceipt,
    /// A terminal reads Ctrl+Z as a job stop, so no undo key goes to one.
    Terminal,
    /// No app had the focus, or the focus was one of Hushpen's own windows.
    NoTarget,
}

/// What the insertion of one dictation left behind.
pub struct Candidate<'a> {
    /// The AI step ended `used`.
    pub ai_used: bool,
    pub ai_text: &'a str,
    pub rule_text: &'a str,
    pub report: &'a Report,
    /// The `WM_CLASS` words or the bundle id of the app that got the text.
    pub classes: &'a [String],
    pub window: Option<u64>,
    pub own: bool,
}

/// Whether to offer the undo, and the window to watch when it is.
pub fn decide(candidate: &Candidate<'_>) -> Result<u64, Declined> {
    if !candidate.ai_used {
        return Err(Declined::NotAiText);
    }
    if candidate.ai_text == candidate.rule_text {
        return Err(Declined::SameText);
    }
    if candidate.report.outcome != Outcome::Pasted || candidate.report.first_receipt_ms.is_none() {
        return Err(Declined::NoReceipt);
    }
    let window = match (candidate.window, candidate.own) {
        (Some(window), false) => window,
        _ => return Err(Declined::NoTarget),
    };
    if is_terminal(candidate.classes) {
        return Err(Declined::Terminal);
    }
    Ok(window)
}

/// An undo on offer: the text to put back and the app it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub row_id: String,
    pub rule_text: String,
    pub window: u64,
    pub expires_at: u64,
}

/// What an offer is worth right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    Open,
    Expired,
    FocusChanged,
}

impl Offer {
    pub fn new(row_id: String, rule_text: String, window: u64, now_ms: u64) -> Self {
        Self {
            row_id,
            rule_text,
            window,
            expires_at: now_ms + UNDO_WINDOW_MS,
        }
    }

    /// `focus` is the window that has the focus now. A different window, or none, ends the
    /// offer for good: the user moved on, and an undo key would hit the wrong place.
    pub fn standing(&self, now_ms: u64, focus: Option<u64>) -> Standing {
        if now_ms >= self.expires_at {
            Standing::Expired
        } else if focus != Some(self.window) {
            Standing::FocusChanged
        } else {
            Standing::Open
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pasted() -> Report {
        let mut report = Report::new("Gtk-target.py");
        report.outcome = Outcome::Pasted;
        report.first_receipt_ms = Some(40);
        report
    }

    fn classes(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_owned()).collect()
    }

    fn candidate<'a>(report: &'a Report, classes: &'a [String]) -> Candidate<'a> {
        Candidate {
            ai_used: true,
            ai_text: "A fast fox.",
            rule_text: "The quick fox.",
            report,
            classes,
            window: Some(7),
            own: false,
        }
    }

    #[test]
    fn a_pasted_ai_text_in_a_normal_app_is_offered_with_its_window() {
        let report = pasted();
        let classes = classes(&["gtk-target.py", "Gtk-target.py"]);
        assert_eq!(decide(&candidate(&report, &classes)), Ok(7));
    }

    #[test]
    fn a_text_that_is_not_the_models_is_not_offered() {
        let report = pasted();
        let classes = classes(&["gtk-target.py"]);
        let mut input = candidate(&report, &classes);
        input.ai_used = false;
        assert_eq!(decide(&input), Err(Declined::NotAiText));
        let mut input = candidate(&report, &classes);
        input.ai_text = input.rule_text;
        assert_eq!(decide(&input), Err(Declined::SameText));
    }

    #[test]
    fn no_receipt_means_no_undo() {
        let classes = classes(&["gtk-target.py"]);
        for outcome in [
            Outcome::CopiedOnly,
            Outcome::NoPermission,
            Outcome::BlockedSecure,
            Outcome::BlockedGrab,
            Outcome::Failed,
        ] {
            let mut report = pasted();
            report.outcome = outcome;
            assert_eq!(
                decide(&candidate(&report, &classes)),
                Err(Declined::NoReceipt),
                "{outcome:?}"
            );
        }
        let mut report = pasted();
        report.first_receipt_ms = None;
        assert_eq!(
            decide(&candidate(&report, &classes)),
            Err(Declined::NoReceipt)
        );
    }

    #[test]
    fn a_terminal_never_gets_an_offer() {
        let report = pasted();
        for words in [
            classes(&["xterm", "XTerm"]),
            classes(&["kitty", "kitty"]),
            classes(&["com.apple.Terminal"]),
        ] {
            assert_eq!(
                decide(&candidate(&report, &words)),
                Err(Declined::Terminal),
                "{words:?}"
            );
        }
    }

    #[test]
    fn no_focused_app_and_our_own_window_get_no_offer() {
        let report = pasted();
        let classes = classes(&["gtk-target.py"]);
        let mut input = candidate(&report, &classes);
        input.window = None;
        assert_eq!(decide(&input), Err(Declined::NoTarget));
        let mut input = candidate(&report, &classes);
        input.own = true;
        assert_eq!(decide(&input), Err(Declined::NoTarget));
    }

    #[test]
    fn an_offer_lasts_five_seconds_in_the_same_window() {
        let offer = Offer::new("row".into(), "The quick fox.".into(), 7, 1_000);
        assert_eq!(offer.standing(1_000, Some(7)), Standing::Open);
        assert_eq!(offer.standing(5_999, Some(7)), Standing::Open);
        assert_eq!(offer.standing(6_000, Some(7)), Standing::Expired);
    }

    #[test]
    fn another_window_or_none_ends_the_offer() {
        let offer = Offer::new("row".into(), "The quick fox.".into(), 7, 0);
        assert_eq!(offer.standing(100, Some(8)), Standing::FocusChanged);
        assert_eq!(offer.standing(100, None), Standing::FocusChanged);
        assert_eq!(offer.standing(5_000, Some(8)), Standing::Expired);
    }
}
