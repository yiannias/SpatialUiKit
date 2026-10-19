//! A font picker: the closed control looks like every other dropdown (one
//! dropdown style app-wide); opened, it is a searchable list of typefaces in
//! which every row shows a sample **in its own face**.
//!
//! Modelled on what design tools converge on (Figma, Word, Pages, Illustrator,
//! Affinity, Lunacy):
//!
//! - **Type to search.** The control row becomes a search field the moment the
//!   list opens; typing filters by name (anywhere in the name, any case), so a
//!   list of several hundred installed fonts is a few keystrokes deep.
//! - **Groups with headings**: the caller's special choices first (no heading),
//!   then *Recently used* (this session, kept per app across pickers), *Used in
//!   this drawing*, *Built in*, *Installed on this computer*.
//! - **Name in the interface font, sample in the face.** Word draws each name
//!   in its own face; that turns a symbol face (AMGDT, Wingdings) into an
//!   unreadable row of symbols. Pages and Affinity keep the name readable and
//!   show the face beside it, which is what this does: the name is always
//!   legible, the sample on the right is the truth about the face.
//! - **Keyboard**: arrows / Page Up / Page Down move, Enter picks, Escape
//!   closes; the search field keeps focus throughout.
//!
//! Performance with hundreds of faces: rows are virtualised (only the visible
//! ones are laid out), the face of a row is loaded only when that row is
//! visible ([`crate::runtime_fonts`], with its per-frame budget), the choice
//! list is built once when the list opens (the caller's builder is not called
//! while it is closed), and filtering runs only when the search text changes.

use crate::dropdown::{choose_direction, paint_arrow, paint_text, row_rounding, DropdownStyle};
use crate::runtime_fonts::{self, FaceState, FontBytes};
use egui::{
    Align2, Color32, FontId, Id, Key, Modifiers, Order, Pos2, Rect, Sense, Stroke, StrokeKind, Ui,
    UiBuilder, Vec2, WidgetInfo, WidgetType,
};
use std::sync::Arc;

/// Which heading a choice is listed under.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FontGroup {
    /// The caller's own non-font choices ("Each text's own"), listed first
    /// without a heading.
    Special,
    /// Faces the document already uses.
    Document,
    /// Faces that ship with the app.
    BuiltIn,
    /// Faces installed on this computer.
    System,
}

impl FontGroup {
    /// The heading shown above the group, in plain words.
    pub fn heading(self) -> Option<&'static str> {
        match self {
            FontGroup::Special => None,
            FontGroup::Document => Some("Used in this drawing"),
            FontGroup::BuiltIn => Some("Built in"),
            FontGroup::System => Some("Installed on this computer"),
        }
    }
}

/// The heading of the session's recently picked faces.
pub const RECENT_HEADING: &str = "Recently used";

/// How many recently picked faces are kept.
pub const RECENT_LIMIT: usize = 5;

/// One entry in the list.
#[derive(Clone, Debug, PartialEq)]
pub struct FontChoice {
    /// What the picker reports when this is picked, and what the current
    /// selection is compared against.
    pub key: String,
    /// The name shown, in the interface font.
    pub label: String,
    /// The face whose sample the row shows (`None`: no sample, e.g. a special
    /// choice that is not a face).
    pub face: Option<String>,
    /// The heading it is listed under.
    pub group: FontGroup,
}

impl FontChoice {
    /// A face listed under `group`, keyed and labelled by its family name.
    pub fn face(name: impl Into<String>, group: FontGroup) -> Self {
        let name = name.into();
        Self {
            key: name.clone(),
            label: name.clone(),
            face: Some(name),
            group,
        }
    }

    /// A non-face choice listed first ("Each text's own").
    pub fn special(key: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            label: label.into(),
            face: None,
            group: FontGroup::Special,
        }
    }
}

