// SPDX-License-Identifier: GPL-3.0-only

//! planche-contact-verb — the link between File Explorer's context
//! menu and the Planche-Contact window. See `main.rs` for the command line.
//!
//! The modules that describe *what* to do (`extensions`, `labels`, `plan`,
//! `launch`) are plain Rust and are tested on any platform; `registry` and
//! `com`, which talk to Windows, only exist in a Windows build.

#![cfg_attr(not(windows), allow(dead_code))]

pub mod extensions;
pub mod labels;
pub mod launch;
pub mod plan;

#[cfg(windows)]
pub mod com;
#[cfg(windows)]
pub mod registry;

/// Appends a line to `planche-contact-verb.log` in the user's
/// temporary folder (best effort: never fails the caller). The program has
/// neither window nor console, so this is where failures are reported.
pub fn log(message: &str) {
    use std::io::Write;
    let path = std::env::temp_dir().join("planche-contact-verb.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(file, "[{seconds}] {message}");
    }
}
