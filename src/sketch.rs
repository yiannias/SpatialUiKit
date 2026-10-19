//! Sketch-style widgets: a selectable list with thumbnails, framed regions,
//! pill buttons and the hover actions on a preview tile.
//!
//! Generic over the host: nothing here names a document, a style or an ECS
//! type, so any app can draw the same list, tile and button chrome. Callers
//! that need a renderer-specific preview keep that half in their own module
//! and pass in a paint closure.

use egui::{Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

/// Colors taken from the sketch's fills and borders. Color stays out of the
/// chrome: only previews, color chips and the selection use it.
pub mod palette {
    use egui::Color32;
    /// Window background (sketch `1E1E23`).
    pub const WINDOW_BG: Color32 = Color32::from_rgb(0x1E, 0x1E, 0x23);
    /// Frame outlines (sketch `3C3C3D`).
    pub const FRAME: Color32 = Color32::from_rgb(0x3C, 0x3C, 0x3D);
    /// Headings and muted labels (sketch `828282`).
    pub const MUTED: Color32 = Color32::from_rgb(0x82, 0x82, 0x82);
    /// Values, style names and field text.
    pub const BRIGHT: Color32 = Color32::WHITE;
    /// Field outlines (sketch `808080`).
    pub const FIELD_BORDER: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);
    /// Secondary button fill. The sketch's `828282` is darkened a step so
    /// white button text keeps contrast.
    pub const BUTTON: Color32 = Color32::from_rgb(0x5A, 0x5A, 0x5E);
    /// Primary button fill and the list selection bar (sketch `557EAF`).
    pub const PRIMARY: Color32 = Color32::from_rgb(0x55, 0x7E, 0xAF);
    /// The hover pill on a preview tile (sketch `6B6B6B`, near-black edge).
    pub const PILL: Color32 = Color32::from_rgb(0x6B, 0x6B, 0x6B);
    /// Outline of the hover pill, near-black so it reads on the tile.
    pub const PILL_EDGE: Color32 = Color32::from_rgb(0x05, 0x05, 0x05);
    /// Tile and field backgrounds.
    pub const TILE_BG: Color32 = Color32::from_rgb(0x16, 0x16, 0x1A);
}

/// The sketch's text sizes are 20 pt on a 1618-pt-wide window; drawn at full
/// size they crowd the 24 pt row pitch the sketch also uses, so text is set a
/// little smaller than drawn. Geometry is not scaled by this.
const FONT_RATIO: f32 = 0.95;

/// Maps the sketch's coordinate grid onto the window.
///
/// Built once per frame from the window's content rect by [`Self::fit`]. The
/// scale is uniform, so everything (fonts included) keeps the sketch's
/// proportions at any window size.
#[derive(Clone, Copy, Debug)]
pub struct SketchFrame {
    /// Top-left corner of the sketch, in screen points.
    pub origin: Pos2,
    /// Uniform factor from sketch units to screen points.
    pub scale: f32,
}

impl SketchFrame {
    /// Fits a `design`-sized sketch into `available`, anchored top-left.
    pub fn fit(available: Rect, design: Vec2) -> Self {
        let scale = (available.width() / design.x)
            .min(available.height() / design.y)
            .max(0.25);
        Self {
            origin: available.min,
            scale,
        }
    }
    /// Maps a sketch-space point to screen space.
    pub fn pos(&self, x: f32, y: f32) -> Pos2 {
        self.origin + Vec2::new(x, y) * self.scale
    }
    /// Maps a sketch-space rectangle (x, y, width, height) to screen space.
    pub fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(self.pos(x, y), Vec2::new(w, h) * self.scale)
    }
    /// Scales a sketch-space length to screen points.
    pub fn len(&self, v: f32) -> f32 {
        v * self.scale
    }
    /// A proportional font for a sketch text size (20 for body, 14 for small).
    pub fn font(&self, sketch_size: f32) -> FontId {
        FontId::proportional(sketch_size * FONT_RATIO * self.scale)
    }
    /// Scales a sketch-space corner radius to a whole-pixel `CornerRadius`.
    pub fn radius(&self, r: f32) -> CornerRadius {
        CornerRadius::same(self.len(r).round().clamp(0.0, 255.0) as u8)
    }
}

/// A rounded frame with an optional heading sitting just above its top edge,
/// as the sketch draws its titled panels.
pub fn framed_region(ui: &egui::Ui, sk: &SketchFrame, frame: Rect, heading: Option<&str>) {
    let painter = ui.painter();
    painter.rect_stroke(
        frame,
        sk.radius(12.0),
        Stroke::new(1.0, palette::FRAME),
        StrokeKind::Inside,
    );
    if let Some(heading) = heading {
        painter.text(
            frame.left_top() + Vec2::new(sk.len(8.0), -sk.len(4.0)),
            Align2::LEFT_BOTTOM,
            heading,
            sk.font(20.0),
            palette::MUTED,
        );
    }
}

