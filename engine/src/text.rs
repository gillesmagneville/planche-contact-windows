//! Text drawn on the sheets and on the watermark, with the DejaVu fonts
//! bundled in the executable (the same as the Linux version: identical
//! rendering everywhere, whatever fonts the system has).
//!
//! Positions follow the Pillow conventions of the Linux version: a text
//! drawn at (x, y) has its left edge at x and the top of its line (the
//! font's ascender) at y.

use ab_glyph::{Font, FontRef, GlyphId, PxScale, ScaleFont};
use image::RgbImage;

static REGULAR_TTF: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");
static BOLD_TTF: &[u8] = include_bytes!("../assets/fonts/DejaVuSans-Bold.ttf");

pub fn font_bytes(bold: bool) -> &'static [u8] {
    if bold { BOLD_TTF } else { REGULAR_TTF }
}

fn face(bold: bool) -> FontRef<'static> {
    FontRef::try_from_slice(font_bytes(bold)).expect("bundled font")
}

/// A font at a size in pixels (the em size, as Pillow's `truetype(size)`).
#[derive(Clone)]
pub struct SizedFont {
    font: FontRef<'static>,
    pub size: f32,
    pub bold: bool,
}

impl SizedFont {
    pub fn new(size: i64, bold: bool) -> Self {
        SizedFont { font: face(bold), size: size.max(1) as f32, bold }
    }

    fn scale(&self) -> PxScale {
        // ab_glyph's scale is the height (ascent - descent), Pillow's the em.
        let upem = self.font.units_per_em().unwrap_or(2048.0);
        PxScale::from(self.size * self.font.height_unscaled() / upem)
    }

    /// Distance from the top of the line to the baseline, in pixels
    /// (Pillow's `getmetrics()[0]`).
    pub fn ascent(&self) -> f32 {
        self.font.as_scaled(self.scale()).ascent().ceil()
    }

    /// Glyphs and their pen position along the line.
    fn layout(&self, text: &str) -> (Vec<(GlyphId, f32)>, f32) {
        let scaled = self.font.as_scaled(self.scale());
        let mut pen = 0.0f32;
        let mut previous: Option<GlyphId> = None;
        let mut glyphs = Vec::with_capacity(text.len());
        for c in text.chars() {
            let id = scaled.glyph_id(c);
            if let Some(prev) = previous {
                pen += scaled.kern(prev, id);
            }
            glyphs.push((id, pen));
            pen += scaled.h_advance(id);
            previous = Some(id);
        }
        (glyphs, pen)
    }

    /// Advance width of the text (Pillow's `textlength`).
    pub fn text_length(&self, text: &str) -> f32 {
        self.layout(text).1
    }

    /// Width of the text's box (Pillow's `textbbox` right - left, which
    /// for these fonts is the advance width, rounded).
    pub fn text_width(&self, text: &str) -> i64 {
        self.text_length(text).round() as i64
    }

    /// Calls `plot(x, y, coverage)` for each pixel of the text drawn with
    /// its top-left corner at (x, y).
    pub fn rasterize(&self, x: f32, y: f32, text: &str, mut plot: impl FnMut(i64, i64, f32)) {
        let (glyphs, _) = self.layout(text);
        let scale = self.scale();
        let baseline = y + self.ascent();
        for (id, pen) in glyphs {
            let glyph = id.with_scale_and_position(scale, ab_glyph::point(x + pen, baseline));
            if let Some(outlined) = self.font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|gx, gy, coverage| {
                    plot(bounds.min.x as i64 + gx as i64, bounds.min.y as i64 + gy as i64, coverage);
                });
            }
        }
    }

    /// Draws `text` in `color` on `image`.
    pub fn draw(&self, image: &mut RgbImage, x: f32, y: f32, text: &str, color: [u8; 3]) {
        let (w, h) = (image.width() as i64, image.height() as i64);
        self.rasterize(x, y, text, |px, py, coverage| {
            if px < 0 || py < 0 || px >= w || py >= h || coverage <= 0.0 {
                return;
            }
            let a = coverage.min(1.0);
            let pixel = image.get_pixel_mut(px as u32, py as u32);
            for i in 0..3 {
                pixel[i] = (pixel[i] as f32 * (1.0 - a) + color[i] as f32 * a).round() as u8;
            }
        });
    }
}

/// `text` shortened with "…" to fit `max_width`, or "" if not even "…"
/// fits (the Linux version's `_fit_text_to_width`).
pub fn fit_text_to_width(text: &str, font: &SizedFont, max_width: i64) -> String {
    if max_width <= 0 {
        return String::new();
    }
    if font.text_width(text) <= max_width {
        return text.to_string();
    }
    const ELLIPSIS: &str = "…";
    if font.text_width(ELLIPSIS) > max_width {
        return String::new();
    }
    let mut truncated: Vec<char> = text.chars().collect();
    while !truncated.is_empty() {
        truncated.pop();
        let kept: String = truncated.iter().collect();
        let candidate = format!("{}{ELLIPSIS}", kept.trim_end());
        if font.text_width(&candidate) <= max_width {
            return candidate;
        }
    }
    ELLIPSIS.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_grow_with_text_and_size() {
        let font = SizedFont::new(26, false);
        let short = font.text_width("Planche");
        let long = font.text_width("Planche 1/3");
        assert!(short > 50 && long > short, "{short} {long}");
        assert!(SizedFont::new(52, false).text_width("Planche") > short * 19 / 10);
        // Values measured by Pillow (Linux version) for the same texts.
        assert_eq!(short, 102);
        assert_eq!(long, 152);
        assert_eq!(font.ascent(), 25.0);
        let title = SizedFont::new(54, true);
        assert_eq!((title.text_width("Mon titre"), title.ascent()), (281, 51.0));
        let caption = SizedFont::new(37, false);
        assert_eq!((caption.text_width("IMG_0001.jpg · ISO 400"), caption.ascent()), (434, 35.0));
    }

    #[test]
    fn truncation_with_ellipsis() {
        let font = SizedFont::new(20, false);
        let text = "a_very_long_file_name_that_does_not_fit.jpg";
        let fitted = fit_text_to_width(text, &font, 120);
        assert!(fitted.ends_with('…') && font.text_width(&fitted) <= 120, "{fitted}");
        assert_eq!(fit_text_to_width("ok", &font, 120), "ok");
        assert_eq!(fit_text_to_width(text, &font, 3), "");
    }

    #[test]
    fn drawing_changes_pixels() {
        let mut image = RgbImage::from_pixel(200, 50, image::Rgb([255, 255, 255]));
        SizedFont::new(26, true).draw(&mut image, 5.0, 5.0, "Test", [0, 0, 0]);
        let dark = image.pixels().filter(|p| p[0] < 128).count();
        assert!(dark > 100, "{dark}");
    }
}
