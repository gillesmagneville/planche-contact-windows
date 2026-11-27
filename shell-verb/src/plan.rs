// SPDX-License-Identifier: GPL-3.0-only

//! What the handler writes to the registry, described as plain data so that
//! it can be checked by tests on any platform. `registry.rs` applies it.
//!
//! Everything lives under `HKEY_CURRENT_USER\Software\Classes`: the
//! installation is per user and needs no administrator rights.

use crate::extensions::IMAGE_EXTENSIONS;

/// Class identifier of the handler (a fixed value, generated once for this
/// project: changing it would orphan the entries of existing installations).
pub const CLSID: u128 = 0x8BA32AF5_D148_4EEE_8021_9DF550039E2F;

/// The class identifier in its registry form: `{XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX}`.
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

/// Name of the verb key under each `shell` key.
pub const VERB_KEY: &str = "PlancheContact";

/// File name of the graphical application, installed next to this program.
pub const APP_EXE: &str = "PlancheContact.exe";

/// File name of this program.
pub const VERB_EXE: &str = "planche-contact-verb.exe";

const CLASSES: &str = r"Software\Classes";

/// A string value to write: `name` empty means the default value of the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegValue {
    pub key: String,
    pub name: &'static str,
    pub data: String,
}

/// Keys under which the verb is offered: one per image extension (whatever
/// program the extension is associated with), folders, and the background
/// of a folder window.
fn verb_parents() -> Vec<String> {
    let mut parents: Vec<String> = IMAGE_EXTENSIONS
        .iter()
        .map(|ext| format!(r"{CLASSES}\SystemFileAssociations\.{ext}"))
        .collect();
    parents.push(format!(r"{CLASSES}\Directory"));
    parents.push(format!(r"{CLASSES}\Directory\Background"));
    parents
}

fn clsid_key() -> String {
    format!(r"{CLASSES}\CLSID\{}", clsid_braced())
}

/// All the values to write for an installation in `install_dir`, with
/// `label` as the text of the context-menu entry.
pub fn registration(install_dir: &str, label: &str) -> Vec<RegValue> {
    let dir = install_dir.trim_end_matches('\\');
    let value = |key: String, name: &'static str, data: String| RegValue { key, name, data };

    let mut values = vec![
        value(
            clsid_key(),
            "",
            "planche-contact-windows context-menu handler".to_string(),
        ),
        // File Explorer starts this program (with the -Embedding argument) to
        // obtain the handler: it never runs inside explorer.exe.
        value(
            format!(r"{}\LocalServer32", clsid_key()),
            "",
            format!("\"{dir}\\{VERB_EXE}\""),
        ),
    ];
    for parent in verb_parents() {
        let verb = format!(r"{parent}\shell\{VERB_KEY}");
        values.push(value(verb.clone(), "", label.to_string()));
        values.push(value(
            verb.clone(),
            "Icon",
            format!("\"{dir}\\{APP_EXE}\",0"),
        ));
        // "Player": the entry stays available whatever the number of selected
        // files, and the whole selection arrives in a single call.
        values.push(value(
            verb.clone(),
            "MultiSelectModel",
            "Player".to_string(),
        ));
        // Never the default action: double-clicking a file must keep opening
        // it, even for a type that no program is associated with.
        values.push(value(verb.clone(), "NeverDefault", String::new()));
        values.push(value(
            format!(r"{verb}\command"),
            "DelegateExecute",
            clsid_braced(),
        ));
    }
    values
}

/// Keys to delete with everything below them when uninstalling.
pub fn trees_to_delete() -> Vec<String> {
    let mut trees: Vec<String> = verb_parents()
        .into_iter()
        .map(|parent| format!(r"{parent}\shell\{VERB_KEY}"))
        .collect();
    trees.push(clsid_key());
    trees
}

