//! Drawing of one contact sheet (the Linux version's contactsheet.py):
//! header, thumbnails with their watermark and captions, footer. Every
//! text drawn is also recorded (`TextRun`) for the PDF's selectable text.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::RgbImage;
use rayon::prelude::*;

use crate::captions::layout_caption_lines;
use crate::imaging::{apply_watermark, make_thumbnail};
use crate::layout::{
    compute_effective_per_page, compute_page_layout, pt_to_px, PageLayout, CAPTION_LINE_SPACING_PX,
    FOOTER_CREDIT_OFFSET_PX, FOOTER_LICENSE_OFFSET_PX, FOOTER_PAGE_NUMBER_OFFSET_PX,
};
use crate::metadata::photo_metadata;
use crate::profile::Profile;
use crate::scanner::Photo;
use crate::text::{fit_text_to_width, SizedFont};
use crate::themes::{resolve_theme, rgb, Palette};

/// Developer named in the credits of the sheets and of the gallery.
pub const AUTHOR_NAME: &str = "Gilles MAGNEVILLE";

/// Most caption lines the layout adds by itself when the wrapped text
/// needs more than the configured number.
const MAX_AUTO_CAPTION_LINES: i64 = 10;

/// Everything about a generation that the sheets need.
#[derive(Clone, Debug, Default)]
pub struct SheetSettings {
    pub input_dir: PathBuf,
    pub title: Option<String>,
    pub author: Option<String>,
    pub watermark_text: Option<String>,
    pub watermark_orientation: String,
    pub watermark_opacity: i64,
    pub profile: Profile,
}

/// A text drawn on a sheet, as the PDF must reproduce it (coordinates in
/// pixels from the top left; `baseline` is the ordinate of the baseline).
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    pub text: String,
    pub x: f32,
    pub baseline: f32,
    pub size: f32,
    pub width: f32,
    pub bold: bool,
}

pub struct Sheet {
    pub image: RgbImage,
    pub text_runs: Vec<TextRun>,
}

struct Canvas<'a> {
    image: RgbImage,
    runs: Vec<TextRun>,
    _settings: &'a SheetSettings,
}

impl Canvas<'_> {
    /// Draws a text and records it. `copy_text`: the text to copy from the
    /// PDF when it differs from the one shown (a line shortened with "…").
    fn put_text(&mut self, x: f32, y: f32, text: &str, color: &str, font: &SizedFont, copy_text: Option<&str>) {
        font.draw(&mut self.image, x, y, text, rgb(color));
        if text.is_empty() {
            return;
        }
        self.runs.push(TextRun {
            text: copy_text.unwrap_or(text).to_string(),
            x,
            baseline: y + font.ascent(),
            size: font.size,
            width: font.text_length(text),
            bold: font.bold,
        });
    }
}

/// Offset of a thumbnail inside its square cell, so that the photo sits
/// against its caption (or is centred when there is none).
fn thumb_offset(caption_position: &str, alignment: &str, thumb: (u32, u32), cell_size: i64) -> (i64, i64) {
    let free_w = (cell_size - thumb.0 as i64).max(0);
    let free_h = (cell_size - thumb.1 as i64).max(0);
    match caption_position {
        "right" => (free_w, 0),
        "below" | "above" => {
            let dy = if caption_position == "below" { free_h } else { 0 };
            let dx = match alignment {
                "left" => 0,
                "right" => free_w,
                _ => free_w / 2,
            };
            (dx, dy)
        }
        _ => (free_w / 2, free_h / 2),
    }
}

/// The largest font (down to `min_size`, by steps of 2) for which `text`
/// fits `max_width`; shortened with "…" if even that is too wide.
fn fit_header_text(text: &str, size: i64, max_width: i64, min_size: i64) -> (SizedFont, String) {
    let mut font = SizedFont::new(size, true);
    let mut candidate = size;
    while candidate >= min_size {
        font = SizedFont::new(candidate, true);
        if font.text_width(text) <= max_width {
            return (font, text.to_string());
        }
        candidate -= 2;
    }
    let fitted = fit_text_to_width(text, &font, max_width);
    (font, fitted)
}

fn header_x(position: &str, canvas_w: i64, margin_px: i64, text_w: i64) -> f32 {
    match position {
        "top_left" => margin_px as f32,
        "top_right" => (canvas_w - margin_px - text_w) as f32,
        _ => (canvas_w - text_w) as f32 / 2.0,
    }
}

fn caption_font(profile: &Profile) -> SizedFont {
    SizedFont::new(pt_to_px(profile.captions.font_size as f64).max(6), false)
}

