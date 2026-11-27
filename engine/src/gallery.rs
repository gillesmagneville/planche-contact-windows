//! The HTML gallery (the Linux version's htmlgallery.py): reduced copies
//! (2000 px) and thumbnails (400 px) of every photo, with the watermark,
//! paginated HTML pages with a viewer, and, as an option, the full-size
//! originals (a RAW converted to JPEG) with a companion script that lets a
//! gallery opened by double-click download them.
//!
//! The HTML, CSS and JavaScript of the pages are those of the Linux version
//! (assets/gallery/), with the values filled in here.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use image::RgbImage;
use rayon::prelude::*;
use serde_json::{json, Map, Value};

use crate::captions::resolve_caption_lines;
use crate::contactsheet::{save_jpeg, SheetSettings, AUTHOR_NAME};
use crate::imaging::{apply_watermark, load_full_resolution, load_image, thumbnail};
use crate::metadata::{exif_date, original_dimensions, photo_metadata};
use crate::raw::is_raw_file;
use crate::scanner::{file_name, Photo};
use crate::themes::{apply_custom_colors, get_palette, Palette};
use crate::t;

const GALLERY_FULL_MAX_SIZE: u32 = 2000;
const GALLERY_THUMB_SIZE: u32 = 400;
const GALLERY_ORIGINAL_JPEG_QUALITY: u8 = 95;
const DOWNLOAD_SCRIPT_CHUNK_BYTES: usize = 3 * 1024 * 1024;

static PAGE_HEAD: &str = include_str!("../assets/gallery/page_head.html");
static PAGE_TAIL: &str = include_str!("../assets/gallery/page_tail.html");

pub struct GallerySettings {
    pub sheet: SheetSettings,
    pub images_per_page: usize,
    pub include_originals: bool,
}

// ------------------------------------------------------------------ helpers

/// Python's `urllib.parse.quote` (letters, digits, `_.-~` and `/` kept).
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"_.-~/".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Python's `html.escape` (quotes included).
pub fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