/// Keys the registration may have created on the way, to delete afterwards
/// only if they are left completely empty (no value, no subkey) — deepest
/// first. Another program's entries are never touched.
pub fn keys_to_prune() -> Vec<String> {
    let mut keys = Vec::new();
    for ext in IMAGE_EXTENSIONS {
        keys.push(format!(r"{CLASSES}\SystemFileAssociations\.{ext}\shell"));
        keys.push(format!(r"{CLASSES}\SystemFileAssociations\.{ext}"));
    }
    keys.push(format!(r"{CLASSES}\SystemFileAssociations"));
    keys.push(format!(r"{CLASSES}\Directory\Background\shell"));
    keys.push(format!(r"{CLASSES}\Directory\Background"));
    keys.push(format!(r"{CLASSES}\Directory\shell"));
    keys.push(format!(r"{CLASSES}\Directory"));
    keys.push(format!(r"{CLASSES}\CLSID"));
    keys
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(values: &'a [RegValue], key: &str, name: &str) -> &'a str {
        values
            .iter()
            .find(|v| v.key == key && v.name == name)
            .map(|v| v.data.as_str())
            .unwrap_or_else(|| panic!("missing value {key} / {name}"))
    }

    #[test]
    fn clsid_has_the_registry_form() {
        assert_eq!(clsid_braced(), "{8BA32AF5-D148-4EEE-8021-9DF550039E2F}");
    }

    #[test]
    fn clsid_points_to_the_handler_program() {
        let values = registration(r"C:\Users\Zoé\App Dir\", "Open in Planche-Contact");
        let clsid = r"Software\Classes\CLSID\{8BA32AF5-D148-4EEE-8021-9DF550039E2F}";
        assert_eq!(
            find(&values, &format!(r"{clsid}\LocalServer32"), ""),
            "\"C:\\Users\\Zoé\\App Dir\\planche-contact-verb.exe\""
        );
    }

    #[test]
    fn every_extension_and_folders_get_the_verb() {
        let values = registration(r"C:\App", "Ouvrir dans Planche-Contact");
        let mut parents: Vec<String> = IMAGE_EXTENSIONS
            .iter()
            .map(|e| format!(r"Software\Classes\SystemFileAssociations\.{e}"))
            .collect();
        parents.push(r"Software\Classes\Directory".to_string());
        parents.push(r"Software\Classes\Directory\Background".to_string());
        for parent in parents {
            let verb = format!(r"{parent}\shell\PlancheContact");
            assert_eq!(find(&values, &verb, ""), "Ouvrir dans Planche-Contact");
            assert_eq!(find(&values, &verb, "MultiSelectModel"), "Player");
            assert_eq!(find(&values, &verb, "NeverDefault"), "");
            assert_eq!(
                find(&values, &verb, "Icon"),
                "\"C:\\App\\PlancheContact.exe\",0"
            );
            assert_eq!(
                find(&values, &format!(r"{verb}\command"), "DelegateExecute"),
                "{8BA32AF5-D148-4EEE-8021-9DF550039E2F}"
            );
        }
        // 2 values for the class + 5 per parent.
        assert_eq!(values.len(), 2 + 5 * (IMAGE_EXTENSIONS.len() + 2));
    }

    #[test]
    fn uninstall_removes_exactly_what_install_wrote() {
        let values = registration(r"C:\App", "x");
        let trees = trees_to_delete();
        for v in &values {
            assert!(
                trees
                    .iter()
                    .any(|t| v.key == *t || v.key.starts_with(&format!(r"{t}\"))),
                "{} would survive an uninstall",
                v.key
            );
        }
        // Never a whole `shell` key or an extension key: only our own verb.
        for t in &trees {
            assert!(t.ends_with(VERB_KEY) || t.ends_with(&clsid_braced()), "{t}");
        }
    }

    fn repository_file(relative: &str) -> String {
        let path = format!("{}/../{relative}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    /// The programs find each other by file name: the names written here
    /// and in the application project must agree.
    #[test]
    fn program_names_match_the_application_project() {
        let project = repository_file("app/PlancheContact/PlancheContact.csproj");
        let assembly = APP_EXE.trim_end_matches(".exe");
        assert!(
            project.contains(&format!("<AssemblyName>{assembly}</AssemblyName>")),
            "the application project does not build {APP_EXE}"
        );
        assert_eq!(VERB_EXE, concat!(env!("CARGO_PKG_NAME"), ".exe"));
    }

    /// One version for the whole project: the VERSION file. Cargo.toml must
    /// be bumped along with it.
    #[test]
    fn cargo_version_is_the_version_file() {
        assert_eq!(repository_file("VERSION").trim(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn pruning_goes_from_the_deepest_key_up() {
        let keys = keys_to_prune();
        for (i, key) in keys.iter().enumerate() {
            for later in &keys[i + 1..] {
                assert!(
                    !later.starts_with(&format!(r"{key}\")),
                    "{later} must be pruned before {key}"
                );
            }
        }
    }
}
