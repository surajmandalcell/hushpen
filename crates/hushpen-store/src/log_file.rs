//! The app log: `<iso> <LEVEL> <scope> <CODE> <detail>`, one line per record,
//! in `logs/hushpen.log` with two rotated copies.
//!
//! Callers must keep transcript text, selections, instructions, audio, and
//! the API key out of log messages. Validation messages name the field, not
//! the value.

use crate::{Error, Result, time};
use log::{Level, LevelFilter, Log, Metadata, Record};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const LOG_MAX_BYTES: u64 = 2_000_000;
/// The live file plus `.1` and `.2`.
pub const LOG_FILES: usize = 3;
const DETAIL_LIMIT: usize = 500;
const DEFAULT_LEVEL: LevelFilter = LevelFilter::Info;

pub struct RotatingFile {
    path: PathBuf,
    max_bytes: u64,
    files: usize,
    file: File,
    size: u64,
}

impl RotatingFile {
    /// Opens `path` for appending. A file already at the size limit rotates
    /// first, so a start never appends to a full log.
    pub fn open(path: &Path, max_bytes: u64, files: usize) -> Result<Self> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)
                .map_err(|e| Error::io(format!("could not create {}", dir.display()), e))?;
        }
        let size = fs::metadata(path).map_or(0, |meta| meta.len());
        let mut log = Self {
            path: path.to_path_buf(),
            max_bytes,
            files: files.max(1),
            file: open_append(path)?,
            size,
        };
        if size >= max_bytes {
            log.rotate()?;
        }
        Ok(log)
    }

    pub fn write_line(&mut self, line: &str) -> Result<()> {
        let length = line.len() as u64;
        if self.size > 0 && self.size + length > self.max_bytes {
            self.rotate()?;
        }
        self.file
            .write_all(line.as_bytes())
            .map_err(|e| Error::io(format!("could not write {}", self.path.display()), e))?;
        self.size += length;
        Ok(())
    }

    fn rotate(&mut self) -> Result<()> {
        let numbered = |n: usize| PathBuf::from(format!("{}.{n}", self.path.display()));
        let context =
            |e: io::Error| Error::io(format!("could not rotate {}", self.path.display()), e);
        let _ = fs::remove_file(numbered(self.files - 1));
        for n in (1..self.files - 1).rev() {
            if numbered(n).exists() {
                fs::rename(numbered(n), numbered(n + 1)).map_err(context)?;
            }
        }
        if self.files > 1 {
            fs::rename(&self.path, numbered(1)).map_err(context)?;
        } else {
            fs::remove_file(&self.path).map_err(context)?;
        }
        self.file = open_append(&self.path)?;
        self.size = 0;
        Ok(())
    }
}

fn open_append(path: &Path) -> Result<File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| Error::io(format!("could not open {}", path.display()), e))
}

/// `RUST_LOG`-style directives: a bare level, `scope=level`, or a bare scope
/// (all levels). The longest matching scope wins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filter {
    default: LevelFilter,
    scopes: Vec<(String, LevelFilter)>,
}

impl Filter {
    pub fn parse(spec: Option<&str>) -> Self {
        let mut filter = Self {
            default: DEFAULT_LEVEL,
            scopes: Vec::new(),
        };
        for directive in spec.unwrap_or("").split(',').map(str::trim) {
            if directive.is_empty() {
                continue;
            }
            match directive.split_once('=') {
                Some((scope, level)) => {
                    if let Ok(level) = level.trim().parse() {
                        filter.scopes.push((scope.trim().to_string(), level));
                    }
                }
                None => match directive.parse() {
                    Ok(level) => filter.default = level,
                    Err(_) => filter
                        .scopes
                        .push((directive.to_string(), LevelFilter::Trace)),
                },
            }
        }
        filter
            .scopes
            .sort_by_key(|(scope, _)| std::cmp::Reverse(scope.len()));
        filter
    }

