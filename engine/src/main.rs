//! planche-contact-engine: the contact sheet engine of planche-contact-
//! windows, a Rust port of the Linux version's Python engine. The WinUI
//! window (app/) starts it and reads what it writes on its standard output.
//!
//! Commands (the first argument):
//!
//!   generate <options of the Linux version's portfolio.py>
//!       A generation: "PROGRESS:X/100 message" lines, then the end (exit
//!       code 0). With --preview-output FILE, only the first sheet, as PNG.
//!   info        version, choice lists and default profiles ("info" event)
//!   photos --input DIR
//!               supported photos placed directly in DIR ("photo" events)
//!   folders --input DIR [--output DIR]
//!               folders of DIR holding photos ("folder" events)
//!   selection [--paths-from FILE] [PATH...]
//!               what a context menu selection designates ("selection")
//!   thumbnails --size N --output-dir DIR --input-list FILE
//!               a PNG thumbnail of each listed photo ("thumbnail" events)
//!   profile --load FILE
//!               the profile of FILE as the engine understands it

mod captions;
mod contactsheet;
mod events;
mod gallery;
mod i18n;
mod imaging;
mod launch;
mod layout;
mod log;
mod metadata;
mod pdf;
mod profile;
mod raw;
mod scanner;
mod text;
mod themes;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::json;

use contactsheet::{create_contact_sheet, save_jpeg, SheetSettings};
use profile::{Profile, WATERMARK_ORIENTATIONS};

const VERSION: &str = env!("CARGO_PKG_VERSION");

// ------------------------------------------------------------ arguments

/// A minimal option parser in the style of Python's argparse: `--name
/// value`, `--name=value`, `-x value`, flags, repeatable options.
struct Args {
    values: HashMap<String, Vec<String>>,
    flags: Vec<String>,
    positional: Vec<String>,
}

struct OptionSpec {
    names: &'static [&'static str],
    takes_value: bool,
}

fn parse_args(arguments: &[String], specs: &[OptionSpec]) -> Result<Args, String> {
    let mut args = Args { values: HashMap::new(), flags: Vec::new(), positional: Vec::new() };
    let mut i = 0;
    while i < arguments.len() {
        let arg = &arguments[i];
        i += 1;
        if !arg.starts_with('-') || arg == "-" {
            args.positional.push(arg.clone());
            continue;
        }
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) if arg.starts_with("--") => (n.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };
        let spec = specs.iter().find(|s| s.names.contains(&name.as_str()))
            .ok_or_else(|| format!("unrecognized arguments: {arg}"))?;
        let key = spec.names.iter().find(|n| n.starts_with("--")).unwrap_or(&spec.names[0]).to_string();
        if spec.takes_value {
            let value = match inline {
                Some(v) => v,
                None => {
                    let v = arguments.get(i).ok_or_else(|| format!("argument {name}: expected one argument"))?;
                    i += 1;
                    v.clone()
                }
            };
            args.values.entry(key).or_default().push(value);
        } else {
            args.flags.push(key);
        }
    }
    Ok(args)
}

impl Args {
    fn get(&self, name: &str) -> Option<&str> {
        self.values.get(name).and_then(|v| v.last()).map(String::as_str)
    }
    fn all(&self, name: &str) -> Option<&Vec<String>> {
        self.values.get(name)
    }
    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|f| f == name)
    }
    fn int(&self, name: &str, default: i64) -> Result<i64, String> {
        match self.get(name) {
            None => Ok(default),
            Some(text) => text.trim().parse().map_err(|_| format!("argument {name}: invalid int value: '{text}'")),
        }
    }
    fn choice(&self, name: &str, choices: &[&str], default: &str) -> Result<String, String> {
        let value = self.get(name).unwrap_or(default);
        if choices.contains(&value) {
            Ok(value.to_string())
        } else {
            Err(format!("argument {name}: invalid choice: '{value}'"))
        }
    }
}

fn spec(names: &'static [&'static str], takes_value: bool) -> OptionSpec {
    OptionSpec { names, takes_value }
}

/// Lines of a UTF-8 list file (byte order mark allowed), blank lines out.
fn read_list_file(path: &Path) -> Result<Vec<String>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    let text = std::str::from_utf8(bytes).map_err(|e| e.to_string())?;
    Ok(launch::split_lines(text))
}

