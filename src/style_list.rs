//! One list behaviour for every manager: the style list with in-place rename,
//! the right-click menu, the List / Thumbnails icon switch, and the shared
//! Cancel / Reset / Apply / OK footer.
//!
//! Every app's style, pattern or category manager shows the same left-hand
//! list, and the user expects the same gestures in all of them (walkthrough
//! item 21): double-click, F2 or the right-click menu renames the row where it
//! stands; a New or Duplicate opens the editor on the new row; the two small
//! icons in the list header switch between rows and thumbnails. This module is
//! that behaviour, once. A host supplies row data ([`ListRow`]), a name
//! validation callback and which menu items are allowed, and acts on the
//! [`ListEvent`]s it gets back. Nothing here knows what a style is.
//!
//! The rows are painted by [`crate::sketch::style_list`] except while a rename
//! is open, when the List rows are painted here with the same geometry and the
//! renamed row's text is swapped for a field.
//!
//! Escape and F2 ownership work through `egui` memory, like
//! [`crate::dropdown::dropdown_owns_escape`]: [`rename_editor_owns_escape`] tells a
//! global Escape handler that runs before the panels paint to leave the key to
//! the open editor, and F2 belongs to the one list the user is working in.

use crate::sketch::{
    button_row, palette, truncate_middle, StyleListMode, StyleListResponse, StyleListRow,
    ThumbnailPaint,
};
use crate::sketch::{ButtonKind, SketchFrame};
use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

// ---------------------------------------------------------------------------
// The List / Thumbnails icon switch
// ---------------------------------------------------------------------------

/// Side of one List / Thumbnails button, in sketch points.
const VIEW_SWITCH_BUTTON: f32 = 20.0;

/// The two button rects of [`view_mode_switch`] (list, thumbnails), packed
/// against the right edge of `area` and centred on its height. Square, at most
/// [`VIEW_SWITCH_BUTTON`] sketch points a side.
pub fn view_mode_switch_rects(sk: &SketchFrame, area: Rect) -> [Rect; 2] {
    let side = sk.len(VIEW_SWITCH_BUTTON).min(area.height());
    let gap = sk.len(2.0);
    let top = area.center().y - side / 2.0;
    let thumbs = Rect::from_min_size(Pos2::new(area.right() - side, top), Vec2::splat(side));
    let list = thumbs.translate(Vec2::new(-(side + gap), 0.0));
    [list, thumbs]
}

/// Where the switch sits in a list `frame`: 4 sketch points below the frame's
/// top edge and 7 short of its right edge, inside the header band the frame
/// leaves empty for it.
pub fn view_mode_switch_area(sk: &SketchFrame, frame: Rect) -> Rect {
    Rect::from_min_max(
        Pos2::new(frame.right() - sk.len(157.0), frame.top() + sk.len(4.0)),
        Pos2::new(frame.right() - sk.len(7.0), frame.top() + sk.len(24.0)),
    )
}

/// Paints the list glyph (three rules) or the thumbnails glyph (2x2 squares)
/// in `rect`. Stroke weight ~7% of the icon side, never under a pixel.
fn paint_view_mode_glyph(painter: &egui::Painter, rect: Rect, mode: StyleListMode, color: Color32) {
    let size = rect.width().min(rect.height());
    let stroke = Stroke::new((size * 0.072).max(1.0), color);
    let at = |x: f32, y: f32| rect.center() + Vec2::new(x - 50.0, y - 50.0) * (size / 100.0);
    match mode {
        StyleListMode::List => {
            for y in [30.0, 50.0, 70.0] {
                painter.line_segment([at(22.0, y), at(78.0, y)], stroke);
            }
        }
        StyleListMode::Thumbnails => {
            for (x, y) in [(22.0, 22.0), (54.0, 22.0), (22.0, 54.0), (54.0, 54.0)] {
                painter.rect_stroke(
                    Rect::from_min_max(at(x, y), at(x + 24.0, y + 24.0)),
                    CornerRadius::ZERO,
                    stroke,
                    StrokeKind::Middle,
                );
            }
        }
    }
}

