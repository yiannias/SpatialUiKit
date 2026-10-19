use super::*;
use egui::{Event, Key, Modifiers, PointerButton, RawInput};
use std::cell::RefCell;

/// A headless egui frame loop: feeds pointer and key events frame by frame.
struct Harness {
    ctx: egui::Context,
    frame: u32,
}

impl Harness {
    fn new() -> Self {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(1.0);
        Self { ctx, frame: 0 }
    }
    fn frame(
        &mut self,
        events: Vec<Event>,
        ui_fn: &mut dyn FnMut(&mut egui::Ui),
    ) -> egui::FullOutput {
        self.frame += 1;
        self.ctx.run_ui(
            RawInput {
                time: Some(self.frame as f64 / 60.0),
                events,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
                ..Default::default()
            },
            ui_fn,
        )
    }
    fn advance(&mut self, seconds: f64) {
        self.frame += (seconds * 60.0) as u32;
    }
    fn settle(&mut self, n: u32, ui_fn: &mut dyn FnMut(&mut egui::Ui)) {
        for _ in 0..n {
            self.frame(Vec::new(), ui_fn);
        }
    }
    fn click(&mut self, pos: Pos2, ui_fn: &mut dyn FnMut(&mut egui::Ui)) {
        self.frame(vec![Event::PointerMoved(pos)], ui_fn);
        for pressed in [true, false] {
            self.frame(
                vec![Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                }],
                ui_fn,
            );
        }
        self.settle(2, ui_fn);
    }
    fn key(&mut self, key: Key, ui_fn: &mut dyn FnMut(&mut egui::Ui)) {
        for pressed in [true, false] {
            self.frame(
                vec![Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
                ui_fn,
            );
        }
        self.settle(2, ui_fn);
    }
}

fn painted_text(output: &egui::FullOutput) -> Vec<String> {
    fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::Shape::Text(t) => out.push(t.galley.text().to_owned()),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in &output.shapes {
        walk(&clipped.shape, &mut out);
    }
    out
}

// ---- List / Thumbnails icon switch ----------------------------------------

fn show_switch(ui: &mut egui::Ui, mode: &RefCell<StyleListMode>) {
    let sk = SketchFrame::fit(ui.max_rect(), Vec2::new(900.0, 700.0));
    view_mode_switch(
        ui,
        &sk,
        sk.rect(600.0, 40.0, 150.0, 20.0),
        &mut mode.borrow_mut(),
    );
}

fn switch_rects() -> [Rect; 2] {
    let sk = SketchFrame::fit(
        Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0)),
        Vec2::new(900.0, 700.0),
    );
    view_mode_switch_rects(&sk, sk.rect(600.0, 40.0, 150.0, 20.0))
}

/// Stroke colours of the line/rect shapes whose bounds sit inside `rect`.
fn glyph_colors(output: &egui::FullOutput, rect: Rect) -> Vec<Color32> {
    fn walk(shape: &egui::Shape, rect: Rect, out: &mut Vec<Color32>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, rect, out)),
            egui::Shape::LineSegment { points, stroke } if rect.contains(points[0]) => {
                out.push(stroke.color)
            }
            egui::Shape::Rect(r)
                if rect.contains_rect(r.rect) && r.fill == Color32::TRANSPARENT =>
            {
                out.push(r.stroke.color)
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    for clipped in &output.shapes {
        walk(&clipped.shape, rect, &mut out);
    }
    out
}

#[test]
fn clicking_each_icon_switches_mode() {
    let mode = RefCell::new(StyleListMode::List);
    let mut h = Harness::new();
    let mut show = |ui: &mut egui::Ui| show_switch(ui, &mode);
    h.settle(2, &mut show);
    let [list, thumbs] = switch_rects();
    h.click(thumbs.center(), &mut show);
    assert_eq!(*mode.borrow(), StyleListMode::Thumbnails);
    h.click(thumbs.center(), &mut show);
    assert_eq!(
        *mode.borrow(),
        StyleListMode::Thumbnails,
        "re-click keeps it"
    );
    h.click(list.center(), &mut show);
    assert_eq!(*mode.borrow(), StyleListMode::List);
}

