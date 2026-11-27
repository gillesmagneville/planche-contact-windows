//! The PDF: one sheet per page (the JPEG file itself, not re-encoded),
//! with the texts of the sheet laid over it as INVISIBLE text - the page
//! looks exactly like the sheet, but its title, captions and footer can be
//! selected, copied and searched (the Linux version's pdfexport.py and
//! textlayer.py).
//!
//! Written directly (the format is simple for this use): a sheet is placed
//! at 1 pixel = 1 point with a 12-point margin, as in the Linux version.
//! The texts use the DejaVu fonts by glyph number (Type 0 fonts, Identity-H
//! encoding) with a ToUnicode table, so that any character copies right.
//! The fonts are not embedded: the text is never drawn, only selected.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use ab_glyph::{Font, FontRef, GlyphId};

use crate::contactsheet::TextRun;
use crate::text::font_bytes;

const PDF_MARGIN: f64 = 12.0;

struct PdfFont {
    face: FontRef<'static>,
    upem: f32,
    /// Glyphs used, with the character each one stands for.
    used: BTreeMap<u16, char>,
}

impl PdfFont {
    fn new(bold: bool) -> Self {
        let face = FontRef::try_from_slice(font_bytes(bold)).expect("bundled font");
        let upem = face.units_per_em().unwrap_or(2048.0);
        PdfFont { face, upem, used: BTreeMap::new() }
    }

    /// Glyph numbers of `text` (hex for the content stream) and its natural
    /// width at `size`.
    fn encode(&mut self, text: &str, size: f32) -> (String, f32) {
        let mut hex = String::with_capacity(text.len() * 4);
        let mut width = 0.0;
        for c in text.chars() {
            let GlyphId(id) = self.face.glyph_id(c);
            self.used.entry(id).or_insert(c);
            width += self.face.h_advance_unscaled(GlyphId(id)) / self.upem * size;
            hex.push_str(&format!("{id:04X}"));
        }
        (hex, width)
    }

    fn width_1000(&self, id: u16) -> i64 {
        (self.face.h_advance_unscaled(GlyphId(id)) / self.upem * 1000.0).round() as i64
    }

    fn scaled(&self, value: f32) -> i64 {
        (value / self.upem * 1000.0).round() as i64
    }
}

/// Text of a PDF string literal.
fn literal(text: &str) -> String {
    let mut out = String::from("(");
    for c in text.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if c.is_ascii() && !c.is_ascii_control() => out.push(c),
            _ => out.push('?'),
        }
    }
    out.push(')');
    out
}

struct Writer {
    buffer: Vec<u8>,
    offsets: Vec<usize>,
}

impl Writer {
    fn new() -> Self {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        Writer { buffer, offsets: vec![0] }
    }

    /// Reserves an object number (written later with `object`).
    fn reserve(&mut self) -> usize {
        self.offsets.push(0);
        self.offsets.len() - 1
    }

    fn object(&mut self, id: usize, body: &str) {
        self.offsets[id] = self.buffer.len();
        let _ = write!(self.buffer, "{id} 0 obj\n{body}\nendobj\n");
    }

    fn stream(&mut self, id: usize, dictionary: &str, data: &[u8]) {
        self.offsets[id] = self.buffer.len();
        let _ = write!(self.buffer, "{id} 0 obj\n<< {dictionary} /Length {} >>\nstream\n", data.len());
        self.buffer.extend_from_slice(data);
        self.buffer.extend_from_slice(b"\nendstream\nendobj\n");
    }

    fn finish(mut self, root: usize, info: usize) -> Vec<u8> {
        let xref = self.buffer.len();
        let _ = write!(self.buffer, "xref\n0 {}\n0000000000 65535 f \n", self.offsets.len());
        for offset in &self.offsets[1..] {
            let _ = write!(self.buffer, "{offset:010} 00000 n \n");
        }
        let _ = write!(
            self.buffer,
            "trailer\n<< /Size {} /Root {root} 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            self.offsets.len()
        );
        self.buffer
    }
}

