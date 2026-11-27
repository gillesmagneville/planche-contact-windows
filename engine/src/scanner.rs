//! Finding the photos to put on the sheets, and the folders that hold some
//! (the Linux version's scanner.py).

use std::path::{Component, Path, PathBuf};

use chrono::NaiveDateTime;
use rayon::prelude::*;

use crate::metadata::exif_date;

pub const SUPPORTED_EXTENSIONS: [&str; 14] = [
    "arw", "bmp", "cr2", "cr3", "dng", "heic", "heif", "jpeg", "jpg", "nef", "png", "tif", "tiff", "webp",
];

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// A photo to place, with its sequence number ("001", "002"...).
#[derive(Clone, Debug)]
pub struct Photo {
    pub path: PathBuf,
    pub filename: String,
    pub num: String,
}

#[derive(Clone, Debug, Default)]
pub struct ScanOptions {
    pub input_dir: PathBuf,
    pub output_dir: Option<PathBuf>,
    pub recursive: bool,
    /// Subfolders whose photos are kept (relative paths, "." = the input
    /// folder itself); implies a recursive search.
    pub include_dirs: Option<Vec<String>>,
    /// Exact photos to keep (relative or absolute paths): the folder is not
    /// walked.
    pub only_files: Option<Vec<String>>,
    /// "date" (EXIF date, else modification date) or "name".
    pub sort_by: String,
}

/// Python's `posixpath.normpath` of a relative path.
fn normpath(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|last| *last != "..") {
                    parts.pop();
                } else {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    if parts.is_empty() { ".".into() } else { parts.join("/") }
}

fn to_posix(path: &Path) -> String {
    path.components()
        .filter_map(|c| match c {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            Component::ParentDir => Some("..".into()),
            Component::CurDir => Some(".".into()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// `entry` relative to `input_dir` as a POSIX path ("." for the folder
/// itself), or None when it designates a place outside it.
pub fn normalize_include_dir(input_dir: &Path, entry: &str) -> Option<String> {
    let candidate = PathBuf::from(entry);
    let relative = if candidate.is_absolute() {
        match candidate.strip_prefix(input_dir) {
            Ok(rest) => to_posix(rest),
            Err(_) => {
                let resolved = candidate.canonicalize().ok()?;
                let base = input_dir.canonicalize().ok()?;
                to_posix(resolved.strip_prefix(&base).ok()?)
            }
        }
    } else {
        entry.replace('\\', "/")
    };
    let relative = normpath(&relative);
    if relative == ".." || relative.starts_with("../") || relative.starts_with('/') {
        return None;
    }
    Some(relative)
}

/// The `planches` and `gallery` folders of a previous generation, which
/// must never be taken as photos to place.
fn excluded_dirs(output_dir: Option<&Path>) -> Vec<PathBuf> {
    let Some(output) = output_dir else { return vec![] };
    ["planches", "gallery"]
        .iter()
        .filter_map(|name| output.join(name).canonicalize().ok())
        .collect()
}

fn is_excluded(path: &Path, excluded: &[PathBuf]) -> bool {
    if excluded.is_empty() {
        return false;
    }
    match path.canonicalize() {
        Ok(resolved) => excluded.iter().any(|dir| resolved.starts_with(dir)),
        Err(_) => false,
    }
}

fn walk(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if recursive {
                walk(&path, true, out);
            }
        } else {
            out.push(path);
        }
    }
}

#[derive(Clone, PartialEq, PartialOrd)]
enum SortKey {
    Date(NaiveDateTime),
    Name(String),
}

fn modification_date(path: &Path) -> NaiveDateTime {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(|time| chrono::DateTime::<chrono::Local>::from(time).naive_local())
        .unwrap_or(NaiveDateTime::MIN)
}

/// The photos to place, sorted and numbered.
pub fn scan(options: &ScanOptions) -> Vec<Photo> {
    let input_dir = &options.input_dir;
    let include_dirs: Option<Vec<String>> = options.include_dirs.as_ref().map(|wanted| {
        let mut kept = Vec::new();
        for entry in wanted {
            match normalize_include_dir(input_dir, entry) {
                Some(relative) => {
                    if !kept.contains(&relative) {
                        kept.push(relative);
                    }
                }
                None => crate::log::warning(&crate::t!("engine.warn_include_dir_outside", dir = entry)),
            }
        }
        kept
    });
    let only_files: Option<Vec<String>> = options.only_files.as_ref().map(|wanted| {
        let mut kept: Vec<String> = Vec::new();
        for entry in wanted {
            match normalize_include_dir(input_dir, entry) {
                Some(relative) if relative != "." => {
                    if !kept.contains(&relative) {
                        kept.push(relative);
                    }
                }
                _ => crate::log::warning(&crate::t!("engine.warn_file_outside", file = entry)),
            }
        }
        kept
    });
    let recursive = options.recursive || include_dirs.is_some();

    crate::log::info(&crate::t!("engine.log_scan_folder", dir = input_dir.display(),
                                recursive = if recursive { "True" } else { "False" }));
    if let Some(dirs) = &include_dirs {
        crate::log::info(&crate::t!("engine.log_include_dirs", count = dirs.len()));
    }
    if let Some(files) = &only_files {
        crate::log::info(&crate::t!("engine.log_only_files", count = files.len()));
    }

    let excluded = excluded_dirs(options.output_dir.as_deref());
    let mut candidates = Vec::new();
    match &only_files {
        Some(files) => candidates.extend(files.iter().map(|relative| input_dir.join(relative))),
        None => walk(input_dir, recursive, &mut candidates),
    }

    let mut skipped_output = 0usize;
    let mut kept = Vec::new();
    for path in candidates {
        if !path.is_file() || !is_supported(&path) {
            continue;
        }
        if let Some(dirs) = &include_dirs {
            let Some(parent) = path.parent().and_then(|p| p.strip_prefix(input_dir).ok()) else { continue };
            let relative = normpath(&to_posix(parent));
            if !dirs.contains(&relative) {
                continue;
            }
        }
        if is_excluded(&path, &excluded) {
            skipped_output += 1;
            continue;
        }
        kept.push(path);
    }

    let by_name = options.sort_by == "name";
    let mut keyed: Vec<(SortKey, PathBuf)> = kept
        .into_par_iter()
        .map(|path| {
            let key = if by_name {
                SortKey::Name(file_name(&path).to_lowercase())
            } else {
                SortKey::Date(exif_date(&path).unwrap_or_else(|| modification_date(&path)))
            };
            (key, path)
        })
        .collect();
    keyed.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    if skipped_output > 0 {
        crate::log::info(&crate::t!("engine.log_skipped_in_output", count = skipped_output));
    }
    crate::log::info(&crate::t!("engine.log_images_found_sorted", count = keyed.len()));

    keyed
        .into_iter()
        .enumerate()
        .map(|(i, (_, path))| Photo { filename: file_name(&path), num: format!("{:03}", i + 1), path })
        .collect()
}

pub fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Folders of `input_dir` (itself included) holding photos directly, with
/// their count: "." first, then alphabetically. Only names are read.
pub fn list_image_folders(input_dir: &Path, output_dir: Option<&Path>) -> Vec<(String, usize)> {
    let excluded = excluded_dirs(output_dir);
    let mut found = Vec::new();
    fn visit(dir: &Path, root: &Path, excluded: &[PathBuf], found: &mut Vec<(String, usize)>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let mut subdirs = Vec::new();
        let mut count = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => {
                    let name = file_name(&path);
                    let skip = (name == "planches" || name == "gallery")
                        && path.canonicalize().map(|p| excluded.contains(&p)).unwrap_or(false);
                    if !skip {
                        subdirs.push(path);
                    }
                }
                Ok(_) if is_supported(&path) => count += 1,
                _ => {}
            }
        }
        if count > 0 {
            let relative = dir.strip_prefix(root).map(to_posix).unwrap_or_default();
            found.push((if relative.is_empty() { ".".into() } else { relative }, count));
        }
        subdirs.sort_by_key(|p| file_name(p).to_lowercase());
        for sub in subdirs {
            visit(&sub, root, excluded, found);
        }
    }
    visit(input_dir, input_dir, &excluded, &mut found);
    found.sort_by(|a, b| {
        (a.0 != ".", a.0.to_lowercase(), &a.0).cmp(&(b.0 != ".", b.0.to_lowercase(), &b.0))
    });
    found
}