/// Shortens `text` to fit `max_width` by cutting out its middle
/// ("Very long hat….ttern name"), so both a style's family prefix and its
/// distinguishing suffix stay readable.
/// Middle-ellipsis truncation, as a pure function of a width measurer.
///
/// The head and the tail of the string stay visible and a single `…` stands in
/// for the middle, which is what Chris asked for (2026-09-30) and why this is
/// shared: a truncated tail hides *which* item it is, and in the Help command
/// reference and the Library Manager conflict view the ends are the identifying
/// parts. "Mac-style" and "Always 12345" survive; "SomeVeryLongCategoryName…"
/// would not.
///
/// **Separated from [`truncate_middle`] so the algorithm is testable.** The
/// painter-based version needs a live `egui::Ui` -- a `Context`, a pass, and in
/// practice a GPU -- so its behaviour could not be checked by a normal
/// `cargo test`. Passing the measurer in means the whole algorithm, including
/// the awkward cases, is covered by ordinary unit tests: see
/// `truncate_middle_with_keeps_both_ends` and friends.
///
/// There is exactly one implementation of this. Do not add a second truncator
/// for a new panel; pass a measurer here.
pub fn truncate_middle_with(text: &str, max_width: f32, width: impl Fn(&str) -> f32) -> String {
    if width(text) <= max_width {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut keep = chars.len();
    while keep > 1 {
        keep -= 1;
        let head = keep.div_ceil(2);
        let tail = keep / 2;
        let candidate: String = chars[..head]
            .iter()
            .chain(std::iter::once(&'…'))
            .chain(chars[chars.len() - tail..].iter())
            .collect();
        if width(&candidate) <= max_width {
            return candidate;
        }
    }
    "…".to_owned()
}

/// Truncate at the **end**, keeping a prefix: `"XREFRESHALL…"`, not `"XRE…XD"`.
///
/// The counterpart to [`truncate_middle_with`], for the columns where a middle
/// ellipsis is wrong. A hotkey or a command alias is a literal thing the reader
/// types or presses, so the *end* is the identifying part and must survive — but
/// unlike a filename it must also never be split through the middle, which is
/// what makes the middle form unusable here.
///
/// Exists because the Command Reference panel's first draft "clipped" overflowing cells by
/// painting the full string with a trailing `…` appended, which is not clipping
/// at all: the neighbour column's text was overpainted and the row read
/// `XREFRESHALLXDload`. Anything that has to fit in a cell needs a real
/// measurement here, not a shorter string.
pub fn truncate_end_with(text: &str, max_width: f32, width: impl Fn(&str) -> f32) -> String {
    if width(text) <= max_width {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut keep = chars.len();
    while keep > 1 {
        keep -= 1;
        let candidate: String = chars[..keep].iter().chain(std::iter::once(&'…')).collect();
        if width(&candidate) <= max_width {
            return candidate;
        }
    }
    "…".to_owned()
}

/// `truncate_middle_with`, measured with the real font through the painter.
pub fn truncate_middle(ui: &egui::Ui, text: &str, font: &FontId, max_width: f32) -> String {
    truncate_middle_with(text, max_width, |s| {
        ui.painter()
            .layout_no_wrap(s.to_owned(), font.clone(), Color32::WHITE)
            .size()
            .x
    })
}

/// Paint `text` left-aligned in `rect`, middle-truncated to fit, with the **full**
/// text on hover. Returns what was painted, so a caller can assert on it.
///
/// The tooltip is not optional decoration: a truncated string is unreadable by
/// construction, so a panel that truncates without it has hidden information with
/// no way back to it. Doing both in one function is what stops the second half
/// being forgotten at a call site.
pub fn paint_truncated_middle(
    ui: &egui::Ui,
    rect: Rect,
    text: &str,
    font: &FontId,
    color: Color32,
) -> String {
    let shown = truncate_middle(ui, text, font, rect.width());
    let galley = ui
        .painter()
        .layout_no_wrap(shown.clone(), font.clone(), color);
    ui.painter().galley(
        egui::pos2(rect.left(), rect.center().y - galley.size().y / 2.0),
        galley,
        color,
    );
    if shown != text {
        ui.interact(
            rect,
            ui.id().with(("truncated_middle", text)),
            Sense::hover(),
        )
        .on_hover_text(text.to_owned());
    }
    shown
}

/// How the style list shows its entries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StyleListMode {
    #[default]
    /// Plain text rows, one name per line.
    List,
    /// Thumbnail previews instead of text rows.
    Thumbnails,
}

/// The small "List ▾" / "Thumbnails ▾" pulldown in the top-right corner of a
/// style list frame, underlined as in the sketch.
pub fn view_mode_dropdown(
    ui: &mut egui::Ui,
    sk: &SketchFrame,
    rect: Rect,
    mode: &mut StyleListMode,
) {
    let response = ui.interact(rect, ui.id().with("style_list_mode"), Sense::click());
    let painter = ui.painter();
    let label = match mode {
        StyleListMode::List => "List",
        StyleListMode::Thumbnails => "Thumbnails",
    };
    let color = if response.hovered() {
        palette::BRIGHT
    } else {
        palette::MUTED
    };
    let arrow_w = sk.len(12.0);
    painter.text(
        Pos2::new(rect.right() - arrow_w - sk.len(10.0), rect.center().y),
        Align2::RIGHT_CENTER,
        label,
        sk.font(14.0),
        color,
    );
    let c = Pos2::new(rect.right() - arrow_w * 0.5 - sk.len(4.0), rect.center().y);
    let h = arrow_w * 0.35;
    painter.add(egui::Shape::convex_polygon(
        vec![
            c + Vec2::new(-h * 1.2, -h * 0.6),
            c + Vec2::new(h * 1.2, -h * 0.6),
            c + Vec2::new(0.0, h * 0.8),
        ],
        color,
        Stroke::NONE,
    ));
    painter.hline(
        rect.x_range(),
        rect.bottom(),
        Stroke::new(1.0, palette::FRAME),
    );
    egui::Popup::menu(&response).show(|ui| {
        ui.selectable_value(mode, StyleListMode::List, "List");
        ui.selectable_value(mode, StyleListMode::Thumbnails, "Thumbnails");
    });
}

/// The closure that paints a [`StyleListRow`]'s thumbnail into the given rect.
/// Named so the row's field type stays readable; the lifetime ties the closure
/// to whatever the host borrows while painting.
pub type ThumbnailPaint<'a> = Box<dyn Fn(&egui::Ui, Rect) + 'a>;

/// One entry in a [`style_list`].
pub struct StyleListRow<'a> {
    /// The name shown on the row.
    pub name: &'a str,
    /// Paints this row's thumbnail cell in [`StyleListMode::Thumbnails`], and is
    /// ignored in [`StyleListMode::List`].
    ///
    /// A closure rather than a swatch colour or a fixed preview type: a thumbnail
    /// is whatever makes a row recognisable at 72 pt, and only the panel that
    /// owns the row knows what that is. A hatch row paints its own line geometry;
    /// a text row paints a glyph run in its own typeface.
    ///
    /// `None` leaves the empty outlined square, which is what the sketch uses for
    /// the `none` and `...new` rows -- they name an absence, so there is nothing
    /// to draw.
    pub thumbnail: Option<ThumbnailPaint<'a>>,
    /// Draw the name in italics -- for an entry the user cannot change (a
    /// built-in).
    pub italic: bool,
}

