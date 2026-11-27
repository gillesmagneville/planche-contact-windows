//! RAW files through LibRaw (statically embedded in `vendor/libraw-sys`, no
//! system library needed) - the library the Linux version uses too, through
//! rawpy, so a RAW gives the same image on both systems.
//!
//! What is used: the size of the shot, the preview the camera embedded in
//! the file (almost instant, enough for thumbnails and sheets), the full
//! development when that preview is missing or too small, and the shooting
//! information (for the captions of files the EXIF reader does not know).

use std::ffi::CStr;
use std::os::raw::{c_char, c_int};
use std::path::Path;

use image::RgbImage;
use rsraw_sys as ffi;

pub const RAW_EXTENSIONS: [&str; 5] = ["cr2", "cr3", "nef", "dng", "arw"];

pub fn is_raw_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| RAW_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// The preview embedded by the camera.
pub enum Thumbnail {
    Jpeg(Vec<u8>),
    Bitmap(RgbImage),
}

/// Shooting information read by LibRaw.
#[derive(Debug, Default)]
pub struct RawInfo {
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens: Option<String>,
    pub iso: Option<f64>,
    pub shutter: Option<f64>,
    pub aperture: Option<f64>,
    pub focal_length: Option<f64>,
    /// Seconds since 1970, as LibRaw computed it from the local date.
    pub timestamp: Option<i64>,
}

struct Handle(*mut ffi::libraw_data_t);

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the pointer comes from `libraw_init` and is released only here.
        unsafe { ffi::libraw_close(self.0) }
    }
}

struct Processed(*mut ffi::libraw_processed_image_t);

impl Drop for Processed {
    fn drop(&mut self) {
        // SAFETY: the pointer comes from `libraw_dcraw_make_mem_image/thumb`.
        unsafe { ffi::libraw_dcraw_clear_mem(self.0) }
    }
}

fn libraw_error(code: c_int) -> String {
    // SAFETY: `libraw_strerror` returns a pointer to a static string.
    let text = unsafe { CStr::from_ptr(ffi::libraw_strerror(code)) };
    format!("LibRaw: {}", text.to_string_lossy())
}

fn check(code: c_int) -> Result<(), String> {
    if code == 0 { Ok(()) } else { Err(libraw_error(code)) }
}

/// What LibRaw reads the file from: on Unix its path; on Windows the bytes
/// of the file, read by Rust, because LibRaw's `char *` API only knows the
/// legacy code page (a path with characters outside it would fail).
#[cfg(unix)]
struct Source(std::ffi::CString);

#[cfg(not(unix))]
struct Source(Vec<u8>);

impl Source {
    #[cfg(unix)]
    fn new(path: &Path) -> Result<Self, String> {
        use std::os::unix::ffi::OsStrExt;
        std::ffi::CString::new(path.as_os_str().as_bytes())
            .map(Source)
            .map_err(|_| "invalid path (null byte)".to_string())
    }

    #[cfg(not(unix))]
    fn new(path: &Path) -> Result<Self, String> {
        std::fs::read(path).map(Source).map_err(|e| e.to_string())
    }

    /// SAFETY: `handle` must be a live LibRaw instance, dropped before `self`.
    #[cfg(unix)]
    unsafe fn open_in(&self, handle: *mut ffi::libraw_data_t) -> c_int {
        ffi::libraw_open_file(handle, self.0.as_ptr())
    }

    #[cfg(not(unix))]
    unsafe fn open_in(&self, handle: *mut ffi::libraw_data_t) -> c_int {
        ffi::libraw_open_buffer(handle, self.0.as_ptr().cast(), self.0.len())
    }
}

/// An opened RAW file. The source is declared after the handle, hence
/// dropped after it: LibRaw reads from it until it is closed.
pub struct RawFile {
    handle: Handle,
    _source: Source,
}

// SAFETY: a RawFile is used by one thread at a time (it is never shared).
unsafe impl Send for RawFile {}