    pub fn level_for(&self, target: &str) -> LevelFilter {
        self.scopes
            .iter()
            .find(|(scope, _)| {
                target == scope
                    || target
                        .strip_prefix(scope.as_str())
                        .is_some_and(|rest| rest.starts_with("::"))
            })
            .map_or(self.default, |(_, level)| *level)
    }

    fn max_level(&self) -> LevelFilter {
        self.scopes
            .iter()
            .map(|(_, level)| *level)
            .fold(self.default, |a, b| a.max(b))
    }
}

pub struct FileLogger {
    sink: Mutex<RotatingFile>,
    filter: Filter,
}

impl FileLogger {
    pub fn new(sink: RotatingFile, filter: Filter) -> Self {
        Self {
            sink: Mutex::new(sink),
            filter,
        }
    }
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.filter.level_for(metadata.target())
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format_line(
            time::now_unix_ms(),
            record.level(),
            record.target(),
            &record.args().to_string(),
        );
        if let Ok(mut sink) = self.sink.lock() {
            // A full disk must not take the app down with it.
            let _ = sink.write_line(&line);
        }
    }

    fn flush(&self) {}
}

fn format_line(unix_ms: u64, level: Level, scope: &str, message: &str) -> String {
    let single_line: String = message
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(DETAIL_LIMIT)
        .collect();
    format!(
        "{} {level} {scope} {single_line}\n",
        time::iso_millis(unix_ms)
    )
}

/// Installs the file logger for the whole process. `rust_log` is the value of
/// `RUST_LOG`; `None` means `info`.
pub fn install(log_path: &Path, rust_log: Option<&str>) -> Result<()> {
    let filter = Filter::parse(rust_log);
    let max_level = filter.max_level();
    let sink = RotatingFile::open(log_path, LOG_MAX_BYTES, LOG_FILES)?;
    log::set_boxed_logger(Box::new(FileLogger::new(sink, filter)))
        .map_err(|_| Error::LoggerAlreadySet)?;
    log::set_max_level(max_level);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_spec_means_info() {
        let filter = Filter::parse(None);
        assert_eq!(filter.level_for("anything"), LevelFilter::Info);
    }

    #[test]
    fn a_bare_level_sets_the_default() {
        assert_eq!(
            Filter::parse(Some("debug")).level_for("x"),
            LevelFilter::Debug
        );
    }

    #[test]
    fn the_longest_scope_wins() {
        let filter = Filter::parse(Some("warn,hushpen_app=debug,hushpen_app::shell=error"));
        assert_eq!(filter.level_for("gpui"), LevelFilter::Warn);
        assert_eq!(filter.level_for("hushpen_app::app"), LevelFilter::Debug);
        assert_eq!(filter.level_for("hushpen_app::shell"), LevelFilter::Error);
        assert_eq!(filter.level_for("hushpen_app_extra"), LevelFilter::Warn);
    }

    #[test]
    fn a_junk_spec_falls_back_to_info() {
        let filter = Filter::parse(Some("foo=bar,,"));
        assert_eq!(filter.level_for("x"), LevelFilter::Info);
    }

    #[test]
    fn the_max_level_covers_the_most_verbose_scope() {
        assert_eq!(
            Filter::parse(Some("warn,a=trace")).max_level(),
            LevelFilter::Trace
        );
    }

    #[test]
    fn a_line_has_iso_time_level_scope_and_one_line_of_detail() {
        let line = format_line(0, Level::Warn, "hushpen_store::settings", "CODE a\nb\tc");
        assert_eq!(
            line,
            "1970-01-01T00:00:00.000Z WARN hushpen_store::settings CODE a b c\n"
        );
    }

    #[test]
    fn long_detail_is_cut_at_500_characters() {
        let line = format_line(0, Level::Info, "s", &"é".repeat(900));
        let detail = line.trim_end().splitn(4, ' ').nth(3).unwrap();
        assert_eq!(detail.chars().count(), 500);
    }
}