// -------------------------------------------------------------- generate

fn generate(arguments: &[String]) -> ExitCode {
    let specs = [
        spec(&["-i", "--input"], true),
        spec(&["-o", "--output"], true),
        spec(&["-r", "--recursive"], false),
        spec(&["-n", "--num"], true),
        spec(&["--thumb"], true),
        spec(&["--format"], true),
        spec(&["--title"], true),
        spec(&["--author"], true),
        spec(&["--sort"], true),
        spec(&["--pdf"], false),
        spec(&["--html"], false),
        spec(&["--csv"], false),
        spec(&["--html-per-page"], true),
        spec(&["--html-originals"], false),
        spec(&["--watermark"], true),
        spec(&["--watermark-orientation"], true),
        spec(&["--watermark-opacity"], true),
        spec(&["--language"], true),
        spec(&["--profile"], true),
        spec(&["--include-dir"], true),
        spec(&["--include-dirs-file"], true),
        spec(&["--files-from"], true),
        spec(&["--preview-output"], true),
    ];
    let usage_error = |message: String| {
        eprintln!("usage: planche-contact-engine generate -i INPUT [options]\nerror: {message}");
        ExitCode::from(2)
    };
    let args = match parse_args(arguments, &specs) {
        Ok(args) => args,
        Err(e) => return usage_error(e),
    };
    let (_, unsupported) = i18n::set_language(args.get("--language").unwrap_or("system"));
    if let Some(detected) = unsupported {
        eprintln!("{}", t!("lang.fallback_warning_detail", detected = detected));
    }
    let Some(input) = args.get("--input").map(PathBuf::from) else {
        return usage_error("the following arguments are required: -i/--input".into());
    };
    let parsed = (|| -> Result<_, String> {
        Ok((
            args.int("--num", 12)?,
            args.choice("--format", &profile::PAGE_FORMATS, "A4")?,
            args.choice("--sort", &["date", "name"], "date")?,
            args.int("--html-per-page", 48)?,
            args.choice("--watermark-orientation", &WATERMARK_ORIENTATIONS, "Horizontal")?,
            args.int("--watermark-opacity", 40)?,
            args.int("--thumb", 600)?,
        ))
    })();
    let (num, format, sort, html_per_page, orientation, opacity, _thumb) = match parsed {
        Ok(values) => values,
        Err(e) => return usage_error(e),
    };

    // The profile, with the "simple" options applied when the profile does
    // not already set them (the Linux version's Config.__post_init__).
    let mut profile = args.get("--profile").map(|p| profile::load_profile(Path::new(p))).unwrap_or_default();
    let defaults = profile::LayoutSettings::default();
    if profile.layout.page_format == defaults.page_format && format != defaults.page_format {
        profile.layout.page_format = format.clone();
    }
    if profile.layout.per_page == defaults.per_page && num != defaults.per_page as i64 && num > 0 {
        profile.layout.per_page = num as u32;
    }

    let include_dirs = if args.all("--include-dir").is_none() && args.get("--include-dirs-file").is_none() {
        None
    } else {
        let mut dirs = args.all("--include-dir").cloned().unwrap_or_default();
        if let Some(file) = args.get("--include-dirs-file") {
            match read_list_file(Path::new(file)) {
                Ok(lines) => dirs.extend(lines),
                Err(error) => {
                    eprintln!("{}", t!("cli.error_include_dirs_file", path = file, error = error));
                    return ExitCode::from(2);
                }
            }
        }
        Some(dirs)
    };
    let only_files = match args.get("--files-from") {
        None => None,
        Some(file) => match read_list_file(Path::new(file)) {
            Ok(lines) => Some(lines),
            Err(error) => {
                eprintln!("{}", t!("cli.error_files_from", path = file, error = error));
                return ExitCode::from(2);
            }
        },
    };

    let settings = SheetSettings {
        input_dir: input.clone(),
        title: args.get("--title").map(str::to_string),
        author: args.get("--author").map(str::to_string),
        watermark_text: args.get("--watermark").map(str::to_string),
        watermark_orientation: orientation,
        watermark_opacity: opacity,
        profile,
    };
    let mut scan = scanner::ScanOptions {
        input_dir: input.clone(),
        output_dir: None,
        recursive: args.flag("--recursive"),
        include_dirs,
        only_files,
        sort_by: sort,
    };

    if let Some(preview) = args.get("--preview-output") {
        scan.output_dir = Some(input.clone());
        return generate_preview(&settings, &scan, Path::new(preview));
    }

    let output_dir = args.get("--output").map(PathBuf::from).unwrap_or_else(|| input.join("Portfolio"));
    if let Err(e) = std::fs::create_dir_all(&output_dir) {
        eprintln!("{e}");
        return ExitCode::from(1);
    }
    scan.output_dir = Some(output_dir.clone());
    log::open_file(&output_dir.join("generation.log"));
    let start = std::time::Instant::now();
    log::info(&t!("cli.log_starting", input = input.display()));

    let (want_pdf, want_html, want_csv, originals) =
        (args.flag("--pdf"), args.flag("--html"), args.flag("--csv"), args.flag("--html-originals"));
    if want_html && originals && settings.watermark_text.as_deref().is_some_and(|w| !w.is_empty()) {
        log::warning(&t!("cli.warn_originals_watermark"));
    }

    events::progress(0, &t!("cli.progress_scanning"));
    let photos = scanner::scan(&scan);
    events::progress(10, &t!("cli.progress_found", count = photos.len()));
    if photos.is_empty() {
        log::warning(&t!("cli.log_no_images"));
        events::progress(100, &t!("cli.progress_done_empty"));
        return ExitCode::SUCCESS;
    }

    let per_page = layout::compute_effective_per_page(&settings.profile).max(1) as usize;
    let total_pages = photos.len().div_ceil(per_page);

    let planches_dir = output_dir.join("planches");
    if planches_dir.exists() {
        let _ = std::fs::remove_dir_all(&planches_dir);
    }
    let _ = std::fs::create_dir_all(&planches_dir);
    let pdf_path = output_dir.join("portfolio.pdf");
    if pdf_path.exists() {
        let _ = std::fs::remove_file(&pdf_path);
    }

    events::progress(15, &t!("cli.progress_sheets"));
    let mut sheet_files = Vec::new();
    let mut text_runs = HashMap::new();
    for (index, batch) in photos.chunks(per_page).enumerate() {
        let page_num = index + 1;
        let Some(sheet) = create_contact_sheet(&settings, batch, page_num, total_pages) else { continue };
        let path = planches_dir.join(format!("planche_{page_num:03}.jpg"));
        match save_jpeg(&sheet.image, &path, 95) {
            Ok(()) => {
                sheet_files.push(path.clone());
                text_runs.insert(path, sheet.text_runs);
                events::line(&t!("cli.sheet_generated", page = page_num, total = total_pages));
                events::progress(15 + (55.0 * page_num as f64 / total_pages as f64) as u32, "");
            }
            Err(error) => log::error(&t!("cli.log_error_sheet", page = page_num, error = error)),
        }
    }

    if want_pdf && !sheet_files.is_empty() {
        events::progress(75, &t!("cli.progress_pdf"));
        if let Err(error) = pdf::create_pdf(&sheet_files, &pdf_path, &text_runs) {
            log::error(&t!("engine.error_pdf_add_sheet", name = "portfolio.pdf", error = error));
        }
        events::progress(80, &t!("cli.progress_pdf_done"));
    }
    if want_csv {
        events::progress(82, &t!("cli.progress_csv"));
        if let Err(error) = write_csv(&photos, &output_dir.join("index.csv")) {
            log::error(&error);
        }
        events::progress(85, &t!("cli.progress_csv_done"));
    }
    if want_html {
        events::progress(86, &t!("cli.progress_html"));
        let gallery_settings = gallery::GallerySettings {
            sheet: settings.clone(),
            images_per_page: html_per_page.max(1) as usize,
            include_originals: originals,
        };
        if let Err(error) = gallery::create_gallery(&gallery_settings, &photos, &output_dir.join("gallery")) {
            log::error(&error);
        }
        events::progress(98, &t!("cli.progress_html_done"));
    }

    events::progress(100, &t!("cli.progress_success"));
    log::info(&t!("cli.log_finished", elapsed = format!("{:.1}", start.elapsed().as_secs_f64())));
    ExitCode::SUCCESS
}

