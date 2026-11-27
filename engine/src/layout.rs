//! Geometry of the contact sheets, in pixels at 300 dpi, independent of the
//! rendering (the Linux version's portfolio/presentation/layout_engine.py,
//! ported rule for rule: same constants, same integer arithmetic).

use crate::profile::Profile;

pub const DPI: f64 = 300.0;
pub const HEADER_RESERVED_PX_GRID: i64 = 180;
pub const FOOTER_PAGE_NUMBER_OFFSET_PX: i64 = 114;
pub const FOOTER_CREDIT_OFFSET_PX: i64 = 76;
pub const FOOTER_LICENSE_OFFSET_PX: i64 = 50;
pub const FOOTER_CLEARANCE_PX: i64 = 12;
pub const FOOTER_RESERVED_PX: i64 = FOOTER_PAGE_NUMBER_OFFSET_PX + FOOTER_CLEARANCE_PX;
pub const GRID_MAX_COLUMNS_TESTED: i64 = 8;
pub const LARGE_THUMBS_MAX_COLUMNS_TESTED: i64 = 4;
pub const LARGE_THUMBS_MAX_ROWS_TESTED: i64 = 2;
pub const LARGE_THUMBS_MAX_EFFECTIVE_PER_PAGE: i64 = LARGE_THUMBS_MAX_COLUMNS_TESTED * LARGE_THUMBS_MAX_ROWS_TESTED;
pub const MIN_THUMB_SIZE_PX: i64 = 70;
pub const CAPTION_LINE_SPACING_PX: i64 = 4;
pub const CAPTION_PHOTO_GAP_PX: i64 = 18;

/// Python's `int(mm * 300 / 25.4)` (truncation toward zero).
pub fn mm_to_px(mm: f64) -> i64 {
    (mm * DPI / 25.4) as i64
}

pub fn pt_to_px(pt: f64) -> i64 {
    (pt * DPI / 72.0) as i64
}

fn paper_mm(format: &str) -> (f64, f64) {
    match format {
        "A5" => (148.0, 210.0),
        "A3" => (297.0, 420.0),
        "A2" => (420.0, 594.0),
        "Letter" => (216.0, 279.0),
        _ => (210.0, 297.0),
    }
}

pub fn canvas_size_px(page_format: &str, orientation: &str) -> (i64, i64) {
    let (mut w, mut h) = paper_mm(page_format);
    if orientation == "landscape" {
        std::mem::swap(&mut w, &mut h);
    }
    (mm_to_px(w), mm_to_px(h))
}

