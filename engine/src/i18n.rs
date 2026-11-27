//! Translations of the texts the engine writes: the sheets (footer), the
//! HTML gallery, the CSV header, the log and the progress messages.
//!
//! The texts are those of the Linux version, kept as they are in
//! `locales/xx.py` (one Python dictionary per language) and embedded in the
//! executable: the window (app/) reads the very same files, and a language
//! file can be copied over from planche-contact-linux without any change.
//!
//! Only the shape of those files is understood: one `"key": "value",` entry
//! per line, double-quoted strings with the usual escapes, comments and blank
//! lines (see `parse`, and the test that checks every line is understood).

use std::collections::HashMap;
use std::sync::OnceLock;

/// Display order of the language selector; "system" follows Windows.
pub const AVAILABLE_LANGUAGES: [&str; 5] = ["system", "en", "fr", "de", "es"];

const FALLBACK_LANGUAGE: &str = "en";

const SOURCES: [(&str, &str); 4] = [
    ("en", include_str!("../../locales/en.py")),
    ("fr", include_str!("../../locales/fr.py")),
    ("de", include_str!("../../locales/de.py")),
    ("es", include_str!("../../locales/es.py")),
];

type Table = HashMap<String, String>;

fn tables() -> &'static HashMap<&'static str, Table> {
    static TABLES: OnceLock<HashMap<&'static str, Table>> = OnceLock::new();
    TABLES.get_or_init(|| {
        SOURCES
            .iter()
            .map(|(language, source)| (*language, parse(source).0))
            .collect()
    })
}

static CURRENT: OnceLock<String> = OnceLock::new();

/// Parses a locale file. Also returns the lines inside the dictionary that
/// are neither an entry, a comment nor blank (none in a well-formed file).
pub fn parse(source: &str) -> (Table, Vec<String>) {
    let mut table = Table::new();
    let mut unread = Vec::new();
    let mut inside = false;
    for raw in source.lines() {
        let line = raw.trim();
        if !inside {
            inside = line.starts_with("STRINGS = {");
            continue;
        }
        if line == "}" {
            break;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        match parse_entry(line) {
            Some((key, value)) => {
                table.insert(key, value);
            }
            None => unread.push(line.to_string()),
        }
    }
    (table, unread)
}

/// One `"key": "value",` line (a trailing comment allowed).
fn parse_entry(line: &str) -> Option<(String, String)> {
    let (key, rest) = quoted(line)?;
    let rest = rest.trim_start().strip_prefix(':')?.trim_start();
    let (value, rest) = quoted(rest)?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(',').unwrap_or(rest).trim_start();
    if !rest.is_empty() && !rest.starts_with('#') {
        return None;
    }
    Some((unescape(&key), unescape(&value)))
}

/// A double-quoted string at the start of `text` (escapes left as they
/// are), and what follows it.
fn quoted(text: &str) -> Option<(String, &str)> {
    let body = text.strip_prefix('"')?;
    let mut escaped = false;
    for (i, c) in body.char_indices() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '"' {
            return Some((body[..i].to_string(), &body[i + 1..]));
        }
    }
    None
}

/// Content of a Python string literal, escapes resolved.
pub fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('u') => {
                let code: String = chars.clone().take(4).collect();
                match u32::from_str_radix(&code, 16).ok().and_then(char::from_u32) {
                    Some(decoded) if code.len() == 4 => {
                        out.push(decoded);
                        for _ in 0..4 {
                            chars.next();
                        }
                    }
                    _ => out.push('u'),
                }
            }
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// Two-letter code of the language of the system ("fr", "de"...), or None.
pub fn detect_system_language() -> Option<String> {
    system_locale().and_then(|locale| {
        let code = locale.replace('-', "_");
        let code = code.split(['_', '.']).next().unwrap_or("").to_lowercase();
        if code.is_empty() || code == "c" || code == "posix" {
            None
        } else {
            Some(code)
        }
    })
}

#[cfg(windows)]
fn system_locale() -> Option<String> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultLocaleName(name: *mut u16, length: i32) -> i32;
    }
    let mut buffer = [0u16; 85];
    // SAFETY: the buffer is LOCALE_NAME_MAX_LENGTH (85) characters long.
    let written = unsafe { GetUserDefaultLocaleName(buffer.as_mut_ptr(), buffer.len() as i32) };
    if written <= 1 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..written as usize - 1]))
}

#[cfg(not(windows))]
fn system_locale() -> Option<String> {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
}

