//! The generation log: each message goes to the standard error (which the
//! window shows in its log) and, once `open_file` was called, to
//! `generation.log` in the output folder - the same lines as the Linux
//! version ("2026-01-31 18:05:12,345 [INFO] message").

use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

static FILE: Mutex<Option<File>> = Mutex::new(None);

pub fn open_file(path: &Path) {
    if let Ok(file) = File::create(path) {
        *FILE.lock().unwrap_or_else(|e| e.into_inner()) = Some(file);
    }
}

fn write(level: &str, message: &str) {
    let now = chrono::Local::now();
    let line = format!(
        "{},{:03} [{level}] {message}",
        now.format("%Y-%m-%d %H:%M:%S"),
        now.timestamp_subsec_millis().min(999)
    );
    {
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(stderr, "{line}");
    }
    if let Some(file) = FILE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        let _ = writeln!(file, "{line}");
    }
}

pub fn info(message: &str) {
    write("INFO", message);
}

pub fn warning(message: &str) {
    write("WARNING", message);
}

pub fn error(message: &str) {
    write("ERROR", message);
}
