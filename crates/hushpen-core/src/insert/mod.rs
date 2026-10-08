//! Putting the transcript into the focused app.
//!
//! [`method`] picks the paste chord for a target app, [`flow`] runs the clipboard sequence
//! through a platform [`flow::Backend`], and [`report`] describes what happened. Nothing here
//! touches a display or a pasteboard.

pub mod flow;
pub mod guard;
pub mod method;
pub mod report;

pub use method::{Chord, Method, Overrides, Selection, choose_mac, choose_x11};
pub use report::{Outcome, Report, Restore};
