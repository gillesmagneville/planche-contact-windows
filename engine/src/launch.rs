//! What a context menu selection designates (the Linux version's
//! launch.py): one folder alone is opened as the input folder; photos, or
//! several folders, become an exact list of photos under their common
//! parent folder.

use std::path::{Path, PathBuf};

use crate::scanner::{file_name, is_supported};

/// Start of the name of the list files written by the context menu: such
/// a file is deleted once read (nobody else would).
pub const SELECTION_FILE_PREFIX: &str = "planche-contact-selection-";

#[derive(Debug, PartialEq)]
pub struct Selection {
    pub input_dir: PathBuf,
    /// None: the whole folder; otherwise POSIX paths relative to it.
    pub files: Option<Vec<String>>,
}

/// Lines of a UTF-8 list file (byte order mark allowed), blank lines left
/// out. A file written by the context menu is deleted once read.
pub fn read_paths_file(path: &Path) -> Vec<String> {
    let text = std::fs::read(path).ok().and_then(|bytes| {
        let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").map(<[u8]>::to_vec).unwrap_or(bytes);
        String::from_utf8(bytes).ok()
    });
    let name = file_name(path);
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if name.starts_with(SELECTION_FILE_PREFIX) && extension == "txt" {
        let _ = std::fs::remove_file(path);
    }
    text.map(|t| split_lines(&t)).unwrap_or_default()
}

/// Python's `splitlines()` without the blank lines.
pub fn split_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect()
}

/// Paths given on the command line: `--paths-from FILE` (or
/// `--paths-from=FILE`) and plain paths; `--` ends the options, any other
/// option is ignored.
pub fn paths_from_arguments(args: &[String]) -> Vec<String> {
    let mut collected = Vec::new();
    let mut pending_file = false;
    let mut options_ended = false;
    for arg in args {
        if pending_file {
            collected.extend(read_paths_file(Path::new(arg)));
            pending_file = false;
        } else if options_ended || !arg.starts_with('-') {
            collected.push(arg.clone());
        } else if arg == "--" {
            options_ended = true;
        } else if arg == "--paths-from" {
            pending_file = true;
        } else if let Some(file) = arg.strip_prefix("--paths-from=") {
            collected.extend(read_paths_file(Path::new(file)));
        }
    }
    collected
}

fn absolute(path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir().map(|cwd| cwd.join(&path)).unwrap_or(path)
    };
    // os.path.abspath: "." and ".." resolved textually.
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn is_photo(path: &Path) -> bool {
    path.is_file() && is_supported(path)
}

fn photos_directly_in(folder: &Path) -> Vec<PathBuf> {
    let mut photos: Vec<PathBuf> = std::fs::read_dir(folder)
        .map(|entries| entries.flatten().map(|e| e.path()).filter(|p| is_photo(p)).collect())
        .unwrap_or_default();
    photos.sort_by_key(|p| file_name(p).to_lowercase());
    photos
}

fn root_of(path: &Path) -> String {
    path.components()
        .next()
        .and_then(|c| match c {
            std::path::Component::Prefix(prefix) => Some(prefix.as_os_str().to_string_lossy().to_lowercase()),
            _ => None,
        })
        .unwrap_or_default()
}

/// The selection to open, or None when no path is a folder or a photo.
pub fn resolve_selection(paths: &[String]) -> Option<Selection> {
    let mut folders = Vec::new();
    let mut photos = Vec::new();
    for raw in paths {
        let path = absolute(raw);
        if path.is_dir() {
            folders.push(path);
        } else if is_photo(&path) {
            photos.push(path);
        }
    }
    if folders.is_empty() && photos.is_empty() {
        return None;
    }
    if folders.len() == 1 && photos.is_empty() {
        return Some(Selection { input_dir: folders.remove(0), files: None });
    }
    for folder in &folders {
        photos.extend(photos_directly_in(folder));
    }
    let mut unique = Vec::new();
    for photo in photos {
        if !unique.contains(&photo) {
            unique.push(photo);
        }
    }
    if unique.is_empty() {
        return None;
    }
    // A selection lies on one drive; if not, the first photo's drive wins.
    let drive = root_of(&unique[0]);
    unique.retain(|p| root_of(p) == drive);

    let mut common: PathBuf = unique[0].parent()?.to_path_buf();
    for photo in &unique[1..] {
        let parent = photo.parent()?;
        while !parent.starts_with(&common) {
            if !common.pop() {
                break;
            }
        }
    }
    let files = unique
        .iter()
        .map(|p| {
            p.strip_prefix(&common)
                .map(|r| r.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"))
                .unwrap_or_default()
        })
        .collect();
    Some(Selection { input_dir: common, files: Some(files) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_and_photos() {
        let root = std::env::temp_dir().join(format!("pc-launch-{}", std::process::id()));
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::fs::create_dir_all(root.join("b")).unwrap();
        for f in ["a/1.jpg", "a/2.JPG", "b/3.nef", "b/readme.txt"] {
            std::fs::write(root.join(f), b"x").unwrap();
        }
        let s = |p: &str| root.join(p).to_string_lossy().into_owned();

        let one = resolve_selection(&[s("a")]).unwrap();
        assert_eq!(one, Selection { input_dir: root.join("a"), files: None });

        let mixed = resolve_selection(&[s("a"), s("b/3.nef"), s("b/readme.txt"), s("missing.jpg")]).unwrap();
        assert_eq!(mixed.input_dir, root);
        assert_eq!(mixed.files.unwrap(), vec!["b/3.nef", "a/1.jpg", "a/2.JPG"]);

        assert_eq!(resolve_selection(&[s("b/readme.txt")]), None);

        let list = root.join(format!("{SELECTION_FILE_PREFIX}x.txt"));
        std::fs::write(&list, format!("\u{feff}{}\r\n\r\n{}\r\n", s("a/1.jpg"), s("a/2.JPG"))).unwrap();
        let args = vec!["--paths-from".to_string(), list.to_string_lossy().into_owned(), "--other".into()];
        let paths = paths_from_arguments(&args);
        assert_eq!(paths, vec![s("a/1.jpg"), s("a/2.JPG")]);
        assert!(!list.exists(), "a context menu list is deleted once read");
        let _ = std::fs::remove_dir_all(&root);
    }
}