/// Page geometry and the (wrapped) caption lines of each photo. With
/// captions below/above, the reserved height follows the text actually
/// needed, recomputed until stable.
fn plan_page(profile: &Profile, photos: &[Photo]) -> (PageLayout, HashMap<usize, Vec<String>>) {
    let fields = profile.captions.effective_fields();
    let position = profile.layout.caption_position.as_str();
    if fields.is_empty() || !matches!(position, "below" | "above" | "right") {
        return (compute_page_layout(profile, photos.len(), None), HashMap::new());
    }
    let per_page = compute_effective_per_page(profile).max(0) as usize;
    let metadata: Vec<_> = photos.par_iter().take(per_page).map(|p| photo_metadata(&p.path)).collect();
    let font = caption_font(profile);
    let measure = |text: &str| font.text_width(text);

    let mut n_lines = profile.captions.lines.max(1) as i64;
    let max_lines = n_lines.max(MAX_AUTO_CAPTION_LINES);
    loop {
        let page = compute_page_layout(profile, photos.len(), Some(n_lines));
        let mut texts = HashMap::new();
        let mut needed = 1;
        for (idx, cell) in page.cells.iter().enumerate() {
            let (Some(m), Some(width)) = (metadata.get(idx), cell.caption_w) else { continue };
            let lines = layout_caption_lines(profile, m, width, &measure);
            if !lines.is_empty() {
                needed = needed.max(lines.len() as i64);
                texts.insert(idx, lines);
            }
        }
        if position == "right" || needed <= n_lines || n_lines >= max_lines {
            return (page, texts);
        }
        n_lines = needed.min(max_lines);
    }
}

fn draw_header(canvas: &mut Canvas, settings: &SheetSettings, canvas_w: i64, margin_px: i64, palette: &Palette) -> i64 {
    let layout = &settings.profile.layout;
    let max_width = canvas_w - 2 * margin_px;
    let mut y = (6.0 * 300.0 / 25.4) as i64;

    let has = |v: &Option<String>| v.as_deref().is_some_and(|s| !s.is_empty());
    let mut display_title = settings.title.clone().filter(|t| !t.is_empty());
    if display_title.is_none() && !has(&settings.author) && !has(&settings.watermark_text) {
        let name = crate::scanner::file_name(&settings.input_dir);
        if !name.is_empty() {
            display_title = Some(name);
        }
    }

    if let Some(title) = display_title.filter(|_| layout.title_position != "none") {
        let (font, text) = fit_header_text(&title, 54, max_width, 28);
        let x = header_x(&layout.title_position, canvas_w, margin_px, font.text_width(&text));
        let copy = (text != title).then_some(title.as_str());
        canvas.put_text(x, y as f32, &text, &palette.text, &font, copy);
        y += 58;
    }
    if let Some(author) = settings.author.as_deref().filter(|a| !a.is_empty() && layout.author_position != "none") {
        let (font, text) = fit_header_text(author, 36, max_width, 20);
        let x = header_x(&layout.author_position, canvas_w, margin_px, font.text_width(&text));
        let copy = (text != author).then_some(author);
        canvas.put_text(x, y as f32, &text, &palette.text, &font, copy);
        y += 42;
    }
    y
}

#[allow(clippy::too_many_arguments)]
fn draw_caption(canvas: &mut Canvas, lines: &[String], x: i64, y: i64, width: i64, height: i64, profile: &Profile, palette: &Palette) {
    let font = caption_font(profile);
    let line_height = font.size as i64 + CAPTION_LINE_SPACING_PX;
    let max_lines = (height / line_height).max(1) as usize;
    let mut shown: Vec<String> = lines.to_vec();
    let mut full: Vec<String> = lines.to_vec();
    if lines.len() > max_lines {
        full = lines[..max_lines - 1].to_vec();
        full.push(lines[max_lines - 1..].join(" "));
        shown = lines[..max_lines].to_vec();
        if let Some(last) = shown.last_mut() {
            last.push('…');
        }
    }
    for (i, line) in shown.iter().enumerate() {
        let line = fit_text_to_width(line, &font, width);
        let text_w = font.text_width(&line);
        let text_x = match profile.captions.alignment.as_str() {
            "left" => x as f32,
            "right" => (x + width - text_w) as f32,
            _ => x as f32 + (width - text_w) as f32 / 2.0,
        };
        if !line.is_empty() {
            let copy = (full[i] != line).then_some(full[i].as_str());
            canvas.put_text(text_x, (y + i as i64 * line_height) as f32, &line, &palette.text, &font, copy);
        }
    }
}

fn draw_footer(canvas: &mut Canvas, canvas_w: i64, canvas_h: i64, page_num: usize, total_pages: usize, palette: &Palette, show_page_number: bool) {
    let font = SizedFont::new(26, false);
    let small = SizedFont::new(20, false);
    let mut lines: Vec<(String, i64, &SizedFont, &str)> = Vec::new();
    if show_page_number {
        lines.push((crate::t!("sheet.page_label", page = page_num, total = total_pages),
                    FOOTER_PAGE_NUMBER_OFFSET_PX, &font, &palette.text));
    }
    lines.push((crate::t!("sheet.footer"), FOOTER_CREDIT_OFFSET_PX, &small, &palette.text_secondary));
    lines.push((crate::t!("sheet.footer_credit", name = AUTHOR_NAME), FOOTER_LICENSE_OFFSET_PX, &small, &palette.text_secondary));
    for (text, offset, line_font, color) in lines {
        let x = (canvas_w - line_font.text_width(&text)) as f32 / 2.0;
        canvas.put_text(x, (canvas_h - offset) as f32, &text, color, line_font, None);
    }
}