impl<'a> StyleListRow<'a> {
    /// A row with no thumbnail of its own: `none`, `...new`, and any style whose
    /// preview cannot be built. Renders as the sketch's empty outlined square.
    pub fn plain(name: &'a str) -> Self {
        Self {
            name,
            thumbnail: None,
            italic: false,
        }
    }

    /// A row whose thumbnail the caller paints.
    pub fn with_thumbnail(name: &'a str, paint: impl Fn(&egui::Ui, Rect) + 'a) -> Self {
        Self {
            name,
            thumbnail: Some(Box::new(paint)),
            italic: false,
        }
    }

    /// This row's name in italics.
    pub fn italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }
}

/// What happened in a [`style_list`] this frame.
#[derive(Default)]
pub struct StyleListResponse {
    /// Index of the row clicked this frame.
    pub clicked: Option<usize>,
    /// Index of the row double-clicked this frame.
    pub double_clicked: Option<usize>,
    /// Index of the row right-clicked this frame.
    pub secondary_clicked: Option<usize>,
    /// First and last row the viewport showed, as indices into the `rows` slice
    /// [`style_list`] was given -- `None` for an empty list.
    ///
    /// Reported because a caller whose rows cost something to *prepare* needs to
    /// know what was on screen: painting alone tells it nothing, and every flick
    /// of the scrollbar lands on cells that were culled and so never prepared.
    pub shown: Option<(usize, usize)>,
}

