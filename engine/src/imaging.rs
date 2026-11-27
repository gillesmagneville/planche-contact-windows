//! Loading of every supported image (RAW included) as an RGB image the
//! right way up, resizing, and the tiled watermark - the Linux version's
//! rawloader.py, thumbnail.py and utils.apply_watermark.

use std::io::BufReader;
use std::path::Path;

use image::{imageops, imageops::FilterType, RgbImage};

use crate::metadata::{orientation, read_exif, read_exif_bytes};
use crate::raw::{guarded, is_raw_file, RawFile, Thumbnail};
use crate::text::SizedFont;

/// Share of the shot's resolution an embedded RAW preview must cover to
/// stand for the full-size image.
pub const FULL_RESOLUTION_PREVIEW_MIN_RATIO: f64 = 0.98;

/// Applies an EXIF orientation (Pillow's `ImageOps.exif_transpose`).
pub fn apply_orientation(image: RgbImage, orientation: u32) -> RgbImage {
    match orientation {
        2 => imageops::flip_horizontal(&image),
        3 => imageops::rotate180(&image),
        4 => imageops::flip_vertical(&image),
        5 => imageops::flip_horizontal(&imageops::rotate90(&image)),
        6 => imageops::rotate90(&image),
        7 => imageops::flip_horizontal(&imageops::rotate270(&image)),
        8 => imageops::rotate270(&image),
        _ => image,
    }
}

fn is_jpeg(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF])
}

/// Decodes a JPEG, at a reduced scale (1/2, 1/4, 1/8) when `target` allows
/// it - Pillow's `draft()`: much faster than decoding everything and then
/// shrinking. The image stays at least as large as `target` on both sides.
fn decode_jpeg<R: std::io::Read>(reader: R, target: Option<(u32, u32)>) -> Result<RgbImage, String> {
    let mut decoder = jpeg_decoder::Decoder::new(reader);
    decoder.read_info().map_err(|e| e.to_string())?;
    if let Some((w, h)) = target {
        let w = w.clamp(1, u16::MAX as u32) as u16;
        let h = h.clamp(1, u16::MAX as u32) as u16;
        decoder.scale(w, h).map_err(|e| e.to_string())?;
    }
    let pixels = decoder.decode().map_err(|e| e.to_string())?;
    let info = decoder.info().ok_or("JPEG: no image information")?;
    let (w, h) = (info.width as u32, info.height as u32);
    let rgb: Vec<u8> = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => pixels,
        jpeg_decoder::PixelFormat::L8 => pixels.iter().flat_map(|&l| [l, l, l]).collect(),
        jpeg_decoder::PixelFormat::L16 => pixels
            .chunks_exact(2)
            .flat_map(|c| {
                let l = c[0];
                [l, l, l]
            })
            .collect(),
        jpeg_decoder::PixelFormat::CMYK32 => pixels
            .chunks_exact(4)
            .flat_map(|c| {
                let k = 255 - c[3] as u32;
                let channel = |v: u8| ((255 - v as u32) * k / 255) as u8;
                [channel(c[0]), channel(c[1]), channel(c[2])]
            })
            .collect(),
    };
    RgbImage::from_raw(w, h, rgb).ok_or_else(|| "JPEG: inconsistent size".to_string())
}

/// True if `size` covers about `tolerance` of the target's long side (its
/// short side when the image will be cropped to fill a square, `fill`).
fn large_enough(size: (u32, u32), target: (u32, u32), tolerance: f64, fill: bool) -> bool {
    let side = if fill { size.0.min(size.1) } else { size.0.max(size.1) };
    side as f64 >= target.0.max(target.1) as f64 * tolerance
}

/// Rotation of a RAW (LibRaw's flip code) as an EXIF orientation.
fn flip_to_orientation(flip: i32) -> u32 {
    match flip {
        3 => 3,
        5 => 8,
        6 => 6,
        _ => 1,
    }
}