/// The List / Thumbnails switch: two small monochrome icon buttons in the
/// right corner of a list header, Explorer-style. Only the active mode's
/// button carries colour (the selection blue); the other stays muted until
/// hovered. Tooltips read "List" and "Thumbnails". Returns whether the mode
/// changed this frame.
pub fn view_mode_switch(
    ui: &mut egui::Ui,
    sk: &SketchFrame,
    area: Rect,
    mode: &mut StyleListMode,
) -> bool {
    let mut changed = false;
    let rects = view_mode_switch_rects(sk, area);
    for (rect, this) in rects
        .into_iter()
        .zip([StyleListMode::List, StyleListMode::Thumbnails])
    {
        let (id, tip) = match this {
            StyleListMode::List => ("style_list_mode_list", "List"),
            StyleListMode::Thumbnails => ("style_list_mode_thumbnails", "Thumbnails"),
        };
        let response = ui
            .interact(rect, ui.id().with(id), Sense::click())
            .on_hover_text(tip);
        if response.clicked() && *mode != this {
            *mode = this;
            changed = true;
        }
        let active = *mode == this;
        let color = if active {
            palette::PRIMARY
        } else if response.hovered() {
            palette::BRIGHT
        } else {
            palette::MUTED
        };
        let painter = ui.painter();
        if active {
            painter.rect_filled(rect, sk.radius(4.0), palette::PRIMARY.gamma_multiply(0.22));
        }
        paint_view_mode_glyph(painter, rect.shrink(rect.width() * 0.1), this, color);
    }
    changed
}

// ---------------------------------------------------------------------------
// Row data and events
// ---------------------------------------------------------------------------

/// What a row stands for, which decides what the rename gestures do on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    /// A named entry that can be renamed in place.
    Style,
    /// A named entry whose name cannot be changed here (a saved pattern other
    /// things refer to). Rename gestures report [`ListEvent::RenameDenied`]
    /// instead of opening an editor; Duplicate and Delete still apply.
    Locked,
    /// A row that names an absence or an action (`none`, `...new`): no editor,
    /// no context menu.
    Placeholder,
}

/// One entry in a [`StyleList`].
pub struct ListRow<'a> {
    /// The entry's name; identifies it for rename, Duplicate and Delete.
    pub name: &'a str,
    /// Text shown after the name, e.g. `*` for unapplied edits. Never part of
    /// the name an event carries.
    pub marker: Option<&'a str>,
    /// Paints this row's thumbnail in [`StyleListMode::Thumbnails`]; `None`
    /// leaves the empty outlined square.
    pub thumbnail: Option<ThumbnailPaint<'a>>,
    /// Draw the name in italics (a built-in the user cannot change).
    pub italic: bool,
    /// What the row stands for.
    pub kind: RowKind,
}

impl<'a> ListRow<'a> {
    /// A renamable row with no thumbnail.
    pub fn style(name: &'a str) -> Self {
        Self {
            name,
            marker: None,
            thumbnail: None,
            italic: false,
            kind: RowKind::Style,
        }
    }

    /// A row that only reports clicks (`none`, `...new`).
    pub fn placeholder(name: &'a str) -> Self {
        Self {
            kind: RowKind::Placeholder,
            ..Self::style(name)
        }
    }

    /// A named row whose name cannot be edited.
    pub fn locked(mut self) -> Self {
        self.kind = RowKind::Locked;
        self
    }

    /// Thumbnail painted by `paint` in Thumbnails mode.
    pub fn with_thumbnail(mut self, paint: impl Fn(&egui::Ui, Rect) + 'a) -> Self {
        self.thumbnail = Some(Box::new(paint));
        self
    }

    /// Text after the name (e.g. `*` for unsaved edits).
    pub fn marker(mut self, marker: &'a str) -> Self {
        self.marker = Some(marker);
        self
    }

    /// Name in italics.
    pub fn italic(mut self, italic: bool) -> Self {
        self.italic = italic;
        self
    }
}

/// What the user did in a [`StyleList`] this frame, in the order the host
/// should apply it (a rename lands before the click that ended it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListEvent {
    /// A row was clicked (index into the rows given).
    Select(usize),
    /// A rename passed validation: `(old, new)`. The host performs it.
    RenameRequested(String, String),
    /// An earlier `RenameRequested` did not take effect (the old row is still
    /// listed, the new one never appeared). The editor reopens with a message;
    /// a host that moved its selection to `new` should move it back to `old`.
    RenameRefused(String, String),
    /// A rename gesture hit a [`RowKind::Locked`] row (index).
    RenameDenied(usize),
    /// Duplicate chosen from the right-click menu, with the entry's name.
    Duplicate(String),
    /// Delete chosen from the right-click menu, with the entry's name.
    Delete(String),
    /// The List / Thumbnails mode changed (by the switch, or because a rename
    /// needs List rows). The `mode` passed in already holds the new value.
    ViewModeChanged(StyleListMode),
}

/// The result of [`StyleList::show`].
#[derive(Default)]
pub struct ListOutput {
    /// What happened, in order.
    pub events: Vec<ListEvent>,
    /// First and last row index the viewport showed (see
    /// [`StyleListResponse::shown`]).
    pub shown: Option<(usize, usize)>,
}

