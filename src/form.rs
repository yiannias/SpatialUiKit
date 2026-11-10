//! One row rhythm for form-style panels: every label starts at one left edge,
//! every control starts at one second edge, every row has the same height and
//! gap.
//!
//! Before this module each panel did it its own way, with different label
//! columns (116, 118, 128 and 132 points were all in use). The metrics below
//! are decided once, in [`FormMetrics::default`], and every form row goes
//! through [`row`] / [`row_with`].
//!
//! # Prior art: why this is not an `egui::Grid`
//!
//! A named `egui::Grid` paints nothing on the pass that first sees it and
//! leans on a `request_discard` it does not always get, so a whole form can
//! appear blank for a frame. A row here holds no persisted state and is correct
//! on the first pass. This is a port of the Properties panel's own `form_row`
//! (Chris's 2026-09-30 decision: a fixed label column, not one auto-sized to
//! the longest label — auto-sizing let one long label push every value right
//! for every row, while short labels left a dead gap, so values did not line up
//! down the column, which is the whole point of a label column).
//!
//! The label height is **measured before the row is allocated**, because a
//! fixed-size allocation does not grow to fit a label that wraps to two lines
//! and would overpaint the row below.

/// The shared form metrics, in points at the default text scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormMetrics {
    /// Width of the whole label cell, **including** [`Self::label_gutter`].
    /// Values start exactly this far from the row's left edge.
    ///
    /// 120 pt is one value between the four widths that were in use
    /// (116/118/128/132): it fits the longest single-line labels at the 12 pt
    /// body size and still leaves a usable value column in a narrow docked
    /// Properties tab.
    pub label_column: f32,
    /// Space at the right of the label cell that label text never enters, so
    /// label text wraps within `label_column - label_gutter` (112 pt).
    pub label_gutter: f32,
    /// Minimum row height: the height of the standard controls at 12 pt body
    /// text, so text-only rows and control rows are the same height. A row
    /// grows for a wrapped label or a taller control.
    pub row_height: f32,
    /// Vertical gap between consecutive rows.
    pub row_gap: f32,
    /// Left/right margin used by option cards and modal sheet content.
    pub side_margin: f32,
    /// A label wraps to at most this many lines, then elides with '…'. Three
    /// lines covers every label in the forms at the column width; past that the
    /// text is unreadable anyway and the value matters more.
    pub label_max_rows: usize,
}

impl Default for FormMetrics {
    fn default() -> Self {
        Self {
            label_column: 120.0,
            label_gutter: 8.0,
            row_height: 22.0,
            row_gap: 4.0,
            side_margin: 12.0,
            label_max_rows: 3,
        }
    }
}

impl FormMetrics {
    /// The metrics with every point size multiplied by `factor`. The label row
    /// limit is a count, not a size, so it is unchanged.
    pub fn scaled(self, factor: f32) -> Self {
        Self {
            label_column: self.label_column * factor,
            label_gutter: self.label_gutter * factor,
            row_height: self.row_height * factor,
            row_gap: self.row_gap * factor,
            side_margin: self.side_margin * factor,
            label_max_rows: self.label_max_rows,
        }
    }

    /// The default metrics scaled by the text scale in effect for `ui`, so a
    /// form honours the app's text scale setting.
    pub fn for_ui(ui: &egui::Ui) -> Self {
        Self::default().scaled(crate::theme::current_text_scale(ui))
    }
}

/// Builds the layout job for one form label.
///
/// Split out from [`row_with`] so the *wrap configuration* can be asserted in a
/// plain unit test: a test `Ui` may have no usable fonts, so no galley wraps
/// there. The setting is the part this module owns; egui's wrapping itself is
/// not ours to test.
pub fn label_job(ui: &egui::Ui, label: &str, metrics: &FormMetrics) -> egui::text::LayoutJob {
    // The Body *font size*, not `text_style_height` (the row height, which is
    // larger), so labels are never bigger than the values beside them.
    let font = egui::FontId::proportional(egui::TextStyle::Body.resolve(ui.style()).size);
    // Quiet label grey in dark mode. This is the same value as `os_style`'s
    // private `grey::TEXT_LABEL`, which cannot be reached without making that
    // module public; light keeps the ordinary text colour.
    let color = if ui.visuals().dark_mode {
        egui::Color32::from_rgb(0xB4, 0xB5, 0xBC)
    } else {
        ui.visuals().text_color()
    };
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_owned(), font, color);
    // A bounded width inside the fixed column, rather than `f32::INFINITY`,
    // which is what the single-line constructor starts from.
    job.wrap.max_width = metrics.label_column - metrics.label_gutter;
    job.wrap.max_rows = metrics.label_max_rows;
    job.wrap.overflow_character = Some('…');
    job
}

/// Sets the vertical item spacing to the shared row gap. Call once at the top
/// of a form so consecutive rows get the same gap.
pub fn apply_row_gap(ui: &mut egui::Ui, metrics: &FormMetrics) {
    ui.spacing_mut().item_spacing.y = metrics.row_gap;
}

