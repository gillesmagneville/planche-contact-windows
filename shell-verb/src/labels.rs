// SPDX-License-Identifier: GPL-3.0-only

//! Text of the context-menu entry, in the languages of the application.
//!
//! The entry is a static registry value read by File Explorer itself: it is
//! written once, at installation time, in the display language of the user
//! who installs (English when that language is not one of ours). The texts
//! are those of the Nautilus entry of the Linux version.

/// (primary language identifier, ISO code, label).
pub const LABELS: &[(u16, &str, &str)] = &[
    (0x09, "en", "Open in Planche-Contact"),
    (0x0c, "fr", "Ouvrir dans Planche-Contact"),
    (0x07, "de", "In Planche-Contact öffnen"),
    (0x0a, "es", "Abrir en Planche-Contact"),
];

/// Label for a Windows language identifier (LANGID, as returned by
/// `GetUserDefaultUILanguage`). Its low 10 bits are the primary language.
pub fn menu_label(lang_id: u16) -> &'static str {
    let primary = lang_id & 0x03ff;
    LABELS
        .iter()
        .find(|(id, _, _)| *id == primary)
        .map_or(LABELS[0].2, |(_, _, label)| label)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_languages_and_fallback() {
        assert_eq!(menu_label(0x040c), "Ouvrir dans Planche-Contact"); // fr-FR
        assert_eq!(menu_label(0x0c0c), "Ouvrir dans Planche-Contact"); // fr-CA
        assert_eq!(menu_label(0x0407), "In Planche-Contact öffnen"); // de-DE
        assert_eq!(menu_label(0x0c0a), "Abrir en Planche-Contact"); // es-ES
        assert_eq!(menu_label(0x0409), "Open in Planche-Contact"); // en-US
        assert_eq!(menu_label(0x0411), "Open in Planche-Contact"); // ja-JP -> English
    }

    /// The last page of the installer quotes the menu entry, in the same
    /// languages: it must name it as it really appears.
    #[test]
    fn labels_are_those_quoted_by_the_installer() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../installer/installer.nsi");
        let installer = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        for (_, lang, label) in LABELS {
            assert!(installer.contains(label), "installer.nsi does not quote the {lang} label");
        }
    }
}