impl ListOutput {
    /// Index of the row clicked this frame.
    pub fn selected(&self) -> Option<usize> {
        self.events.iter().find_map(|e| match e {
            ListEvent::Select(i) => Some(*i),
            _ => None,
        })
    }
    /// `(old, new)` of a rename to perform.
    pub fn rename_requested(&self) -> Option<(&str, &str)> {
        self.events.iter().find_map(|e| match e {
            ListEvent::RenameRequested(a, b) => Some((a.as_str(), b.as_str())),
            _ => None,
        })
    }
    /// `(old, new)` of a rename the host's command refused.
    pub fn rename_refused(&self) -> Option<(&str, &str)> {
        self.events.iter().find_map(|e| match e {
            ListEvent::RenameRefused(a, b) => Some((a.as_str(), b.as_str())),
            _ => None,
        })
    }
    /// Name chosen for Duplicate from the menu.
    pub fn duplicate(&self) -> Option<&str> {
        self.events.iter().find_map(|e| match e {
            ListEvent::Duplicate(n) => Some(n.as_str()),
            _ => None,
        })
    }
    /// Name chosen for Delete from the menu.
    pub fn delete(&self) -> Option<&str> {
        self.events.iter().find_map(|e| match e {
            ListEvent::Delete(n) => Some(n.as_str()),
            _ => None,
        })
    }
    /// Index of a row whose rename was denied.
    pub fn rename_denied(&self) -> Option<usize> {
        self.events.iter().find_map(|e| match e {
            ListEvent::RenameDenied(i) => Some(*i),
            _ => None,
        })
    }
}

// ---------------------------------------------------------------------------
// Rename state in egui memory
// ---------------------------------------------------------------------------

/// One open rename, kept in egui memory under the list's key.
#[derive(Clone, Debug, PartialEq)]
struct RenameEdit {
    original: String,
    text: String,
    error: Option<String>,
    /// Select the whole name and take focus on the next painted frame.
    fresh: bool,
    /// The field has held focus since it was (re)opened, so losing it now is a
    /// click elsewhere rather than the frame before focus arrives.
    focused: bool,
    /// A row carrying `original` has been listed. Until then the edit is only
    /// waiting for the command that creates the entry (a New) to run, and the
    /// list is left alone.
    seen: bool,
    /// `egui` input time when the edit was requested; an entry that has not
    /// shown up within [`PENDING_RENAME_SECS`] never will (the command failed).
    started: f64,
}

/// How long a rename may wait for its row to be listed.
const PENDING_RENAME_SECS: f64 = 2.0;
/// How long a committed rename may take to show its new name in the list
/// before it counts as refused by the command. The new name appearing is the
/// confirmation (the host's command result, as the list sees it); this is only a
/// safety net for a command that never reports, so it is long: a busy main
/// window can take seconds, and a premature "refused" moves the host's drafts
/// back to the old name. Even on expiry the list reports a refusal only while
/// the old name is still listed and the new one is not.
const RENAME_CONFIRM_SECS: f64 = 10.0;

impl RenameEdit {
    fn new(name: &str, now: f64) -> Self {
        Self {
            original: name.to_owned(),
            text: name.to_owned(),
            error: None,
            fresh: true,
            focused: false,
            seen: false,
            started: now,
        }
    }
}

fn rename_edit_id(key: &str) -> egui::Id {
    egui::Id::new(("style_list_rename", key))
}

fn rename_menu_id(key: &str) -> egui::Id {
    egui::Id::new(("style_list_menu", key))
}

/// Whether the right-click menu of the list identified by `key` is open. A
/// window's own Escape handler must leave Escape to the menu then: it closes
/// the menu and nothing else.
pub fn menu_is_open(ctx: &egui::Context, key: &str) -> bool {
    ctx.memory(|m| {
        m.data
            .get_temp::<Option<(String, Pos2)>>(rename_menu_id(key))
    })
    .flatten()
    .is_some()
}

/// Opens the in-place editor on the row for `name` in the list identified by
/// `key`. Safe to call before the row exists (right after a New or Duplicate
/// command): the editor appears once a row with that name is listed.
pub fn start_rename(ctx: &egui::Context, key: &str, name: &str) {
    let edit = RenameEdit::new(name, ctx.input(|i| i.time));
    ctx.memory_mut(|m| m.data.insert_temp(rename_edit_id(key), Some(edit)));
    ctx.request_repaint();
}

