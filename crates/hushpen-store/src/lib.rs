//! Data folder, settings, SQLite history, and logs.

mod atomic;
pub mod data_dir;
pub mod db;
pub mod dictionary;
mod error;
pub mod log_file;
pub mod model_files;
pub mod settings;
pub mod time;

pub use error::{Error, Result};