/// Python's `json.dumps(value, ensure_ascii=False)`: ", " and ": " separators.
pub fn py_json(value: &Value) -> String {
    match value {
        Value::Array(items) => format!("[{}]", items.iter().map(py_json).collect::<Vec<_>>().join(", ")),
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| format!("{}: {}", serde_json::to_string(k).unwrap_or_default(), py_json(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// Human-readable size ("4,2 Mo", "4.2 MB"): unit and decimal separator
/// follow the language.
pub fn format_file_size(bytes: u64) -> String {
    let separator = if crate::i18n::language() != "en" { "," } else { "." };
    let mut size = bytes as f64;
    for key in ["unit.bytes", "unit.kb", "unit.mb", "unit.gb"] {
        if size < 1024.0 || key == "unit.gb" {
            let unit = t!(key);
            if key == "unit.bytes" {
                return format!("{} {unit}", size as u64);
            }
            return format!("{size:.1} {unit}").replace('.', separator);
        }
        size /= 1024.0;
    }
    unreachable!()
}

/// Name served in the gallery: a RAW becomes a JPEG.
pub fn display_filename(path: &Path) -> String {
    if is_raw_file(path) {
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        format!("{stem}.jpg")
    } else {
        file_name(path)
    }
}

/// A name unique in the gallery, case ignored ("IMG_1.jpg", "IMG_1_2.jpg"...).
pub fn unique_file_name(name: &str, used: &mut HashSet<String>) -> String {
    let path = Path::new(name);
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let suffix = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let mut candidate = name.to_string();
    let mut counter = 2;
    while used.contains(&candidate.to_lowercase()) {
        candidate = format!("{stem}_{counter}{suffix}");
        counter += 1;
    }
    used.insert(candidate.to_lowercase());
    candidate
}

fn css_vars(p: &Palette) -> String {
    format!(
        "--bg: {}; --text: {}; --text-secondary: {}; --accent: {}; --border: {};",
        p.background, p.text, p.text_secondary, p.accent, p.border
    )
}

fn color_scheme(mode: &str) -> &'static str {
    match mode {
        "dark" => "dark",
        "system" => "light dark",
        _ => "only light",
    }
}

fn header_align_attr(position: &str) -> String {
    match position {
        "top_left" => r#" style="text-align: left""#.into(),
        "top_right" => r#" style="text-align: right""#.into(),
        _ => String::new(),
    }
}

/// localStorage key of a visitor's theme choice, unique to this generation.
fn storage_key() -> String {
    use std::hash::{BuildHasher, RandomState};
    let value = RandomState::new().hash_one(std::time::SystemTime::now());
    format!("planche-contact-theme-{:012x}", value & 0xFFFF_FFFF_FFFF)
}

// ------------------------------------------------------------- the images

struct Task {
    path: PathBuf,
    thumb_filename: String,
    image_file_name: String,
    original_name: Option<String>,
}

/// Writes `originals/<name>.js`: the original in base64 in a
/// `pcOriginalLoaded(name, data)` call, which a page opened by double-click
/// can load (it cannot read a neighbouring file otherwise).
fn write_download_script(original: &Path, name: &str) -> bool {
    use base64::Engine;
    let script = original.with_file_name(format!("{name}.js"));
    let result = (|| -> std::io::Result<()> {
        let data = std::fs::read(original)?;
        let mut out = std::io::BufWriter::new(std::fs::File::create(&script)?);
        // Escaped to ASCII, as Python's json.dumps.
        let quoted: String = serde_json::to_string(name)
            .unwrap_or_default()
            .chars()
            .flat_map(|c| {
                if c.is_ascii() {
                    vec![c.to_string()]
                } else {
                    let mut units = [0u16; 2];
                    c.encode_utf16(&mut units).iter().map(|u| format!("\\u{u:04x}")).collect()
                }
            })
            .collect();
        write!(out, "window.pcOriginalLoaded({quoted},\"")?;
        for chunk in data.chunks(DOWNLOAD_SCRIPT_CHUNK_BYTES) {
            out.write_all(base64::engine::general_purpose::STANDARD.encode(chunk).as_bytes())?;
        }
        out.write_all(b"\");")?;
        out.flush()
    })();
    match result {
        Ok(()) => true,
        Err(e) => {
            crate::log::warning(&t!("engine.warn_html_download_file", name = name, error = e));
            let _ = std::fs::remove_file(&script);
            false
        }
    }
}

fn copy_with_date(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::copy(from, to)?;
    if let Ok(modified) = std::fs::metadata(from).and_then(|m| m.modified()) {
        if let Ok(file) = std::fs::File::options().write(true).open(to) {
            let _ = file.set_modified(modified);
        }
    }
    Ok(())
}

/// Makes the reduced copy and the thumbnail of one photo (one decoding),
/// and its original when asked. Returns the viewer's information, or None
/// when the photo could not be read.
fn process_image(task: &Task, settings: &GallerySettings, images_dir: &Path, thumbs_dir: &Path, originals_dir: &Path) -> Option<Map<String, Value>> {
    let path = &task.path;
    if !path.exists() {
        return None;
    }
    let visible_name = display_filename(path);
    let out_name = &task.image_file_name;
    let convert_raw_original = task.original_name.is_some() && is_raw_file(path);

    let result = (|| -> Result<Map<String, Value>, String> {
        let loaded: RgbImage = if convert_raw_original {
            load_full_resolution(path)?
        } else {
            load_image(path, Some((GALLERY_FULL_MAX_SIZE, GALLERY_FULL_MAX_SIZE)), false)?
        };
        let full_resolution = convert_raw_original.then(|| loaded.clone());
        let mut image = thumbnail(loaded, GALLERY_FULL_MAX_SIZE, GALLERY_FULL_MAX_SIZE);
        let sheet = &settings.sheet;
        if let Some(text) = sheet.watermark_text.as_deref().filter(|t| !t.is_empty()) {
            apply_watermark(&mut image, text, sheet.watermark_opacity, &sheet.watermark_orientation);
        }
        let thumb = thumbnail(image.clone(), GALLERY_THUMB_SIZE, GALLERY_THUMB_SIZE);
        save_jpeg(&image, &images_dir.join(out_name), 90)?;
        save_jpeg(&thumb, &thumbs_dir.join(&task.thumb_filename), 85)?;

        let dimensions = original_dimensions(path);
        let date = exif_date(path);
        let size = std::fs::metadata(path).ok().map(|m| m.len());
        let mut meta = Map::new();
        meta.insert("filename".into(), json!(visible_name));
        meta.insert("image_file".into(), json!(out_name));
        meta.insert("width".into(), json!(dimensions.map(|d| d.0)));
        meta.insert("height".into(), json!(dimensions.map(|d| d.1)));
        meta.insert("size".into(), json!(size.map(format_file_size)));
        meta.insert("date".into(), json!(date.map(|d| d.format("%d/%m/%Y %H:%M").to_string())));

        if let Some(original_name) = &task.original_name {
            let original_name = if is_raw_file(Path::new(original_name)) {
                display_filename(Path::new(original_name))
            } else {
                original_name.clone()
            };
            let target = originals_dir.join(&original_name);
            let written = match &full_resolution {
                Some(full) => save_jpeg(full, &target, GALLERY_ORIGINAL_JPEG_QUALITY),
                None => copy_with_date(path, &target).map_err(|e| e.to_string()),
            };
            match written {
                Ok(()) => {
                    meta.insert("original".into(), json!(format!("originals/{}", quote(&original_name))));
                    meta.insert("original_filename".into(), json!(visible_name));
                    write_download_script(&target, &original_name);
                }
                Err(error) => {
                    crate::log::warning(&t!("engine.warn_html_original", name = file_name(path), error = error));
                    let _ = std::fs::remove_file(&target);
                }
            }
        }
        Ok(meta)
    })();

    match result {
        Ok(meta) => Some(meta),
        Err(error) => {
            crate::log::warning(&t!("engine.warn_html_image", name = file_name(path), error = error));
            if !is_raw_file(path) {
                let _ = copy_with_date(path, &images_dir.join(out_name));
            }
            None
        }
    }
}

// ---------------------------------------------------------------- the pages

fn page_file(page: usize) -> String {
    if page == 1 { "index.html".into() } else { format!("page_{page:03}.html") }
}

fn nav_html(current: usize, total: usize) -> String {
    if total <= 1 {
        return String::new();
    }
    let mut nav = String::from(r#"<div class="nav">"#);
    if current > 1 {
        nav += &format!(r#"<a href="{}">{}</a>&nbsp;&nbsp;"#, page_file(current - 1), t!("gallery.nav_prev"));
    }
    let mut pages: Vec<Option<usize>> = Vec::new();
    if total <= 15 {
        pages.extend((1..=total).map(Some));
    } else {
        pages.push(Some(1));
        let start = current.saturating_sub(2).max(2);
        let end = (current + 2).min(total - 1);
        if start > 2 {
            pages.push(None);
        }
        pages.extend((start..=end).map(Some));
        if end < total - 1 {
            pages.push(None);
        }
        pages.push(Some(total));
    }
    for page in pages {
        match page {
            None => nav += r#" <span class="ellipsis">…</span> "#,
            Some(p) if p == current => nav += &format!(r#" <span class="current">{p}</span> "#),
            Some(p) => nav += &format!(r#" <a href="{}">{p}</a> "#, page_file(p)),
        }
    }
    if current < total {
        nav += &format!(r#"&nbsp;&nbsp;<a href="{}">{}</a>"#, page_file(current + 1), t!("gallery.nav_next"));
    }
    nav += "</div>";
    nav
}

struct PageContext<'a> {
    settings: &'a GallerySettings,
    palettes: [Palette; 3],
    storage_key: String,
    display_title: String,
    total_pages: usize,
    total_images: usize,
}

impl PageContext<'_> {
    fn theme_style_block(&self) -> String {
        let mode = self.settings.sheet.profile.theme.mode.as_str();
        let [light, dark, neutral] = &self.palettes;
        let baseline = match mode {
            "dark" => dark,
            "neutral" => neutral,
            _ => light,
        };
        let media = if mode == "system" {
            format!("\n        @media (prefers-color-scheme: dark) {{\n            :root {{ {} }}\n        }}", css_vars(dark))
        } else {
            String::new()
        };
        format!(
            "\n        :root {{ {} color-scheme: {}; }}{media}\n        :root[data-theme=\"light\"] {{ {} color-scheme: only light; }}\n        :root[data-theme=\"dark\"] {{ {} color-scheme: dark; }}\n        :root[data-theme=\"neutral\"] {{ {} color-scheme: only light; }}",
            css_vars(baseline), color_scheme(mode), css_vars(light), css_vars(dark), css_vars(neutral)
        )
    }

    fn render(&self, current: usize, photos: &[Photo], results: &[Option<Map<String, Value>>]) -> String {
        let sheet = &self.settings.sheet;
        let profile = &sheet.profile;
        let layout = &profile.layout;
        let mode = profile.theme.mode.as_str();

        let mut header = String::new();
        if layout.title_position != "none" {
            header += &format!("<h1{}>{}</h1>", header_align_attr(&layout.title_position), html_escape(&self.display_title));
        }
        if let Some(author) = sheet.author.as_deref().filter(|a| !a.is_empty() && layout.author_position != "none") {
            header += &format!("<p{}>{}</p>", header_align_attr(&layout.author_position), t!("gallery.by_author", author = html_escape(author)));
        }
        header += &format!("<p>{}</p>", t!("gallery.header_count", count = self.total_images, current = current, total = self.total_pages));
        let nav = nav_html(current, self.total_pages);

        let mut html = fill(PAGE_HEAD, &[
            ("lang", crate::i18n::language().to_string()),
            ("color_scheme", color_scheme(mode).to_string()),
            ("title", html_escape(&self.display_title)),
            ("storage_key", self.storage_key.clone()),
            ("theme_style", self.theme_style_block()),
            ("header_html", header),
            ("nav_html", nav.clone()),
        ]);

        let caption_fields = !profile.captions.effective_fields().is_empty();
        let mut image_urls = Vec::new();
        let mut gallery_meta = Vec::new();
        for (idx, (photo, result)) in photos.iter().zip(results).enumerate() {
            let Some(meta) = result else { continue };
            let filename = display_filename(&photo.path);
            let thumb_filename = format!("thumb_p{current}_{idx:04}.jpg");
            let position = image_urls.len();
            let image_file = meta.get("image_file").and_then(Value::as_str).unwrap_or(&filename);
            let image_url = format!("images/{}", quote(image_file));
            image_urls.push(json!(image_url));

            let unknown = t!("gallery.unknown");
            let text = |key: &str| meta.get(key).and_then(Value::as_str).map(str::to_string);
            let resolution = match (meta.get("width").and_then(Value::as_u64), meta.get("height").and_then(Value::as_u64)) {
                (Some(w), Some(h)) if w > 0 && h > 0 => format!("{w} × {h} px"),
                _ => unknown.clone(),
            };
            let mut entry = Map::new();
            let shown_name = text("filename").unwrap_or_else(|| filename.clone());
            entry.insert("filename".into(), json!(shown_name));
            entry.insert("resolution".into(), json!(resolution));
            entry.insert("size".into(), json!(text("size").unwrap_or_else(|| unknown.clone())));
            entry.insert("date".into(), json!(text("date").unwrap_or_else(|| unknown.clone())));
            if let Some(original) = text("original") {
                entry.insert("original".into(), json!(original));
                entry.insert("originalFilename".into(), json!(text("original_filename").unwrap_or(shown_name)));
            }
            gallery_meta.push(Value::Object(entry));

            let thumb_link = format!(
                " <a href=\"{image_url}\" onclick=\"return openLightbox(event, {position})\">\n        <img src=\"thumbs/{thumb_filename}\" alt=\"\">\n    </a>\n"
            );
            if caption_fields {
                let lines = resolve_caption_lines(profile, &photo_metadata(&photo.path));
                if !lines.is_empty() {
                    let caption = lines.iter().map(|l| html_escape(l)).collect::<Vec<_>>().join("<br>");
                    html += &format!(" <div class=\"gallery-item\">\n{thumb_link}    <div class=\"gallery-caption\">{caption}</div>\n </div>\n");
                    continue;
                }
            }
            html += &thumb_link;
        }

        let total = self.total_pages;
        let goto = if total > 1 {
            format!(
                r#"<div class="lightbox-goto"><label for="goto-page-input">{}</label><input type="number" id="goto-page-input" min="1" max="{total}" placeholder="1-{total}"><button onclick="goToPage()">{}</button></div>"#,
                t!("gallery.goto_page_label"), t!("gallery.goto_page_ok")
            )
        } else {
            String::new()
        };
        let label = html_escape(&t!("gallery.theme_toggle"));
        let toggle = format!(
            r#"<button type="button" class="theme-toggle" onclick="toggleTheme()" aria-label="{label}" title="{label}">&#9680;</button>"#
        );
        let dark_expr = match mode {
            "dark" => "true",
            "system" => "prefersDark",
            _ => "false",
        };
        let js = |key: &str| py_json(&json!(t!(key)));
        html += &fill(PAGE_TAIL, &[
            ("nav_html", nav),
            ("footer_credit", t!("gallery.footer_credit", name = AUTHOR_NAME)),
            ("theme_toggle", toggle),
            ("lightbox_page", t!("gallery.lightbox_page", current = current, total = total)),
            ("goto_page_html", goto),
            ("images_json", py_json(&Value::Array(image_urls))),
            ("meta_json", py_json(&Value::Array(gallery_meta))),
            ("download_label_js", js("gallery.download")),
            ("download_original_label_js", js("gallery.download_original")),
            ("total_pages", total.to_string()),
            ("current_page", current.to_string()),
            ("storage_key", self.storage_key.clone()),
            ("dark_expr", dark_expr.to_string()),
            ("download_toast_js", js("gallery.download_toast")),
            ("wrap_forward_js", js("gallery.wrap_forward")),
            ("wrap_backward_js", js("gallery.wrap_backward")),
        ]);
        html
    }
}

/// Replaces the `@@name@@` markers of a template (`@@t:key@@` is the text
/// of a translation key).
fn fill(template: &str, values: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len() + 4096);
    let mut rest = template;
    while let Some(start) = rest.find("@@") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find("@@") else {
            out.push_str(&rest[start..]);
            return out;
        };
        let name = &after[..end];
        if let Some(key) = name.strip_prefix("t:") {
            out.push_str(&t!(key));
        } else if let Some((_, value)) = values.iter().find(|(n, _)| *n == name) {
            out.push_str(value);
        } else {
            out.push_str(&rest[start..start + end + 4]);
        }
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

/// Creates the gallery in `output_dir` (emptied first).
pub fn create_gallery(settings: &GallerySettings, photos: &[Photo], output_dir: &Path) -> Result<(), String> {
    if output_dir.exists() {
        std::fs::remove_dir_all(output_dir).map_err(|e| e.to_string())?;
    }
    let thumbs_dir = output_dir.join("thumbs");
    let images_dir = output_dir.join("images");
    let originals_dir = output_dir.join("originals");
    for dir in [&thumbs_dir, &images_dir] {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    if settings.include_originals {
        std::fs::create_dir_all(&originals_dir).map_err(|e| e.to_string())?;
    }

    let per_page = settings.images_per_page.max(1);
    let sheet = &settings.sheet;
    let display_title = sheet
        .title
        .clone()
        .filter(|t| !t.is_empty())
        .or_else(|| Some(file_name(&sheet.input_dir)).filter(|n| !n.is_empty()))
        .unwrap_or_else(|| "Galerie Photo".into());
    let total_pages = photos.len().div_ceil(per_page);

    // Unique names are decided here, before the parallel work.
    let mut used_originals = HashSet::new();
    let mut used_images = HashSet::new();
    let tasks: Vec<Task> = photos
        .iter()
        .enumerate()
        .map(|(i, photo)| Task {
            path: photo.path.clone(),
            thumb_filename: format!("thumb_p{}_{:04}.jpg", i / per_page + 1, i % per_page),
            original_name: settings
                .include_originals
                .then(|| unique_file_name(&display_filename(&photo.path), &mut used_originals)),
            image_file_name: unique_file_name(&display_filename(&photo.path), &mut used_images),
        })
        .collect();
    let results: Vec<Option<Map<String, Value>>> = tasks
        .par_iter()
        .map(|task| process_image(task, settings, &images_dir, &thumbs_dir, &originals_dir))
        .collect();

    let theme = &sheet.profile.theme;
    let context = PageContext {
        settings,
        palettes: ["light", "dark", "neutral"].map(|mode| apply_custom_colors(get_palette(mode), &theme.custom)),
        storage_key: storage_key(),
        display_title,
        total_pages,
        total_images: photos.len(),
    };
    for page in 1..=total_pages {
        let start = (page - 1) * per_page;
        let end = (start + per_page).min(photos.len());
        let html = context.render(page, &photos[start..end], &results[start..end]);
        std::fs::write(output_dir.join(page_file(page)), html).map_err(|e| e.to_string())?;
    }
    crate::events::line(&t!("gallery.created_log", pages = total_pages, dir = output_dir.display()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::interface_default_profile;

    #[test]
    fn helpers_like_python() {
        assert_eq!(quote("a b#c/é.jpg"), "a%20b%23c/%C3%A9.jpg");
        assert_eq!(py_json(&json!([{"a": 1, "b": "é"}])), r#"[{"a": 1, "b": "é"}]"#);
        let mut used = HashSet::new();
        assert_eq!(unique_file_name("IMG_1.jpg", &mut used), "IMG_1.jpg");
        assert_eq!(unique_file_name("img_1.JPG", &mut used), "img_1_2.JPG");
        assert_eq!(display_filename(Path::new("/x/IMG_2.CR2")), "IMG_2.jpg");
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(4_404_019), "4.2 MB");
    }

    #[test]
    fn nav_with_gaps() {
        let nav = nav_html(10, 30);
        assert!(nav.contains(r#"<span class="current">10</span>"#));
        assert_eq!(nav.matches("ellipsis").count(), 2);
        assert!(nav.contains("page_030.html"));
        assert!(nav_html(1, 1).is_empty());
    }

    #[test]
    fn a_gallery_of_three_photos_on_two_pages() {
        let dir = std::env::temp_dir().join(format!("pc-gallery-{}", std::process::id()));
        let input = dir.join("Vacances");
        std::fs::create_dir_all(&input).unwrap();
        let mut photos = Vec::new();
        for i in 0..3 {
            let path = input.join(format!("p {i}.jpg"));
            RgbImage::from_pixel(800, 600, image::Rgb([10, 20, 30 * i as u8])).save(&path).unwrap();
            photos.push(Photo { filename: file_name(&path), num: format!("{:03}", i + 1), path });
        }
        let settings = GallerySettings {
            sheet: SheetSettings { input_dir: input.clone(), profile: interface_default_profile(), ..Default::default() },
            images_per_page: 2,
            include_originals: true,
        };
        let out = dir.join("gallery");
        create_gallery(&settings, &photos, &out).unwrap();
        let index = std::fs::read_to_string(out.join("index.html")).unwrap();
        assert!(index.contains("<title>Vacances</title>"));
        assert!(index.contains("images/p%200.jpg"));
        assert!(index.contains(r#""originalFilename": "p 1.jpg""#), "{}", &index[index.find("galleryMeta").unwrap()..][..400]);
        assert!(index.contains(r#"<div class="gallery-caption">p 0.jpg</div>"#));
        assert!(!index.contains("@@"));
        assert!(out.join("page_002.html").exists());
        assert!(out.join("thumbs/thumb_p2_0000.jpg").exists());
        assert!(out.join("originals/p 2.jpg.js").exists());
        let thumb = image::open(out.join("thumbs/thumb_p1_0001.jpg")).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (400, 300));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
