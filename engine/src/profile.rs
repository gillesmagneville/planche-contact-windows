//! The "profile": layout, captions and theme settings, and its JSON form
//! (the files saved by the window, or given with `--profile`).
//!
//! Same rules as the Linux version (portfolio/presentation/profile.py): the
//! absence of a profile gives the historical rendering, and a missing,
//! invalid or unknown value falls back to its default - never an error,
//! never a generation stopped because of a profile file.

use serde_json::{json, Map, Value};
use std::path::Path;

pub const LAYOUT_TEMPLATES: [&str; 5] = ["grid", "landscape", "portrait", "large_thumbs", "free"];
pub const FIT_MODES: [&str; 2] = ["contain", "cover"];
pub const PAGE_FORMATS: [&str; 5] = ["A5", "A4", "A3", "A2", "Letter"];
pub const ORIENTATIONS: [&str; 2] = ["portrait", "landscape"];
pub const HEADER_POSITIONS: [&str; 4] = ["top_left", "top_center", "top_right", "none"];
pub const CAPTION_POSITIONS: [&str; 4] = ["below", "above", "right", "none"];
pub const CAPTION_ALIGNMENTS: [&str; 3] = ["left", "center", "right"];
pub const CAPTION_FIELDS: [&str; 11] = [
    "filename", "datetime", "camera", "lens", "focal_length", "focal_35mm",
    "aperture", "shutter_speed", "iso", "dimensions", "gps",
];
pub const THEME_MODES: [&str; 4] = ["light", "dark", "neutral", "system"];

/// Internal values of `--watermark-orientation`: compared as they are,
/// never translated (they are those of the Linux version).
pub const WATERMARK_ORIENTATIONS: [&str; 3] = ["Horizontal", "Diagonale horaire", "Diagonale anti-horaire"];

#[derive(Clone, Debug, PartialEq)]
pub struct LayoutSettings {
    pub template: String,
    pub columns: Option<u32>,
    pub rows: Option<u32>,
    pub per_page: u32,
    pub margin_mm: f64,
    pub spacing_mm: f64,
    pub page_format: String,
    pub orientation: String,
    pub fit_mode: String,
    pub title_position: String,
    pub author_position: String,
    pub page_numbering: bool,
    pub caption_position: String,
    pub caption_area_mm: f64,
}