/// What [`FontPicker::show`] returns.
pub struct FontPickerResponse {
    /// The closed control's response.
    pub response: egui::Response,
    /// The key of the choice picked this frame.
    pub chosen: Option<String>,
    /// The open box, when open (search row included).
    pub popup_rect: Option<Rect>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Row {
    Heading(Option<FontGroup>),
    Choice(usize),
}

#[derive(Clone, Default)]
struct State {
    open: bool,
    query: String,
    /// The query `rows` was built for (`None`: not built).
    rows_for: Option<String>,
    choices: Arc<Vec<FontChoice>>,
    lower: Arc<Vec<String>>,
    rows: Arc<Vec<Row>>,
    /// Index into `rows` of the keyboard highlight.
    cursor: Option<usize>,
    /// The highlight is outlined only after a key moved it, as in the dropdown.
    kbd: bool,
    scroll_to_cursor: bool,
    seen: u64,
}

fn recent_id() -> Id {
    Id::new("spatial_ui_kit_font_picker_recent")
}

/// The session's recently picked faces, newest first.
pub fn recent_faces(ctx: &egui::Context) -> Vec<String> {
    ctx.data(|d| d.get_temp::<Vec<String>>(recent_id()))
        .unwrap_or_default()
}

/// Records `face` as the most recently picked.
pub fn note_recent(ctx: &egui::Context, face: &str) {
    ctx.data_mut(|d| {
        let list = d.get_temp_mut_or_default::<Vec<String>>(recent_id());
        list.retain(|f| f != face);
        list.insert(0, face.to_owned());
        list.truncate(RECENT_LIMIT);
    });
}

/// Builds the visible rows for `query`: special choices, then (without a
/// query) the recent faces, then each group under its heading. Rows a query
/// removes take their now-empty heading with them.
fn build_rows(
    choices: &[FontChoice],
    lower: &[String],
    query: &str,
    recent: &[String],
) -> Vec<Row> {
    let q = query.trim().to_lowercase();
    let matches = |i: usize| q.is_empty() || lower[i].contains(&q);
    let mut rows = Vec::new();
    for (i, c) in choices.iter().enumerate() {
        if c.group == FontGroup::Special && matches(i) {
            rows.push(Row::Choice(i));
        }
    }
    if q.is_empty() {
        let recent_rows: Vec<Row> = recent
            .iter()
            .filter_map(|r| {
                choices
                    .iter()
                    .position(|c| c.group != FontGroup::Special && c.face.as_deref() == Some(r))
            })
            .map(Row::Choice)
            .collect();
        if !recent_rows.is_empty() {
            rows.push(Row::Heading(None));
            rows.extend(recent_rows);
        }
    }
    for group in [FontGroup::Document, FontGroup::BuiltIn, FontGroup::System] {
        let start = rows.len();
        rows.push(Row::Heading(Some(group)));
        for (i, c) in choices.iter().enumerate() {
            if c.group == group && matches(i) {
                rows.push(Row::Choice(i));
            }
        }
        if rows.len() == start + 1 {
            rows.pop();
        }
    }
    rows
}

/// The next choice row from `from` in direction `delta` (skipping headings),
/// clamped to the ends.
fn step(rows: &[Row], from: Option<usize>, delta: isize) -> Option<usize> {
    let choices: Vec<usize> = (0..rows.len())
        .filter(|&i| matches!(rows[i], Row::Choice(_)))
        .collect();
    if choices.is_empty() {
        return None;
    }
    let pos = match from.and_then(|f| choices.iter().position(|&c| c == f)) {
        Some(p) => (p as isize + delta).clamp(0, choices.len() as isize - 1) as usize,
        None if delta >= 0 => 0,
        None => choices.len() - 1,
    };
    Some(choices[pos])
}

/// The largest font size, at most `max`, at which `text` drawn in `family`
/// fits inside `area` -- measured, not guessed, because faces differ wildly
/// (a symbol face's glyphs can be far wider than a text face's at one size).
/// The caller still clips to the area: a face may draw outside its own metrics.
pub fn fitted_size(
    ctx: &egui::Context,
    text: &str,
    family: egui::FontFamily,
    max: f32,
    area: Vec2,
) -> f32 {
    if text.is_empty() || max <= 0.0 {
        return max.max(1.0);
    }
    let size = ctx.fonts_mut(|f| {
        f.layout_no_wrap(text.to_owned(), FontId::new(max, family), Color32::WHITE)
            .size()
    });
    let scale = (area.x / size.x.max(1.0))
        .min(area.y / size.y.max(1.0))
        .min(1.0);
    (max * scale).max(1.0)
}

/// The searchable font list. Build, configure, [`Self::show`].
pub struct FontPicker<'a> {
    id_salt: Id,
    selected_key: Option<String>,
    selected_label: String,
    selected_face: Option<String>,
    choices: Option<Box<dyn FnOnce() -> Vec<FontChoice> + 'a>>,
    load: &'a dyn Fn(&str) -> Option<FontBytes>,
    rect: Option<Rect>,
    width: f32,
    font: Option<FontId>,
    row_height: Option<f32>,
    sample: String,
    visible_rows: f32,
}