/// Closes any rename editor (and a pending one) on the list identified by
/// `key`, e.g. when the thing the list belongs to is closed or retargeted.
pub fn cancel_rename(ctx: &egui::Context, key: &str) {
    ctx.memory_mut(|m| {
        m.data
            .insert_temp(rename_edit_id(key), Option::<RenameEdit>::None)
    });
}

/// Whether the list identified by `key` has a rename editor open.
pub fn rename_is_open(ctx: &egui::Context, key: &str) -> bool {
    ctx.memory(|m| m.data.get_temp::<Option<RenameEdit>>(rename_edit_id(key)))
        .flatten()
        .is_some()
}

fn rename_focus_stamp_id(ctx: &egui::Context) -> egui::Id {
    egui::Id::new(("style_list_rename_focus_stamp", ctx.viewport_id()))
}

/// True while a style-list rename field (or right-click menu) of this window holds the keyboard: it
/// was focused during the previous frame, so an Esc arriving now belongs to the
/// edit (which cancels itself) and must not also reach the canvas. A global
/// Escape handler runs *before* the panels paint, so it can only ask about last
/// frame; a stale stamp (the window was closed mid-edit) expires after one
/// frame.
pub fn rename_editor_owns_escape(ctx: &egui::Context) -> bool {
    let now = ctx.cumulative_frame_nr();
    ctx.memory(|m| m.data.get_temp::<u64>(rename_focus_stamp_id(ctx)))
        .is_some_and(|stamp| stamp.saturating_add(1) >= now)
}

enum EditResult {
    Open,
    Commit,
    Cancel,
}

fn always(_: &str) -> bool {
    true
}

// ---------------------------------------------------------------------------
// The widget
// ---------------------------------------------------------------------------

/// Extra items a host adds to the right-click menu: gets the entry name and
/// returns true to close the menu.
pub type MenuExtra<'a> = &'a mut dyn FnMut(&mut egui::Ui, &str) -> bool;

/// The style list with in-place rename, right-click menu and the
/// List / Thumbnails switch. Build it with [`StyleList::new`], adjust it with
/// the builder methods, then call [`StyleList::show`].
///
/// A rename in Thumbnails mode switches the list to List first, because the
/// editor lives on a List row.
pub struct StyleList<'a> {
    key: &'a str,
    validate: &'a dyn Fn(&str, &str) -> Result<(), String>,
    can_duplicate: Option<&'a dyn Fn(&str) -> bool>,
    can_delete: Option<&'a dyn Fn(&str) -> bool>,
    extra_menu: Option<MenuExtra<'a>>,
    switch: bool,
}

impl<'a> StyleList<'a> {
    /// A list identified by `key` (unique per manager, used for the memory the
    /// rename and menu keep). `validate(original, new)` decides whether `new`
    /// may name the entry now called `original`; its `Err` text is shown beside
    /// the editor and keeps it open. Unchanged and empty names never reach it.
    pub fn new(key: &'a str, validate: &'a dyn Fn(&str, &str) -> Result<(), String>) -> Self {
        Self {
            key,
            validate,
            can_duplicate: None,
            can_delete: None,
            extra_menu: None,
            switch: true,
        }
    }

    /// Offer Duplicate in the right-click menu.
    pub fn duplicate(mut self) -> Self {
        self.can_duplicate = Some(&always);
        self
    }

    /// Offer Duplicate, enabled per entry name.
    pub fn duplicate_if(mut self, allowed: &'a dyn Fn(&str) -> bool) -> Self {
        self.can_duplicate = Some(allowed);
        self
    }

    /// Offer Delete in the right-click menu; `allowed` says whether it is
    /// enabled for a given entry.
    pub fn delete_if(mut self, allowed: &'a dyn Fn(&str) -> bool) -> Self {
        self.can_delete = Some(allowed);
        self
    }

    /// Add host items to the right-click menu.
    pub fn extra_menu(mut self, extra: MenuExtra<'a>) -> Self {
        self.extra_menu = Some(extra);
        self
    }

    /// Do not draw the List / Thumbnails switch.
    pub fn without_switch(mut self) -> Self {
        self.switch = false;
        self
    }