/// Width and height of a baseline or progressive JPEG, from its header.
fn jpeg_size(bytes: &[u8]) -> Option<(u32, u32, u8)> {
    let mut i = 2;
    while i + 9 < bytes.len() {
        if bytes[i] != 0xFF {
            return None;
        }
        let marker = bytes[i + 1];
        let length = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
            let height = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
            let width = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
            return Some((width, height, bytes[i + 9]));
        }
        i += 2 + length;
    }
    None
}

fn tounicode(used: &BTreeMap<u16, char>) -> String {
    let mut cmap = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    let entries: Vec<_> = used.iter().collect();
    for chunk in entries.chunks(100) {
        cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (id, c) in chunk {
            let mut units = [0u16; 2];
            let hex: String = c.encode_utf16(&mut units).iter().map(|u| format!("{u:04X}")).collect();
            cmap.push_str(&format!("<{id:04X}> <{hex}>\n"));
        }
        cmap.push_str("endbfchar\n");
    }
    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    cmap
}

fn write_font(w: &mut Writer, id: usize, font: &PdfFont, name: &str) {
    let (descriptor, cid, cmap) = (w.reserve(), w.reserve(), w.reserve());
    let widths: String = font.used.keys().map(|g| format!("{g} [{}]", font.width_1000(*g))).collect::<Vec<_>>().join(" ");
    w.object(id, &format!(
        "<< /Type /Font /Subtype /Type0 /BaseFont /{name} /Encoding /Identity-H \
         /DescendantFonts [{cid} 0 R] /ToUnicode {cmap} 0 R >>"
    ));
    w.object(cid, &format!(
        "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{name} \
         /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> \
         /FontDescriptor {descriptor} 0 R /CIDToGIDMap /Identity /DW 1000 /W [{widths}] >>"
    ));
    let ascent = font.scaled(font.face.ascent_unscaled());
    let descent = font.scaled(font.face.descent_unscaled());
    w.object(descriptor, &format!(
        "<< /Type /FontDescriptor /FontName /{name} /Flags 32 /FontBBox [-1021 -463 1793 1232] \
         /ItalicAngle 0 /Ascent {ascent} /Descent {descent} /CapHeight 729 /StemV 80 >>"
    ));
    w.stream(cmap, "", tounicode(&font.used).as_bytes());
}

