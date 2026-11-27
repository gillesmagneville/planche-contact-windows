//! What a photo says about itself: shooting date (sorting, gallery
//! viewer), EXIF orientation, real size, and the caption values (the Linux
//! version's get_exif_date / get_exif_metadata / get_original_dimensions).
//! Metadata-only reads, never a full decode; never an error - a missing
//! value is simply absent.

use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek};
use std::path::Path;

use chrono::{NaiveDateTime, TimeZone};
use exif::{Exif, In, Tag, Value};

use crate::captions::Metadata;
use crate::raw::{is_raw_file, RawFile};

fn read_exif_from<R: std::io::BufRead + Seek>(reader: &mut R) -> Option<Exif> {
    exif::Reader::new().read_from_container(reader).ok()
}

pub fn read_exif(path: &Path) -> Option<Exif> {
    let file = File::open(path).ok()?;
    read_exif_from(&mut BufReader::new(file))
}

pub fn read_exif_bytes(bytes: &[u8]) -> Option<Exif> {
    read_exif_from(&mut Cursor::new(bytes))
}

fn ascii(exif: &Exif, tag: Tag) -> Option<String> {
    match &exif.get_field(tag, In::PRIMARY)?.value {
        Value::Ascii(parts) => {
            let bytes = parts.first()?;
            let text = String::from_utf8_lossy(bytes);
            let text = text.trim_matches(|c: char| c == '\0' || c.is_whitespace()).to_string();
            (!text.is_empty()).then_some(text)
        }
        _ => None,
    }
}

fn rational(value: &Value, index: usize) -> Option<f64> {
    let number = match value {
        Value::Rational(v) => v.get(index).filter(|r| r.denom != 0).map(|r| r.num as f64 / r.denom as f64),
        Value::SRational(v) => v.get(index).filter(|r| r.denom != 0).map(|r| r.num as f64 / r.denom as f64),
        Value::Float(v) => v.get(index).map(|&f| f as f64),
        Value::Double(v) => v.get(index).copied(),
        other => other.get_uint(index).map(f64::from),
    }?;
    number.is_finite().then_some(number)
}

fn number(exif: &Exif, tag: Tag) -> Option<f64> {
    rational(&exif.get_field(tag, In::PRIMARY)?.value, 0)
}

fn integer(exif: &Exif, tag: Tag) -> Option<i64> {
    exif.get_field(tag, In::PRIMARY)?.value.get_uint(0).map(i64::from)
}

pub fn parse_exif_date(text: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(text.trim_matches(|c: char| c == '\0' || c.is_whitespace()), "%Y:%m:%d %H:%M:%S").ok()
}

/// Shooting date: DateTimeOriginal, DateTimeDigitized, then DateTime.
fn date_from_exif(exif: &Exif) -> Option<NaiveDateTime> {
    [Tag::DateTimeOriginal, Tag::DateTimeDigitized, Tag::DateTime]
        .into_iter()
        .find_map(|tag| ascii(exif, tag).as_deref().and_then(parse_exif_date))
}

fn local_date(timestamp: i64) -> Option<NaiveDateTime> {
    chrono::Local.timestamp_opt(timestamp, 0).single().map(|d| d.naive_local())
}

/// Most relevant EXIF date of a photo (RAW included), or None.
pub fn exif_date(path: &Path) -> Option<NaiveDateTime> {
    if let Some(date) = read_exif(path).as_ref().and_then(date_from_exif) {
        return Some(date);
    }
    if is_raw_file(path) {
        // A RAW the EXIF reader does not understand (CR3): LibRaw's date.
        let raw = crate::raw::guarded(|| RawFile::open(path)).ok()?;
        return raw.info().timestamp.and_then(local_date);
    }
    None
}

/// EXIF orientation (1 to 8), 1 when absent.
pub fn orientation(exif: Option<&Exif>) -> u32 {
    exif.and_then(|e| e.get_field(Tag::Orientation, In::PRIMARY))
        .and_then(|f| f.value.get_uint(0))
        .filter(|o| (1..=8).contains(o))
        .unwrap_or(1)
}

/// Size of the image file as seen by the user (orientation applied), from
/// its header only.
pub fn original_dimensions(path: &Path) -> Option<(u32, u32)> {
    if is_raw_file(path) {
        return crate::raw::guarded(|| RawFile::open(path)).ok().map(|raw| raw.dimensions());
    }
    let (w, h) = image::ImageReader::open(path).ok()?.with_guessed_format().ok()?.into_dimensions().ok()?;
    if matches!(orientation(read_exif(path).as_ref()), 5..=8) {
        Some((h, w))
    } else {
        Some((w, h))
    }
}

