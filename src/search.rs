//! Shared search controls and text marking for panels that browse long lists.

use egui::{Color32, FontId, Response, Stroke, Ui};
use std::ops::Range;

/// What the field reports beside its query.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchCount {
    /// Do not show a count.
    #[default]
    Hidden,
    /// A list is filtered to this many matching rows.
    Results(usize),
    /// A scrollable document has a current hit among all hits.
    Hits {
        current: Option<usize>,
        total: usize,
    },
}

impl SearchCount {
    fn label(self) -> Option<String> {
        match self {
            SearchCount::Hidden => None,
            SearchCount::Results(0) | SearchCount::Hits { total: 0, .. } => {
                Some("No matches".to_owned())
            }
            SearchCount::Results(1) => Some("1 result".to_owned()),
            SearchCount::Results(n) => Some(format!("{n} results")),
            SearchCount::Hits {
                current: Some(i),
                total,
            } => Some(format!("{} of {total}", i + 1)),
            SearchCount::Hits {
                current: None,
                total,
            } => Some(format!("{total} hits")),
        }
    }
}

/// An action requested from a search field by a key or its controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchAction {
    /// Advance to the next hit.
    Next,
    /// Move to the previous hit.
    Previous,
    /// Clear the query.
    Clear,
}

/// Options shared by search fields across panels and detached windows.
#[derive(Clone, Copy, Debug)]
pub struct SearchFieldOptions<'a> {
    /// Window-specific placeholder text.
    pub hint: &'a str,
    /// Field height in points.
    pub height: f32,
    /// Optional filtered-row or document-hit count.
    pub count: SearchCount,
}

