use std::{io::Write, time::{SystemTime, UNIX_EPOCH}};

const MAX_LOG_BYTES: u64 = 1024 * 1024;

/// Writes a line to stderr and to `%LOCALAPPDATA%\Auris\auris.log`.
/// Release builds have no console, so the log file is the only place errors surface.
pub fn log(message: impl std::fmt::Display) {
    eprintln!("{message}");
    let dir = crate::config::data_dir();
    if std::fs::create_dir_all(&dir).is_err() { return; }
    let path = dir.join("auris.log");
    if std::fs::metadata(&path).is_ok_and(|meta| meta.len() > MAX_LOG_BYTES) {
        let _ = std::fs::rename(&path, dir.join("auris.old.log"));
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let _ = writeln!(file, "[{seconds}] {message}");
    }
}