    /// Draws the list in `frame` (the framed region; the switch sits in its
    /// header band) and returns what the user did.
    pub fn show(
        self,
        ui: &mut egui::Ui,
        sk: &SketchFrame,
        frame: Rect,
        rows: Vec<ListRow<'_>>,
        selected: Option<usize>,
        mode: &mut StyleListMode,
    ) -> ListOutput {
        let StyleList {
            key,
            validate,
            can_duplicate,
            can_delete,
            extra_menu: mut extra,
            switch,
        } = self;
        let mode_before = *mode;
        let mut out = ListOutput::default();
        let edit_id = rename_edit_id(key);
        let mut edit: Option<RenameEdit> = ui
            .memory(|m| m.data.get_temp::<Option<RenameEdit>>(edit_id))
            .flatten();

        // What a name means: placeholders have none.
        let names: Vec<Option<&str>> = rows
            .iter()
            .map(|r| (r.kind != RowKind::Placeholder).then_some(r.name))
            .collect();
        let listed = |name: &str| names.iter().any(|n| *n == Some(name));
        let row_names: Vec<&str> = rows.iter().map(|r| r.name).collect();
        let row_kinds: Vec<RowKind> = rows.iter().map(|r| r.kind).collect();
        let kind_of = |name: &str| {
            (0..row_kinds.len())
                .find(|&i| row_kinds[i] != RowKind::Placeholder && row_names[i] == name)
                .map(|i| row_kinds[i])
        };

        let mut switch_changed = false;
        if switch {
            switch_changed = view_mode_switch(ui, sk, view_mode_switch_area(sk, frame), mode);
        }

        // An edit whose row is not listed yet (a New whose command has not
        // run) is only waiting: the list stays as it is, in whatever mode, and
        // the wait is bounded by time, not by how many frames were painted.
        let now = ui.input(|i| i.time);
        if let Some(e) = edit.as_mut() {
            if listed(&e.original) {
                e.seen = true;
            } else if e.seen || now - e.started > PENDING_RENAME_SECS {
                edit = None;
            } else {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(250));
            }
        }

