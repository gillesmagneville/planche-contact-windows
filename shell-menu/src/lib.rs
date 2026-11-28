// SPDX-License-Identifier: GPL-3.0-only

//! planche_contact_menu.dll — "Open in Planche-Contact" on the first level of
//! the Windows 11 context menu.
//!
//! Windows 11 only puts on that level the entries declared by a package: the
//! sparse package of `packaging/` declares this DLL's class as the handler of
//! the entry (`IExplorerCommand`), in a COM *surrogate*. File Explorer asks
//! for the class, COM loads the DLL into `dllhost.exe` and File Explorer
//! talks to it from there: nothing of ours runs inside `explorer.exe`, as
//! with `planche-contact-verb.exe` for the classic menu.
//!
//! When the entry is clicked, the selected paths are handed to the window
//! exactly as the classic handler does (`planche_contact_verb::launch`).
//!
//! What decides whether the entry is shown is plain Rust (`offers_entry`),
//! tested on any platform; the COM part only exists in a Windows build.

#![cfg_attr(not(windows), allow(dead_code))]

use planche_contact_verb::extensions::IMAGE_EXTENSIONS;

/// Class identifier of the Windows 11 entry. Distinct from the class of the
/// classic handler (`planche_contact_verb::plan::CLSID`), which is registered
/// in the registry rather than in the package. A fixed value: it is written
/// in the package manifest of existing installations.
pub const CLSID: u128 = 0x5E3D6C2A_9B41_4F7E_A0C8_3F2B7D91C4E6;

/// File name of this DLL, as the package manifest names it.
pub const DLL_NAME: &str = "planche_contact_menu.dll";

/// Name of the sparse package (its `Identity Name`).
pub const PACKAGE_NAME: &str = "PlancheContact.ExplorerMenu";

/// The class identifier in its manifest form: `{XXXXXXXX-XXXX-...}`.
pub fn clsid_braced() -> String {
    let hex = format!("{CLSID:032X}");
    format!(
        "{{{}-{}-{}-{}-{}}}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Whether the entry is offered for a selection, given for each item its
/// path and whether it is a folder: as soon as one item is a folder or a
/// photo of a supported format, as the classic entry (registered on those
/// file types only).
pub fn offers_entry(items: &[(String, bool)]) -> bool {
    items.iter().any(|(path, is_folder)| {
        *is_folder
            || std::path::Path::new(path)
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
    })
}

#[cfg(windows)]
mod com;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photos_and_folders_only() {
        let item = |p: &str, folder| (p.to_string(), folder);
        assert!(offers_entry(&[item(r"C:\Photos", true)]));
        assert!(offers_entry(&[item(r"C:\a.JPG", false)]));
        assert!(offers_entry(&[
            item(r"C:\notes.txt", false),
            item(r"C:\b.nef", false)
        ]));
        assert!(!offers_entry(&[item(r"C:\notes.txt", false)]));
        assert!(!offers_entry(&[item(r"C:\archive", false)]));
        assert!(!offers_entry(&[]));
    }

    #[test]
    fn distinct_from_the_classic_handler() {
        assert_ne!(CLSID, planche_contact_verb::plan::CLSID);
        assert_eq!(clsid_braced(), "{5E3D6C2A-9B41-4F7E-A0C8-3F2B7D91C4E6}");
    }

    /// The package manifest names this DLL, its class and the package.
    #[test]
    fn package_manifest_agrees() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/packaging/AppxManifest.template.xml"
        );
        let manifest = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert!(
            manifest.contains(&format!("Path=\"{DLL_NAME}\"")),
            "the manifest does not name {DLL_NAME}"
        );
        assert!(
            manifest.contains(&format!("Name=\"{PACKAGE_NAME}\"")),
            "the manifest does not name {PACKAGE_NAME}"
        );
        let clsid = clsid_braced();
        let bare = &clsid[1..clsid.len() - 1];
        assert_eq!(
            manifest.matches(bare).count(),
            4,
            "the manifest must use the class {bare} for its 3 entries and its server"
        );
        assert!(
            manifest.contains(&format!(
                "Executable=\"{}\"",
                planche_contact_verb::plan::APP_EXE
            )),
            "the manifest must name the window"
        );
    }
}