/// The first sheet only, as a PNG, and "PREVIEW:shown/total" for the window.
fn generate_preview(settings: &SheetSettings, scan: &scanner::ScanOptions, output: &Path) -> ExitCode {
    let photos = scanner::scan(scan);
    if photos.is_empty() {
        return ExitCode::SUCCESS;
    }
    let per_page = layout::compute_effective_per_page(&settings.profile).max(1) as usize;
    let total_pages = photos.len().div_ceil(per_page);
    let batch = &photos[..per_page.min(photos.len())];
    if let Some(sheet) = create_contact_sheet(settings, batch, 1, total_pages) {
        if let Some(parent) = output.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Err(e) = sheet.image.save_with_format(output, image::ImageFormat::Png) {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
        events::line(&format!("PREVIEW:{}/{}", batch.len(), photos.len()));
    }
    ExitCode::SUCCESS
}

/// index.csv: number and file name of each photo (Python's csv module:
/// quoted when needed, CRLF line ends).
fn write_csv(photos: &[scanner::Photo], path: &Path) -> Result<(), String> {
    fn field(text: &str) -> String {
        if text.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", text.replace('"', "\"\""))
        } else {
            text.to_string()
        }
    }
    let mut out = format!("{},{}\r\n", field(&t!("csv.header_num")), field(&t!("csv.header_filename")));
    for photo in photos {
        out += &format!("{},{}\r\n", field(&photo.num), field(&photo.filename));
    }
    std::fs::write(path, out).map_err(|e| e.to_string())
}