        // A committed rename is confirmed when its new name is listed. If the
        // command refused it (the UI's check is a mirror, not the authority)
        // the old row is still there: say so, and give the user their text
        // back.
        let pending_id = edit_id.with("committed");
        let committed: Option<(String, String, f64)> = ui
            .memory(|m| m.data.get_temp::<Option<(String, String, f64)>>(pending_id))
            .flatten();
        let mut refused: Option<(String, String)> = None;
        let mut committed = committed.and_then(|(old, new, at)| {
            if listed(&new) {
                None
            } else if now - at <= RENAME_CONFIRM_SECS {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(250));
                Some((old, new, at))
            } else {
                if edit.is_none() && listed(&old) {
                    let mut reopened = RenameEdit::new(&old, now);
                    reopened.text = new.clone();
                    reopened.error = Some("The rename was refused".to_owned());
                    reopened.seen = true;
                    edit = Some(reopened);
                    refused = Some((old, new));
                }
                None
            }
        });
        let editor_open = edit.as_ref().is_some_and(|e| e.seen);

        let mut renamed: Option<(String, String)> = None;
        let mut raw = StyleListResponse::default();
        // The rows as the kit's plain list wants them.
        let labels: Vec<String> = rows
            .iter()
            .map(|r| match r.marker {
                Some(m) => format!("{} {m}", r.name),
                None => r.name.to_owned(),
            })
            .collect();
        let plain: Vec<StyleListRow<'_>> = rows
            .into_iter()
            .zip(labels.iter())
            .map(|(r, label)| StyleListRow {
                name: label.as_str(),
                thumbnail: r.thumbnail,
                italic: r.italic,
            })
            .collect();

        if editor_open {
            *mode = StyleListMode::List;
            let mut result = EditResult::Open;
            let mut message: Option<(Rect, String)> = None;
            let e = edit.as_mut().expect("checked above");
            draw_list_with_editor(
                ui,
                sk,
                frame,
                &plain,
                selected,
                key,
                &|index, original| {
                    row_kinds[index] != RowKind::Placeholder && row_names[index] == original
                },
                e,
                &mut raw,
                &mut result,
                &mut message,
            );
            match result {
                EditResult::Cancel => edit = None,
                EditResult::Commit => {
                    let e = edit.as_mut().expect("still open");
                    let new = e.text.trim().to_owned();
                    let verdict = if new == e.original {
                        Ok(())
                    } else if new.is_empty() {
                        Err("A name cannot be empty".to_owned())
                    } else {
                        validate(&e.original, &new)
                    };
                    if new == e.original {
                        edit = None;
                    } else if let Err(problem) = verdict {
                        e.error = Some(problem);
                        e.focused = false;
                        e.fresh = false;
                    } else {
                        committed = Some((e.original.clone(), new.clone(), now));
                        renamed = Some((e.original.clone(), new));
                        edit = None;
                    }
                }
                EditResult::Open => {}
            }
            if let (Some(e), Some((field, text))) = (&edit, message) {
                if e.error.is_some() {
                    paint_error_bubble(ui, sk, edit_id, field, &text);
                }
            }
        } else {
            raw = crate::sketch::style_list(ui, sk, frame, &plain, selected, *mode);
        }
        out.shown = raw.shown;

        // Triggers that open the editor.
        let mut start: Option<String> = None;
        if let Some(name) = raw
            .double_clicked
            .and_then(|i| names.get(i).copied().flatten())
        {
            start = Some(name.to_owned());
        }
        // F2 belongs to the one list the user is working in: the list the
        // pointer is over, or the one they last pressed in. It is consumed, so
        // neither a second open list nor a global F2 (rename the active sheet)
        // also reacts to the same keystroke.
        let pointer_over = ui.rect_contains_pointer(frame);
        let last_pressed_id = egui::Id::new("style_list_last_pressed");
        if ui.input(|i| i.pointer.any_pressed()) {
            let mine = ui
                .memory(|m| m.data.get_temp::<String>(last_pressed_id))
                .is_some_and(|k| k == key);
            if pointer_over {
                ui.memory_mut(|m| m.data.insert_temp(last_pressed_id, key.to_owned()));
            } else if mine {
                ui.memory_mut(|m| m.data.remove::<String>(last_pressed_id));
            }
        }
        let focused_list = pointer_over
            || ui
                .memory(|m| m.data.get_temp::<String>(last_pressed_id))
                .is_some_and(|k| k == key);
        if !editor_open && start.is_none() && focused_list && !ui.ctx().egui_wants_keyboard_input()
        {
            if let Some(name) = selected.and_then(|i| names.get(i).copied().flatten()) {
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F2)) {
                    start = Some(name.to_owned());
                }
            }
        }

        // Right-click menu.
        let menu_id = rename_menu_id(key);
        let mut menu: Option<(String, Pos2)> = ui
            .memory(|m| m.data.get_temp::<Option<(String, Pos2)>>(menu_id))
            .flatten();
        if let Some(name) = raw
            .secondary_clicked
            .and_then(|i| names.get(i).copied().flatten())
        {
            let at = ui
                .input(|i| i.pointer.interact_pos())
                .unwrap_or(frame.center());
            menu = Some((name.to_owned(), at));
        }
        let mut context: Option<ListEvent> = None;
        if let Some((name, at)) = menu.clone() {
            // An open menu owns Escape for a global handler asking before the
            // panels paint, exactly like an open rename editor.
            let frame_nr = ui.ctx().cumulative_frame_nr();
            let stamp = rename_focus_stamp_id(ui.ctx());
            ui.memory_mut(|m| m.data.insert_temp(stamp, frame_nr));
            let mut close = false;
            let area = egui::Area::new(menu_id.with("area"))
                .order(egui::Order::Foreground)
                .fixed_pos(at)
                .show(ui.ctx(), |ui| {
                    egui::Frame::menu(ui.style()).show(ui, |ui| {
                        ui.set_min_width(110.0);
                        if ui.button("Rename").clicked() {
                            start = Some(name.clone());
                            close = true;
                        }
                        if let Some(allowed) = can_duplicate {
                            let ok = allowed(&name);
                            if ui.add_enabled(ok, egui::Button::new("Duplicate")).clicked() {
                                context = Some(ListEvent::Duplicate(name.clone()));
                                close = true;
                            }
                        }
                        if let Some(allowed) = can_delete {
                            let ok = allowed(&name);
                            if ui.add_enabled(ok, egui::Button::new("Delete")).clicked() {
                                context = Some(ListEvent::Delete(name.clone()));
                                close = true;
                            }
                        }
                        if let Some(extra) = extra.as_mut() {
                            if extra(ui, &name) {
                                close = true;
                            }
                        }
                    });
                });
            let outside = ui.input(|i| {
                i.pointer.any_pressed()
                    && i.pointer
                        .interact_pos()
                        .is_some_and(|p| !area.response.rect.contains(p))
            });
            if close || outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                menu = None;
            }
        }
        ui.memory_mut(|m| m.data.insert_temp(menu_id, menu));

        // Events, in the order a host should apply them.
        if let Some((old, new)) = renamed {
            out.events.push(ListEvent::RenameRequested(old, new));
        }
        if let Some((old, new)) = refused {
            out.events.push(ListEvent::RenameRefused(old, new));
        }
        if let Some(i) = raw.clicked {
            out.events.push(ListEvent::Select(i));
        }
        if let Some(event) = context {
            out.events.push(event);
        }

        if let Some(name) = start {
            if kind_of(&name) == Some(RowKind::Locked) {
                if let Some(i) = names.iter().position(|n| *n == Some(name.as_str())) {
                    out.events.push(ListEvent::RenameDenied(i));
                }
            } else {
                edit = Some(RenameEdit::new(&name, now));
                ui.ctx().request_repaint();
            }
        }
        ui.memory_mut(|m| {
            m.data.insert_temp(edit_id, edit);
            m.data.insert_temp(pending_id, committed);
        });
        if switch_changed || *mode != mode_before {
            out.events.push(ListEvent::ViewModeChanged(*mode));
        }
        out
    }
}