impl<'a> FontPicker<'a> {
    /// A picker with a unique id source; `load` hands over a face's bytes by
    /// family name (`None`: not available on this machine).
    pub fn new(
        id_salt: impl std::hash::Hash + std::fmt::Debug,
        load: &'a dyn Fn(&str) -> Option<FontBytes>,
    ) -> Self {
        Self {
            id_salt: Id::new(id_salt),
            selected_key: None,
            selected_label: String::new(),
            selected_face: None,
            choices: None,
            load,
            rect: None,
            width: 220.0,
            font: None,
            row_height: None,
            sample: "Ag".to_owned(),
            visible_rows: 12.0,
        }
    }

    /// The current choice: its key, the label the closed control shows, and
    /// the face of its sample (`None` for a special choice).
    pub fn selected(mut self, key: &str, label: &str, face: Option<&str>) -> Self {
        self.selected_key = Some(key.to_owned());
        self.selected_label = label.to_owned();
        self.selected_face = face.map(str::to_owned);
        self
    }

    /// The choices, built only on the frame the list opens.
    pub fn choices(mut self, build: impl FnOnce() -> Vec<FontChoice> + 'a) -> Self {
        self.choices = Some(Box::new(build));
        self
    }

    /// Draws the closed control into `rect` instead of allocating one.
    pub fn at_rect(mut self, rect: Rect) -> Self {
        self.rect = Some(rect);
        self
    }

    /// Width of the closed control when allocated in the layout.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Text font of the control and the rows (default: the button style).
    pub fn font(mut self, font: FontId) -> Self {
        self.font = Some(font);
        self
    }

    /// Height of one list row.
    pub fn row_height(mut self, h: f32) -> Self {
        self.row_height = Some(h);
        self
    }

    /// The sample each row draws in its face (default `Ag`).
    pub fn sample(mut self, text: impl Into<String>) -> Self {
        self.sample = text.into();
        self
    }