#[test]
fn only_the_active_icon_carries_the_state_colour() {
    let mode = RefCell::new(StyleListMode::List);
    let mut h = Harness::new();
    let mut show = |ui: &mut egui::Ui| show_switch(ui, &mode);
    let [list, thumbs] = switch_rects();
    h.frame(vec![Event::PointerMoved(Pos2::new(5.0, 5.0))], &mut show);
    let out = h.frame(Vec::new(), &mut show);
    assert!(glyph_colors(&out, list)
        .iter()
        .all(|c| *c == palette::PRIMARY));
    assert!(!glyph_colors(&out, list).is_empty());
    assert!(glyph_colors(&out, thumbs)
        .iter()
        .all(|c| *c == palette::MUTED));
    assert!(!glyph_colors(&out, thumbs).is_empty());

    *mode.borrow_mut() = StyleListMode::Thumbnails;
    h.frame(vec![Event::PointerMoved(Pos2::new(5.0, 5.0))], &mut show);
    let out = h.frame(Vec::new(), &mut show);
    assert!(glyph_colors(&out, list)
        .iter()
        .all(|c| *c == palette::MUTED));
    assert!(glyph_colors(&out, thumbs)
        .iter()
        .all(|c| *c == palette::PRIMARY));
}

#[test]
fn icons_show_list_and_thumbnails_tooltips() {
    let mode = RefCell::new(StyleListMode::List);
    let [list, thumbs] = switch_rects();
    for (rect, tip) in [(list, "List"), (thumbs, "Thumbnails")] {
        let mut h = Harness::new();
        let mut show = |ui: &mut egui::Ui| show_switch(ui, &mode);
        h.settle(2, &mut show);
        h.frame(vec![Event::PointerMoved(rect.center())], &mut show);
        h.advance(2.0);
        let mut seen = false;
        for _ in 0..6 {
            let out = h.frame(Vec::new(), &mut show);
            seen |= painted_text(&out).iter().any(|t| t == tip);
            h.advance(0.2);
        }
        assert!(seen, "tooltip {tip:?} not shown");
    }
}

#[test]
fn buttons_are_icon_sized_and_right_aligned() {
    let sk = SketchFrame::fit(
        Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0)),
        Vec2::new(900.0, 700.0),
    );
    let area = sk.rect(600.0, 40.0, 150.0, 20.0);
    let [list, thumbs] = view_mode_switch_rects(&sk, area);
    assert_eq!(thumbs.right(), area.right());
    assert!(list.right() < thumbs.left());
    assert!(
        (16.0..=20.0).contains(&thumbs.width()),
        "{}",
        thumbs.width()
    );
}

// ---- The list ---------------------------------------------------------------

const KEY: &str = "rename_test";
const NAMES: [&str; 3] = ["Alpha", "Beta", "Gamma"];

/// A host's naming rule, as a callback.
fn validate_like_a_host(existing: &[String], new: &str) -> Result<(), String> {
    let lower = new.to_ascii_lowercase();
    if lower == "default" || lower == "none" {
        return Err("'Default' and 'None' are reserved names".into());
    }
    if existing.iter().any(|e| e == new) {
        return Err("A style with that name already exists".into());
    }
    Ok(())
}

/// A three-row list ("...new" last) wired to the widget.
struct Fixture {
    names: Vec<String>,
    mode: StyleListMode,
    selected: Option<usize>,
    events: Vec<ListEvent>,
    locked: Option<&'static str>,
    marked: Option<&'static str>,
}

impl Fixture {
    fn new() -> Self {
        Self {
            names: NAMES.iter().map(|n| (*n).to_owned()).collect(),
            mode: StyleListMode::List,
            selected: None,
            events: Vec::new(),
            locked: None,
            marked: None,
        }
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        let sk = SketchFrame::fit(ui.max_rect(), Vec2::new(900.0, 700.0));
        let mut rows: Vec<ListRow<'_>> = self
            .names
            .iter()
            .map(|n| {
                let mut row = ListRow::style(n);
                if self.locked == Some(n.as_str()) {
                    row = row.locked();
                }
                if self.marked == Some(n.as_str()) {
                    row = row.marker("*");
                }
                row
            })
            .collect();
        rows.push(ListRow::placeholder("...new"));
        let existing = self.names.clone();
        let validate = |_: &str, n: &str| validate_like_a_host(&existing, n);
        let can_delete = |n: &str| n != "Alpha";
        let out = StyleList::new(KEY, &validate)
            .duplicate()
            .delete_if(&can_delete)
            .show(
                ui,
                &sk,
                sk.rect(40.0, 40.0, 300.0, 300.0),
                rows,
                self.selected,
                &mut self.mode,
            );
        if let Some(i) = out.selected() {
            self.selected = Some(i);
        }
        self.events.extend(out.events);
    }

