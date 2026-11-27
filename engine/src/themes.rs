//! Colour palettes of the sheets, the PDF and the HTML gallery (the Linux
//! version's portfolio/presentation/themes.py).

use crate::profile::{ThemeCustomColors, ThemeSettings};

#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub background: String,
    pub text: String,
    pub text_secondary: String,
    pub accent: String,
    pub border: String,
}

fn palette(background: &str, text: &str, text_secondary: &str, accent: &str, border: &str) -> Palette {
    Palette {
        background: background.into(),
        text: text.into(),
        text_secondary: text_secondary.into(),
        accent: accent.into(),
        border: border.into(),
    }
}

/// Preset palette of a literal mode ("light", "dark", "neutral"); any other
/// value gives "light".
pub fn get_palette(mode: &str) -> Palette {
    match mode {
        "dark" => palette("#1b1b1b", "#f2f2f2", "#a0a0a0", "#4da3ff", "#3a3a3a"),
        "neutral" => palette("#f0efe9", "#2b2a28", "#6e6a63", "#8a7968", "#dcd8d0"),
        _ => palette("#ffffff", "#000000", "#555555", "#0066cc", "#dddddd"),
    }
}

/// A custom colour, if it is a well-formed "#rgb" or "#rrggbb".
fn valid_hex(value: &Option<String>) -> Option<String> {
    let candidate = value.as_deref()?.trim();
    let digits = candidate.strip_prefix('#')?;
    let ok = (digits.len() == 3 || digits.len() == 6) && digits.chars().all(|c| c.is_ascii_hexdigit());
    ok.then(|| candidate.to_string())
}

pub fn apply_custom_colors(base: Palette, custom: &ThemeCustomColors) -> Palette {
    Palette {
        background: valid_hex(&custom.background).unwrap_or(base.background),
        text: valid_hex(&custom.text).unwrap_or(base.text),
        text_secondary: base.text_secondary,
        accent: valid_hex(&custom.accent).unwrap_or(base.accent),
        border: valid_hex(&custom.border).unwrap_or(base.border),
    }
}

/// The palette of a static output (sheet, PDF): "system" gives "light".
pub fn resolve_theme(theme: &ThemeSettings) -> Palette {
    let mode = if theme.mode == "system" { "light" } else { theme.mode.as_str() };
    apply_custom_colors(get_palette(mode), &theme.custom)
}

/// RGB value of a "#rgb" or "#rrggbb" colour (black if malformed).
pub fn rgb(hex: &str) -> [u8; 3] {
    let digits = hex.trim().trim_start_matches('#');
    let parse = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0);
    match digits.len() {
        3 => {
            let c: Vec<u8> = digits.chars().map(|c| parse(&c.to_string()) * 17).collect();
            [c[0], c[1], c[2]]
        }
        6 => [parse(&digits[0..2]), parse(&digits[2..4]), parse(&digits[4..6])],
        _ => [0, 0, 0],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_colours_are_validated() {
        let custom = ThemeCustomColors {
            background: Some(" #abc ".into()),
            text: Some("#12345".into()),
            accent: Some("#A0B0C0".into()),
            border: None,
        };
        let p = apply_custom_colors(get_palette("dark"), &custom);
        assert_eq!(p.background, "#abc");
        assert_eq!(p.text, "#f2f2f2");
        assert_eq!(p.accent, "#A0B0C0");
        assert_eq!(p.border, "#3a3a3a");
        assert_eq!(rgb("#abc"), [0xaa, 0xbb, 0xcc]);
        assert_eq!(rgb("#0066cc"), [0, 0x66, 0xcc]);
    }

    #[test]
    fn system_is_light_for_static_outputs() {
        let theme = ThemeSettings { mode: "system".into(), ..Default::default() };
        assert_eq!(resolve_theme(&theme), get_palette("light"));
    }
}