// ---------------------------------------------------------- other commands

fn command_info() -> ExitCode {
    let mut extensions: Vec<String> = scanner::SUPPORTED_EXTENSIONS.iter().map(|e| format!(".{e}")).collect();
    extensions.sort();
    events::emit("info", json!({
        "protocol": events::PROTOCOL_VERSION,
        "version": VERSION,
        "raw_support": true,
        "extensions": extensions,
        "languages": i18n::AVAILABLE_LANGUAGES,
        "choices": {
            "layout_templates": profile::LAYOUT_TEMPLATES,
            "fit_modes": profile::FIT_MODES,
            "page_formats": profile::PAGE_FORMATS,
            "orientations": profile::ORIENTATIONS,
            "header_positions": profile::HEADER_POSITIONS,
            "caption_positions": profile::CAPTION_POSITIONS,
            "caption_alignments": profile::CAPTION_ALIGNMENTS,
            "caption_fields": profile::CAPTION_FIELDS,
            "theme_modes": profile::THEME_MODES,
            "watermark_orientations": WATERMARK_ORIENTATIONS,
        },
        "default_profile": Profile::default().to_json(),
        "interface_default_profile": profile::interface_default_profile().to_json(),
    }));
    ExitCode::SUCCESS
}

fn fatal(message: impl std::fmt::Display) -> ExitCode {
    events::emit("fatal_error", json!({ "message": message.to_string() }));
    ExitCode::from(1)
}

fn command_photos(arguments: &[String]) -> ExitCode {
    let args = match parse_args(arguments, &[spec(&["--input"], true)]) {
        Ok(a) => a,
        Err(e) => return fatal(e),
    };
    let Some(input) = args.get("--input") else { return fatal("--input is required") };
    match scanner::photos_in(Path::new(input)) {
        Ok(photos) => {
            for path in &photos {
                events::emit("photo", json!({ "path": path.display().to_string() }));
            }
            events::emit("summary", json!({ "total": photos.len() }));
            ExitCode::SUCCESS
        }
        Err(e) => fatal(e),
    }
}

fn command_folders(arguments: &[String]) -> ExitCode {
    let args = match parse_args(arguments, &[spec(&["--input"], true), spec(&["--output"], true)]) {
        Ok(a) => a,
        Err(e) => return fatal(e),
    };
    let Some(input) = args.get("--input").map(PathBuf::from) else { return fatal("--input is required") };
    if !input.is_dir() {
        return fatal(format!("not a folder: {}", input.display()));
    }
    let output = args.get("--output").map(PathBuf::from).unwrap_or_else(|| input.join("Portfolio"));
    let folders = scanner::list_image_folders(&input, Some(&output));
    for (path, count) in &folders {
        events::emit("folder", json!({ "path": path, "count": count }));
    }
    let photos: usize = folders.iter().map(|(_, c)| c).sum();
    events::emit("summary", json!({ "total": folders.len(), "photos": photos }));
    ExitCode::SUCCESS
}