    fn renamed(&self) -> Vec<(String, String)> {
        self.events
            .iter()
            .filter_map(|e| match e {
                ListEvent::RenameRequested(a, b) => Some((a.clone(), b.clone())),
                _ => None,
            })
            .collect()
    }
}

/// Centre of list row `i` in the 1:1 test frame.
fn row_pos(i: usize) -> Pos2 {
    Pos2::new(120.0, 40.0 + 30.0 + i as f32 * (24.2 + 3.0) + 12.0)
}

fn type_text(h: &mut Harness, f: &mut Fixture, text: &str) {
    h.frame(vec![Event::Text(text.to_owned())], &mut |ui| f.show(ui));
    h.settle(1, &mut |ui| f.show(ui));
}

fn edit_of(h: &Harness) -> Option<RenameEdit> {
    h.ctx
        .memory(|m| m.data.get_temp::<Option<RenameEdit>>(rename_edit_id(KEY)))
        .flatten()
}

fn renaming(name: &str) -> (Harness, Fixture) {
    let mut h = Harness::new();
    let mut f = Fixture::new();
    h.settle(2, &mut |ui| f.show(ui));
    start_rename(&h.ctx, KEY, name);
    h.settle(3, &mut |ui| f.show(ui));
    (h, f)
}

fn double_click(h: &mut Harness, f: &mut Fixture, pos: Pos2) {
    h.click(pos, &mut |ui| f.show(ui));
    h.click(pos, &mut |ui| f.show(ui));
}

#[test]
fn double_click_on_a_row_starts_the_rename_there() {
    let mut h = Harness::new();
    let mut f = Fixture::new();
    h.settle(2, &mut |ui| f.show(ui));
    assert!(edit_of(&h).is_none());
    double_click(&mut h, &mut f, row_pos(1));
    assert_eq!(edit_of(&h).map(|e| e.original), Some("Beta".to_owned()));
}

#[test]
fn double_click_on_the_placeholder_row_does_not_rename() {
    let mut h = Harness::new();
    let mut f = Fixture::new();
    h.settle(2, &mut |ui| f.show(ui));
    double_click(&mut h, &mut f, row_pos(3));
    assert!(edit_of(&h).is_none());
}

#[test]
fn f2_renames_the_selected_row() {
    let mut h = Harness::new();
    let mut f = Fixture::new();
    h.settle(2, &mut |ui| f.show(ui));
    h.key(Key::F2, &mut |ui| f.show(ui));
    assert!(edit_of(&h).is_none(), "nothing selected, nothing to rename");
    h.click(row_pos(2), &mut |ui| f.show(ui));
    h.key(Key::F2, &mut |ui| f.show(ui));
    assert_eq!(edit_of(&h).map(|e| e.original), Some("Gamma".to_owned()));
}

fn show_list_at(ui: &mut egui::Ui, key: &str, x: f32, selected: Option<usize>) {
    let sk = SketchFrame::fit(ui.max_rect(), Vec2::new(900.0, 700.0));
    let mut mode = StyleListMode::List;
    let mut rows: Vec<ListRow<'_>> = NAMES.iter().map(|n| ListRow::style(n)).collect();
    rows.push(ListRow::placeholder("...new"));
    let validate = |_: &str, _: &str| Ok(());
    let _ = StyleList::new(key, &validate).show(
        ui,
        &sk,
        sk.rect(x, 40.0, 300.0, 300.0),
        rows,
        selected,
        &mut mode,
    );
}

fn open_edit(h: &Harness, key: &str) -> bool {
    rename_is_open(&h.ctx, key)
}