/// The text response, action, and edit state returned by [`render_field`].
pub struct SearchFieldResponse {
    /// Response for the text editor itself.
    pub text: Response,
    /// A button or keyboard action requested this frame.
    pub action: Option<SearchAction>,
    /// Whether the query changed or was cleared.
    pub changed: bool,
    /// Zero-based current result index for filtered-list searches.
    pub current_index: Option<usize>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ResultCursor {
    query: String,
    current: Option<usize>,
}

impl ResultCursor {
    fn update(&mut self, query: &str, total: usize, action: Option<SearchAction>) -> Option<usize> {
        if action == Some(SearchAction::Clear) || query.is_empty() || total == 0 {
            self.query = query.to_owned();
            self.current = None;
            return None;
        }
        if self.query != query {
            self.query = query.to_owned();
            self.current = Some(0);
        } else if self.current.is_some_and(|index| index >= total) {
            self.current = Some(total - 1);
        }
        self.current = match (action, self.current) {
            (Some(SearchAction::Next), current) => Some(match current {
                Some(index) => (index + 1) % total,
                None => 0,
            }),
            (Some(SearchAction::Previous), current) => Some(match current {
                Some(index) => (index + total - 1) % total,
                None => total - 1,
            }),
            _ => self.current,
        };
        self.current
    }
}

/// Paints a monochrome magnifier inside `rect` (TASK-244/260).
///
/// Kept as a separate helper so the icon can be verified and reused without
/// standing up a whole search field. The circle sits in the upper-left of the
/// square and the handle runs to the lower-right, the usual magnifier silhouette.
fn paint_search_icon(painter: &egui::Painter, rect: egui::Rect, color: Color32) {
    crate::glyphs::magnifier(painter, rect, color);
}

/// A step button's chevron, painted over its (empty) button: dimmed when it
/// cannot step.
fn step_chevron(ui: &egui::Ui, response: &egui::Response, enabled: bool, dir: crate::glyphs::Dir) {
    let color = if !enabled {
        ui.visuals().weak_text_color().gamma_multiply(0.6)
    } else if response.hovered() {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().text_color()
    };
    crate::glyphs::chevron(
        ui.painter(),
        egui::Rect::from_center_size(response.rect.center(), egui::Vec2::splat(14.0)),
        dir,
        color,
    );
}

/// Render the common pill search field, counter, step buttons, and clear control.
///
/// Each panel owns its query and match state; this widget standardizes their
/// presentation and keyboard behavior without taking ownership of either.
pub fn render_field(
    ui: &mut Ui,
    query: &mut String,
    options: SearchFieldOptions<'_>,
) -> SearchFieldResponse {
    let height = options.height.max(ui.spacing().interact_size.y);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let visuals = ui.visuals();
    let accent = visuals.selection.bg_fill;
    let active = !query.is_empty();
    let radius = height * 0.5;
    ui.painter()
        .rect_filled(rect, radius, visuals.extreme_bg_color);
    ui.painter().rect_stroke(
        rect,
        radius,
        if active {
            Stroke::new(1.25, accent)
        } else {
            visuals.widgets.noninteractive.bg_stroke
        },
        egui::StrokeKind::Inside,
    );

    let mut inner = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(9.0, 0.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    inner.spacing_mut().item_spacing.x = 3.0;
    let weak = inner.visuals().weak_text_color();
    // Draw the magnifier as geometry, not the `⌕` (U+2315) text glyph: that
    // character is absent from the bundled fonts and rendered as a hollow
    // missing-glyph box in every search field (TASK-244/260). A painted circle
    // plus handle needs no font coverage and inherits the field's text color.
    let icon_size = 15.0;
    let (icon_rect, _) =
        inner.allocate_exact_size(egui::vec2(icon_size, icon_size), egui::Sense::hover());
    paint_search_icon(inner.painter(), icon_rect, weak);

    let counter = if active { options.count.label() } else { None };
    let counter_width = counter
        .as_deref()
        .map(|text| {
            inner
                .painter()
                .layout_no_wrap(
                    text.to_owned(),
                    FontId::proportional(11.0),
                    inner.visuals().weak_text_color(),
                )
                .size()
                .x
                + 4.0
        })
        .unwrap_or(0.0);
    let button_width = (height * 0.62).clamp(16.0, 21.0);
    let controls_width = counter_width
        + if active {
            button_width * 3.0 + inner.spacing().item_spacing.x * 3.0
        } else {
            0.0
        };
    let icon_gap = inner.spacing().item_spacing.x;
    let text_width = (inner.available_width() - controls_width - icon_gap).max(24.0);
    let text = inner.add_sized(
        egui::vec2(text_width, height - 6.0),
        egui::TextEdit::singleline(query)
            .hint_text(options.hint)
            .desired_width(text_width)
            .margin(egui::Margin::ZERO)
            .frame(egui::Frame::NONE)
            .vertical_align(egui::Align::Center),
    );
    let mut changed = text.changed();
    let mut action = None;
    if text.has_focus() {
        ui.input(|input| {
            if input.key_pressed(egui::Key::Escape) {
                action = Some(SearchAction::Clear);
            } else if options.count != SearchCount::Hidden && input.key_pressed(egui::Key::Enter) {
                action = Some(if input.modifiers.shift {
                    SearchAction::Previous
                } else {
                    SearchAction::Next
                });
            } else if options.count != SearchCount::Hidden && input.key_pressed(egui::Key::F3) {
                action = Some(SearchAction::Next);
            }
        });
    }
    if let Some(label) = counter {
        inner.label(
            egui::RichText::new(label)
                .size(11.0)
                .color(inner.visuals().weak_text_color()),
        );
    }
    if active && options.count != SearchCount::Hidden {
        let can_step = match options.count {
            SearchCount::Hits { total, .. } | SearchCount::Results(total) => total > 0,
            SearchCount::Hidden => false,
        };
        for (dir, hover, step) in [
            (
                crate::glyphs::Dir::Up,
                "Previous match",
                SearchAction::Previous,
            ),
            (crate::glyphs::Dir::Down, "Next match", SearchAction::Next),
        ] {
            let response = inner
                .add_enabled(
                    can_step,
                    egui::Button::new("")
                        .frame(false)
                        .min_size(egui::vec2(button_width, height - 6.0)),
                )
                .on_hover_text(hover);
            step_chevron(&inner, &response, can_step, dir);
            if response.clicked() {
                action = Some(step);
            }
        }
    }
    if active {
        let response = inner
            .add(
                egui::Button::new("")
                    .frame(false)
                    .min_size(egui::vec2(button_width, height - 6.0)),
            )
            .on_hover_text("Clear search");
        let color = if response.hovered() {
            inner.visuals().strong_text_color()
        } else {
            inner.visuals().text_color()
        };
        crate::glyphs::close_x(
            inner.painter(),
            egui::Rect::from_center_size(response.rect.center(), egui::Vec2::splat(9.0)),
            color,
        );
        if response.clicked() {
            action = Some(SearchAction::Clear);
        }
    }
    if action == Some(SearchAction::Clear) && !query.is_empty() {
        query.clear();
        changed = true;
        text.request_focus();
    } else if action.is_some() {
        text.request_focus();
    }
    let current_index = match options.count {
        SearchCount::Hidden => None,
        SearchCount::Hits { current, .. } => current,
        SearchCount::Results(total) => {
            let cursor_id = text.id.with("spatial_ui_kit_search_result_cursor");
            let mut cursor = ui
                .ctx()
                .data(|data| data.get_temp::<ResultCursor>(cursor_id))
                .unwrap_or_default();
            let current = cursor.update(query, total, action);
            ui.ctx()
                .data_mut(|data| data.insert_temp(cursor_id, cursor));
            current
        }
    };
    SearchFieldResponse {
        text,
        action,
        changed,
        current_index,
    }
}

/// Every non-overlapping case-insensitive occurrence of `query` in `text`.
/// ASCII folding preserves byte offsets, so returned ranges index `text`.
pub fn match_ranges(text: &str, query: &str) -> Vec<Range<usize>> {
    let query = query.trim().to_ascii_lowercase();
    if query.is_empty() {
        return Vec::new();
    }
    let folded = text.to_ascii_lowercase();
    let mut matches = Vec::new();
    let mut offset = 0;
    while offset <= folded.len() {
        let Some(relative) = folded[offset..].find(&query) else {
            break;
        };
        let start = offset + relative;
        let end = start + query.len();
        matches.push(start..end);
        offset = end;
    }
    matches
}

/// The first match, kept for consumers that only need a yes/no range.
pub fn match_range(text: &str, query: &str) -> Option<Range<usize>> {
    match_ranges(text, query).into_iter().next()
}

/// Build text that marks every occurrence and gives one occurrence a solid chip.
pub fn marked_job(
    text: &str,
    query: &str,
    current: Option<usize>,
    font: FontId,
    color: Color32,
    accent: Color32,
) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let ranges = match_ranges(text, query);
    let plain = egui::TextFormat::simple(font.clone(), color);
    if ranges.is_empty() {
        job.append(text, 0.0, plain);
        return job;
    }
    let soft = accent.gamma_multiply(0.25);
    let mut cursor = 0;
    for (index, range) in ranges.iter().enumerate() {
        if cursor < range.start {
            job.append(&text[cursor..range.start], 0.0, plain.clone());
        }
        let mut format = egui::TextFormat::simple(font.clone(), color);
        format.background = if current == Some(index) { accent } else { soft };
        format.underline = Stroke::new(1.0, accent);
        if current == Some(index) {
            format.color = contrast_color(accent);
        }
        job.append(&text[range.clone()], 0.0, format);
        cursor = range.end;
    }
    if cursor < text.len() {
        job.append(&text[cursor..], 0.0, plain);
    }
    job
}

fn contrast_color(color: Color32) -> Color32 {
    let luminance =
        0.2126 * color.r() as f32 + 0.7152 * color.g() as f32 + 0.0722 * color.b() as f32;
    if luminance > 140.0 {
        Color32::BLACK
    } else {
        Color32::WHITE
    }
}

/// Search-hit marking for a row in a settings content page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowMark {
    /// The query to chip in this row's text; empty for unmatched rows.
    pub query: String,
    /// Whether this row contains the current hit.
    pub current: bool,
    /// Scroll this row into view once it is drawn.
    pub scroll_to: bool,
    /// Whether this row has a hit.
    pub hit: bool,
    /// Whether text should step back while other rows are searched.
    pub dim: bool,
}