/// Number of photos actually placed on a page: `per_page`, except for the
/// "large thumbnails" template which limits itself.
pub fn compute_effective_per_page(profile: &Profile) -> i64 {
    let per_page = profile.layout.per_page as i64;
    if profile.layout.template == "large_thumbs" {
        per_page.min(LARGE_THUMBS_MAX_EFFECTIVE_PER_PAGE)
    } else {
        per_page
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CellLayout {
    pub index: usize,
    pub row: i64,
    pub col: i64,
    pub photo_x: i64,
    pub photo_y: i64,
    pub photo_w: i64,
    pub photo_h: i64,
    pub caption_x: Option<i64>,
    pub caption_y: Option<i64>,
    pub caption_w: Option<i64>,
    pub caption_h: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageLayout {
    pub canvas_w: i64,
    pub canvas_h: i64,
    pub margin_px: i64,
    pub spacing_px: i64,
    pub content_top_px: i64,
    pub cols: i64,
    pub rows: i64,
    pub thumb_size: i64,
    pub effective_per_page: i64,
    pub cells: Vec<CellLayout>,
    pub grid_w: i64,
    pub grid_h: i64,
    pub content_bottom_px: i64,
}

fn ceil_div(a: i64, b: i64) -> i64 {
    (a + b - 1).div_euclid(b)
}

/// Python's floor division, for possibly negative numerators.
fn floor_div(a: i64, b: i64) -> i64 {
    a.div_euclid(b)
}

fn reserve_caption_px(profile: &Profile, caption_lines: Option<i64>) -> (i64, i64) {
    let layout = &profile.layout;
    match layout.caption_position.as_str() {
        "below" | "above" => {
            let configured_h = mm_to_px(layout.caption_area_mm);
            let font_px = pt_to_px(profile.captions.font_size as f64).max(6);
            let lines = caption_lines.unwrap_or(profile.captions.lines as i64);
            let needed_h = lines.max(1) * (font_px + CAPTION_LINE_SPACING_PX);
            (0, configured_h.max(needed_h))
        }
        "right" => (mm_to_px(layout.caption_area_mm), 0),
        _ => (0, 0),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_spacing_clamp(
    thumb_size: i64, available_w: i64, available_h: i64, cols: i64, rows: i64,
    spacing_px: i64, caption_w_px: i64, caption_h_px: i64,
) -> i64 {
    let max_thumb_w = floor_div(available_w - (cols - 1) * spacing_px, cols) - caption_w_px;
    let thumb_size = thumb_size.min(max_thumb_w.max(1));
    let max_thumb_h = floor_div(available_h - (rows - 1) * spacing_px, rows) - caption_h_px;
    thumb_size.min(max_thumb_h.max(1))
}

fn cell_sizes(available_w: i64, available_h: i64, cols: i64, rows: i64) -> (i64, i64) {
    let cell_w = if cols > 1 { floor_div(available_w, cols) } else { available_w };
    let cell_h = if rows > 1 { floor_div(available_h, rows) } else { available_h };
    (cell_w, cell_h)
}

fn grid_search(
    available_w: i64, available_h: i64, per_page: i64, spacing_px: i64,
    max_columns_tested: i64, caption_w_px: i64, caption_h_px: i64,
) -> (i64, i64) {
    let mut best_cols = 1;
    let mut best_thumb_size = 0;
    for cols in 1..(per_page + 1).min(max_columns_tested + 1) {
        let rows = ceil_div(per_page, cols);
        let (cell_w, cell_h) = cell_sizes(available_w, available_h, cols, rows);
        let thumb_size = (cell_w - caption_w_px).max(1).min((cell_h - caption_h_px).max(1));
        if thumb_size > best_thumb_size {
            best_thumb_size = thumb_size;
            best_cols = cols;
        }
    }
    let thumb_size = best_thumb_size.max(MIN_THUMB_SIZE_PX);
    let cols = best_cols;
    let rows = ceil_div(per_page, cols);
    let thumb_size = apply_spacing_clamp(
        thumb_size, available_w, available_h, cols, rows, spacing_px, caption_w_px, caption_h_px,
    );
    (thumb_size, cols)
}

fn free_layout(
    profile: &Profile, available_w: i64, available_h: i64, spacing_px: i64,
    caption_w_px: i64, caption_h_px: i64,
) -> (i64, i64) {
    let layout = &profile.layout;
    let per_page = layout.per_page as i64;
    let (cols, rows) = match (layout.columns.map(i64::from), layout.rows.map(i64::from)) {
        (None, None) => {
            return grid_search(available_w, available_h, per_page, spacing_px,
                               GRID_MAX_COLUMNS_TESTED, caption_w_px, caption_h_px);
        }
        (None, Some(rows)) => (ceil_div(per_page, rows).max(1), rows),
        (Some(cols), None) => (cols, ceil_div(per_page, cols).max(1)),
        (Some(cols), Some(rows)) => (cols, rows),
    };
    let (cell_w, cell_h) = cell_sizes(available_w, available_h, cols, rows);
    let thumb_size = MIN_THUMB_SIZE_PX.max((cell_w - caption_w_px).max(1).min((cell_h - caption_h_px).max(1)));
    let thumb_size = apply_spacing_clamp(
        thumb_size, available_w, available_h, cols, rows, spacing_px, caption_w_px, caption_h_px,
    );
    (thumb_size, cols)
}

/// Geometry of a page holding `photo_count` photos. `caption_lines`: the
/// number of caption lines to reserve below/above each photo (default:
/// the profile's).
pub fn compute_page_layout(profile: &Profile, photo_count: usize, caption_lines: Option<i64>) -> PageLayout {
    let layout = &profile.layout;
    let (canvas_w, canvas_h) = canvas_size_px(&layout.page_format, &layout.orientation);
    let margin_px = mm_to_px(layout.margin_mm);
    let spacing_px = mm_to_px(layout.spacing_mm);

    let bottom_px = margin_px.max(FOOTER_RESERVED_PX);
    let available_w = canvas_w - 2 * margin_px;
    let available_h = canvas_h - margin_px - bottom_px - HEADER_RESERVED_PX_GRID;

    let (caption_w_px, caption_h_px) = reserve_caption_px(profile, caption_lines);
    let position = layout.caption_position.as_str();
    let gap_px = if matches!(position, "below" | "above" | "right") { CAPTION_PHOTO_GAP_PX } else { 0 };
    let reserved_w_px = caption_w_px + if position == "right" { gap_px } else { 0 };
    let reserved_h_px = caption_h_px + if matches!(position, "below" | "above") { gap_px } else { 0 };

    let mut effective_per_page = compute_effective_per_page(profile);

    let (thumb_size, cols) = match layout.template.as_str() {
        "free" => {
            effective_per_page = layout.per_page as i64;
            free_layout(profile, available_w, available_h, spacing_px, reserved_w_px, reserved_h_px)
        }
        "large_thumbs" => grid_search(available_w, available_h, effective_per_page, spacing_px,
                                      LARGE_THUMBS_MAX_COLUMNS_TESTED, reserved_w_px, reserved_h_px),
        "landscape" | "portrait" => {
            let (mut thumb_size, mut cols) = grid_search(
                available_w, available_h, effective_per_page, spacing_px,
                GRID_MAX_COLUMNS_TESTED, reserved_w_px, reserved_h_px,
            );
            let mut rows_at_cols = ceil_div(effective_per_page, cols);
            let prefer_more_cols = layout.template == "landscape";
            for alt_cols in 1..(effective_per_page + 1).min(GRID_MAX_COLUMNS_TESTED + 1) {
                if (alt_cols > cols) != prefer_more_cols {
                    continue;
                }
                let alt_rows = ceil_div(effective_per_page, alt_cols);
                let (cell_w, cell_h) = cell_sizes(available_w, available_h, alt_cols, alt_rows);
                let alt_thumb = (cell_w - reserved_w_px).max(1).min((cell_h - reserved_h_px).max(1));
                let better_side = (prefer_more_cols && alt_cols > cols) || (!prefer_more_cols && alt_cols < cols);
                if alt_thumb as f64 >= thumb_size as f64 * 0.95 && better_side {
                    cols = alt_cols;
                    thumb_size = alt_thumb;
                    rows_at_cols = alt_rows;
                }
            }
            let clamped = apply_spacing_clamp(
                thumb_size, available_w, available_h, cols, rows_at_cols, spacing_px,
                reserved_w_px, reserved_h_px,
            );
            (MIN_THUMB_SIZE_PX.max(clamped), cols)
        }
        _ => grid_search(available_w, available_h, effective_per_page, spacing_px,
                         GRID_MAX_COLUMNS_TESTED, reserved_w_px, reserved_h_px),
    };

    let rows = ceil_div(effective_per_page, cols);
    let content_top_px = margin_px + HEADER_RESERVED_PX_GRID;
    let grid_w = cols * (thumb_size + reserved_w_px) + (cols - 1) * spacing_px;
    let grid_h = rows * (thumb_size + reserved_h_px) + (rows - 1) * spacing_px;
    let grid_left_px = margin_px + floor_div((available_w - grid_w).max(0), 2);

    let n_cells = (photo_count as i64).min(effective_per_page).max(0) as usize;
    let mut cells = Vec::with_capacity(n_cells);
    for idx in 0..n_cells {
        let row = idx as i64 / cols;
        let col = idx as i64 % cols;
        let cell_x = grid_left_px + col * (thumb_size + reserved_w_px + spacing_px);
        let cell_y = content_top_px + row * (thumb_size + reserved_h_px + spacing_px);
        let (photo_y, caption_y) = if position == "above" {
            (cell_y + caption_h_px + gap_px, cell_y)
        } else {
            (cell_y, cell_y + thumb_size + gap_px)
        };
        let mut cell = CellLayout {
            index: idx, row, col,
            photo_x: cell_x, photo_y, photo_w: thumb_size, photo_h: thumb_size,
            caption_x: None, caption_y: None, caption_w: None, caption_h: None,
        };
        if position == "right" {
            cell.caption_x = Some(cell_x + thumb_size + gap_px);
            cell.caption_y = Some(cell_y);
            cell.caption_w = Some(caption_w_px);
            cell.caption_h = Some(thumb_size);
        } else if matches!(position, "below" | "above") {
            cell.caption_x = Some(cell_x);
            cell.caption_y = Some(caption_y);
            cell.caption_w = Some(thumb_size);
            cell.caption_h = Some(caption_h_px);
        }
        cells.push(cell);
    }

    PageLayout {
        canvas_w, canvas_h, margin_px, spacing_px, content_top_px, cols, rows, thumb_size,
        effective_per_page, cells, grid_w, grid_h,
        content_bottom_px: canvas_h - bottom_px,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::interface_default_profile;

    #[test]
    fn a4_portrait_size() {
        assert_eq!(canvas_size_px("A4", "portrait"), (2480, 3507));
        assert_eq!(canvas_size_px("A4", "landscape"), (3507, 2480));
        assert_eq!(canvas_size_px("unknown", "portrait"), (2480, 3507));
    }

    #[test]
    fn large_thumbs_limits_the_photos_per_page() {
        let mut p = Profile::default();
        p.layout.template = "large_thumbs".into();
        p.layout.per_page = 48;
        assert_eq!(compute_effective_per_page(&p), 8);
        let page = compute_page_layout(&p, 48, None);
        assert_eq!(page.cells.len(), 8);
        assert!(page.cols <= 4);
    }

    #[test]
    fn cells_stay_inside_the_usable_area() {
        for template in ["grid", "landscape", "portrait", "large_thumbs", "free"] {
            for per_page in [8u32, 12, 24, 48] {
                for format in ["A5", "A4", "A3", "Letter"] {
                    let mut p = interface_default_profile();
                    p.layout.template = template.into();
                    p.layout.per_page = per_page;
                    p.layout.page_format = format.into();
                    let page = compute_page_layout(&p, per_page as usize, None);
                    for cell in &page.cells {
                        assert!(cell.photo_x >= page.margin_px, "{template} {per_page} {format}");
                        assert!(cell.photo_x + cell.photo_w <= page.canvas_w - page.margin_px);
                        let bottom = cell.caption_y.unwrap() + cell.caption_h.unwrap();
                        assert!(bottom <= page.content_bottom_px + 1, "{template} {per_page} {format}");
                    }
                }
            }
        }
    }

    /// Values computed by the Linux version (Python) for the same settings.
    #[test]
    fn same_geometry_as_the_linux_version() {
        let page = compute_page_layout(&Profile::default(), 12, None);
        assert_eq!((page.cols, page.rows, page.thumb_size), (3, 4, 693));
        assert_eq!((page.cells[0].photo_x, page.cells[0].photo_y), (141, 321));
        assert_eq!((page.cells[4].photo_x, page.cells[4].photo_y), (893, 1073));
        let page = compute_page_layout(&interface_default_profile(), 12, None);
        assert_eq!((page.cols, page.rows, page.thumb_size), (3, 4, 605));
        assert_eq!(page.cells[1].caption_y, Some(944));
    }
}
