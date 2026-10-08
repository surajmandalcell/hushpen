//! Playing the audio of the open transcript: Play and Pause, seek, and the position that the
//! detail view and the test hook show.

use super::History;
use crate::hook;
use gpui_kit::Context;
use hushpen_audio::Player;
use hushpen_store::history::Row;
use serde_json::{Value, json};
use std::time::Duration;

/// How often the position on the screen is drawn again while a recording plays.
const TICK: Duration = Duration::from_millis(100);
/// How far the arrow keys move on the seek bar.
pub const KEY_STEP_MS: i64 = 5_000;

/// The player of one transcript. Only the open transcript has one, and it ends with the
/// detail view.
pub(super) struct Playback {
    pub(super) id: String,
    player: Player,
}

/// What the detail view draws.
pub struct PlaybackInfo {
    pub playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    /// Why the sound output did not work, when it did not.
    pub error: Option<String>,
}

pub fn clock(ms: u64) -> String {
    let seconds = ms / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

impl History {
    pub fn playback_info(&self, row: &Row) -> PlaybackInfo {
        match self.playback.as_ref().filter(|open| open.id == row.id) {
            Some(open) => PlaybackInfo {
                playing: open.player.is_playing(),
                position_ms: open.player.position_ms(),
                duration_ms: open.player.duration_ms(),
                error: open.player.error(),
            },
            None => PlaybackInfo {
                playing: false,
                position_ms: 0,
                duration_ms: u64::try_from(row.duration_ms).unwrap_or(0),
                error: None,
            },
        }
    }

    /// Opens the player of a transcript, which decodes its audio. Another transcript's player
    /// ends.
    fn player_for(&mut self, id: Option<&str>, cx: &gpui_kit::App) -> Result<&mut Player, String> {
        let row = self.row(id)?;
        if self.playback.as_ref().is_none_or(|open| open.id != row.id) {
            self.playback = None;
            let file = self
                .controller
                .read(cx)
                .audio_file(&row)
                .ok_or("The audio of this transcript is not available.")?;
            let opened = if self.silent_playback {
                Player::silent(&file)
            } else {
                Player::open(&file)
            };
            let player = opened.map_err(|error| {
                log::warn!("the audio of a transcript could not be read: {error}");
                "The audio of this transcript could not be read.".to_owned()
            })?;
            self.playback = Some(Playback {
                id: row.id.clone(),
                player,
            });
        }
        self.playback
            .as_mut()
            .map(|open| &mut open.player)
            .ok_or_else(|| "The audio of this transcript is not available.".to_owned())
    }

    pub fn play(&mut self, id: Option<&str>, cx: &mut Context<Self>) -> Result<(), String> {
        match self.player_for(id, cx) {
            Ok(player) => player.play(),
            Err(message) => return self.refuse(message, cx),
        }
        self.message = None;
        hook::record_event("history", "play");
        self.start_ticks(cx);
        cx.notify();
        Ok(())
    }

    pub fn pause(&mut self, id: Option<&str>, cx: &mut Context<Self>) -> Result<(), String> {
        match self.player_for(id, cx) {
            Ok(player) => player.pause(),
            Err(message) => return self.refuse(message, cx),
        }
        hook::record_event("history", "pause");
        cx.notify();
        Ok(())
    }

    /// Play when it is paused, Pause when it plays: what the one button does.
    pub fn toggle_playback(
        &mut self,
        id: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let playing = match self.player_for(id, cx) {
            Ok(player) => player.is_playing(),
            Err(message) => return self.refuse(message, cx),
        };
        if playing {
            self.pause(id, cx)
        } else {
            self.play(id, cx)
        }
    }

    pub fn seek_ms(
        &mut self,
        id: Option<&str>,
        ms: u64,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        match self.player_for(id, cx) {
            Ok(player) => player.seek_ms(ms),
            Err(message) => return self.refuse(message, cx),
        }
        hook::record_event("history", &format!("seek {ms}"));
        cx.notify();
        Ok(())
    }

    /// Moves to a place on the seek bar: 0.0 is the start and 1.0 the end.
    pub fn seek_fraction(
        &mut self,
        id: Option<&str>,
        fraction: f32,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let duration = match self.player_for(id, cx) {
            Ok(player) => player.duration_ms(),
            Err(message) => return self.refuse(message, cx),
        };
        let at = (f64::from(fraction.clamp(0.0, 1.0)) * duration as f64) as u64;
        self.seek_ms(id, at, cx)
    }

    /// Moves forward or back from where the player is now.
    pub fn seek_by(
        &mut self,
        id: Option<&str>,
        delta_ms: i64,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let now = match self.player_for(id, cx) {
            Ok(player) => player.position_ms() as i64,
            Err(message) => return self.refuse(message, cx),
        };
        self.seek_ms(id, (now + delta_ms).max(0) as u64, cx)
    }

    pub fn stop_playback(&mut self) {
        self.playback = None;
    }

    /// Draws the position again every tick while the audio plays, and once more when it ends.
    fn start_ticks(&mut self, cx: &mut Context<Self>) {
        self.tick_generation += 1;
        let generation = self.tick_generation;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(TICK).await;
                let again = this.update(cx, |me, cx| {
                    cx.notify();
                    me.tick_generation == generation
                        && me
                            .playback
                            .as_ref()
                            .is_some_and(|open| open.player.is_playing())
                });
                if !again.unwrap_or(false) {
                    break;
                }
            }
        })
        .detach();
    }

    pub(super) fn playback_json(&self) -> Value {
        self.playback.as_ref().map_or(Value::Null, |open| {
            json!({
                "id": open.id,
                "playing": open.player.is_playing(),
                "position_ms": open.player.position_ms(),
                "duration_ms": open.player.duration_ms(),
                "error": open.player.error(),
            })
        })
    }

    /// Ends the player when its transcript lost its audio.
    pub(super) fn drop_playback_without_audio(&mut self) {
        let Some(open) = &self.playback else { return };
        let has_audio = hushpen_store::history::get(&self.storage.database, &open.id)
            .ok()
            .flatten()
            .is_some_and(|row| row.audio_path.is_some());
        if !has_audio {
            self.playback = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_shows_minutes_and_two_digit_seconds() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(59_999), "0:59");
        assert_eq!(clock(61_000), "1:01");
        assert_eq!(clock(600_000), "10:00");
    }
}
