//! Start-up of the data folder: marker, log, settings, and history database.

use hushpen_store::Result;
use hushpen_store::data_dir::DataDir;
use hushpen_store::db::Database;
use hushpen_store::log_file;
use hushpen_store::settings::SettingsStore;

pub struct Storage {
    pub data: DataDir,
    pub settings: SettingsStore,
    pub database: Database,
}

/// Opens settings and the history database and writes the start-up log line.
/// The file logger must be installed first, or the line goes nowhere.
pub fn open(data: DataDir) -> Result<Storage> {
    let settings = SettingsStore::open(&data.config_dir())?;
    let database = Database::open(&data.database_path())?;
    let report = database.report();
    log::info!(
        "STARTUP version={} sqlite={} journal_mode={} user_version={} fts5_trigram=passed read_only={}",
        hushpen_core::BUILD_VERSION,
        report.sqlite_version,
        report.journal_mode,
        report.user_version,
        report.read_only
    );
    if report.read_only {
        log::warn!("HISTORY_NEWER_SCHEMA the history database was written by a newer version");
    }
    Ok(Storage {
        data,
        settings,
        database,
    })
}

/// Sends `log` records to `logs/hushpen.log`, filtered by `RUST_LOG`.
pub fn install_logger(data: &DataDir) -> Result<()> {
    let rust_log = std::env::var("RUST_LOG").ok();
    log_file::install(&data.log_path(), rust_log.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_folder_gets_settings_and_a_wal_database() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("data");
        let storage = open(DataDir::open(&root).unwrap()).unwrap();

        assert!(root.join(".hushpen-data").is_file());
        assert!(root.join("config/settings.json").is_file());
        assert!(root.join("history/history.db").is_file());
        let report = storage.database.report();
        assert_eq!(report.journal_mode, "wal");
        assert!(report.user_version >= 1);
        assert!(report.fts5_trigram_ok);
    }
}