/// Reduce text emphasis for a row outside the active query without disabling it.
pub(crate) fn apply_row_dim(ui: &mut Ui, mark: Option<&RowMark>) {
    if mark.is_some_and(|mark| mark.dim) {
        let color = ui.visuals().weak_text_color();
        ui.visuals_mut().override_text_color = Some(color);
    }
}

/// Reserve a painter slot so the row's search treatment can be drawn behind it.
pub fn reserve_row_band(ui: &Ui) -> egui::layers::ShapeIdx {
    ui.painter().add(egui::Shape::Noop)
}

/// Paint the warm row band, leading hit bar, and current-hit outline.
pub fn paint_row_band(ui: &Ui, slot: egui::layers::ShapeIdx, rect: egui::Rect, mark: &RowMark) {
    let accent = ui.visuals().selection.bg_fill;
    let rect = rect.expand2(egui::vec2(6.0, 2.0));
    let mut shapes = Vec::new();
    if mark.hit {
        shapes.push(egui::Shape::rect_filled(
            rect,
            4.0,
            accent.gamma_multiply(if mark.current { 0.20 } else { 0.12 }),
        ));
        shapes.push(egui::Shape::rect_filled(
            egui::Rect::from_min_max(
                rect.min,
                egui::pos2(
                    rect.min.x + if mark.current { 5.0 } else { 3.0 },
                    rect.max.y,
                ),
            ),
            2.0,
            accent,
        ));
    }
    if mark.current {
        shapes.push(egui::Shape::rect_stroke(
            rect,
            4.0,
            Stroke::new(1.0, accent),
            egui::StrokeKind::Inside,
        ));
    }
    ui.painter().set(slot, egui::Shape::Vec(shapes));
    if mark.scroll_to {
        ui.scroll_to_rect(rect, Some(egui::Align::Center));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        marked_job, match_ranges, render_field, ResultCursor, SearchAction, SearchCount,
        SearchFieldOptions,
    };
    use egui::{Color32, FontId, Pos2, Rect};

    /// TASK-244/260: the magnifier is drawn as a circle plus a handle line,
    /// never as a text glyph (U+2315 was missing from the bundled fonts and
    /// rendered as an empty box).
    #[test]
    fn search_icon_is_painted_as_shapes_not_text() {
        fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
            match shape {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                egui::Shape::Circle(_) => out.push("circle".into()),
                egui::Shape::LineSegment { .. } => out.push("line".into()),
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let mut query = String::new();
        let mut render: &mut dyn FnMut(&mut egui::Ui) = &mut |ui| {
            render_field(
                ui,
                &mut query,
                SearchFieldOptions {
                    hint: "Search",
                    height: 24.0,
                    count: SearchCount::Hidden,
                },
            );
        };
        let out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(400.0, 200.0))),
                ..Default::default()
            },
            &mut render,
        );
        let mut painted = Vec::new();
        for c in &out.shapes {
            walk(&c.shape, &mut painted);
        }
        assert!(painted.iter().any(|s| s == "circle"), "magnifier circle");
        assert!(painted.iter().any(|s| s == "line"), "magnifier handle");
        assert!(
            !painted.iter().any(|s| s.contains('\u{2315}')),
            "U+2315 must not be painted as text: {painted:?}"
        );
    }

    #[test]
    fn finds_every_non_overlapping_case_insensitive_occurrence() {
        assert_eq!(match_ranges("Snap snap points", "SNAP"), vec![0..4, 5..9]);
        assert!(match_ranges("nothing", "").is_empty());
    }

    #[test]
    fn marks_each_match_and_distinguishes_the_current_occurrence() {
        let job = marked_job(
            "Snap snap points",
            "snap",
            Some(1),
            FontId::proportional(12.0),
            Color32::WHITE,
            Color32::from_rgb(255, 170, 40),
        );
        assert_eq!(job.sections.len(), 4);
        assert_eq!(
            job.sections[0].format.background,
            Color32::from_rgb(255, 170, 40).gamma_multiply(0.25)
        );
        assert_eq!(
            job.sections[2].format.background,
            Color32::from_rgb(255, 170, 40)
        );
    }

    #[test]
    fn counters_distinguish_filtered_rows_from_document_hits() {
        assert_eq!(
            SearchCount::Results(6).label().as_deref(),
            Some("6 results")
        );
        assert_eq!(
            SearchCount::Hits {
                current: Some(2),
                total: 21
            }
            .label()
            .as_deref(),
            Some("3 of 21")
        );
        assert_eq!(
            SearchCount::Results(0).label().as_deref(),
            Some("No matches")
        );
    }

    #[test]
    fn filtered_result_cursor_resets_and_wraps() {
        let mut cursor = ResultCursor::default();
        assert_eq!(cursor.update("snap", 3, None), Some(0));
        assert_eq!(cursor.update("snap", 3, Some(SearchAction::Next)), Some(1));
        assert_eq!(
            cursor.update("snap", 3, Some(SearchAction::Previous)),
            Some(0)
        );
        assert_eq!(
            cursor.update("snap", 3, Some(SearchAction::Previous)),
            Some(2)
        );
        assert_eq!(cursor.update("none", 0, None), None);
        assert_eq!(cursor.update("snap", 3, None), Some(0));
    }

    #[test]
    fn dimming_a_nonmatch_changes_text_emphasis_but_keeps_controls_enabled() {
        let ctx = egui::Context::default();
        let mark = super::RowMark {
            query: String::new(),
            current: false,
            scroll_to: false,
            hit: false,
            dim: true,
        };
        let mut enabled = false;
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 120.0),
                )),
                ..Default::default()
            },
            |ui| {
                let expected = ui.visuals().weak_text_color();
                super::apply_row_dim(ui, Some(&mark));
                assert_eq!(ui.visuals().override_text_color, Some(expected));
                enabled = ui.is_enabled();
            },
        );
        assert!(enabled);
    }
}
