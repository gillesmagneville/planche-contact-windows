//! Per-photo captions: formatting of the EXIF values and line layout (the
//! Linux version's portfolio/presentation/captions.py). A missing value is
//! simply left out of the caption, never an error.

use crate::profile::Profile;
use chrono::NaiveDateTime;

/// What can be shown under a photo (the keys of `CAPTION_FIELDS`). Values
/// are unprocessed: their text is made here.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metadata {
    pub filename: String,
    pub datetime: Option<NaiveDateTime>,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub focal_length: Option<f64>,
    pub focal_35mm: Option<i64>,
    pub aperture: Option<f64>,
    pub shutter_speed: Option<f64>,
    pub iso: Option<i64>,
    pub dimensions: Option<(u32, u32)>,
    pub gps: Option<(f64, f64)>,
}

/// Python's `round(value, decimals)` then the shortest text ("50", "2.8").
pub fn format_number(value: f64, decimals: usize) -> String {
    let factor = 10f64.powi(decimals as i32);
    let rounded = (value * factor).round_ties_even() / factor;
    if rounded == rounded.trunc() {
        return format!("{}", rounded as i64);
    }
    let text = format!("{rounded:.decimals$}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn format_shutter_speed(seconds: f64) -> String {
    if seconds <= 0.0 {
        return "0".into();
    }
    if seconds >= 1.0 {
        return format_number(seconds, 1);
    }
    let denominator = (1.0 / seconds).round_ties_even();
    if denominator <= 0.0 {
        return format_number(seconds, 3);
    }
    format!("1/{}", denominator as i64)
}

fn format_gps(lat: f64, lon: f64) -> String {
    let lat_ref = if lat >= 0.0 { "N" } else { "S" };
    let lon_ref = if lon >= 0.0 { "E" } else { "W" };
    format!("{:.4}°{lat_ref}, {:.4}°{lon_ref}", lat.abs(), lon.abs())
}

/// Default text of one field, or None if the value is missing.
fn default_display(field: &str, m: &Metadata) -> Option<String> {
    let text = match field {
        "filename" => m.filename.clone(),
        "datetime" => m.datetime?.format("%Y-%m-%d").to_string(),
        "camera" => m.camera.as_deref()?.trim().to_string(),
        "lens" => m.lens.as_deref()?.trim().to_string(),
        "focal_length" => format!("{} mm", format_number(m.focal_length?, 1)),
        "focal_35mm" => format!("{} mm ({})", m.focal_35mm?, crate::t!("caption.equiv_24x36")),
        "aperture" => format!("f/{}", format_number(m.aperture?, 1)),
        "shutter_speed" => format!("{}s", format_shutter_speed(m.shutter_speed?)),
        "iso" => format!("ISO {}", m.iso?),
        "dimensions" => {
            let (w, h) = m.dimensions?;
            format!("{w} × {h}")
        }
        "gps" => {
            let (lat, lon) = m.gps?;
            format_gps(lat, lon)
        }
        _ => return None,
    };
    (!text.is_empty()).then_some(text)
}

/// A user template ("f/{value}"), as Python's `template.format(value=...)`:
/// an invalid template gives the default text.
fn apply_template(template: &str, display: &str) -> String {
    let value: &dyn std::fmt::Display = &display;
    crate::i18n::try_format(template, &[("value", value)]).unwrap_or_else(|| display.to_string())
}

fn format_field(field: &str, metadata: &Metadata, profile: &Profile) -> Option<String> {
    let display = default_display(field, metadata)?;
    match profile.captions.format_override(field) {
        Some(template) if !template.is_empty() => Some(apply_template(template, &display)),
        _ => Some(display),
    }
}

/// Splits `items` into at most `n` consecutive groups as equal as possible.
fn chunk_evenly(items: &[String], n: usize) -> Vec<Vec<String>> {
    if n <= 1 || items.len() <= 1 {
        return if items.is_empty() { vec![] } else { vec![items.to_vec()] };
    }
    let (base, extra) = (items.len() / n, items.len() % n);
    let mut chunks = Vec::new();
    let mut start = 0;
    for i in 0..n {
        let size = base + usize::from(i < extra);
        if size == 0 {
            continue;
        }
        chunks.push(items[start..start + size].to_vec());
        start += size;
    }
    chunks
}

/// Texts of the configured fields present for this photo, in order.
pub fn resolve_caption_parts(profile: &Profile, metadata: &Metadata) -> Vec<String> {
    profile
        .captions
        .effective_fields()
        .into_iter()
        .filter_map(|field| format_field(field, metadata, profile))
        .filter(|text| !text.is_empty())
        .collect()
}

/// Caption lines without a width limit (the HTML gallery).
pub fn resolve_caption_lines(profile: &Profile, metadata: &Metadata) -> Vec<String> {
    let parts = resolve_caption_parts(profile, metadata);
    chunk_evenly(&parts, profile.captions.lines.max(1) as usize)
        .into_iter()
        .map(|chunk| chunk.join(&profile.captions.separator))
        .collect()
}

fn wrap_words(text: &str, max_width: i64, measure: &dyn Fn(&str) -> i64) -> Vec<String> {
    if measure(text) <= max_width {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split(' ') {
        let candidate = if current.is_empty() { word.to_string() } else { format!("{current} {word}") };
        if current.is_empty() || measure(&candidate) <= max_width {
            current = candidate;
        } else {
            lines.push(std::mem::replace(&mut current, word.to_string()));
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn wrap_chunk(chunk: &[String], separator: &str, max_width: i64, measure: &dyn Fn(&str) -> i64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for part in chunk {
        if !current.is_empty() {
            let candidate = format!("{current}{separator}{part}");
            if measure(&candidate) <= max_width {
                current = candidate;
                continue;
            }
            lines.push(std::mem::take(&mut current));
        }
        let mut pieces = wrap_words(part, max_width, measure);
        current = pieces.pop().unwrap_or_default();
        lines.extend(pieces);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Caption lines for a rendering in pixels: no line wider than
/// `max_width` (as long as each word fits), wrapping between fields.
pub fn layout_caption_lines(
    profile: &Profile, metadata: &Metadata, max_width: i64, measure: &dyn Fn(&str) -> i64,
) -> Vec<String> {
    if max_width <= 0 {
        return resolve_caption_lines(profile, metadata);
    }
    let parts = resolve_caption_parts(profile, metadata);
    let separator = &profile.captions.separator;
    chunk_evenly(&parts, profile.captions.lines.max(1) as usize)
        .iter()
        .flat_map(|chunk| wrap_chunk(chunk, separator, max_width, measure))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Metadata {
        Metadata {
            filename: "IMG_0001.jpg".into(),
            datetime: NaiveDateTime::parse_from_str("2024:05:17 10:11:12", "%Y:%m:%d %H:%M:%S").ok(),
            camera: Some("Canon EOS R5".into()),
            focal_length: Some(50.0),
            aperture: Some(2.8),
            shutter_speed: Some(1.0 / 250.0),
            iso: Some(400),
            dimensions: Some((6000, 4000)),
            gps: Some((48.85837, -2.294481)),
            ..Default::default()
        }
    }

    fn profile(fields: &[&str]) -> Profile {
        let mut p = Profile::default();
        p.captions.fields = fields.iter().map(|f| f.to_string()).collect();
        p
    }

    #[test]
    fn numbers_like_python() {
        assert_eq!(format_number(50.0, 1), "50");
        assert_eq!(format_number(2.8, 1), "2.8");
        assert_eq!(format_number(2.5, 1), "2.5");
        assert_eq!(format_shutter_speed(1.0 / 250.0), "1/250");
        assert_eq!(format_shutter_speed(2.5), "2.5");
    }

    #[test]
    fn fields_in_order_gps_hidden_unless_allowed() {
        let mut p = profile(&["iso", "aperture", "gps", "lens", "shutter_speed", "datetime", "dimensions"]);
        let parts = resolve_caption_parts(&p, &sample());
        assert_eq!(parts, vec!["ISO 400", "f/2.8", "1/250s", "2024-05-17", "6000 × 4000"]);
        p.captions.show_gps = true;
        assert!(resolve_caption_parts(&p, &sample()).contains(&"48.8584°N, 2.2945°W".to_string()));
    }

    #[test]
    fn templates_and_lines() {
        let mut p = profile(&["filename", "iso", "aperture"]);
        p.captions.format_overrides = vec![("iso".into(), "[{value}]".into()), ("aperture".into(), "{oops}".into())];
        p.captions.lines = 2;
        assert_eq!(resolve_caption_lines(&p, &sample()), vec!["IMG_0001.jpg · [ISO 400]", "f/2.8"]);
    }

    #[test]
    fn wrapping_between_fields() {
        let mut p = profile(&["filename", "iso", "aperture"]);
        p.captions.separator = " | ".into();
        let measure = |text: &str| text.chars().count() as i64;
        let lines = layout_caption_lines(&p, &sample(), 20, &measure);
        assert_eq!(lines, vec!["IMG_0001.jpg", "ISO 400 | f/2.8"]);
        let lines = layout_caption_lines(&profile(&["camera"]), &sample(), 6, &measure);
        assert_eq!(lines, vec!["Canon", "EOS R5"]);
    }
}
