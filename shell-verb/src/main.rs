// SPDX-License-Identifier: GPL-3.0-only

//! planche-contact-verb — the link between File Explorer's context
//! menu and the graphical application.
//!
//! Usage:
//!   planche-contact-verb.exe -Embedding     started by COM: serve one command
//!   planche-contact-verb.exe --register     add the menu entries (current user)
//!   planche-contact-verb.exe --unregister   remove them
//!   planche-contact-verb.exe PATH...        open the application on these paths
//!
//! The program has no window and no console; failures are appended to
//! `planche-contact-verb.log` in the user's temporary folder.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn run() -> i32 {
    use planche_contact_verb::{com, labels, launch, log, registry};
    use windows::Win32::Globalization::GetUserDefaultUILanguage;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let first = args.first().map(|a| a.to_ascii_lowercase());
    let outcome: Result<(), String> = match first.as_deref() {
        Some("-embedding") | Some("/embedding") => com::serve().map_err(|e| e.to_string()),
        Some("--register") => {
            // SAFETY: plain call without arguments.
            let language = unsafe { GetUserDefaultUILanguage() };
            registry::register(labels::menu_label(language)).map_err(|e| e.to_string())
        }
        Some("--unregister") => registry::unregister().map_err(|e| e.to_string()),
        Some(_) => launch::launch_app(&args)
            .map(|_| ())
            .map_err(|e| e.to_string()),
        None => Err("no argument (expected -Embedding, --register, --unregister or paths)".into()),
    };
    match outcome {
        Ok(()) => 0,
        Err(message) => {
            log(&format!("{}: {message}", first.unwrap_or_default()));
            1
        }
    }
}

#[cfg(not(windows))]
fn run() -> i32 {
    eprintln!("planche-contact-verb is a Windows program; build it for a Windows target.");
    1
}

fn main() {
    std::process::exit(run());
}