/// The style list inside `frame`: one row per entry, the selected row on a
/// full-width blue bar, or a grid of swatches in thumbnail mode. Rows scroll
/// when they overflow the frame.
pub fn style_list(
    ui: &mut egui::Ui,
    sk: &SketchFrame,
    frame: Rect,
    rows: &[StyleListRow<'_>],
    selected: Option<usize>,
    mode: StyleListMode,
) -> StyleListResponse {
    let mut out = StyleListResponse::default();
    let font = sk.font(20.0);
    let inner = Rect::from_min_max(
        frame.min + Vec2::new(sk.len(8.0), sk.len(30.0)),
        frame.max - Vec2::new(sk.len(4.0), sk.len(8.0)),
    );
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
        egui::ScrollArea::vertical()
            .id_salt("style_list_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut visible: Option<(usize, usize)> = None;
                let mut note_visible = |index: usize| {
                    visible = Some(match visible {
                        Some((first, _)) => (first.min(index), index),
                        None => (index, index),
                    });
                };
                match mode {
                    StyleListMode::List => {
                        let row_h = sk.len(24.2);
                        let width = ui.available_width();
                        for (index, row) in rows.iter().enumerate() {
                            let (rect, response) =
                                ui.allocate_exact_size(Vec2::new(width, row_h), Sense::click());
                            if ui.is_rect_visible(rect) {
                                note_visible(index);
                            }
                            let is_selected = selected == Some(index);
                            if is_selected {
                                ui.painter().rect_filled(
                                    rect.shrink2(Vec2::new(sk.len(2.0), 0.0)),
                                    sk.radius(12.0),
                                    palette::PRIMARY,
                                );
                            } else if response.hovered() {
                                ui.painter().rect_filled(
                                    rect.shrink2(Vec2::new(sk.len(2.0), 0.0)),
                                    sk.radius(12.0),
                                    Color32::from_white_alpha(12),
                                );
                            }
                            let text_x = rect.left() + sk.len(10.0);
                            let shown = truncate_middle(
                                ui,
                                row.name,
                                &font,
                                rect.right() - text_x - sk.len(8.0),
                            );
                            if row.italic {
                                let galley =
                                    ui.painter()
                                        .layout_job(egui::text::LayoutJob::single_section(
                                            shown,
                                            egui::TextFormat {
                                                font_id: font.clone(),
                                                color: palette::BRIGHT,
                                                italics: true,
                                                ..Default::default()
                                            },
                                        ));
                                ui.painter().galley(
                                    Pos2::new(text_x, rect.center().y - galley.size().y / 2.0),
                                    galley,
                                    palette::BRIGHT,
                                );
                            } else {
                                ui.painter().text(
                                    Pos2::new(text_x, rect.center().y),
                                    Align2::LEFT_CENTER,
                                    shown,
                                    font.clone(),
                                    palette::BRIGHT,
                                );
                            }
                            if response.clicked() {
                                out.clicked = Some(index);
                            }
                            if response.double_clicked() {
                                out.double_clicked = Some(index);
                            }
                            if response.secondary_clicked() {
                                out.secondary_clicked = Some(index);
                            }
                            if row.name.chars().count() > 24 {
                                response.on_hover_text(row.name);
                            }
                        }
                    }
                    StyleListMode::Thumbnails => {
                        // Sketch: 72-pt squares on a 97 by 122 pitch, name below.
                        let cell = Vec2::new(sk.len(97.0), sk.len(122.0));
                        let swatch = sk.len(72.0);
                        let per_row = ((ui.available_width() / cell.x).floor() as usize).max(1);
                        let small = sk.font(14.0);
                        let left = ui.cursor().left();
                        for chunk_start in (0..rows.len()).step_by(per_row) {
                            let (band, _) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), cell.y),
                                Sense::hover(),
                            );
                            for (offset, row) in
                                rows[chunk_start..].iter().take(per_row).enumerate()
                            {
                                let index = chunk_start + offset;
                                let cell_rect = Rect::from_min_size(
                                    Pos2::new(left + cell.x * offset as f32, band.top()),
                                    cell,
                                );
                                // Rows the scroll viewport does not show are skipped
                                // whole -- no cell, no interact, no thumbnail
                                // closure. The ScrollArea would clip the output
                                // anyway, so this costs nothing visually; what it
                                // saves is the per-cell work, which for a caller
                                // whose closure resolves a preview means the
                                // difference between a dozen previews a frame and
                                // one per style in the document. A click cannot land
                                // on a cell that is off screen either, so the
                                // response is not being given up here.
                                if !ui.is_rect_visible(cell_rect) {
                                    continue;
                                }
                                note_visible(index);
                                let square = Rect::from_center_size(
                                    Pos2::new(
                                        cell_rect.center().x,
                                        cell_rect.top() + sk.len(8.0) + swatch / 2.0,
                                    ),
                                    Vec2::splat(swatch),
                                );
                                let response = ui.interact(
                                    cell_rect,
                                    ui.id().with(("style_thumb", index)),
                                    Sense::click(),
                                );
                                if let Some(paint) = &row.thumbnail {
                                    // Filled for the caller, as the fill path used to
                                    // do for itself: a consumer that only draws
                                    // glyphs still gets a legible cell behind them.
                                    // Clipping is left to the caller, which already
                                    // has the rect and the `Ui` it needs to set one.
                                    ui.painter().rect_filled(
                                        square,
                                        sk.radius(6.0),
                                        palette::TILE_BG,
                                    );
                                    paint(ui, square);
                                }
                                let is_selected = selected == Some(index);
                                ui.painter().rect_stroke(
                                    square,
                                    sk.radius(6.0),
                                    if is_selected {
                                        Stroke::new(sk.len(3.0), palette::PRIMARY)
                                    } else if response.hovered() {
                                        Stroke::new(1.0, palette::BRIGHT)
                                    } else {
                                        Stroke::new(1.0, palette::FIELD_BORDER)
                                    },
                                    StrokeKind::Inside,
                                );
                                let shown =
                                    truncate_middle(ui, row.name, &small, cell.x - sk.len(4.0));
                                ui.painter().text(
                                    Pos2::new(cell_rect.center().x, square.bottom() + sk.len(8.0)),
                                    Align2::CENTER_TOP,
                                    shown,
                                    small.clone(),
                                    palette::MUTED,
                                );
                                if response.clicked() {
                                    out.clicked = Some(index);
                                }
                                if response.double_clicked() {
                                    out.double_clicked = Some(index);
                                }
                                if response.secondary_clicked() {
                                    out.secondary_clicked = Some(index);
                                }
                                response.on_hover_text(row.name);
                            }
                        }
                    }
                }
                out.shown = visible;
            });
    });
    out
}