impl Default for LayoutSettings {
    fn default() -> Self {
        LayoutSettings {
            template: "grid".into(),
            columns: None,
            rows: None,
            per_page: 12,
            margin_mm: 12.0,
            spacing_mm: 5.0,
            page_format: "A4".into(),
            orientation: "portrait".into(),
            fit_mode: "contain".into(),
            title_position: "top_center".into(),
            author_position: "top_center".into(),
            page_numbering: true,
            caption_position: "none".into(),
            caption_area_mm: 8.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CaptionSettings {
    pub fields: Vec<String>,
    /// In the order of the file (kept when saved again).
    pub format_overrides: Vec<(String, String)>,
    pub lines: u32,
    pub font_size: u32,
    pub alignment: String,
    pub separator: String,
    pub show_gps: bool,
}

impl Default for CaptionSettings {
    fn default() -> Self {
        CaptionSettings {
            fields: Vec::new(),
            format_overrides: Vec::new(),
            lines: 1,
            font_size: 9,
            alignment: "center".into(),
            separator: " · ".into(),
            show_gps: false,
        }
    }
}

impl CaptionSettings {
    /// Fields actually shown: GPS is left out unless `show_gps` is on, so
    /// that a location can never be shown by mistake.
    pub fn effective_fields(&self) -> Vec<&str> {
        self.fields
            .iter()
            .map(String::as_str)
            .filter(|field| self.show_gps || *field != "gps")
            .collect()
    }

    pub fn format_override(&self, field: &str) -> Option<&str> {
        self.format_overrides
            .iter()
            .find(|(key, _)| key == field)
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ThemeCustomColors {
    pub background: Option<String>,
    pub text: Option<String>,
    pub accent: Option<String>,
    pub border: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ThemeSettings {
    pub mode: String,
    pub custom: ThemeCustomColors,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        ThemeSettings { mode: "light".into(), custom: ThemeCustomColors::default() }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub name: Option<String>,
    pub layout: LayoutSettings,
    pub captions: CaptionSettings,
    pub theme: ThemeSettings,
}

/// The profile the window shows at start and on "Reset": the default one
/// plus the file name under each photo. The command line, without
/// `--profile`, keeps the default profile (no caption).
pub fn interface_default_profile() -> Profile {
    let mut profile = Profile::default();
    profile.layout.caption_position = "below".into();
    profile.captions.fields = vec!["filename".into()];
    profile
}

// ------------------------------------------------------------ JSON reading

fn choice(value: Option<&Value>, choices: &[&str], default: &str) -> String {
    match value.and_then(Value::as_str) {
        Some(text) if choices.contains(&text) => text.to_string(),
        _ => default.to_string(),
    }
}

/// A number (an integer is accepted, a boolean is not).
fn number(value: Option<&Value>, default: f64) -> f64 {
    value.and_then(Value::as_f64).unwrap_or(default)
}

fn boolean(value: Option<&Value>, default: bool) -> bool {
    value.and_then(Value::as_bool).unwrap_or(default)
}

/// A strictly positive integer, or `default`.
fn positive(value: Option<&Value>, default: Option<u32>) -> Option<u32> {
    match value {
        None | Some(Value::Null) => default,
        Some(v) => match v.as_u64() {
            Some(n) if n > 0 && v.is_u64() => Some(n.min(u32::MAX as u64) as u32),
            _ => default,
        },
    }
}

fn color(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|text| text.starts_with('#'))
        .map(str::to_string)
}

impl LayoutSettings {
    fn from_json(data: Option<&Value>) -> Self {
        let d = LayoutSettings::default();
        let Some(data) = data.and_then(Value::as_object) else { return d };
        LayoutSettings {
            template: choice(data.get("template"), &LAYOUT_TEMPLATES, &d.template),
            columns: positive(data.get("columns"), None),
            rows: positive(data.get("rows"), None),
            per_page: positive(data.get("per_page"), Some(d.per_page)).unwrap_or(d.per_page),
            margin_mm: number(data.get("margin_mm"), d.margin_mm),
            spacing_mm: number(data.get("spacing_mm"), d.spacing_mm),
            page_format: choice(data.get("page_format"), &PAGE_FORMATS, &d.page_format),
            orientation: choice(data.get("orientation"), &ORIENTATIONS, &d.orientation),
            fit_mode: choice(data.get("fit_mode"), &FIT_MODES, &d.fit_mode),
            title_position: choice(data.get("title_position"), &HEADER_POSITIONS, &d.title_position),
            author_position: choice(data.get("author_position"), &HEADER_POSITIONS, &d.author_position),
            page_numbering: boolean(data.get("page_numbering"), d.page_numbering),
            caption_position: choice(data.get("caption_position"), &CAPTION_POSITIONS, &d.caption_position),
            caption_area_mm: number(data.get("caption_area_mm"), d.caption_area_mm),
        }
    }
}

impl CaptionSettings {
    fn from_json(data: Option<&Value>) -> Self {
        let d = CaptionSettings::default();
        let Some(data) = data.and_then(Value::as_object) else { return d };
        let mut fields: Vec<String> = Vec::new();
        if let Some(list) = data.get("fields").and_then(Value::as_array) {
            for field in list.iter().filter_map(Value::as_str) {
                if CAPTION_FIELDS.contains(&field) && !fields.iter().any(|f| f == field) {
                    fields.push(field.to_string());
                }
            }
        }
        let mut format_overrides = Vec::new();
        if let Some(map) = data.get("format_overrides").and_then(Value::as_object) {
            for (key, value) in map {
                if let (true, Some(text)) = (CAPTION_FIELDS.contains(&key.as_str()), value.as_str()) {
                    format_overrides.push((key.clone(), text.to_string()));
                }
            }
        }
        CaptionSettings {
            fields,
            format_overrides,
            lines: positive(data.get("lines"), Some(d.lines)).unwrap_or(d.lines),
            font_size: positive(data.get("font_size"), Some(d.font_size)).unwrap_or(d.font_size),
            alignment: choice(data.get("alignment"), &CAPTION_ALIGNMENTS, &d.alignment),
            separator: data
                .get("separator")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or(d.separator),
            show_gps: boolean(data.get("show_gps"), d.show_gps),
        }
    }
}

impl ThemeSettings {
    fn from_json(data: Option<&Value>) -> Self {
        let d = ThemeSettings::default();
        let Some(data) = data.and_then(Value::as_object) else { return d };
        let custom = data.get("custom").and_then(Value::as_object);
        let get = |key: &str| color(custom.and_then(|c| c.get(key)));
        ThemeSettings {
            mode: choice(data.get("mode"), &THEME_MODES, &d.mode),
            custom: ThemeCustomColors {
                background: get("background"),
                text: get("text"),
                accent: get("accent"),
                border: get("border"),
            },
        }
    }
}

impl Profile {
    pub fn from_json(data: &Value) -> Profile {
        let Some(object) = data.as_object() else {
            crate::log::warning(&crate::t!("profile.warn_not_object"));
            return Profile::default();
        };
        Profile {
            name: object.get("name").and_then(Value::as_str).map(str::to_string),
            layout: LayoutSettings::from_json(object.get("layout")),
            captions: CaptionSettings::from_json(object.get("captions")),
            theme: ThemeSettings::from_json(object.get("theme")),
        }
    }

    /// The JSON form, with the keys in the order of the Linux version.
    pub fn to_json(&self) -> Value {
        let l = &self.layout;
        let c = &self.captions;
        let t = &self.theme;
        let overrides: Map<String, Value> = c
            .format_overrides
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect();
        json!({
            "name": self.name,
            "layout": {
                "template": l.template,
                "columns": l.columns,
                "rows": l.rows,
                "per_page": l.per_page,
                "margin_mm": l.margin_mm,
                "spacing_mm": l.spacing_mm,
                "page_format": l.page_format,
                "orientation": l.orientation,
                "fit_mode": l.fit_mode,
                "title_position": l.title_position,
                "author_position": l.author_position,
                "page_numbering": l.page_numbering,
                "caption_position": l.caption_position,
                "caption_area_mm": l.caption_area_mm,
            },
            "captions": {
                "fields": c.fields,
                "format_overrides": overrides,
                "lines": c.lines,
                "font_size": c.font_size,
                "alignment": c.alignment,
                "separator": c.separator,
                "show_gps": c.show_gps,
            },
            "theme": {
                "mode": t.mode,
                "custom": {
                    "background": t.custom.background,
                    "text": t.custom.text,
                    "accent": t.custom.accent,
                    "border": t.custom.border,
                },
            },
        })
    }
}

/// Reads a profile file. Never fails: a missing, unreadable or invalid
/// file gives the default profile, with a warning in the log.
pub fn load_profile(path: &Path) -> Profile {
    let text = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            crate::log::warning(&crate::t!("profile.warn_not_found", path = path.display()));
            return Profile::default();
        }
        Err(e) => {
            crate::log::warning(&crate::t!("profile.warn_unreadable", path = path.display(), error = e));
            return Profile::default();
        }
    };
    let text = text.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&text);
    match serde_json::from_slice::<Value>(text) {
        Ok(value) => Profile::from_json(&value),
        Err(e) => {
            crate::log::warning(&crate::t!("profile.warn_unreadable", path = path.display(), error = e));
            Profile::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_round_trip() {
        let profile = Profile::default();
        assert_eq!(Profile::from_json(&profile.to_json()), profile);
        let ui = interface_default_profile();
        assert_eq!(Profile::from_json(&ui.to_json()), ui);
    }

    #[test]
    fn invalid_values_fall_back_to_their_default() {
        let data = json!({
            "layout": {"template": "spiral", "per_page": -3, "margin_mm": true, "columns": 4,
                       "spacing_mm": 3, "page_numbering": "yes"},
            "captions": {"fields": ["iso", "nope", "iso", "gps"], "lines": 0,
                         "format_overrides": {"iso": "ISO:{value}", "x": "y", "lens": 3}},
            "theme": {"mode": "pink", "custom": {"background": "#123", "text": "red"}},
        });
        let p = Profile::from_json(&data);
        assert_eq!(p.layout.template, "grid");
        assert_eq!(p.layout.per_page, 12);
        assert_eq!(p.layout.margin_mm, 12.0);
        assert_eq!(p.layout.spacing_mm, 3.0);
        assert_eq!(p.layout.columns, Some(4));
        assert!(p.layout.page_numbering);
        assert_eq!(p.captions.fields, vec!["iso", "gps"]);
        assert_eq!(p.captions.effective_fields(), vec!["iso"]);
        assert_eq!(p.captions.lines, 1);
        assert_eq!(p.captions.format_overrides, vec![("iso".to_string(), "ISO:{value}".to_string())]);
        assert_eq!(p.theme.mode, "light");
        assert_eq!(p.theme.custom.background.as_deref(), Some("#123"));
        assert_eq!(p.theme.custom.text, None);
    }

    #[test]
    fn json_keys_follow_the_linux_order() {
        let text = serde_json::to_string(&Profile::default().to_json()).unwrap();
        assert!(text.starts_with(r#"{"name":null,"layout":{"template":"grid","columns":null"#), "{text}");
        assert!(text.contains(r#""margin_mm":12.0"#), "{text}");
    }
}