fn paint_error_bubble(ui: &egui::Ui, sk: &SketchFrame, edit_id: egui::Id, field: Rect, text: &str) {
    let painter = ui
        .ctx()
        .layer_painter(egui::LayerId::new(egui::Order::Tooltip, edit_id));
    let galley = painter.layout_no_wrap(text.to_owned(), sk.font(16.0), Color32::LIGHT_RED);
    let pad = Vec2::new(sk.len(6.0), sk.len(3.0));
    let bubble = Rect::from_min_size(
        field.left_bottom() + Vec2::new(0.0, sk.len(3.0)),
        galley.size() + pad * 2.0,
    );
    painter.rect_filled(bubble, sk.radius(6.0), palette::TILE_BG);
    painter.rect_stroke(
        bubble,
        sk.radius(6.0),
        Stroke::new(1.0, Color32::LIGHT_RED),
        StrokeKind::Inside,
    );
    painter.galley(bubble.min + pad, galley, Color32::LIGHT_RED);
}

/// List-mode rows with the renamed row's text swapped for a field. Geometry
/// and painting mirror [`crate::sketch::style_list`]'s List branch, including
/// the `style_list_scroll` salt, so the list does not jump when the editor
/// opens.
#[allow(clippy::too_many_arguments)]
fn draw_list_with_editor(
    ui: &mut egui::Ui,
    sk: &SketchFrame,
    frame: Rect,
    rows: &[StyleListRow<'_>],
    selected: Option<usize>,
    key: &str,
    is_edited: &dyn Fn(usize, &str) -> bool,
    edit: &mut RenameEdit,
    out: &mut StyleListResponse,
    result: &mut EditResult,
    message: &mut Option<(Rect, String)>,
) {
    let font = sk.font(20.0);
    let inner = Rect::from_min_max(
        frame.min + Vec2::new(sk.len(8.0), sk.len(30.0)),
        frame.max - Vec2::new(sk.len(4.0), sk.len(8.0)),
    );
    let mut found = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(inner), |ui| {
        egui::ScrollArea::vertical()
            .id_salt("style_list_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let row_h = sk.len(24.2);
                let width = ui.available_width();
                for (index, row) in rows.iter().enumerate() {
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::new(width, row_h), Sense::click());
                    let bar = rect.shrink2(Vec2::new(sk.len(2.0), 0.0));
                    let editing_row = !found && is_edited(index, &edit.original);
                    if editing_row {
                        found = true;
                        ui.painter()
                            .rect_filled(bar, sk.radius(12.0), palette::PRIMARY);
                        let field = Rect::from_min_max(
                            Pos2::new(rect.left() + sk.len(6.0), rect.top() + 1.0),
                            Pos2::new(rect.right() - sk.len(6.0), rect.bottom() - 1.0),
                        );
                        *result = rename_field(ui, sk, field, key, edit);
                        if let Some(error) = &edit.error {
                            *message = Some((field, error.clone()));
                        }
                        continue;
                    }
                    if selected == Some(index) {
                        ui.painter()
                            .rect_filled(bar, sk.radius(12.0), palette::PRIMARY);
                    } else if response.hovered() {
                        ui.painter().rect_filled(
                            bar,
                            sk.radius(12.0),
                            Color32::from_white_alpha(12),
                        );
                    }
                    let text_x = rect.left() + sk.len(10.0);
                    let shown =
                        truncate_middle(ui, row.name, &font, rect.right() - text_x - sk.len(8.0));
                    let galley = ui
                        .painter()
                        .layout_job(egui::text::LayoutJob::single_section(
                            shown,
                            egui::TextFormat {
                                font_id: font.clone(),
                                color: palette::BRIGHT,
                                italics: row.italic,
                                ..Default::default()
                            },
                        ));
                    ui.painter().galley(
                        Pos2::new(text_x, rect.center().y - galley.size().y / 2.0),
                        galley,
                        palette::BRIGHT,
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
                }
            });
    });
}