/// A check box that always draws its box: a rounded outline on the field
/// background, with a tick when checked. `ui.checkbox` takes its box colours
/// from the theme's inactive widget fill and stroke, which a dark theme can set
/// to nothing -- an unchecked box then vanishes and the row looks empty. Returns
/// the response; `changed()` is true on the frame the value flipped.
pub fn sketch_checkbox(ui: &mut egui::Ui, checked: &mut bool) -> egui::Response {
    let side = ui.spacing().icon_width.max(16.0);
    let (rect, mut response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    if response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    let border = if response.hovered() {
        palette::BRIGHT
    } else {
        palette::FIELD_BORDER
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, palette::TILE_BG);
    painter.rect_stroke(rect, 3.0, Stroke::new(1.0, border), StrokeKind::Inside);
    if *checked {
        let tick = Stroke::new((side * 0.11).max(1.5), palette::BRIGHT);
        let at = |x: f32, y: f32| rect.min + Vec2::new(x, y) * side;
        painter.line_segment([at(0.24, 0.52), at(0.43, 0.72)], tick);
        painter.line_segment([at(0.43, 0.72), at(0.78, 0.30)], tick);
    }
    response
}

/// Visual weight of a [`button_row`] button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    /// The ordinary grey button.
    Normal,
    /// The blue default action (Apply).
    Primary,
}

/// One rounded button in the sketch's style.
pub fn sketch_button(
    ui: &mut egui::Ui,
    sk: &SketchFrame,
    rect: Rect,
    label: &str,
    kind: ButtonKind,
    enabled: bool,
) -> egui::Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, ui.id().with(("sketch_button", label)), sense);
    let base = match kind {
        ButtonKind::Normal => palette::BUTTON,
        ButtonKind::Primary => palette::PRIMARY,
    };
    let fill = if !enabled {
        base.gamma_multiply(0.45)
    } else if response.is_pointer_button_down_on() {
        base.gamma_multiply(0.8)
    } else if response.hovered() {
        base.lerp_to_gamma(Color32::WHITE, 0.12)
    } else {
        base
    };
    ui.painter().rect_filled(rect, sk.radius(12.0), fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        sk.font(20.0),
        if enabled {
            palette::BRIGHT
        } else {
            palette::BRIGHT.gamma_multiply(0.45)
        },
    );
    response
}

/// Equal buttons laid across `rect` with `gap` sketch points between them.
/// Returns the index of the button clicked this frame. `enabled[i]` and
/// `kinds[i]` default to enabled / [`ButtonKind::Normal`] when short.
pub fn button_row(
    ui: &mut egui::Ui,
    sk: &SketchFrame,
    rect: Rect,
    gap: f32,
    labels: &[&str],
    kinds: &[ButtonKind],
    enabled: &[bool],
) -> Option<usize> {
    let n = labels.len() as f32;
    let gap = sk.len(gap);
    let w = (rect.width() - gap * (n - 1.0)) / n;
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(rect.left() + (w + gap) * i as f32, rect.top()),
            Vec2::new(w, rect.height()),
        );
        let kind = kinds.get(i).copied().unwrap_or(ButtonKind::Normal);
        let on = enabled.get(i).copied().unwrap_or(true);
        if sketch_button(ui, sk, r, label, kind, on).clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// The literal `=` or `+` between composition tiles, centered in `rect`.