fn command_selection(arguments: &[String]) -> ExitCode {
    match launch::resolve_selection(&launch::paths_from_arguments(arguments)) {
        None => events::emit("selection", json!({ "input_dir": null, "files": null })),
        Some(s) => events::emit("selection", json!({ "input_dir": s.input_dir.display().to_string(), "files": s.files })),
    }
    ExitCode::SUCCESS
}

fn command_thumbnails(arguments: &[String]) -> ExitCode {
    let specs = [spec(&["--size"], true), spec(&["--output-dir"], true), spec(&["--input-list"], true)];
    let args = match parse_args(arguments, &specs) {
        Ok(a) => a,
        Err(e) => return fatal(e),
    };
    let size = match args.int("--size", 90) {
        Ok(s) => s.clamp(16, 1024) as u32,
        Err(e) => return fatal(e),
    };
    let (Some(output_dir), Some(list)) = (args.get("--output-dir"), args.get("--input-list")) else {
        return fatal("--output-dir and --input-list are required");
    };
    let output_dir = PathBuf::from(output_dir);
    let paths = match read_list_file(Path::new(list)).and_then(|p| {
        std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
        Ok(p)
    }) {
        Ok(p) => p,
        Err(e) => return fatal(e),
    };
    use rayon::prelude::*;
    // Made in parallel, reported in order.
    let results: Vec<_> = paths
        .par_iter()
        .enumerate()
        .map(|(index, raw)| {
            let output = output_dir.join(format!("thumbnail-{index:05}.png"));
            let made = imaging::load_image(Path::new(raw), Some((size, size)), false)
                .map(|image| imaging::thumbnail(image, size, size))
                .and_then(|image| {
                    image.save_with_format(&output, image::ImageFormat::Png).map_err(|e| e.to_string())?;
                    Ok(image.dimensions())
                });
            (index, raw, output, made)
        })
        .collect();
    for (index, raw, output, made) in results {
        match made {
            Ok((width, height)) => events::emit("thumbnail", json!({
                "index": index, "input": raw, "output": output.display().to_string(),
                "width": width, "height": height,
            })),
            Err(error) => events::emit("thumbnail", json!({ "index": index, "input": raw, "error": error })),
        }
    }
    ExitCode::SUCCESS
}

fn command_profile(arguments: &[String]) -> ExitCode {
    let args = match parse_args(arguments, &[spec(&["--load"], true)]) {
        Ok(a) => a,
        Err(e) => return fatal(e),
    };
    let Some(file) = args.get("--load") else { return fatal("--load is required") };
    events::emit("profile", json!({ "profile": profile::load_profile(Path::new(file)).to_json() }));
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = arguments.split_first() else {
        eprintln!("usage: planche-contact-engine {{generate,info,photos,folders,selection,thumbnails,profile}} ...");
        return ExitCode::from(2);
    };
    match command.as_str() {
        "generate" => generate(rest),
        "info" => command_info(),
        "photos" => command_photos(rest),
        "folders" => command_folders(rest),
        "selection" => command_selection(rest),
        "thumbnails" => command_thumbnails(rest),
        "profile" => command_profile(rest),
        "--version" => {
            println!("planche-contact-engine {VERSION}");
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: planche-contact-engine {{generate,info,photos,folders,selection,thumbnails,profile}} ...");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argparse_style_options() {
        let specs = [spec(&["-i", "--input"], true), spec(&["--pdf"], false), spec(&["--include-dir"], true)];
        let args: Vec<String> = ["-i", "/p", "--pdf", "--include-dir=a", "--include-dir", "b"].iter().map(|s| s.to_string()).collect();
        let parsed = parse_args(&args, &specs).unwrap();
        assert_eq!(parsed.get("--input"), Some("/p"));
        assert!(parsed.flag("--pdf"));
        assert_eq!(parsed.all("--include-dir").unwrap(), &vec!["a".to_string(), "b".to_string()]);
        assert!(parse_args(&["--nope".to_string()], &specs).is_err());
    }
}