/// One form row using [`FormMetrics::for_ui`]: a fixed-width wrapping label on
/// the left, `add_value`'s widgets on the right. See [`row_with`].
pub fn row<R>(ui: &mut egui::Ui, label: &str, add_value: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let metrics = FormMetrics::for_ui(ui);
    row_with(ui, &metrics, label, add_value)
}

/// One form row with explicit metrics.
///
/// The label cell is exactly `label_column` wide. A single-line label is
/// vertically centred in the row, a wrapped label is top-aligned. The value
/// side starts exactly `label_column` from the row's left edge (no item spacing
/// between the cell and the value); inside `add_value` the caller's original
/// `item_spacing.x` applies and the full remaining width is available, so a
/// control may fill to the right edge. Single-line rows centre the value's
/// widgets vertically.
///
/// The label cell is deliberately **not** clamped to `ui.available_width()`:
/// an auto-sizing container must see the true desired width.
pub fn row_with<R>(
    ui: &mut egui::Ui,
    metrics: &FormMetrics,
    label: &str,
    add_value: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let galley = ui.painter().layout_job(label_job(ui, label, metrics));
    let label_h = galley.size().y;
    let single_line = galley.rows.len() <= 1;
    let height = metrics.row_height.max(label_h);
    let width = ui.available_width();
    let layout = if single_line {
        egui::Layout::left_to_right(egui::Align::Center)
    } else {
        egui::Layout::left_to_right(egui::Align::Min)
    };
    ui.allocate_ui_with_layout(egui::vec2(width, height), layout, |ui| {
        let original_spacing = ui.spacing().item_spacing.x;
        // The label cell already carries the gutter in its own width, so no
        // item spacing may follow it or values would start past the column.
        ui.spacing_mut().item_spacing.x = 0.0;
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(metrics.label_column, height),
            egui::Sense::hover(),
        );
        let top = if single_line {
            rect.center().y - label_h / 2.0
        } else {
            rect.top()
        };
        let color = ui.visuals().text_color();
        ui.painter()
            .galley(egui::pos2(rect.left(), top), galley, color);
        ui.spacing_mut().item_spacing.x = original_spacing;
        add_value(ui)
    })
    .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_decided_metrics() {
        let m = FormMetrics::default();
        assert_eq!(m.label_column, 120.0);
        assert_eq!(m.label_gutter, 8.0);
        assert_eq!(m.row_height, 22.0);
        assert_eq!(m.row_gap, 4.0);
        assert_eq!(m.side_margin, 12.0);
        assert_eq!(m.label_max_rows, 3);
    }

    #[test]
    fn scaled_multiplies_sizes_but_not_the_row_limit() {
        let m = FormMetrics::default().scaled(1.5);
        assert_eq!(m.label_column, 180.0);
        assert_eq!(m.label_gutter, 12.0);
        assert_eq!(m.row_height, 33.0);
        assert_eq!(m.row_gap, 6.0);
        assert_eq!(m.side_margin, 18.0);
        assert_eq!(m.label_max_rows, 3);
    }

    fn run_frame(add: impl FnOnce(&mut egui::Ui)) {
        let ctx = egui::Context::default();
        let mut add = Some(add);
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(600.0, 400.0),
                )),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    if let Some(add) = add.take() {
                        add(ui);
                    }
                });
            },
        );
    }

    #[test]
    fn label_job_wraps_within_the_column_then_elides() {
        run_frame(|ui| {
            let m = FormMetrics::default();
            let job = label_job(ui, "A fairly long label", &m);
            assert_eq!(job.wrap.max_width, 112.0);
            assert_eq!(job.wrap.max_rows, 3);
            assert_eq!(job.wrap.overflow_character, Some('…'));
            let scaled = label_job(ui, "x", &m.scaled(2.0));
            assert_eq!(scaled.wrap.max_width, 224.0);
        });
    }

    #[test]
    fn values_share_one_left_edge_and_single_line_rows_share_a_height() {
        run_frame(|ui| {
            let m = FormMetrics::default();
            apply_row_gap(ui, &m);
            let left = ui.min_rect().left();
            let mut edges = Vec::new();
            let mut heights = Vec::new();
            for label in ["ID:", "Longer label:", "A much longer label that wraps:"] {
                let mut value_left = 0.0;
                let top = ui.cursor().top();
                row_with(ui, &m, label, |ui| {
                    let r = ui.add(egui::Label::new("v"));
                    value_left = r.rect.left();
                });
                edges.push(value_left);
                heights.push(ui.cursor().top() - top - m.row_gap);
            }
            for edge in &edges {
                assert!((edge - (left + m.label_column)).abs() < 0.01, "{edges:?}");
            }
            assert!((heights[0] - heights[1]).abs() < 0.01, "{heights:?}");
            assert!(
                heights[2] > heights[0],
                "a wrapped label grows its row: {heights:?}"
            );
            assert!(heights[0] >= m.row_height - 0.01, "{heights:?}");
        });
    }
}