/// The text field on the renamed row. Takes focus with the name selected,
/// commits on losing focus (Enter or a click elsewhere), cancels on Esc.
fn rename_field(
    ui: &mut egui::Ui,
    sk: &SketchFrame,
    field: Rect,
    key: &str,
    edit: &mut RenameEdit,
) -> EditResult {
    let id = rename_edit_id(key).with("field");
    ui.painter()
        .rect_filled(field, sk.radius(8.0), palette::TILE_BG);
    ui.painter().rect_stroke(
        field,
        sk.radius(8.0),
        Stroke::new(1.0, palette::BRIGHT),
        StrokeKind::Inside,
    );
    if edit.fresh {
        edit.fresh = false;
        ui.scroll_to_rect(field, None);
        let end = edit.text.chars().count();
        let mut state = egui::TextEdit::load_state(ui.ctx(), id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(0),
                egui::text::CCursor::new(end),
            )));
        state.store(ui.ctx(), id);
        ui.memory_mut(|m| m.request_focus(id));
    }
    let mut response = None;
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(field.shrink2(Vec2::new(sk.len(6.0), 0.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            response = Some(
                ui.add(
                    egui::TextEdit::singleline(&mut edit.text)
                        .id(id)
                        .font(sk.font(20.0))
                        .text_color(palette::BRIGHT)
                        .frame(egui::Frame::NONE)
                        .desired_width(f32::INFINITY),
                ),
            );
        },
    );
    let response = response.expect("field added");
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        return EditResult::Cancel;
    }
    if response.has_focus() {
        edit.focused = true;
        let frame_nr = ui.ctx().cumulative_frame_nr();
        let stamp = rename_focus_stamp_id(ui.ctx());
        ui.memory_mut(|m| m.data.insert_temp(stamp, frame_nr));
        EditResult::Open
    } else if !edit.focused {
        // Not focused yet (first frame, or reopened after a refused name).
        ui.memory_mut(|m| m.request_focus(id));
        EditResult::Open
    } else {
        // Focus left: Enter, or a click elsewhere.
        EditResult::Commit
    }
}

// ---------------------------------------------------------------------------
// The shared footer: status text and Reset / Cancel / Apply / OK
// ---------------------------------------------------------------------------

/// Whether the work in a manager is safely applied, for the footer's status
/// text: "Saved" in the OK green, "Unsaved changes" in the warning orange.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveStatus {
    /// Applied, and unchanged since.
    Saved,
    /// Changed since the last Apply.
    Unsaved,
    /// An Apply is waiting for the app's answer.
    Saving,
    /// A new entry nobody has applied yet.
    NotSavedYet,
}

impl SaveStatus {
    /// The status line.
    pub fn text(self) -> &'static str {
        match self {
            SaveStatus::Saved => "Saved",
            SaveStatus::Unsaved => "Unsaved changes: press Apply to save",
            SaveStatus::Saving => "Saving…",
            SaveStatus::NotSavedYet => "Not saved yet",
        }
    }

    /// The status colour: green for saved, orange for unsaved, muted
    /// otherwise.
    pub fn color(self, visuals: &egui::Visuals) -> Color32 {
        match self {
            SaveStatus::Saved => crate::theme::ok_color(visuals),
            SaveStatus::Unsaved => crate::theme::warning_color(visuals),
            SaveStatus::Saving | SaveStatus::NotSavedYet => visuals.weak_text_color(),
        }
    }
}

/// A footer button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FooterButton {
    /// Throw away unapplied edits, stay open.
    Reset,
    /// Throw away unapplied edits and close.
    Cancel,
    /// Save, stay open.
    Apply,
    /// Save and close.
    Ok,
}

/// The sketch-style footer row, Reset · Cancel · Apply · OK with OK primary,
/// across `rect`. `enabled` is in that order. Returns the button clicked.
pub fn footer_buttons(
    ui: &mut egui::Ui,
    sk: &SketchFrame,
    rect: Rect,
    enabled: [bool; 4],
) -> Option<FooterButton> {
    button_row(
        ui,
        sk,
        rect,
        15.0,
        &["Reset", "Cancel", "Apply", "OK"],
        &[
            ButtonKind::Normal,
            ButtonKind::Normal,
            ButtonKind::Normal,
            ButtonKind::Primary,
        ],
        &enabled,
    )
    .map(|i| {
        [
            FooterButton::Reset,
            FooterButton::Cancel,
            FooterButton::Apply,
            FooterButton::Ok,
        ][i]
    })
}

/// The label of the button that makes a copy of a list entry in the New / Edit /
/// Copy / Delete rows. The menu entry and the [`ListEvent`] keep the name
/// Duplicate; the button says Copy because "Duplicate" is too wide for the row.
pub const COPY_BUTTON_LABEL: &str = "Copy";

#[cfg(test)]
mod tests;
