use std::fmt;
use std::io;
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    Io {
        context: String,
        source: io::Error,
    },
    Sqlite(rusqlite::Error),
    /// The marker file names another product, so the folder is not ours.
    ForeignDataFolder(PathBuf),
    /// The bundled SQLite lacks FTS5 or the trigram tokenizer.
    Fts5TrigramUnavailable(String),
    UnknownSetting(String),
    /// Carries the key only, never the rejected value.
    InvalidSetting(String),
    LoggerAlreadySet,
}

impl Error {
    pub(crate) fn io(context: impl Into<String>, source: io::Error) -> Self {
        Self::Io {
            context: context.into(),
            source,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { context, source } => write!(f, "{context}: {source}"),
            Self::Sqlite(error) => write!(f, "database error: {error}"),
            Self::ForeignDataFolder(path) => {
                write!(f, "{} belongs to another product", path.display())
            }
            Self::Fts5TrigramUnavailable(reason) => {
                write!(f, "SQLite FTS5 trigram search is unavailable: {reason}")
            }
            Self::UnknownSetting(key) => write!(f, "unknown setting '{key}'"),
            Self::InvalidSetting(key) => write!(f, "invalid value for setting '{key}'"),
            Self::LoggerAlreadySet => write!(f, "a logger is already installed"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Sqlite(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}