fn load_raw(path: &Path, use_embedded: bool, target: Option<(u32, u32)>, fill: bool, min_ratio: f64) -> Result<RgbImage, String> {
    guarded(|| {
        let mut raw = RawFile::open(path)?;
        if use_embedded {
            let preview = match raw.thumbnail() {
                Some(Thumbnail::Jpeg(bytes)) if is_jpeg(&bytes) => {
                    let exif = read_exif_bytes(&bytes);
                    decode_jpeg(&bytes[..], target).ok().map(|image| (image, orientation(exif.as_ref())))
                }
                Some(Thumbnail::Jpeg(bytes)) => image::load_from_memory(&bytes).ok().map(|i| (i.to_rgb8(), 1)),
                Some(Thumbnail::Bitmap(image)) => Some((image, 1)),
                None => None,
            };
            if let Some((image, preview_orientation)) = preview {
                if target.map_or(true, |t| large_enough(image.dimensions(), t, min_ratio, fill)) {
                    // A preview without its own orientation is stored the way
                    // the sensor sees it: the camera's rotation is that of the RAW.
                    let orientation = if preview_orientation != 1 {
                        preview_orientation
                    } else {
                        flip_to_orientation(raw.flip())
                    };
                    return Ok(apply_orientation(image, orientation));
                }
            }
        }
        // No usable preview (or too small): full development, rotation included.
        raw.develop()
    })
}

/// Loads any supported image as RGB, the right way up. `target`: the
/// approximate size wanted, which allows a faster decoding (reduced JPEG
/// decoding, embedded RAW preview); `fill`: the image will be cropped to
/// fill a square of that size.
pub fn load_image(path: &Path, target: Option<(u32, u32)>, fill: bool) -> Result<RgbImage, String> {
    if is_raw_file(path) {
        return load_raw(path, true, target, fill, 0.9);
    }
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(file);
    let head = {
        use std::io::BufRead;
        reader.fill_buf().map_err(|e| e.to_string())?.to_vec()
    };
    let image = if is_jpeg(&head) {
        decode_jpeg(reader, target)?
    } else {
        image::ImageReader::new(reader)
            .with_guessed_format()
            .map_err(|e| e.to_string())?
            .decode()
            .map_err(|e| e.to_string())?
            .to_rgb8()
    };
    Ok(apply_orientation(image, orientation(read_exif(path).as_ref())))
}

/// The image at the full resolution of the shot (downloadable originals of
/// the gallery): for a RAW, the embedded preview only if it is (almost)
/// full size, otherwise a full development.
pub fn load_full_resolution(path: &Path) -> Result<RgbImage, String> {
    if !is_raw_file(path) {
        return load_image(path, None, false);
    }
    let dimensions = crate::metadata::original_dimensions(path);
    load_raw(path, dimensions.is_some(), dimensions, false, FULL_RESOLUTION_PREVIEW_MIN_RATIO)
}

/// Lanczos resize, preceded by a box reduction when shrinking a lot
/// (Pillow's `reducing_gap=2.0`): same look, much faster.
pub fn resize(image: &RgbImage, width: u32, height: u32) -> RgbImage {
    let (w, h) = image.dimensions();
    if (w, h) == (width, height) {
        return image.clone();
    }
    let factor = (w / width.max(1)).min(h / height.max(1)) / 2;
    if factor >= 2 {
        let reduced = box_reduce(image, factor);
        return imageops::resize(&reduced, width, height, FilterType::Lanczos3);
    }
    imageops::resize(image, width, height, FilterType::Lanczos3)
}

/// Averages `factor` x `factor` blocks (Pillow's `Image.reduce`).
fn box_reduce(image: &RgbImage, factor: u32) -> RgbImage {
    let (w, h) = image.dimensions();
    let (nw, nh) = (w.div_ceil(factor), h.div_ceil(factor));
    let mut out = RgbImage::new(nw, nh);
    for oy in 0..nh {
        for ox in 0..nw {
            let mut sum = [0u32; 3];
            let mut count = 0u32;
            for y in oy * factor..((oy + 1) * factor).min(h) {
                for x in ox * factor..((ox + 1) * factor).min(w) {
                    let p = image.get_pixel(x, y);
                    sum[0] += p[0] as u32;
                    sum[1] += p[1] as u32;
                    sum[2] += p[2] as u32;
                    count += 1;
                }
            }
            out.put_pixel(ox, oy, image::Rgb([
                ((sum[0] + count / 2) / count) as u8,
                ((sum[1] + count / 2) / count) as u8,
                ((sum[2] + count / 2) / count) as u8,
            ]));
        }
    }
    out
}

/// Pillow's `Image.thumbnail((max_w, max_h))`: shrinks to fit, keeping the
/// proportions, never enlarges.
pub fn thumbnail(image: RgbImage, max_w: u32, max_h: u32) -> RgbImage {
    let (w, h) = image.dimensions();
    if w <= max_w && h <= max_h {
        return image;
    }
    let scale = (max_w as f64 / w as f64).min(max_h as f64 / h as f64);
    let nw = ((w as f64 * scale).round() as u32).clamp(1, max_w);
    let nh = ((h as f64 * scale).round() as u32).clamp(1, max_h);
    resize(&image, nw, nh)
}

