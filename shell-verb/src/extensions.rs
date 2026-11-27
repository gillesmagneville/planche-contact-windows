// SPDX-License-Identifier: GPL-3.0-only

//! File extensions on which "Open in Planche-Contact" is offered.

/// Extensions (lowercase, without the dot) the engine places on a sheet.
/// Must stay identical to `SUPPORTED_EXTENSIONS` of `engine/src/scanner.rs`
/// — the test below fails otherwise.
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "arw", "bmp", "cr2", "cr3", "dng", "heic", "heif", "jpeg", "jpg", "nef", "png", "tif", "tiff",
    "webp",
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The string literals of `SUPPORTED_EXTENSIONS` in the engine.
    fn engine_extensions() -> BTreeSet<String> {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../engine/src/scanner.rs");
        let source = std::fs::read_to_string(path).expect("engine/src/scanner.rs must be readable");
        let start = source
            .find("pub const SUPPORTED_EXTENSIONS")
            .expect("SUPPORTED_EXTENSIONS not found");
        let end = start + source[start..].find("];").expect("end of the table not found");
        source[start..end]
            .split('=')
            .nth(1)
            .unwrap()
            .split('"')
            .skip(1)
            .step_by(2)
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn list_matches_the_engine() {
        let ours: BTreeSet<String> = IMAGE_EXTENSIONS.iter().map(|e| e.to_string()).collect();
        assert_eq!(ours.len(), IMAGE_EXTENSIONS.len(), "duplicate extension in the list");
        assert_eq!(ours, engine_extensions());
    }

    #[test]
    fn extensions_are_lowercase_without_dot() {
        for ext in IMAGE_EXTENSIONS {
            assert!(!ext.is_empty() && !ext.contains('.'), "{ext}");
            assert_eq!(*ext, ext.to_lowercase(), "{ext}");
        }
    }
}