fn c_text(bytes: &[c_char]) -> Option<String> {
    let bytes: Vec<u8> = bytes.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
    let text = String::from_utf8_lossy(&bytes).trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn positive(value: f32) -> Option<f64> {
    (value.is_finite() && value > 0.0).then_some(value as f64)
}

impl RawFile {
    pub fn open(path: &Path) -> Result<RawFile, String> {
        let source = Source::new(path)?;
        // SAFETY: a LibRaw instance private to this value, released by Handle.
        unsafe {
            let raw = ffi::libraw_init(0);
            if raw.is_null() {
                return Err("LibRaw: initialization failed".into());
            }
            let handle = Handle(raw);
            check(source.open_in(handle.0))?;
            Ok(RawFile { handle, _source: source })
        }
    }

    fn data(&self) -> &ffi::libraw_data_t {
        // SAFETY: the instance lives as long as self.
        unsafe { &*self.handle.0 }
    }

    /// Size of the developed image, with the camera's rotation applied.
    pub fn dimensions(&self) -> (u32, u32) {
        let sizes = &self.data().sizes;
        let (w, h) = (sizes.width as u32, sizes.height as u32);
        if matches!(sizes.flip, 5 | 6) { (h, w) } else { (w, h) }
    }

    /// LibRaw's rotation code (0 none, 3 = 180°, 5/6 = 90°).
    pub fn flip(&self) -> i32 {
        self.data().sizes.flip
    }

    pub fn info(&self) -> RawInfo {
        let d = self.data();
        RawInfo {
            make: c_text(&d.idata.make),
            model: c_text(&d.idata.model),
            lens: c_text(&d.lens.Lens),
            iso: positive(d.other.iso_speed),
            shutter: positive(d.other.shutter),
            aperture: positive(d.other.aperture),
            focal_length: positive(d.other.focal_len),
            timestamp: (d.other.timestamp > 0).then_some(d.other.timestamp as i64),
        }
    }

    /// The embedded preview, or None when the file has none usable.
    pub fn thumbnail(&mut self) -> Option<Thumbnail> {
        // SAFETY: calls on a live instance; the memory image is released by Processed.
        unsafe {
            if ffi::libraw_unpack_thumb(self.handle.0) != 0 {
                return None;
            }
            let mut error: c_int = 0;
            let image = ffi::libraw_dcraw_make_mem_thumb(self.handle.0, &mut error);
            if image.is_null() {
                return None;
            }
            let image = Processed(image);
            let i = &*image.0;
            let bytes = std::slice::from_raw_parts(i.data.as_ptr(), i.data_size as usize).to_vec();
            if i.type_ == ffi::LibRaw_image_formats_LIBRAW_IMAGE_JPEG {
                Some(Thumbnail::Jpeg(bytes))
            } else if i.type_ == ffi::LibRaw_image_formats_LIBRAW_IMAGE_BITMAP && i.colors == 3 && i.bits == 8 {
                RgbImage::from_raw(i.width as u32, i.height as u32, bytes).map(Thumbnail::Bitmap)
            } else {
                None
            }
        }
    }

    /// Full development, with rawpy's `postprocess(use_camera_wb=True,
    /// no_auto_bright=False, output_bps=8)` settings: camera white balance,
    /// automatic brightness, sRGB, the camera's rotation applied.
    pub fn develop(self) -> Result<RgbImage, String> {
        // SAFETY: calls on a live instance; the memory image is released by Processed.
        unsafe {
            {
                let p = &mut (*self.handle.0).params;
                p.use_camera_wb = 1;
                p.no_auto_bright = 0;
                p.output_bps = 8;
                p.output_color = 1;
                p.half_size = 0;
            }
            check(ffi::libraw_unpack(self.handle.0))?;
            check(ffi::libraw_dcraw_process(self.handle.0))?;
            let mut error: c_int = 0;
            let image = ffi::libraw_dcraw_make_mem_image(self.handle.0, &mut error);
            if image.is_null() {
                return Err(libraw_error(error));
            }
            let image = Processed(image);
            let i = &*image.0;
            if i.type_ != ffi::LibRaw_image_formats_LIBRAW_IMAGE_BITMAP || i.colors != 3 || i.bits != 8 {
                return Err(format!("LibRaw: unexpected image ({} channels, {} bits)", i.colors, i.bits));
            }
            let bytes = std::slice::from_raw_parts(i.data.as_ptr(), i.data_size as usize).to_vec();
            RgbImage::from_raw(i.width as u32, i.height as u32, bytes)
                .ok_or_else(|| "LibRaw: inconsistent image size".to_string())
        }
    }
}

/// Runs a LibRaw call, turning an internal panic into an error so that one
/// damaged file never stops a whole generation.
pub fn guarded<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .unwrap_or_else(|_| Err("LibRaw: internal decoder error (damaged file?)".into()))
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// A minimal uncompressed DNG (64 x 48, RGGB mosaic, uniform value).
    pub fn synthetic_dng() -> Vec<u8> {
        include!("testdata/synthetic_dng.rs")
    }

    #[test]
    fn develops_and_measures_a_dng() {
        let dir = std::env::temp_dir().join(format!("pc-raw-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("synthetic.DNG");
        std::fs::write(&file, synthetic_dng()).unwrap();
        assert!(is_raw_file(&file));
        let raw = RawFile::open(&file).unwrap();
        assert_eq!(raw.dimensions(), (64, 48));
        let info = raw.info();
        assert_eq!(info.make.as_deref(), Some("FakeCam"));
        let image = raw.develop().unwrap();
        assert_eq!(image.dimensions(), (64, 48));
        assert!(image.pixels().any(|p| p[1] > 0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn garbage_is_an_error() {
        let dir = std::env::temp_dir().join(format!("pc-raw-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("bad.nef");
        std::fs::write(&file, b"not a raw file").unwrap();
        assert!(RawFile::open(&file).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