/// Make + model, without repeating the make when the model already starts
/// with it ("NIKON CORPORATION" + "NIKON D850" gives "NIKON D850").
pub fn combine_camera(make: Option<&str>, model: Option<&str>) -> Option<String> {
    let make = make.map(str::trim).filter(|s| !s.is_empty());
    let model = model.map(str::trim).filter(|s| !s.is_empty());
    match (make, model) {
        (Some(make), Some(model)) => {
            let first = make.split_whitespace().next().unwrap_or(make).to_lowercase();
            if model.to_lowercase().starts_with(&first) {
                Some(model.to_string())
            } else {
                Some(format!("{make} {model}"))
            }
        }
        (make, model) => make.or(model).map(str::to_string),
    }
}

fn dms(exif: &Exif, tag: Tag, reference: Tag, negative: char) -> Option<f64> {
    let value = &exif.get_field(tag, In::PRIMARY)?.value;
    let decimal = rational(value, 0)? + rational(value, 1)? / 60.0 + rational(value, 2)? / 3600.0;
    let is_negative = ascii(exif, reference)
        .map(|r| r.to_uppercase().starts_with(negative))
        .unwrap_or(false);
    Some(if is_negative { -decimal } else { decimal })
}

fn fill_from_exif(exif: &Exif, m: &mut Metadata) {
    m.camera = combine_camera(ascii(exif, Tag::Make).as_deref(), ascii(exif, Tag::Model).as_deref());
    m.lens = ascii(exif, Tag::LensModel);
    m.focal_length = number(exif, Tag::FocalLength);
    m.focal_35mm = integer(exif, Tag::FocalLengthIn35mmFilm);
    m.aperture = number(exif, Tag::FNumber);
    m.shutter_speed = number(exif, Tag::ExposureTime);
    m.iso = integer(exif, Tag::PhotographicSensitivity);
    if let (Some(lat), Some(lon)) = (
        dms(exif, Tag::GPSLatitude, Tag::GPSLatitudeRef, 'S'),
        dms(exif, Tag::GPSLongitude, Tag::GPSLongitudeRef, 'W'),
    ) {
        m.gps = Some((lat, lon));
    }
}

fn fill_from_libraw(raw: &RawFile, m: &mut Metadata) {
    let info = raw.info();
    m.camera = combine_camera(info.make.as_deref(), info.model.as_deref());
    m.lens = info.lens;
    m.focal_length = info.focal_length;
    m.aperture = info.aperture;
    m.shutter_speed = info.shutter;
    m.iso = info.iso.map(|i| i.round() as i64);
    if m.datetime.is_none() {
        m.datetime = info.timestamp.and_then(local_date);
    }
}

/// Everything the captions can show about one photo.
pub fn photo_metadata(path: &Path) -> Metadata {
    let mut m = Metadata {
        filename: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        ..Default::default()
    };
    let exif = read_exif(path);
    m.datetime = exif.as_ref().and_then(date_from_exif);
    match &exif {
        Some(exif) => fill_from_exif(exif, &mut m),
        None if is_raw_file(path) => {
            if let Ok(raw) = crate::raw::guarded(|| RawFile::open(path)) {
                fill_from_libraw(&raw, &mut m);
            }
        }
        None => {}
    }
    m.dimensions = original_dimensions(path);
    m
}

/// Bytes of the start of a file (used to sniff formats).
#[allow(dead_code)]
pub fn head(path: &Path, n: usize) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(n);
    if let Ok(file) = File::open(path) {
        let _ = file.take(n as u64).read_to_end(&mut buffer);
    }
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_names() {
        assert_eq!(combine_camera(Some("NIKON CORPORATION"), Some("NIKON D850")).unwrap(), "NIKON D850");
        assert_eq!(combine_camera(Some("Canon"), Some("EOS R5")).unwrap(), "Canon EOS R5");
        assert_eq!(combine_camera(None, Some(" X100V ")).unwrap(), "X100V");
        assert_eq!(combine_camera(None, None), None);
    }

    #[test]
    fn dates() {
        let date = parse_exif_date("2023:07:14 18:30:05\0").unwrap();
        assert_eq!(date.format("%d/%m/%Y %H:%M").to_string(), "14/07/2023 18:30");
        assert!(parse_exif_date("2023-07-14").is_none());
    }

    #[test]
    fn a_dng_has_dimensions_and_camera() {
        let dir = std::env::temp_dir().join(format!("pc-meta-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("shot.dng");
        std::fs::write(&file, crate::raw::tests::synthetic_dng()).unwrap();
        let m = photo_metadata(&file);
        assert_eq!(m.filename, "shot.dng");
        assert_eq!(m.dimensions, Some((64, 48)));
        assert_eq!(m.camera.as_deref(), Some("FakeCam Synthetic DNG"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