/// With two lists open, F2 renames in the one the user is working in (pointer
/// over it, or last pressed in), never in both, and the keystroke is consumed.
#[test]
fn f2_only_opens_the_focused_list_and_is_consumed() {
    let mut h = Harness::new();
    let left_over_f2 = std::cell::Cell::new(true);
    let mut show = |ui: &mut egui::Ui| {
        show_list_at(ui, "mgr_a", 40.0, Some(0));
        show_list_at(ui, "mgr_b", 400.0, Some(1));
        left_over_f2.set(ui.input(|i| i.key_pressed(Key::F2)));
    };
    h.settle(2, &mut show);

    h.frame(
        vec![Event::PointerMoved(Pos2::new(350.0, 600.0))],
        &mut show,
    );
    h.key(Key::F2, &mut show);
    assert!(!open_edit(&h, "mgr_a") && !open_edit(&h, "mgr_b"));

    h.frame(
        vec![Event::PointerMoved(Pos2::new(120.0, 100.0))],
        &mut show,
    );
    h.frame(
        vec![Event::Key {
            key: Key::F2,
            physical_key: Some(Key::F2),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        &mut show,
    );
    assert!(!left_over_f2.get(), "F2 must be consumed by the list");
    assert!(open_edit(&h, "mgr_a"), "the hovered list renames");
    assert!(!open_edit(&h, "mgr_b"), "a background list does not");
}

#[test]
fn f2_follows_the_list_last_pressed_in() {
    let mut h = Harness::new();
    let mut show = |ui: &mut egui::Ui| {
        show_list_at(ui, "mgr_a", 40.0, Some(0));
        show_list_at(ui, "mgr_b", 400.0, Some(1));
    };
    h.settle(2, &mut show);
    h.click(Pos2::new(520.0, 100.0), &mut show);
    h.frame(
        vec![Event::PointerMoved(Pos2::new(350.0, 600.0))],
        &mut show,
    );
    h.key(Key::F2, &mut show);
    assert!(open_edit(&h, "mgr_b") && !open_edit(&h, "mgr_a"));

    h.key(Key::Escape, &mut show);
    assert!(!open_edit(&h, "mgr_b"));
    h.click(Pos2::new(350.0, 600.0), &mut show);
    h.key(Key::F2, &mut show);
    assert!(!open_edit(&h, "mgr_a") && !open_edit(&h, "mgr_b"));
}

#[test]
fn new_row_opens_in_rename_with_its_name_selected() {
    let (h, _f) = renaming("Beta");
    let id = rename_edit_id(KEY).with("field");
    let range = egui::TextEdit::load_state(&h.ctx, id)
        .and_then(|s| s.cursor.char_range())
        .expect("cursor state");
    let (a, b) = (range.primary.index.0, range.secondary.index.0);
    assert_eq!((a.min(b), a.max(b)), (0, 4), "whole name selected");
    assert!(h.ctx.memory(|m| m.has_focus(id)), "field has focus");
}

#[test]
fn enter_commits_the_typed_name_once() {
    let (mut h, mut f) = renaming("Alpha");
    type_text(&mut h, &mut f, "Delta");
    h.key(Key::Enter, &mut |ui| f.show(ui));
    assert_eq!(f.renamed(), vec![("Alpha".to_owned(), "Delta".to_owned())]);
    assert!(edit_of(&h).is_none());
}

#[test]
fn escape_cancels_without_a_rename() {
    let (mut h, mut f) = renaming("Alpha");
    type_text(&mut h, &mut f, "Delta");
    h.key(Key::Escape, &mut |ui| f.show(ui));
    assert!(f.renamed().is_empty());
    assert!(edit_of(&h).is_none());
}

#[test]
fn the_open_editor_owns_escape_for_one_frame() {
    let (h, _f) = renaming("Alpha");
    assert!(rename_editor_owns_escape(&h.ctx));
}

#[test]
fn clicking_another_row_commits_and_still_selects_it() {
    let (mut h, mut f) = renaming("Alpha");
    type_text(&mut h, &mut f, "Delta");
    h.click(row_pos(2), &mut |ui| f.show(ui));
    assert_eq!(f.renamed(), vec![("Alpha".to_owned(), "Delta".to_owned())]);
    assert_eq!(f.selected, Some(2), "the click still selects its row");
}

#[test]
fn unchanged_name_closes_quietly() {
    let (mut h, mut f) = renaming("Alpha");
    h.key(Key::Enter, &mut |ui| f.show(ui));
    assert!(f.renamed().is_empty());
    assert!(edit_of(&h).is_none());
}

#[test]
fn invalid_names_keep_the_editor_open_with_a_message() {
    for (typed, message) in [
        ("Beta", "A style with that name already exists"),
        ("default", "'Default' and 'None' are reserved names"),
        ("None", "'Default' and 'None' are reserved names"),
        ("   ", "A name cannot be empty"),
    ] {
        let (mut h, mut f) = renaming("Alpha");
        type_text(&mut h, &mut f, typed);
        h.key(Key::Enter, &mut |ui| f.show(ui));
        assert!(f.renamed().is_empty(), "{typed:?} must not rename");
        let edit = edit_of(&h).unwrap_or_else(|| panic!("{typed:?} closed the editor"));
        assert_eq!(edit.error.as_deref(), Some(message), "{typed:?}");
        assert_eq!(edit.text, typed, "the user's text is kept");
    }
}

#[test]
fn a_refused_name_can_be_fixed_and_committed() {
    let (mut h, mut f) = renaming("Alpha");
    type_text(&mut h, &mut f, "Beta");
    h.key(Key::Enter, &mut |ui| f.show(ui));
    assert!(edit_of(&h).is_some());
    h.frame(
        vec![Event::Key {
            key: Key::A,
            physical_key: Some(Key::A),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::COMMAND,
        }],
        &mut |ui| f.show(ui),
    );
    type_text(&mut h, &mut f, "Delta");
    h.key(Key::Enter, &mut |ui| f.show(ui));
    assert_eq!(f.renamed(), vec![("Alpha".to_owned(), "Delta".to_owned())]);
}

fn right_click(h: &mut Harness, f: &mut Fixture, pos: Pos2) {
    h.frame(vec![Event::PointerMoved(pos)], &mut |ui| f.show(ui));
    for pressed in [true, false] {
        h.frame(
            vec![Event::PointerButton {
                pos,
                button: PointerButton::Secondary,
                pressed,
                modifiers: Modifiers::NONE,
            }],
            &mut |ui| f.show(ui),
        );
    }
    h.settle(2, &mut |ui| f.show(ui));
}

fn menu_state(h: &Harness) -> Option<(String, Pos2)> {
    h.ctx
        .memory(|m| {
            m.data
                .get_temp::<Option<(String, Pos2)>>(rename_menu_id(KEY))
        })
        .flatten()
}

#[test]
fn right_click_menu_offers_rename_duplicate_and_delete() {
    let mut h = Harness::new();
    let mut f = Fixture::new();
    h.settle(2, &mut |ui| f.show(ui));
    right_click(&mut h, &mut f, row_pos(1));
    let (name, at) = menu_state(&h).expect("menu open");
    assert_eq!(name, "Beta");
    // Rename is the first item, just inside the menu frame.
    h.click(at + Vec2::new(20.0, 14.0), &mut |ui| f.show(ui));
    assert_eq!(edit_of(&h).map(|e| e.original), Some("Beta".to_owned()));

    // Duplicate is the second item.
    let (mut h, mut f) = (Harness::new(), Fixture::new());
    h.settle(2, &mut |ui| f.show(ui));
    right_click(&mut h, &mut f, row_pos(0));
    let at = menu_state(&h).expect("menu open").1;
    h.click(at + Vec2::new(20.0, 38.0), &mut |ui| f.show(ui));
    assert!(f.events.contains(&ListEvent::Duplicate("Alpha".to_owned())));
}

#[test]
fn the_placeholder_row_has_no_context_menu() {
    let mut h = Harness::new();
    let mut f = Fixture::new();
    h.settle(2, &mut |ui| f.show(ui));
    right_click(&mut h, &mut f, row_pos(3));
    assert!(menu_state(&h).is_none());
}

#[test]
fn a_locked_row_reports_the_denied_rename_instead_of_opening_an_editor() {
    let mut h = Harness::new();
    let mut f = Fixture::new();
    f.locked = Some("Beta");
    h.settle(2, &mut |ui| f.show(ui));
    double_click(&mut h, &mut f, row_pos(1));
    assert!(edit_of(&h).is_none());
    assert!(f.events.contains(&ListEvent::RenameDenied(1)));
    // F2 on the selected locked row does the same.
    f.events.clear();
    h.key(Key::F2, &mut |ui| f.show(ui));
    assert!(f.events.contains(&ListEvent::RenameDenied(1)));
}

#[test]
fn a_marker_shows_after_the_name_but_never_in_the_name() {
    let mut h = Harness::new();
    let mut f = Fixture::new();
    f.marked = Some("Beta");
    h.settle(2, &mut |ui| f.show(ui));
    let out = h.frame(Vec::new(), &mut |ui| f.show(ui));
    assert!(painted_text(&out).iter().any(|t| t == "Beta *"));
    double_click(&mut h, &mut f, row_pos(1));
    assert_eq!(edit_of(&h).map(|e| e.original), Some("Beta".to_owned()));
}

#[test]
fn a_pending_rename_waits_for_its_row_without_taking_over() {
    let (mut h, mut f) = (Harness::new(), Fixture::new());
    f.mode = StyleListMode::Thumbnails;
    h.settle(2, &mut |ui| f.show(ui));
    start_rename(&h.ctx, KEY, "Zeta");
    h.settle(3, &mut |ui| f.show(ui));
    assert_eq!(f.mode, StyleListMode::Thumbnails, "list left as it was");
    f.mode = StyleListMode::List;
    h.click(row_pos(2), &mut |ui| f.show(ui));
    h.key(Key::F2, &mut |ui| f.show(ui));
    assert_eq!(edit_of(&h).map(|e| e.original), Some("Gamma".to_owned()));

    let (mut h, mut f) = (Harness::new(), Fixture::new());
    f.mode = StyleListMode::Thumbnails;
    h.settle(2, &mut |ui| f.show(ui));
    start_rename(&h.ctx, KEY, "Zeta");
    h.settle(2, &mut |ui| f.show(ui));
    f.names.push("Zeta".to_owned());
    h.settle(3, &mut |ui| f.show(ui));
    assert_eq!(f.mode, StyleListMode::List);
    assert_eq!(edit_of(&h).map(|e| e.original), Some("Zeta".to_owned()));
}

#[test]
fn a_pending_rename_whose_entry_never_appears_expires_by_time() {
    let (mut h, mut f) = (Harness::new(), Fixture::new());
    h.settle(2, &mut |ui| f.show(ui));
    start_rename(&h.ctx, KEY, "Zeta");
    h.settle(2, &mut |ui| f.show(ui));
    assert!(edit_of(&h).is_some(), "still within the grace period");
    h.advance(3.0);
    h.settle(1, &mut |ui| f.show(ui));
    assert!(edit_of(&h).is_none(), "expired");
    f.names.push("Zeta".to_owned());
    h.settle(3, &mut |ui| f.show(ui));
    assert!(
        edit_of(&h).is_none(),
        "no editor on a later entry of that name"
    );
}

#[test]
fn a_rename_in_thumbnail_mode_switches_to_the_list_and_says_so() {
    let (mut h, mut f) = (Harness::new(), Fixture::new());
    f.mode = StyleListMode::Thumbnails;
    h.settle(2, &mut |ui| f.show(ui));
    start_rename(&h.ctx, KEY, "Alpha");
    h.settle(2, &mut |ui| f.show(ui));
    assert_eq!(f.mode, StyleListMode::List);
    assert!(f
        .events
        .contains(&ListEvent::ViewModeChanged(StyleListMode::List)));
}

#[test]
fn a_rename_the_command_refused_reopens_with_a_message() {
    let (mut h, mut f) = renaming("Alpha");
    type_text(&mut h, &mut f, "Delta");
    h.key(Key::Enter, &mut |ui| f.show(ui));
    assert_eq!(f.renamed(), vec![("Alpha".to_owned(), "Delta".to_owned())]);
    assert!(edit_of(&h).is_none());
    h.advance(5.0);
    h.settle(2, &mut |ui| f.show(ui));
    assert!(edit_of(&h).is_none(), "a slow command is not a refusal yet");
    h.advance(6.0);
    h.settle(2, &mut |ui| f.show(ui));
    let edit = edit_of(&h).expect("editor reopened");
    assert_eq!(edit.error.as_deref(), Some("The rename was refused"));
    assert_eq!(
        (edit.original.as_str(), edit.text.as_str()),
        ("Alpha", "Delta")
    );
    assert!(f
        .events
        .contains(&ListEvent::RenameRefused("Alpha".into(), "Delta".into())));
}

#[test]
fn a_rename_that_took_effect_is_not_reported_as_refused() {
    let (mut h, mut f) = renaming("Alpha");
    type_text(&mut h, &mut f, "Delta");
    h.key(Key::Enter, &mut |ui| f.show(ui));
    f.names[0] = "Delta".to_owned();
    h.advance(1.5);
    h.settle(3, &mut |ui| f.show(ui));
    assert!(edit_of(&h).is_none());
    assert!(!f
        .events
        .iter()
        .any(|e| matches!(e, ListEvent::RenameRefused(..))));
}

#[test]
fn the_switch_in_the_list_header_changes_the_mode_and_reports_it() {
    let mut h = Harness::new();
    let mut f = Fixture::new();
    h.settle(2, &mut |ui| f.show(ui));
    let sk = SketchFrame::fit(
        Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0)),
        Vec2::new(900.0, 700.0),
    );
    let area = view_mode_switch_area(&sk, sk.rect(40.0, 40.0, 300.0, 300.0));
    let [_, thumbs] = view_mode_switch_rects(&sk, area);
    h.click(thumbs.center(), &mut |ui| f.show(ui));
    assert_eq!(f.mode, StyleListMode::Thumbnails);
    assert!(f
        .events
        .contains(&ListEvent::ViewModeChanged(StyleListMode::Thumbnails)));
}