/// The language to use for a saved preference ("system", "en", "fr"...),
/// and the code of the system language when it has no translation ("?" if
/// it could not be detected) - the caller then warns the user.
pub fn resolve_language(preference: &str) -> (String, Option<String>) {
    if preference != "system" && tables().contains_key(preference) {
        return (preference.to_string(), None);
    }
    match detect_system_language() {
        Some(code) if tables().contains_key(code.as_str()) => (code, None),
        detected => (
            FALLBACK_LANGUAGE.to_string(),
            Some(detected.unwrap_or_else(|| "?".to_string())),
        ),
    }
}

/// Activates the language for the rest of the process (once: later calls
/// are ignored). Returns the same tuple as `resolve_language`.
pub fn set_language(preference: &str) -> (String, Option<String>) {
    let resolved = resolve_language(preference);
    let _ = CURRENT.set(resolved.0.clone());
    resolved
}

/// The active language, never "system".
pub fn language() -> &'static str {
    CURRENT.get().map(String::as_str).unwrap_or(FALLBACK_LANGUAGE)
}

/// The text of `key` in the active language, then in English, then the key
/// itself: a missing translation never stops anything.
pub fn tr(key: &str) -> String {
    let all = tables();
    all.get(language())
        .and_then(|table| table.get(key))
        .or_else(|| all.get(FALLBACK_LANGUAGE).and_then(|table| table.get(key)))
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

/// `tr` with `{name}` fields replaced, as Python's `str.format(**fields)`:
/// `{{` and `}}` give a brace; if a field is missing, the text is returned
/// as it is.
pub fn trf(key: &str, fields: &[(&str, &dyn std::fmt::Display)]) -> String {
    format_fields(&tr(key), fields)
}

pub fn format_fields(template: &str, fields: &[(&str, &dyn std::fmt::Display)]) -> String {
    try_format(template, fields).unwrap_or_else(|| template.to_string())
}

/// Python's `template.format(**fields)`, or None when the template cannot
/// be applied (unknown field, lone brace).
pub fn try_format(template: &str, fields: &[(&str, &dyn std::fmt::Display)]) -> Option<String> {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") {
            out.push('{');
            rest = &tail[2..];
        } else if tail.starts_with("}}") {
            out.push('}');
            rest = &tail[2..];
        } else if tail.starts_with('{') {
            let end = tail.find('}')?;
            let name = &tail[1..end];
            let (_, value) = fields.iter().find(|(field, _)| *field == name)?;
            out.push_str(&value.to_string());
            rest = &tail[end + 1..];
        } else {
            return None;
        }
    }
    out.push_str(rest);
    Some(out)
}

/// `trf` with a shorter syntax: `t!("key")`, `t!("key", name = value, ...)`.
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::tr($key)
    };
    ($key:expr, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::trf($key, &[$((stringify!($name), &$value as &dyn std::fmt::Display)),+])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_of_every_locale_is_understood() {
        for (language, source) in SOURCES {
            let (table, unread) = parse(source);
            assert!(unread.is_empty(), "{language}: {unread:?}");
            assert!(table.len() > 250, "{language}: {} keys", table.len());
        }
    }

    #[test]
    fn english_has_every_key_of_the_other_languages() {
        let all = tables();
        let english = &all["en"];
        for (language, table) in all {
            for key in table.keys() {
                assert!(english.contains_key(key), "{language}: {key} missing in English");
            }
        }
    }

    #[test]
    fn escapes_are_resolved() {
        assert_eq!(unescape(r#"a\nb \"c\" \\d é"#), "a\nb \"c\" \\d é");
        let (table, _) = parse("STRINGS = {\n    \"k\": \"v \\\"q\\\"\",  # note\n}\n");
        assert_eq!(table["k"], "v \"q\"");
    }

    #[test]
    fn fields_are_formatted_like_python() {
        let n = 3;
        assert_eq!(format_fields("{n} sur {total}", &[("n", &n), ("total", &7)]), "3 sur 7");
        assert_eq!(format_fields("{{x}} {n}", &[("n", &n)]), "{x} 3");
        assert_eq!(format_fields("{missing}", &[("n", &n)]), "{missing}");
    }

    #[test]
    fn known_texts() {
        let fr = &tables()["fr"];
        assert_eq!(fr["csv.header_filename"].is_empty(), false);
        assert!(tables()["en"]["sheet.footer_credit"].contains("{name}"));
    }
}