/// Shrinks `image` for a square cell of `max_size` pixels: "contain" keeps
/// the whole photo, "cover" crops a centred square. Never enlarges.
pub fn fit_thumbnail_image(image: RgbImage, max_size: u32, fit_mode: &str) -> RgbImage {
    if fit_mode == "cover" {
        let (w, h) = image.dimensions();
        let short_side = w.min(h);
        let image = if short_side > max_size {
            let scale = max_size as f64 / short_side as f64;
            let nw = ((w as f64 * scale).round() as u32).max(max_size);
            let nh = ((h as f64 * scale).round() as u32).max(max_size);
            resize(&image, nw, nh)
        } else {
            image
        };
        let (w, h) = image.dimensions();
        let side = w.min(h);
        let (left, top) = ((w - side) / 2, (h - side) / 2);
        return imageops::crop_imm(&image, left, top, side, side).to_image();
    }
    thumbnail(image, max_size, max_size)
}

/// A thumbnail for a sheet (fast loading, orientation, fit mode).
pub fn make_thumbnail(path: &Path, max_size: u32, fit_mode: &str) -> Result<RgbImage, String> {
    let image = load_image(path, Some((max_size, max_size)), fit_mode == "cover")?;
    Ok(fit_thumbnail_image(image, max_size, fit_mode))
}

// ---------------------------------------------------------------- watermark

/// Bicubic weight (a = -0.5, as Pillow).
fn cubic(x: f32) -> f32 {
    let x = x.abs();
    let a = -0.5;
    if x < 1.0 {
        (a + 2.0) * x * x * x - (a + 3.0) * x * x + 1.0
    } else if x < 2.0 {
        a * x * x * x - 5.0 * a * x * x + 8.0 * a * x - 4.0 * a
    } else {
        0.0
    }
}

/// Rotates a coverage mask by `degrees` (counter-clockwise for a positive
/// angle, as Pillow) about its centre, same size, bicubic.
fn rotate_mask(mask: &[f32], w: usize, h: usize, degrees: f32) -> Vec<f32> {
    let (sin, cos) = degrees.to_radians().sin_cos();
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let sample = |x: i64, y: i64| -> f32 {
        if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 { 0.0 } else { mask[y as usize * w + x as usize] }
    };
    let mut out = vec![0.0f32; w * h];
    for y in 0..h {
        for x in 0..w {
            // Inverse mapping: the source point that lands on (x, y).
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let sx = cos * dx - sin * dy + cx - 0.5;
            let sy = sin * dx + cos * dy + cy - 0.5;
            let (ix, iy) = (sx.floor() as i64, sy.floor() as i64);
            if ix < -2 || iy < -2 || ix > w as i64 + 1 || iy > h as i64 + 1 {
                continue;
            }
            let (fx, fy) = (sx - ix as f32, sy - iy as f32);
            let mut value = 0.0;
            for m in -1..=2 {
                let wy = cubic(m as f32 - fy);
                if wy == 0.0 {
                    continue;
                }
                for n in -1..=2 {
                    value += sample(ix + n, iy + m) * cubic(n as f32 - fx) * wy;
                }
            }
            out[y * w + x] = value.clamp(0.0, 1.0);
        }
    }
    out
}

