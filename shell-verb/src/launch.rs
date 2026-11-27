// SPDX-License-Identifier: GPL-3.0-only

//! Hands the selection over to the graphical application.
//!
//! The selected paths are written to a temporary text file (one per line,
//! UTF-8) whose name is passed with `--paths-from`: a command line is limited
//! to about 32,000 characters on Windows, far too few for a large selection.
//! The window hands the file to the engine (`selection --paths-from`), which
//! reads it and deletes it.

use std::path::{Path, PathBuf};

/// Content of the selection file: one path per line.
pub fn selection_list(paths: &[String]) -> String {
    let mut content = String::new();
    for path in paths {
        content.push_str(path);
        content.push_str("\r\n");
    }
    content
}

/// Folder where this program is installed (the application is next to it).
pub fn install_dir() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| std::io::Error::other("the program has no parent folder"))
}

/// Writes the selection file in the user's temporary folder and returns its
/// path. The engine deletes it once read.
pub fn write_selection_file(paths: &[String]) -> std::io::Result<PathBuf> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = format!(
        "planche-contact-selection-{}-{stamp}.txt",
        std::process::id()
    );
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, selection_list(paths))?;
    Ok(path)
}

/// Path of the application: next to this program, unless the
/// PLANCHE_CONTACT_APP environment variable says otherwise
/// (development and tests only).
pub fn app_path() -> std::io::Result<PathBuf> {
    match std::env::var_os("PLANCHE_CONTACT_APP") {
        Some(path) if !path.is_empty() => Ok(PathBuf::from(path)),
        _ => Ok(install_dir()?.join(crate::plan::APP_EXE)),
    }
}

/// A started application: its process identifier and the selection file it
/// was given.
#[derive(Debug)]
pub struct Launched {
    pub pid: u32,
    pub list: PathBuf,
}

/// Starts the application on `paths`.
pub fn launch_app(paths: &[String]) -> std::io::Result<Launched> {
    let list = write_selection_file(paths)?;
    // This program has no console: the application gets none of its (absent)
    // standard streams.
    let child = std::process::Command::new(app_path()?)
        .arg("--paths-from")
        .arg(&list)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(Launched {
        pid: child.id(),
        list,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_path_per_line_with_windows_line_endings() {
        let paths = vec![r"C:\Photos d'été\a b.cr2".to_string(), r"D:\x".to_string()];
        assert_eq!(
            selection_list(&paths),
            "C:\\Photos d'été\\a b.cr2\r\nD:\\x\r\n"
        );
        assert_eq!(selection_list(&[]), "");
    }

    #[test]
    fn selection_file_round_trip() {
        let paths = vec!["first".to_string(), "sécond".to_string()];
        let file = write_selection_file(&paths).unwrap();
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "first\r\nsécond\r\n"
        );
        let _ = std::fs::remove_file(file);
    }
}