pub fn composition_operator(ui: &egui::Ui, sk: &SketchFrame, rect: Rect, op: &str) {
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        op,
        sk.font(48.0 / FONT_RATIO),
        palette::BRIGHT,
    );
}

/// The hover button a tile offers ("Edit Color", "Edit Hatch", ...).
pub struct TileAction<'a> {
    /// The button's text, e.g. "Edit Hatch".
    pub label: &'a str,
    /// Whether the action can be taken right now.
    pub enabled: bool,
    /// Shown on hover when the action is disabled (e.g. "coming soon").
    pub disabled_hint: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The unchecked box is drawn: an outline and a fill, so a row with the
    /// box off does not look empty (Dimension Style Manager, 2026-10-19).
    #[test]
    fn an_unchecked_sketch_checkbox_still_draws_its_box() {
        let ctx = egui::Context::default();
        let mut checked = false;
        let out = ctx.run_ui(egui::RawInput::default(), |ui| {
            sketch_checkbox(ui, &mut checked);
        });
        let mut boxes = 0;
        fn walk(shape: &egui::Shape, boxes: &mut u32) {
            match shape {
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, boxes)),
                egui::Shape::Rect(r) if r.stroke.width > 0.0 => *boxes += 1,
                _ => {}
            }
        }
        for clipped in &out.shapes {
            walk(&clipped.shape, &mut boxes);
        }
        assert_eq!(boxes, 1);
        assert!(!checked);
    }

    #[test]
    fn sketch_frame_fits_uniformly_and_anchors_top_left() {
        let avail = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(809.0, 1000.0));
        let sk = SketchFrame::fit(avail, Vec2::new(1618.0, 873.0));
        assert!((sk.scale - 0.5).abs() < 1e-6);
        assert_eq!(sk.pos(0.0, 0.0), Pos2::new(10.0, 20.0));
        assert_eq!(
            sk.rect(100.0, 100.0, 20.0, 10.0).size(),
            Vec2::new(10.0, 5.0)
        );
    }

    #[test]
    fn truncate_middle_keeps_both_ends() {
        let ctx = egui::Context::default();
        let mut result = String::new();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let font = FontId::proportional(14.0);
            let full = "Very long hatch pattern name";
            let width = ui
                .painter()
                .layout_no_wrap(full.to_owned(), font.clone(), Color32::WHITE)
                .size()
                .x;
            result = truncate_middle(ui, full, &font, width * 0.6);
            assert_eq!(truncate_middle(ui, full, &font, width + 1.0), full);
        });
        assert!(result.starts_with("Very"), "{result}");
        assert!(result.ends_with("name"), "{result}");
        assert!(result.contains('…'));
    }

    // --- truncate_middle_with: the same algorithm, testable without a GPU ----
    //
    // A measurer that gives every character a fixed width, so the arithmetic in
    // the truncation can be checked exactly. `egui`'s real metrics are not needed
    // to know that a 10-unit budget keeps 3 characters and an ellipsis.
    fn fixed() -> impl Fn(&str) -> f32 {
        |s: &str| s.chars().count() as f32
    }

    #[test]
    fn end_truncation_keeps_the_start_and_never_the_end() {
        // The point of this variant: the identifying part of a chord or alias is
        // its tail, but the part that must not be *split* is also the tail, so we
        // keep a prefix and mark the loss rather than inventing a middle form.
        // The mark costs a character, as it does in the middle form: 6 wide
        // keeps 5 letters, 11 wide keeps 10.
        assert_eq!(truncate_end_with("XREFRESHALLXD", 6.0, fixed()), "XREFR…");
        assert_eq!(
            truncate_end_with("XREFRESHALLXD", 11.0, fixed()),
            "XREFRESHAL…"
        );
    }

    #[test]
    fn end_truncation_leaves_short_text_alone_and_fits_every_budget() {
        assert_eq!(truncate_end_with("REC", 3.0, fixed()), "REC");
        // The property a cell actually depends on: whatever the budget, the
        // result fits. A caller that ignores the return value and paints the
        // original string is what produced the `XREFRESHALLXDload` overlap.
        for budget in 1..="XREFRESHALLXD".chars().count() {
            let budget = budget as f32;
            let out = truncate_end_with("XREFRESHALLXD", budget, fixed());
            assert!(
                out.chars().count() as f32 <= budget,
                "{out:?} is wider than a budget of {budget}"
            );
        }
    }

    #[test]
    fn end_truncation_degrades_to_a_bare_ellipsis() {
        // A budget too small for even one character plus the mark.
        assert_eq!(truncate_end_with("ABC", 0.5, fixed()), "…");
    }

    #[test]
    fn truncation_leaves_short_text_alone() {
        // At or under the budget, untouched.
        assert_eq!(truncate_middle_with("Note", 4.0, fixed()), "Note");
        assert_eq!(truncate_middle_with("Note", 10.0, fixed()), "Note");
        // One character over, it truncates -- and the ellipsis *costs* a
        // character, so a 3-wide budget keeps 2 letters, not 3.
        assert_eq!(truncate_middle_with("Note", 3.0, fixed()), "N…e");
    }

    #[test]
    fn truncation_keeps_the_head_and_the_tail_with_one_ellipsis_between() {
        // 4 units is 3 characters plus the ellipsis. The head is the larger
        // half, so the end that identifies the item survives.
        let out = truncate_middle_with("CategoryName", 4.0, fixed());
        assert_eq!(out, "Ca…e");
        assert_eq!(out.chars().filter(|c| *c == '…').count(), 1);
        assert!(out.starts_with('C'), "{out}");
        assert!(out.ends_with('e'), "{out}");
    }

    #[test]
    fn truncation_never_returns_something_wider_than_the_budget() {
        // Sweep every budget: the result must fit, or the panel clips it and the
        // "fits" claim is a lie. This is the property that actually matters --
        // the exact split is an implementation detail.
        let text = "AVeryLongCategoryNameIndeed";
        for budget in 1..=text.chars().count() {
            let budget = budget as f32;
            let out = truncate_middle_with(text, budget, fixed());
            assert!(
                out.chars().count() as f32 <= budget,
                "budget {budget} produced {out:?} ({} chars)",
                out.chars().count()
            );
        }
    }

    #[test]
    fn an_impossible_budget_degrades_to_a_bare_ellipsis_rather_than_looping() {
        // One unit: only the ellipsis fits, and the loop must terminate rather
        // than walk off the start of the string.
        assert_eq!(truncate_middle_with("CategoryName", 1.0, fixed()), "…");
    }

    #[test]
    fn truncation_preserves_every_character_of_the_original() {
        // The head and tail are taken from the real string, so no character can
        // be mangled or duplicated at the seam.
        // 6 units is 5 characters plus the ellipsis, split 3 head / 2 tail --
        // the head is favoured, so the identifying start of the name survives.
        let out = truncate_middle_with("SomeCategoryName", 6.0, fixed());
        assert_eq!(out, "Som…me");
        let head_tail: String = out.chars().filter(|c| *c != '…').collect();
        assert!(
            "SomeCategoryName".starts_with(&head_tail[..3]),
            "{out:?} head is not a prefix of the original"
        );
        assert!(
            "SomeCategoryName".ends_with(&head_tail[head_tail.len() - 2..]),
            "{out:?} tail is not a suffix of the original"
        );
    }

    #[test]
    fn multibyte_text_is_truncated_by_character_not_by_byte() {
        // Counting bytes here would split a code point and panic on slicing.
        let out = truncate_middle_with("Ω≈ç√∫˜µ", 4.0, fixed());
        assert!(out.starts_with('Ω'), "{out}");
        assert!(out.ends_with('µ'), "{out}");
        assert!(out.contains('…'));
    }

    /// A list row's thumbnail is whatever the owning panel says it is, and this
    /// stops compiling the moment that stops being true.
    ///
    /// The row used to carry a swatch colour and a fill-only preview type, so a
    /// text row passed neither and rendered as an empty square -- a grid of holes
    /// that looked like a finished feature. A test that only ever paints a fill
    /// could not tell that seam from a coincidence.
    #[test]
    fn style_list_row_accepts_a_non_fill_thumbnail_paint() {
        // `Fn`, not `FnMut`: the row's paint is stored and may be called any
        // number of times, so it cannot mutate captured state directly.
        let painted = std::cell::Cell::new(0usize);
        let rows = [StyleListRow::with_thumbnail("Serif Note", |ui, rect| {
            painted.set(painted.get() + 1);
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                "Aa",
                egui::FontId::proportional(20.0),
                palette::BRIGHT,
            );
        })];
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let sk = SketchFrame::fit(ui.available_rect_before_wrap(), Vec2::new(300.0, 300.0));
            let frame = Rect::from_min_size(Pos2::ZERO, Vec2::new(280.0, 260.0));
            let _ = style_list(ui, &sk, frame, &rows, None, StyleListMode::Thumbnails);
        });
        assert_eq!(
            painted.get(),
            1,
            "the caller's thumbnail paint must run in thumbnail mode"
        );
    }

    /// The row's paint runs in **thumbnail mode only**. In list mode a row is a
    /// name and a selection bar, so painting a glyph into it would draw over the
    /// text -- the seam has to be inert in the mode that does not use it.
    #[test]
    fn style_list_row_thumbnail_does_not_paint_in_list_mode() {
        let painted = std::cell::Cell::new(0usize);
        let row = StyleListRow::with_thumbnail("Serif Note", |_ui, _rect| {
            painted.set(painted.get() + 1);
        });
        let rows = [row];
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
            let sk = SketchFrame::fit(ui.available_rect_before_wrap(), Vec2::new(300.0, 300.0));
            let frame = Rect::from_min_size(Pos2::ZERO, Vec2::new(280.0, 260.0));
            let _ = style_list(ui, &sk, frame, &rows, Some(0), StyleListMode::List);
        });
        assert_eq!(
            painted.get(),
            0,
            "list mode must not run the thumbnail paint"
        );
    }

    /// `plain` is how `none` and `...new` are built, and it must leave the
    /// sketch's empty outlined square rather than nothing at all -- an unpainted
    /// cell with no stroke reads as a gap in the grid.
    #[test]
    fn a_plain_row_has_no_thumbnail_paint() {
        assert!(StyleListRow::plain("...new").thumbnail.is_none());
        assert!(StyleListRow::plain("none").name == "none");
    }

    /// The thumbnail grid is the one list where every row's cell is a real paint
    /// job, so a Maple-sized document pays it for hundreds of rows per frame --
    /// nearly all of them scrolled out of sight inside the ScrollArea. Only the
    /// cells the viewport actually shows may paint.
    ///
    /// The ScrollArea's clip rect already discards the off-screen output, so this
    /// drops work rather than pixels, and the visible set is not something to
    /// re-derive: `Ui::is_rect_visible` asks egui the same question the painter
    /// would.
    #[test]
    fn scrolled_out_thumbnail_cells_are_never_painted() {
        let names: Vec<String> = (0..200).map(|i| format!("Style {i:03}")).collect();
        let painted = std::cell::RefCell::new(Vec::new());
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 300.0));
        let ctx = egui::Context::default();
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ui| {
                let sk = SketchFrame::fit(screen, screen.size());
                // 400x200 inside a 600x300 screen: 4 columns of 97 pt and bands
                // 122 pt tall, so two bands fit and 48 more are off-screen.
                let frame = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(400.0, 200.0));
                let rows: Vec<StyleListRow<'_>> = names
                    .iter()
                    .map(|name| {
                        let painted = &painted;
                        StyleListRow::with_thumbnail(name, move |_ui, _rect| {
                            painted.borrow_mut().push((*name).to_owned());
                        })
                    })
                    .collect();
                let _ = style_list(ui, &sk, frame, &rows, None, StyleListMode::Thumbnails);
            },
        );
        let painted = painted.into_inner();
        assert!(
            !painted.is_empty(),
            "the rows in view must still paint -- a grid that culls everything is not culling"
        );
        assert_eq!(
            painted.first().map(String::as_str),
            Some("Style 000"),
            "the top-left visible cell must paint; a visibility test that was inverted, \
             or applied to the wrong rect, would skip it and paint somewhere else"
        );
        assert!(
            painted.len() <= 12,
            "only cells inside the scroll viewport may paint, but {} of 200 did -- 4 columns \
             and at most 3 bands of 122 pt fit the 152-pt-tall viewport",
            painted.len()
        );
    }

    /// The list knows which rows the viewport showed, and a caller whose rows
    /// cost something to *prepare* needs to be told which those were. Without it
    /// the caller can only prepare what is painted, and every flick of the
    /// scrollbar lands on cells that were culled and so never prepared.
    #[test]
    fn the_list_reports_the_rows_the_viewport_showed() {
        let names: Vec<String> = (0..200).map(|i| format!("Style {i:03}")).collect();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 300.0));
        let ctx = egui::Context::default();
        let reported = std::cell::Cell::new(None);
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |ui| {
                let sk = SketchFrame::fit(screen, screen.size());
                let frame = Rect::from_min_size(Pos2::new(10.0, 10.0), Vec2::new(400.0, 200.0));
                let rows: Vec<StyleListRow<'_>> =
                    names.iter().map(|name| StyleListRow::plain(name)).collect();
                reported
                    .set(style_list(ui, &sk, frame, &rows, None, StyleListMode::Thumbnails).shown);
            },
        );
        let (first, last) = reported
            .get()
            .expect("200 rows in a 152-pt viewport show some of them");
        assert_eq!(first, 0, "an unscrolled list starts at the first row");
        assert!(last > first, "a screenful is more than one row");
        assert!(
            last < names.len() - 1,
            "the reported range must stop where the viewport does -- reported {last} of {}",
            names.len() - 1
        );
    }
}