#[test]
fn save_status_uses_the_ok_and_warning_colours() {
    let v = egui::Visuals::dark();
    assert_eq!(SaveStatus::Saved.color(&v), crate::theme::ok_color(&v));
    assert_eq!(
        SaveStatus::Unsaved.color(&v),
        crate::theme::warning_color(&v)
    );
    assert_eq!(SaveStatus::Saved.text(), "Saved");
    assert!(SaveStatus::Unsaved.text().starts_with("Unsaved changes"));
}

#[test]
fn the_copy_button_label_is_short() {
    assert_eq!(COPY_BUTTON_LABEL, "Copy");
}

#[test]
fn cancel_rename_closes_an_open_editor() {
    let (h, _f) = renaming("Alpha");
    assert!(rename_is_open(&h.ctx, KEY));
    cancel_rename(&h.ctx, KEY);
    assert!(!rename_is_open(&h.ctx, KEY));
}

#[test]
fn a_slow_command_that_finally_renames_is_never_reported_refused() {
    let (mut h, mut f) = renaming("Alpha");
    type_text(&mut h, &mut f, "Delta");
    h.key(Key::Enter, &mut |ui| f.show(ui));
    // The main window takes 4 s to run the command.
    h.advance(4.0);
    h.settle(2, &mut |ui| f.show(ui));
    f.names[0] = "Delta".to_owned();
    h.settle(3, &mut |ui| f.show(ui));
    h.advance(12.0);
    h.settle(3, &mut |ui| f.show(ui));
    assert!(!f
        .events
        .iter()
        .any(|e| matches!(e, ListEvent::RenameRefused(..))));
    assert!(edit_of(&h).is_none());
}

#[test]
fn an_open_menu_owns_escape_and_reports_itself_open() {
    let mut h = Harness::new();
    let mut f = Fixture::new();
    h.settle(2, &mut |ui| f.show(ui));
    assert!(!menu_is_open(&h.ctx, KEY));
    right_click(&mut h, &mut f, row_pos(1));
    assert!(menu_is_open(&h.ctx, KEY));
    assert!(rename_editor_owns_escape(&h.ctx));
    h.key(Key::Escape, &mut |ui| f.show(ui));
    assert!(!menu_is_open(&h.ctx, KEY));
}

#[test]
fn a_claimed_escape_is_owned_for_one_frame() {
    let h = Harness::new();
    assert!(!rename_editor_owns_escape(&h.ctx));
    claim_escape(&h.ctx);
    assert!(rename_editor_owns_escape(&h.ctx));
}
