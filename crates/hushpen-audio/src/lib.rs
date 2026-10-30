//! Audio capture, session WAV files, resampling, and decoding.

pub mod capture;
pub mod cue;
pub mod decode;
pub mod devices;
pub mod enhance;
pub mod error;
pub mod feed;
pub mod levels;
mod mic_stream;
pub mod playback;
pub mod resample;
pub mod session;

pub use capture::{Capture, CaptureEvent, EventSink, Finished, new_session_path, warm_microphone};
pub use cue::CuePlayer;
pub use devices::{DEFAULT_ID, InputDevice, Selection, list_inputs, resolve_selection};
pub use enhance::Enhance;
pub use error::CaptureError;
pub use feed::{FeedInfo, Feeder};
pub use playback::{PlaybackError, Player, write_wav};
pub use session::{RecoveredSession, Sweep, recover_orphans};