/// Tiled, repeated "© text" watermark, white at `opacity` percent,
/// horizontal or diagonal - identical on the sheets, the PDF and the
/// gallery.
pub fn apply_watermark(image: &mut RgbImage, text: &str, opacity: i64, orientation: &str) {
    if text.is_empty() {
        return;
    }
    let text = if text.starts_with('©') { text.to_string() } else { format!("© {text}") };
    let font = SizedFont::new(14, true);
    let alpha = (255.0 * (opacity as f32 / 100.0)).clamp(0.0, 255.0).trunc() / 255.0;
    let angle: f32 = match orientation {
        "Diagonale horaire" => -32.0,
        "Diagonale anti-horaire" => 32.0,
        _ => 0.0,
    };
    let (w, h) = (image.width() as usize, image.height() as usize);
    let mut mask = vec![0.0f32; w * h];

    let text_width = font.text_width(&text);
    // Pillow's textbbox height for this font: from the top of the tallest
    // glyph to the bottom of the lowest one.
    let text_height = text_height(&font, &text);
    let x_spacing = (text_width + 220).max(1);
    let y_spacing = (text_height + 160).max(1);

    let mut y = -text_height;
    while y < h as i64 + text_height {
        let mut x = -text_width;
        while x < w as i64 + text_width {
            let offset_x = if angle != 0.0 { y.div_euclid(y_spacing) * 50 } else { 0 };
            font.rasterize((x + offset_x) as f32, y as f32, &text, |px, py, coverage| {
                if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
                    let cell = &mut mask[py as usize * w + px as usize];
                    *cell = (*cell + coverage * (1.0 - *cell)).min(1.0);
                }
            });
            x += x_spacing;
        }
        y += y_spacing;
    }
    if angle != 0.0 {
        mask = rotate_mask(&mask, w, h, angle);
    }
    for (pixel, coverage) in image.pixels_mut().zip(mask) {
        let a = coverage * alpha;
        if a > 0.0 {
            for channel in pixel.0.iter_mut() {
                *channel = (*channel as f32 * (1.0 - a) + 255.0 * a).round() as u8;
            }
        }
    }
}

/// Height of Pillow's textbbox for `text`: from the top of the tallest
/// glyph to the bottom of the lowest one.
pub fn text_height(font: &SizedFont, text: &str) -> i64 {
    let (mut top, mut bottom) = (i64::MAX, i64::MIN);
    font.rasterize(0.0, 0.0, text, |_, y, coverage| {
        if coverage > 0.0 {
            top = top.min(y);
            bottom = bottom.max(y + 1);
        }
    });
    if top > bottom { 0 } else { bottom - top }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_jpeg(path: &Path, w: u32, h: u32) {
        let image = RgbImage::from_fn(w, h, |x, y| image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]));
        image.save(path).unwrap();
    }

    #[test]
    fn jpeg_draft_and_thumbnail() {
        let dir = std::env::temp_dir().join(format!("pc-img-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("big.jpg");
        write_jpeg(&file, 1600, 1200);
        let loaded = load_image(&file, Some((300, 300)), false).unwrap();
        assert_eq!(loaded.dimensions(), (400, 300), "1/4 scale is the smallest at least 300 px");
        let thumb = make_thumbnail(&file, 300, "contain").unwrap();
        assert_eq!(thumb.dimensions(), (300, 225));
        let cover = make_thumbnail(&file, 300, "cover").unwrap();
        assert_eq!(cover.dimensions(), (300, 300));
        let small = make_thumbnail(&file, 5000, "contain").unwrap();
        assert_eq!(small.dimensions(), (1600, 1200), "never enlarged");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn watermark_text_box_like_pillow() {
        let font = SizedFont::new(14, true);
        // Pillow: 63 x 11 (its hinting may differ by a pixel).
        assert_eq!(font.text_width("© Gilles"), 63);
        assert!((text_height(&font, "© Gilles") - 11).abs() <= 1);
    }

    #[test]
    fn orientations() {
        let image = RgbImage::from_fn(3, 2, |x, y| image::Rgb([x as u8, y as u8, 0]));
        assert_eq!(apply_orientation(image.clone(), 6).dimensions(), (2, 3));
        let transposed = apply_orientation(image.clone(), 5);
        assert_eq!(transposed.get_pixel(1, 2).0, [2, 1, 0]);
        let transverse = apply_orientation(image, 7);
        assert_eq!(transverse.get_pixel(0, 0).0, [2, 1, 0]);
    }

    #[test]
    fn watermark_lightens_some_pixels() {
        for orientation in ["Horizontal", "Diagonale horaire", "Diagonale anti-horaire"] {
            let mut image = RgbImage::from_pixel(600, 400, image::Rgb([0, 0, 0]));
            apply_watermark(&mut image, "Gilles", 40, orientation);
            let lit = image.pixels().filter(|p| p[0] > 30).count();
            assert!(lit > 200, "{orientation}: {lit}");
            assert!(image.pixels().all(|p| p[0] <= 103), "{orientation}: 40% white at most");
        }
    }

    #[test]
    fn a_raw_loads_through_libraw() {
        let dir = std::env::temp_dir().join(format!("pc-img-raw-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("shot.dng");
        std::fs::write(&file, crate::raw::tests::synthetic_dng()).unwrap();
        assert_eq!(load_image(&file, Some((32, 32)), false).unwrap().dimensions(), (64, 48));
        assert_eq!(load_full_resolution(&file).unwrap().dimensions(), (64, 48));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