/// Supported photos placed directly in `dir`, sorted by path.
pub fn photos_in(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut photos: Vec<PathBuf> = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && is_supported(p))
        .collect();
    photos.sort();
    Ok(photos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> PathBuf {
        let root = std::env::temp_dir().join(format!("pc-scan-{}-{}", std::process::id(), rand_suffix()));
        for (dir, files) in [
            (".", vec!["b.JPG", "a.png", "notes.txt"]),
            ("Sub", vec!["c.jpg"]),
            ("sub2/deep", vec!["d.nef"]),
            ("Portfolio/planches", vec!["planche_001.jpg"]),
        ] {
            let folder = if dir == "." { root.clone() } else { root.join(dir) };
            std::fs::create_dir_all(&folder).unwrap();
            for file in files {
                std::fs::write(folder.join(file), b"x").unwrap();
            }
        }
        root
    }

    fn rand_suffix() -> u64 {
        use std::hash::{BuildHasher, RandomState};
        RandomState::new().hash_one(0u8)
    }

    #[test]
    fn normalization() {
        let root = Path::new("/photos");
        assert_eq!(normalize_include_dir(root, ".").unwrap(), ".");
        assert_eq!(normalize_include_dir(root, "a/./b/").unwrap(), "a/b");
        assert_eq!(normalize_include_dir(root, "a/../b").unwrap(), "b");
        assert_eq!(normalize_include_dir(root, "../x"), None);
        assert_eq!(normalize_include_dir(root, "/photos/x/y").unwrap(), "x/y");
    }

    #[test]
    fn scan_by_name_with_exclusions() {
        let root = tree();
        let mut options = ScanOptions {
            input_dir: root.clone(),
            output_dir: Some(root.join("Portfolio")),
            sort_by: "name".into(),
            ..Default::default()
        };
        let names: Vec<String> = scan(&options).into_iter().map(|p| p.filename).collect();
        assert_eq!(names, vec!["a.png", "b.JPG"]);
        options.recursive = true;
        let photos = scan(&options);
        let names: Vec<&str> = photos.iter().map(|p| p.filename.as_str()).collect();
        assert_eq!(names, vec!["a.png", "b.JPG", "c.jpg", "d.nef"]);
        assert_eq!(photos[3].num, "004");
        options.recursive = false;
        options.include_dirs = Some(vec!["Sub".into(), "sub2/deep".into()]);
        let names: Vec<String> = scan(&options).into_iter().map(|p| p.filename).collect();
        assert_eq!(names, vec!["c.jpg", "d.nef"]);
        options.include_dirs = None;
        options.only_files = Some(vec!["Sub/c.jpg".into(), "../outside.jpg".into(), "notes.txt".into()]);
        let names: Vec<String> = scan(&options).into_iter().map(|p| p.filename).collect();
        assert_eq!(names, vec!["c.jpg"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn folders_with_photos() {
        let root = tree();
        let folders = list_image_folders(&root, Some(&root.join("Portfolio")));
        assert_eq!(folders, vec![(".".into(), 2), ("Sub".into(), 1), ("sub2/deep".into(), 1)]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