/// Assembles `sheets` (JPEG files, sorted by name) into `output`.
/// `text_runs`: the texts of each sheet, laid over it as invisible text.
pub fn create_pdf(sheets: &[PathBuf], output: &Path, text_runs: &std::collections::HashMap<PathBuf, Vec<TextRun>>) -> Result<(), String> {
    if sheets.is_empty() {
        crate::log::warning(&crate::t!("engine.warn_pdf_no_sheets"));
        return Ok(());
    }
    let mut sorted = sheets.to_vec();
    sorted.sort();

    let mut w = Writer::new();
    let (catalog, pages, info) = (w.reserve(), w.reserve(), w.reserve());
    let (regular_id, bold_id) = (w.reserve(), w.reserve());
    let mut fonts = [PdfFont::new(false), PdfFont::new(true)];

    // The page size of the first sheet, for all of them (they are all alike).
    let mut page_size = None;
    let mut kids = Vec::new();
    for sheet in &sorted {
        let name = crate::scanner::file_name(sheet);
        let bytes = match std::fs::read(sheet) {
            Ok(bytes) => bytes,
            Err(e) => {
                crate::log::error(&crate::t!("engine.error_pdf_add_sheet", name = name, error = e));
                continue;
            }
        };
        let Some((img_w, img_h, components)) = jpeg_size(&bytes) else {
            crate::log::error(&crate::t!("engine.error_pdf_add_sheet", name = name, error = "not a JPEG file"));
            continue;
        };
        let (draw_w, draw_h) = *page_size.get_or_insert((img_w as f64, img_h as f64));
        let page_w = draw_w + 2.0 * PDF_MARGIN;
        let page_h = draw_h + 2.0 * PDF_MARGIN;

        let mut content = format!("q\n{draw_w} 0 0 {draw_h} {PDF_MARGIN} {PDF_MARGIN} cm\n/Im0 Do\nQ\n");
        if let Some(runs) = text_runs.get(sheet) {
            let runs: Vec<_> = runs.iter().filter(|r| !r.text.is_empty() && r.size > 0.0).collect();
            if !runs.is_empty() {
                content.push_str("BT\n3 Tr\n");
                for run in runs {
                    let font = &mut fonts[usize::from(run.bold)];
                    let (hex, natural) = font.encode(&run.text, run.size);
                    let scale = if natural > 0.0 && run.width > 0.0 { 100.0 * run.width / natural } else { 100.0 };
                    let x = PDF_MARGIN + run.x as f64;
                    let y = PDF_MARGIN + img_h as f64 - run.baseline as f64;
                    content.push_str(&format!(
                        "/F{} {:.2} Tf\n{:.3} Tz\n1 0 0 1 {:.2} {:.2} Tm\n<{hex}> Tj\n",
                        u8::from(run.bold), run.size, scale, x, y
                    ));
                }
                content.push_str("ET\n");
            }
        }

        let (page, contents, image) = (w.reserve(), w.reserve(), w.reserve());
        let colour = if components == 1 { "/DeviceGray" } else if components == 4 { "/DeviceCMYK" } else { "/DeviceRGB" };
        w.stream(image, &format!(
            "/Type /XObject /Subtype /Image /Width {img_w} /Height {img_h} /ColorSpace {colour} \
             /BitsPerComponent 8 /Filter /DCTDecode"
        ), &bytes);
        w.stream(contents, "", content.as_bytes());
        w.object(page, &format!(
            "<< /Type /Page /Parent {pages} 0 R /MediaBox [0 0 {page_w} {page_h}] /Contents {contents} 0 R \
             /Resources << /XObject << /Im0 {image} 0 R >> /Font << /F0 {regular_id} 0 R /F1 {bold_id} 0 R >> >> >>"
        ));
        kids.push(page);
    }

    let kids_text: String = kids.iter().map(|k| format!("{k} 0 R")).collect::<Vec<_>>().join(" ");
    w.object(pages, &format!("<< /Type /Pages /Kids [{kids_text}] /Count {} >>", kids.len()));
    w.object(catalog, &format!("<< /Type /Catalog /Pages {pages} 0 R >>"));
    w.object(info, &format!("<< /Title {} /Producer {} >>", literal("Portfolio"), literal("Planche-Contact")));
    write_font(&mut w, regular_id, &fonts[0], "DejaVuSans");
    write_font(&mut w, bold_id, &fonts[1], "DejaVuSans-Bold");

    std::fs::write(output, w.finish(catalog, info)).map_err(|e| e.to_string())?;
    crate::log::info(&crate::t!("engine.log_pdf_created", path = output.display()));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbImage;

    #[test]
    fn a_pdf_with_selectable_text() {
        let dir = std::env::temp_dir().join(format!("pc-pdf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sheet = dir.join("planche_001.jpg");
        crate::contactsheet::save_jpeg(&RgbImage::from_pixel(200, 100, image::Rgb([255, 255, 255])), &sheet, 95).unwrap();
        let runs = vec![TextRun { text: "Été (1)".into(), x: 10.0, baseline: 40.0, size: 20.0, width: 70.0, bold: true }];
        let mut map = std::collections::HashMap::new();
        map.insert(sheet.clone(), runs);
        let output = dir.join("portfolio.pdf");
        create_pdf(&[sheet], &output, &map).unwrap();
        let bytes = std::fs::read(&output).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("%PDF-1.4"));
        assert!(text.contains("/MediaBox [0 0 224 124]"), "1 px = 1 pt, plus the margins");
        assert!(text.contains("3 Tr"));
        assert!(text.contains("<00C9>"), "É in the ToUnicode table");
        assert!(text.contains("/Count 1"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