/// Draws sheet `page_num` of `total_pages` with `photos` (those of this
/// page). None when there is no photo.
pub fn create_contact_sheet(settings: &SheetSettings, photos: &[Photo], page_num: usize, total_pages: usize) -> Option<Sheet> {
    if photos.is_empty() {
        return None;
    }
    let profile = &settings.profile;
    let palette = resolve_theme(&profile.theme);
    let (page, caption_lines) = plan_page(profile, photos);
    let thumb_size = page.thumb_size;
    let effective = page.effective_per_page.max(0) as usize;

    let thumbnails: Vec<Option<RgbImage>> = photos
        .par_iter()
        .take(effective.min(page.cells.len()))
        .map(|photo| match make_thumbnail(&photo.path, thumb_size.max(1) as u32, &profile.layout.fit_mode) {
            Ok(mut thumb) => {
                if let Some(text) = settings.watermark_text.as_deref().filter(|t| !t.is_empty()) {
                    apply_watermark(&mut thumb, text, settings.watermark_opacity, &settings.watermark_orientation);
                }
                Some(thumb)
            }
            Err(error) => {
                crate::log::warning(&crate::t!("engine.warn_thumbnail_failed", name = photo.filename, error = error));
                None
            }
        })
        .collect();

    let background = image::Rgb(rgb(&palette.background));
    let mut canvas = Canvas {
        image: RgbImage::from_pixel(page.canvas_w as u32, page.canvas_h as u32, background),
        runs: Vec::new(),
        _settings: settings,
    };

    let header_bottom = draw_header(&mut canvas, settings, page.canvas_w, page.margin_px, &palette);
    let y_start = header_bottom + page.margin_px;
    let free_h = page.content_bottom_px - y_start;
    let y_shift = y_start - page.content_top_px + (free_h - page.grid_h).max(0) / 2;

    let drawn_position = if profile.captions.effective_fields().is_empty() {
        "none"
    } else {
        profile.layout.caption_position.as_str()
    };

    for (idx, thumb) in thumbnails.into_iter().enumerate() {
        let Some(thumb) = thumb else { continue };
        let cell = &page.cells[idx];
        let (dx, dy) = thumb_offset(drawn_position, &profile.captions.alignment, thumb.dimensions(), thumb_size);
        image::imageops::replace(&mut canvas.image, &thumb, cell.photo_x + dx, cell.photo_y + y_shift + dy);
        if let (Some(lines), Some(cx), Some(cy), Some(cw), Some(ch)) =
            (caption_lines.get(&idx), cell.caption_x, cell.caption_y, cell.caption_w, cell.caption_h)
        {
            draw_caption(&mut canvas, lines, cx, cy + y_shift, cw, ch, profile, &palette);
        }
    }

    draw_footer(&mut canvas, page.canvas_w, page.canvas_h, page_num, total_pages, &palette, profile.layout.page_numbering);
    Some(Sheet { image: canvas.image, text_runs: canvas.runs })
}

/// Saves a sheet as JPEG (quality 95, as the Linux version).
pub fn save_jpeg(image: &RgbImage, path: &Path, quality: u8) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut writer = std::io::BufWriter::new(file);
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, quality);
    image.write_with_encoder(encoder).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::interface_default_profile;

    #[test]
    fn offsets_follow_the_caption() {
        assert_eq!(thumb_offset("below", "center", (100, 60), 100), (0, 40));
        assert_eq!(thumb_offset("above", "right", (60, 100), 100), (40, 0));
        assert_eq!(thumb_offset("right", "center", (60, 100), 100), (40, 0));
        assert_eq!(thumb_offset("none", "center", (60, 100), 100), (20, 0));
    }

    #[test]
    fn a_sheet_with_captions_and_watermark() {
        let dir = std::env::temp_dir().join(format!("pc-sheet-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut photos = Vec::new();
        for i in 0..5 {
            let path = dir.join(format!("photo_{i}.jpg"));
            RgbImage::from_pixel(300, 200, image::Rgb([200, 40 * i as u8, 10])).save(&path).unwrap();
            photos.push(Photo { filename: crate::scanner::file_name(&path), num: format!("{:03}", i + 1), path });
        }
        let settings = SheetSettings {
            input_dir: dir.clone(),
            title: Some("Essai".into()),
            watermark_text: Some("Test".into()),
            watermark_orientation: "Horizontal".into(),
            watermark_opacity: 40,
            profile: interface_default_profile(),
            ..Default::default()
        };
        let sheet = create_contact_sheet(&settings, &photos, 1, 2).unwrap();
        assert_eq!(sheet.image.dimensions(), (2480, 3507));
        let texts: Vec<&str> = sheet.text_runs.iter().map(|r| r.text.as_str()).collect();
        assert_eq!(texts[0], "Essai");
        assert!(texts.contains(&"photo_3.jpg"), "{texts:?}");
        assert!(texts.iter().any(|t| t.contains(AUTHOR_NAME)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
