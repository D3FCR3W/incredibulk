//! Reading and writing the two files the app keeps.
//!
//! The model knows how to turn itself into text and back, and nothing more.
//! Everything to do with where those files live, and with a disk that can be
//! full, locked or missing, is here. That is the whole reason the model can be
//! tested without a filesystem.
//!
//! Both files are written the same way: to a temporary neighbour, then
//! renamed. A rename is atomic, so a crash halfway through leaves the previous
//! file intact rather than a truncated one. For settings that matters a great
//! deal, since it is the file the app needs in order to start.

use std::io;
use std::path::Path;

use incredibulk_core::{Config, ConfigError, History};

#[derive(Debug)]
pub enum StoreError {
    /// The file could not be read or written.
    Io(io::Error),
    /// The file was read but its contents are not settings.
    Invalid(ConfigError),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "the settings file could not be read or written: {e}"),
            StoreError::Invalid(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Read the settings.
///
/// A missing file is a first run, not a failure. A file that exists but cannot
/// be parsed *is* a failure worth reporting: silently replacing someone's
/// settings with defaults is worse than telling them their file is broken.
pub fn load_config(path: &Path) -> Result<Config, StoreError> {
    match std::fs::read_to_string(path) {
        Ok(raw) => Config::from_json(&raw).map_err(StoreError::Invalid),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Config::default()),
        Err(e) => Err(StoreError::Io(e)),
    }
}

pub fn save_config(config: &Config, path: &Path) -> Result<(), StoreError> {
    let body = config.to_json().map_err(StoreError::Invalid)?;
    write_atomically(path, &body).map_err(StoreError::Io)
}

/// Read the history. Anything unreadable yields an empty history: it is a
/// convenience, and losing it is not worth interrupting anyone over.
pub fn load_history(path: &Path) -> History {
    match std::fs::read_to_string(path) {
        Ok(raw) => History::from_json(&raw),
        Err(_) => History::default(),
    }
}

pub fn save_history(history: &History, path: &Path) -> io::Result<()> {
    let body = history.to_json().map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    write_atomically(path, &body)
}

/// Write via a temporary neighbour and a rename, so an interrupted write
/// cannot leave a half-written file in place of a good one.
fn write_atomically(path: &Path, body: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, body)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("incredibulk-store-tests");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join(name);
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn a_missing_settings_file_is_a_first_run() {
        let path = scratch("missing.json");
        assert_eq!(load_config(&path).expect("defaults"), Config::default());
    }

    #[test]
    fn settings_survive_a_save_and_a_load() {
        let path = scratch("roundtrip.json");
        let mut config = Config::default();
        config.behavior.poll_interval_ms = 250;

        save_config(&config, &path).expect("saves");
        assert_eq!(load_config(&path).expect("loads"), config);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_broken_settings_file_is_reported_not_swallowed() {
        let path = scratch("broken.json");
        std::fs::write(&path, "{ not json").expect("write");
        assert!(matches!(load_config(&path), Err(StoreError::Invalid(_))));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn saving_creates_the_folder_it_needs() {
        let dir = std::env::temp_dir().join("incredibulk-store-tests").join("nested-4f2a");
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("config.json");

        save_config(&Config::default(), &path).expect("saves into a new folder");
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn no_temporary_file_is_left_behind() {
        let path = scratch("clean.json");
        save_config(&Config::default(), &path).expect("saves");
        assert!(!path.with_extension("json.tmp").exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn an_unreadable_history_starts_empty() {
        let path = scratch("history-broken.json");
        std::fs::write(&path, "{ not json").expect("write");
        assert!(load_history(&path).is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn history_survives_a_save_and_a_load() {
        let path = scratch("history.json");
        let mut history = History::default();
        history.record(
            vec![incredibulk_core::ClipItem::new(
                1,
                incredibulk_core::ClipKind::text("kept"),
                0,
                None,
            )],
            Some("named".into()),
            42,
        );

        save_history(&history, &path).expect("saves");
        let back = load_history(&path);
        assert_eq!(back.len(), 1);
        assert_eq!(back.get(1).and_then(|e| e.name.clone()).as_deref(), Some("named"));
        let _ = std::fs::remove_file(&path);
    }
}