    /// Draws the control and, while open, the list.
    pub fn show(mut self, ui: &mut Ui) -> FontPickerResponse {
        let id = ui.make_persistent_id(self.id_salt);
        let ctx = ui.ctx().clone();
        let now = ctx.cumulative_frame_nr();
        let enabled = ui.is_enabled();
        let mut st: State = ui.data(|d| d.get_temp(id)).unwrap_or_default();
        if st.open && (!enabled || now > st.seen.saturating_add(1)) {
            st.open = false;
        }
        st.seen = now;
        let style = DropdownStyle::for_ui(ui).with_opaque_box();
        let font = self
            .font
            .clone()
            .unwrap_or_else(|| egui::TextStyle::Button.resolve(ui.style()));
        let ctrl_h = self
            .rect
            .map(|r| r.height())
            .unwrap_or_else(|| ui.spacing().interact_size.y.max(font.size + 6.0));
        let row_h = self.row_height.unwrap_or((font.size * 1.9).max(ctrl_h));
        let pad = 8.0;
        let arrow_w = 22.0;
        let control = match self.rect {
            Some(r) => r,
            None => ui.allocate_space(Vec2::new(self.width, ctrl_h)).1,
        };
        let response = ui.interact(
            control,
            id,
            if enabled {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        response.widget_info(|| {
            WidgetInfo::labeled(WidgetType::ComboBox, enabled, &self.selected_label)
        });

        let mut just_opened = false;
        if enabled && response.clicked() && !st.open {
            just_opened = true;
            st.open = true;
            st.query.clear();
            st.rows_for = None;
            st.cursor = None;
            st.kbd = false;
            let choices = self.choices.take().map(|b| b()).unwrap_or_default();
            st.lower = Arc::new(choices.iter().map(|c| c.label.to_lowercase()).collect());
            st.choices = Arc::new(choices);
        }

        // Closed control: the dropdown's face, with the current face's sample
        // between the name and the arrow.
        let sample_w = (font.size * 2.6).min(control.width() * 0.3);
        {
            let fill = if response.hovered() && enabled {
                style.control_fill_hovered
            } else {
                style.control_fill
            };
            let r = style.corner_radius;
            ui.painter().rect_filled(control, r, fill);
            let color = if enabled {
                style.text
            } else {
                style.text.gamma_multiply(0.45)
            };
            let sample_rect = Rect::from_min_max(
                Pos2::new(control.right() - arrow_w - sample_w, control.top()),
                Pos2::new(control.right() - arrow_w, control.bottom()),
            );
            paint_text(
                ui,
                Pos2::new(control.left() + pad, control.center().y),
                sample_rect.left() - control.left() - pad - 4.0,
                &self.selected_label,
                false,
                &font,
                color,
            );
            if let Some(face) = &self.selected_face {
                paint_sample(ui, sample_rect, face, &self.sample, &font, color, self.load);
            }
            paint_arrow(
                ui,
                Pos2::new(control.right() - arrow_w * 0.5, control.center().y),
                st.open,
                color,
            );
            let stroke = if (response.hovered() && enabled) || st.open {
                style.border_strong
            } else {
                style.border
            };
            ui.painter()
                .rect_stroke(control, r, Stroke::new(1.0, stroke), StrokeKind::Inside);
            if response.has_focus() && !st.open {
                ui.painter().rect_stroke(
                    control.expand(1.0),
                    r + 1,
                    style.focus,
                    StrokeKind::Outside,
                );
            }
        }

        let mut chosen: Option<String> = None;
        let mut popup_rect = None;
        if st.open {
            if st.rows_for.as_deref() != Some(st.query.as_str()) {
                let recent = recent_faces(&ctx);
                st.rows = Arc::new(build_rows(&st.choices, &st.lower, &st.query, &recent));
                st.rows_for = Some(st.query.clone());
                // The highlight starts on the current choice, else the first.
                let current = self.selected_key.as_deref();
                st.cursor = st
                    .rows
                    .iter()
                    .position(|r| {
                        matches!(r, Row::Choice(i) if Some(st.choices[*i].key.as_str()) == current)
                    })
                    .filter(|_| st.query.is_empty())
                    .or_else(|| step(&st.rows, None, 1));
                st.scroll_to_cursor = true;
            }
            let popup_w = control.width().max(320.0);
            let bounds = ctx.content_rect();
            let wanted = ctx_h_total(ctrl_h, row_h, st.rows.len(), self.visible_rows);
            let placement = choose_direction(control, wanted, bounds, ctrl_h + 5.0 * row_h);
            let up = placement.direction == crate::dropdown::Direction::Up;
            let mut x = control.left();
            if x + popup_w > bounds.right() {
                x = (bounds.right() - popup_w).max(bounds.left());
            }
            let box_rect = if up {
                Rect::from_min_max(
                    Pos2::new(x, control.bottom() - placement.height),
                    Pos2::new(x + popup_w, control.bottom()),
                )
            } else {
                Rect::from_min_size(
                    Pos2::new(x, control.top()),
                    Vec2::new(popup_w, placement.height),
                )
            };
            // The search field sits on the control's own row, wherever the
            // list extends, so the eye never leaves the control.
            let search_rect =
                Rect::from_min_size(Pos2::new(x, control.top()), Vec2::new(popup_w, ctrl_h));
            let body_rect = if up {
                Rect::from_min_max(
                    Pos2::new(x, box_rect.top() + 1.0),
                    Pos2::new(x + popup_w, search_rect.top()),
                )
            } else {
                Rect::from_min_max(
                    Pos2::new(x, search_rect.bottom()),
                    Pos2::new(x + popup_w, box_rect.bottom() - 1.0),
                )
            };
            popup_rect = Some(box_rect);

            // Keys, consumed before the search field sees them.
            let mut close = false;
            if !just_opened {
                let page = (body_rect.height() / row_h).floor().max(1.0) as isize;
                let (down, upk, pgdn, pgup, enter, esc) = ui.input_mut(|i| {
                    let m = Modifiers::NONE;
                    (
                        i.consume_key(m, Key::ArrowDown),
                        i.consume_key(m, Key::ArrowUp),
                        i.consume_key(m, Key::PageDown),
                        i.consume_key(m, Key::PageUp),
                        i.consume_key(m, Key::Enter),
                        i.consume_key(m, Key::Escape),
                    )
                });
                let before = st.cursor;
                if down {
                    st.cursor = step(&st.rows, st.cursor, 1);
                }
                if upk {
                    st.cursor = step(&st.rows, st.cursor, -1);
                }
                if pgdn {
                    st.cursor = step(&st.rows, st.cursor, page);
                }
                if pgup {
                    st.cursor = step(&st.rows, st.cursor, -page);
                }
                if st.cursor != before {
                    st.scroll_to_cursor = true;
                    st.kbd = true;
                }
                if ui.input(|i| i.pointer.delta() != Vec2::ZERO) {
                    st.kbd = false;
                }
                if esc {
                    close = true;
                } else if enter {
                    if let Some(Row::Choice(i)) = st.cursor.map(|c| st.rows[c]) {
                        chosen = Some(st.choices[i].key.clone());
                    }
                    close = true;
                }
                if ui.input(|i| i.pointer.any_pressed()) {
                    if let Some(p) = ui.input(|i| i.pointer.interact_pos()) {
                        if !box_rect.contains(p) && !control.contains(p) {
                            close = true;
                        }
                    }
                }
            }

            let mut query = std::mem::take(&mut st.query);
            let rows = st.rows.clone();
            let choices = st.choices.clone();
            let cursor = st.cursor;
            let kbd = st.kbd;
            let scroll_to_cursor = std::mem::take(&mut st.scroll_to_cursor);
            let selected_key = self.selected_key.clone();
            let sample = self.sample.clone();
            let load = self.load;
            let search_id = id.with("search");
            egui::Area::new(id.with("box"))
                .order(Order::Foreground)
                .fixed_pos(box_rect.min)
                .constrain(false)
                .sense(Sense::hover())
                .show(&ctx, |pui| {
                    pui.set_clip_rect(box_rect.expand(2.0));
                    let r = style.corner_radius;
                    pui.painter().rect_filled(box_rect, r, style.box_fill);

                    // Search row.
                    let field = search_rect.shrink2(Vec2::new(pad, 2.0));
                    let mut search_ui = pui.new_child(
                        UiBuilder::new()
                            .max_rect(field)
                            .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    );
                    let edit = egui::TextEdit::singleline(&mut query)
                        .id(search_id)
                        .font(font.clone())
                        .hint_text("Search fonts")
                        .frame(egui::Frame::NONE)
                        .desired_width(field.width())
                        .show(&mut search_ui);
                    if just_opened || !edit.response.response.has_focus() {
                        edit.response.response.request_focus();
                    }
                    pui.painter().hline(
                        search_rect.x_range(),
                        if up {
                            search_rect.top()
                        } else {
                            search_rect.bottom()
                        },
                        Stroke::new(1.0, style.border),
                    );

                    // Rows.
                    let mut body = pui.new_child(
                        UiBuilder::new()
                            .max_rect(body_rect)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    );
                    body.set_clip_rect(body_rect);
                    body.spacing_mut().item_spacing = Vec2::ZERO;
                    if rows.is_empty() {
                        body.painter().text(
                            Pos2::new(body_rect.left() + pad, body_rect.top() + row_h / 2.0),
                            Align2::LEFT_CENTER,
                            "No fonts match",
                            font.clone(),
                            style.text.gamma_multiply(0.6),
                        );
                    }
                    let heading_font = FontId::proportional((font.size * 0.85).max(8.0));
                    let sample_w = (font.size * 3.2).min(popup_w * 0.35);
                    egui::ScrollArea::vertical()
                        .id_salt(id.with("scroll"))
                        .max_height(body_rect.height())
                        .auto_shrink([false, false])
                        .show_rows(&mut body, row_h, rows.len(), |sui, range| {
                            sui.spacing_mut().item_spacing = Vec2::ZERO;
                            for k in range {
                                let (rect, row) = sui.allocate_exact_size(
                                    Vec2::new(popup_w - 2.0, row_h),
                                    match rows[k] {
                                        Row::Heading(_) => Sense::hover(),
                                        Row::Choice(_) => Sense::click(),
                                    },
                                );
                                let rect = rect.translate(Vec2::new(1.0, 0.0));
                                let i = match rows[k] {
                                    Row::Heading(group) => {
                                        let text = group
                                            .and_then(FontGroup::heading)
                                            .unwrap_or(RECENT_HEADING);
                                        sui.painter().text(
                                            Pos2::new(rect.left() + pad, rect.bottom() - 4.0),
                                            Align2::LEFT_BOTTOM,
                                            text,
                                            heading_font.clone(),
                                            style.text.gamma_multiply(0.55),
                                        );
                                        continue;
                                    }
                                    Row::Choice(i) => i,
                                };
                                let c = &choices[i];
                                let is_current = selected_key.as_deref() == Some(c.key.as_str());
                                let is_cursor = kbd && cursor == Some(k);
                                let round = row_rounding(r.saturating_sub(1), false, false);
                                if is_current {
                                    sui.painter().rect_filled(rect, round, style.selected_fill);
                                } else if row.hovered() || is_cursor {
                                    sui.painter().rect_filled(rect, round, style.hover_fill);
                                }
                                if is_cursor {
                                    sui.painter().rect_stroke(
                                        rect.shrink(1.0),
                                        round,
                                        style.focus,
                                        StrokeKind::Inside,
                                    );
                                    if scroll_to_cursor {
                                        sui.scroll_to_rect(rect, None);
                                    }
                                }
                                let sample_rect = Rect::from_min_max(
                                    Pos2::new(rect.right() - pad - sample_w, rect.top() + 2.0),
                                    Pos2::new(rect.right() - pad, rect.bottom() - 2.0),
                                );
                                let label_right = if c.face.is_some() {
                                    sample_rect.left() - 6.0
                                } else {
                                    rect.right() - pad
                                };
                                let shown = paint_text(
                                    sui,
                                    Pos2::new(rect.left() + pad, rect.center().y),
                                    label_right - rect.left() - pad,
                                    &c.label,
                                    false,
                                    &font,
                                    style.text,
                                );
                                if let Some(face) = &c.face {
                                    paint_sample(
                                        sui,
                                        sample_rect,
                                        face,
                                        &sample,
                                        &FontId::proportional(font.size * 1.25),
                                        style.text,
                                        load,
                                    );
                                }
                                row.widget_info(|| {
                                    WidgetInfo::selected(
                                        WidgetType::SelectableLabel,
                                        true,
                                        is_current,
                                        &c.label,
                                    )
                                });
                                let row = if shown != c.label {
                                    row.on_hover_text(&c.label)
                                } else {
                                    row
                                };
                                if row.clicked() {
                                    chosen = Some(c.key.clone());
                                    close = true;
                                }
                            }
                        });
                    pui.painter().rect_stroke(
                        box_rect,
                        r,
                        Stroke::new(1.0, style.border_strong),
                        StrokeKind::Inside,
                    );
                });
            st.query = query;
            crate::dropdown::mark_open(&ctx);
            if close {
                st.open = false;
                ctx.memory_mut(|m| m.request_focus(id));
            }
        }
        if let Some(key) = &chosen {
            if let Some(face) = st
                .choices
                .iter()
                .find(|c| &c.key == key)
                .and_then(|c| c.face.clone())
            {
                note_recent(&ctx, &face);
            }
        }
        if !st.open {
            // Release the list's memory while closed; it is rebuilt on open.
            st.choices = Arc::default();
            st.lower = Arc::default();
            st.rows = Arc::default();
            st.rows_for = None;
        }
        let mut response = response;
        if chosen.is_some() {
            response.mark_changed();
        }
        ui.data_mut(|d| d.insert_temp(id, st));
        FontPickerResponse {
            response,
            chosen,
            popup_rect,
        }
    }
}

/// Height of the open box: the search row plus up to `visible` rows.
fn ctx_h_total(ctrl_h: f32, row_h: f32, rows: usize, visible: f32) -> f32 {
    ctrl_h + (rows.max(1) as f32).min(visible) * row_h + 1.0
}

/// Draws `sample` in `face`, fitted into `rect` and clipped to it. While the
/// face is still loading a faint placeholder holds its place (the row never
/// changes width); a face this machine cannot load says so.
fn paint_sample(
    ui: &Ui,
    rect: Rect,
    face: &str,
    sample: &str,
    font: &FontId,
    color: Color32,
    load: &dyn Fn(&str) -> Option<FontBytes>,
) {
    match runtime_fonts::face(ui.ctx(), face, load) {
        FaceState::Ready(family) => {
            let size = fitted_size(ui.ctx(), sample, family.clone(), font.size, rect.size());
            ui.painter()
                .with_clip_rect(rect.intersect(ui.clip_rect()))
                .text(
                    rect.right_center(),
                    Align2::RIGHT_CENTER,
                    sample,
                    FontId::new(size, family),
                    color,
                );
        }
        FaceState::Pending => {
            let bar = Rect::from_min_size(
                Pos2::new(rect.right() - rect.width() * 0.6, rect.center().y - 1.5),
                Vec2::new(rect.width() * 0.6, 3.0),
            );
            ui.painter()
                .rect_filled(bar, 1.5, color.gamma_multiply(0.15));
        }
        FaceState::Unavailable => {
            ui.painter().text(
                rect.right_center(),
                Align2::RIGHT_CENTER,
                "not available",
                FontId::proportional((font.size * 0.75).max(8.0)),
                color.gamma_multiply(0.5),
            );
        }
    }
}

#[cfg(test)]
mod tests;
