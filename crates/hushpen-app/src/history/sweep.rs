//! The retention sweep: removes the audio that the setting no longer keeps, once at start and
//! again every 24 hours while the app runs.

use super::History;
use crate::hook;
use gpui_kit::Context;
use hushpen_store::history::now_ms;
use hushpen_store::retention::{self, Schedule, SweepReport};

impl History {
    /// Sweeps now with the setting as it is now. The list and the open transcript show what
    /// was removed.
    pub fn sweep_audio(&mut self, cx: &mut Context<Self>) -> SweepReport {
        let retention = self.controller.read(cx).retention();
        let report = match retention::sweep(
            &self.storage.database,
            self.storage.data.root(),
            retention,
            now_ms(),
        ) {
            Ok(report) => report,
            Err(error) => {
                log::warn!("the audio sweep did not run: {error}");
                return SweepReport::default();
            }
        };
        if report.removed > 0 {
            self.drop_playback_without_audio();
            self.reload(cx);
            hook::record_event("history", &format!("audio-swept {}", report.removed));
        }
        report
    }

    /// Sweeps at once, then waits a day for each next sweep. The first sweep runs here and
    /// not on the timer, so a row that was too old when the app started shows "Audio removed"
    /// in the first frame.
    pub(super) fn start_sweeps(&mut self, cx: &mut Context<Self>) {
        self.sweep_audio(cx);
        let mut schedule = Schedule::default();
        schedule.ran(now_ms());
        cx.spawn(async move |this, cx| {
            loop {
                let wait = schedule.wait(now_ms());
                if wait.is_zero() {
                    if this.update(cx, |me, cx| me.sweep_audio(cx)).is_err() {
                        break;
                    }
                    schedule.ran(now_ms());
                } else {
                    cx.background_executor().timer(wait).await;
                }
            }
        })
        .detach();
    }
}
