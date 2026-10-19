use super::*;
use egui::{PointerButton, RawInput};

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
    fn frame(&mut self, events: Vec<Event>, ui_fn: &mut dyn FnMut(&mut Ui)) -> egui::FullOutput {
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
    fn settle(&mut self, n: u32, ui_fn: &mut dyn FnMut(&mut Ui)) {
        for _ in 0..n {
            self.frame(Vec::new(), ui_fn);
        }
    }
    fn click(&mut self, pos: Pos2, ui_fn: &mut dyn FnMut(&mut Ui)) {
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
    fn key(&mut self, key: Key, ui_fn: &mut dyn FnMut(&mut Ui)) {
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

const NAMES: [&str; 4] = ["Solid", "Hatch", "Gradient", "None"];

fn control_at(y: f32) -> Rect {
    Rect::from_min_size(Pos2::new(40.0, y), Vec2::new(200.0, 28.0))
}

/// Shared fixture: one list dropdown at a fixed rect, selection kept in `value`.
struct Fx {
    value: usize,
    picked: Vec<usize>,
    last: Option<DropdownResponse>,
    control: Rect,
    options: Vec<&'static str>,
}

impl Fx {
    fn new(control: Rect) -> Self {
        Self {
            value: 1,
            picked: Vec::new(),
            last: None,
            control,
            options: NAMES.to_vec(),
        }
    }
    fn show(&mut self, ui: &mut Ui) {
        let out = Dropdown::new("dd")
            .options(self.options.clone())
            .selected(if self.options.is_empty() {
                None
            } else {
                Some(self.value)
            })
            .at_rect(self.control)
            .show(ui);
        if let Some(i) = out.chosen {
            self.picked.push(i);
            self.value = i;
        }
        self.last = Some(out);
    }
}

fn texts(out: &egui::FullOutput) -> Vec<(String, Pos2)> {
    fn walk(shape: &egui::Shape, out: &mut Vec<(String, Pos2)>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::Shape::Text(t) => out.push((
                t.galley.text().to_owned(),
                t.pos + t.galley.rect.center().to_vec2(),
            )),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for c in &out.shapes {
        walk(&c.shape, &mut v);
    }
    v
}

fn text_pos(out: &egui::FullOutput, label: &str) -> Pos2 {
    texts(out)
        .into_iter()
        .rev()
        .find(|(t, _)| t == label)
        .unwrap_or_else(|| panic!("{label:?} not painted"))
        .1
}

fn open(h: &mut Harness, fx: &mut Fx) {
    h.settle(3, &mut |ui| fx.show(ui));
    let c = fx.control.center();
    h.click(c, &mut |ui| fx.show(ui));
}

const MIN_BELOW: f32 = 150.0;

#[test]
fn direction_is_down_when_it_fits_below_even_if_more_room_above() {
    let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 1000.0));
    let control = Rect::from_min_size(Pos2::new(10.0, 600.0), Vec2::new(100.0, 30.0));
    let p = choose_direction(control, 200.0, bounds, MIN_BELOW);
    assert_eq!(p.direction, Direction::Down);
    assert_eq!(p.height, 200.0);
}

#[test]
fn direction_shrinks_and_scrolls_down_while_about_four_rows_fit_below() {
    // 180 below the control's top: not the whole 400, but more than four rows.
    let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 700.0));
    let control = Rect::from_min_size(Pos2::new(10.0, 520.0), Vec2::new(100.0, 30.0));
    let p = choose_direction(control, 400.0, bounds, MIN_BELOW);
    assert_eq!(
        p.direction,
        Direction::Down,
        "not flipped by the window edge"
    );
    assert_eq!(p.height, 180.0);
}

#[test]
fn direction_flips_up_only_when_fewer_than_four_rows_fit_below_and_above_has_more() {
    let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 700.0));
    let control = Rect::from_min_size(Pos2::new(10.0, 620.0), Vec2::new(100.0, 30.0));
    let p = choose_direction(control, 200.0, bounds, MIN_BELOW);
    assert_eq!(p.direction, Direction::Up);
    assert_eq!(p.height, 200.0);
    // Less room above than below: stays down however little there is.
    let control = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(100.0, 30.0));
    let tiny = Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 120.0));
    assert_eq!(
        choose_direction(control, 200.0, tiny, MIN_BELOW).direction,
        Direction::Down
    );
}

#[test]
fn a_taller_monitor_rect_keeps_the_list_down_where_a_window_rect_flips_it() {
    let control = Rect::from_min_size(Pos2::new(10.0, 300.0), Vec2::new(100.0, 30.0));
    let window = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 400.0));
    let monitor = Rect::from_min_size(Pos2::new(-100.0, -100.0), Vec2::new(1920.0, 1080.0));
    assert_eq!(
        choose_direction(control, 250.0, window, MIN_BELOW).direction,
        Direction::Up
    );
    assert_eq!(
        choose_direction(control, 250.0, monitor, MIN_BELOW).direction,
        Direction::Down
    );
}

#[test]
fn direction_scrolls_on_the_roomier_side_when_neither_fits() {
    let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 300.0));
    let control = Rect::from_min_size(Pos2::new(10.0, 100.0), Vec2::new(100.0, 30.0));
    let p = choose_direction(control, 900.0, bounds, MIN_BELOW);
    assert_eq!(p.direction, Direction::Down);
    assert_eq!(p.height, 200.0, "from the control's top to the bottom edge");
    let control = Rect::from_min_size(Pos2::new(10.0, 200.0), Vec2::new(100.0, 30.0));
    let p = choose_direction(control, 900.0, bounds, MIN_BELOW);
    assert_eq!(p.direction, Direction::Up);
    assert_eq!(p.height, 230.0, "from the top edge to the control's bottom");
}

#[test]
fn rounding_touches_only_the_outer_edges() {
    let r = row_rounding(5, true, false);
    assert_eq!((r.nw, r.ne, r.sw, r.se), (5, 5, 0, 0));
    let r = row_rounding(5, false, true);
    assert_eq!((r.nw, r.ne, r.sw, r.se), (0, 0, 5, 5));
}

#[test]
fn type_ahead_wraps_and_ignores_case() {
    let t = ["Alpha", "beta", "Bravo", "Charlie"];
    assert_eq!(type_ahead(&t, None, 'b'), Some(1));
    assert_eq!(type_ahead(&t, Some(1), 'B'), Some(2));
    assert_eq!(type_ahead(&t, Some(2), 'b'), Some(1));
    assert_eq!(type_ahead(&t, Some(0), 'z'), None);
}

#[test]
fn closed_dropdown_shows_the_current_value_and_no_list() {
    let mut h = Harness::new();
    let mut fx = Fx::new(control_at(40.0));
    h.settle(3, &mut |ui| fx.show(ui));
    let out = h.frame(Vec::new(), &mut |ui| fx.show(ui));
    let t: Vec<String> = texts(&out).into_iter().map(|(t, _)| t).collect();
    assert_eq!(t, vec!["Hatch".to_owned()], "only the current value: {t:?}");
    assert!(fx.last.as_ref().unwrap().popup_rect.is_none());
}

#[test]
fn open_box_is_one_box_that_starts_at_the_control_with_the_current_on_top() {
    let mut h = Harness::new();
    let mut fx = Fx::new(control_at(40.0));
    open(&mut h, &mut fx);
    let out = h.frame(Vec::new(), &mut |ui| fx.show(ui));
    let last = fx.last.as_ref().unwrap();
    let boxr = last.popup_rect.expect("open");
    assert_eq!(boxr.left(), fx.control.left(), "edges line up");
    assert_eq!(boxr.right(), fx.control.right(), "same width");
    assert_eq!(
        boxr.top(),
        fx.control.top(),
        "no gap: the box begins at the control"
    );
    // The current option is the first row, in the control's own rectangle.
    let cur = text_pos(&out, "Hatch");
    assert!(
        fx.control.contains(cur),
        "current sits in the control row: {cur:?}"
    );
    // Remaining options follow in their original order, below it.
    let ys: Vec<f32> = ["Solid", "Gradient", "None"]
        .iter()
        .map(|l| text_pos(&out, l).y)
        .collect();
    assert!(
        cur.y < ys[0] && ys[0] < ys[1] && ys[1] < ys[2],
        "{cur:?} {ys:?}"
    );
    assert!(ys[0] > fx.control.bottom(), "list is under the control row");
    assert_eq!(last.direction, Some(Direction::Down));
    // Hatch is not painted a second time as a list row.
    assert_eq!(
        texts(&out).iter().filter(|(t, _)| t == "Hatch").count(),
        2,
        "once for the closed face under the box, once for the box's own top row"
    );
}

#[test]
fn clicking_a_row_selects_and_emits_once_and_closes() {
    let mut h = Harness::new();
    let mut fx = Fx::new(control_at(40.0));
    open(&mut h, &mut fx);
    let out = h.frame(Vec::new(), &mut |ui| fx.show(ui));
    let at = text_pos(&out, "Gradient");
    h.click(at, &mut |ui| fx.show(ui));
    assert_eq!(fx.picked, vec![2]);
    assert_eq!(fx.value, 2);
    assert!(fx.last.as_ref().unwrap().popup_rect.is_none(), "closed");
}

#[test]
fn clicking_the_current_row_closes_without_emitting() {
    let mut h = Harness::new();
    let mut fx = Fx::new(control_at(40.0));
    open(&mut h, &mut fx);
    assert!(fx.last.as_ref().unwrap().popup_rect.is_some());
    h.click(fx.control.center(), &mut |ui| fx.show(ui));
    assert!(fx.picked.is_empty());
    assert!(fx.last.as_ref().unwrap().popup_rect.is_none());
}

#[test]
fn click_outside_and_escape_close() {
    let mut h = Harness::new();
    let mut fx = Fx::new(control_at(40.0));
    open(&mut h, &mut fx);
    h.click(Pos2::new(700.0, 500.0), &mut |ui| fx.show(ui));
    assert!(
        fx.last.as_ref().unwrap().popup_rect.is_none(),
        "outside click"
    );
    assert!(fx.picked.is_empty());
    h.click(fx.control.center(), &mut |ui| fx.show(ui));
    assert!(fx.last.as_ref().unwrap().popup_rect.is_some(), "reopened");
    h.key(Key::Escape, &mut |ui| fx.show(ui));
    assert!(fx.last.as_ref().unwrap().popup_rect.is_none(), "Esc");
    assert!(fx.picked.is_empty());
}

#[test]
fn keyboard_tab_enter_arrows_enter_picks() {
    let mut h = Harness::new();
    let mut fx = Fx::new(control_at(40.0));
    h.settle(3, &mut |ui| fx.show(ui));
    h.key(Key::Tab, &mut |ui| fx.show(ui));
    assert!(
        fx.last.as_ref().unwrap().response.has_focus(),
        "Tab focuses"
    );
    h.key(Key::Enter, &mut |ui| fx.show(ui));
    assert!(
        fx.last.as_ref().unwrap().popup_rect.is_some(),
        "Enter opens"
    );
    // The cursor starts on the current row (Hatch); two steps down: Solid, Gradient.
    h.key(Key::ArrowDown, &mut |ui| fx.show(ui));
    h.key(Key::ArrowDown, &mut |ui| fx.show(ui));
    h.key(Key::Enter, &mut |ui| fx.show(ui));
    assert_eq!(fx.picked, vec![2], "Gradient");
    assert!(fx.last.as_ref().unwrap().popup_rect.is_none());
}

#[test]
fn keyboard_type_ahead_jumps_to_the_matching_option() {
    let mut h = Harness::new();
    let mut fx = Fx::new(control_at(40.0));
    h.settle(3, &mut |ui| fx.show(ui));
    h.key(Key::Tab, &mut |ui| fx.show(ui));
    h.key(Key::Enter, &mut |ui| fx.show(ui));
    h.frame(vec![Event::Text("n".into())], &mut |ui| fx.show(ui));
    h.settle(1, &mut |ui| fx.show(ui));
    h.key(Key::Enter, &mut |ui| fx.show(ui));
    assert_eq!(fx.picked, vec![3], "None");
}

#[test]
fn list_flips_up_only_when_the_window_has_no_room_below_keeping_reading_order() {
    let mut h = Harness::new();
    // Screen is 700 high; 4 rows do not fit below a control at y=640.
    let mut fx = Fx::new(control_at(640.0));
    open(&mut h, &mut fx);
    let out = h.frame(Vec::new(), &mut |ui| fx.show(ui));
    let last = fx.last.as_ref().unwrap();
    assert_eq!(last.direction, Some(Direction::Up));
    let boxr = last.popup_rect.unwrap();
    assert_eq!(
        boxr.bottom(),
        fx.control.bottom(),
        "box ends at the control"
    );
    assert!(boxr.top() >= 0.0);
    let cur = text_pos(&out, "Hatch");
    assert!(fx.control.contains(cur), "current stays in the control row");
    // Reading order, the same as a list that opens down.
    assert!(text_pos(&out, "Solid").y < text_pos(&out, "Gradient").y);
    assert!(text_pos(&out, "Gradient").y < text_pos(&out, "None").y);
    assert!(
        text_pos(&out, "None").y < cur.y,
        "the current row is the bottom row"
    );
    // Arrow Up from the current row moves to the option right above it.
    h.key(Key::ArrowUp, &mut |ui| fx.show(ui));
    h.key(Key::Enter, &mut |ui| fx.show(ui));
    assert_eq!(
        fx.picked,
        vec![3],
        "None, the last option, sits above the control"
    );
}

#[test]
fn explicit_bounds_decide_the_direction() {
    let mut h = Harness::new();
    let mut fx = Fx::new(control_at(300.0));
    h.settle(3, &mut |ui| fx.show(ui));
    let tall = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 2000.0));
    let short = Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 330.0));
    let dir = std::cell::Cell::new(None);
    let show = |ui: &mut Ui, b: Rect| {
        let out = Dropdown::new("b")
            .options(NAMES)
            .selected(Some(0))
            .at_rect(control_at(300.0))
            .bounds(b)
            .show(ui);
        dir.set(out.direction);
    };
    h.click(control_at(300.0).center(), &mut |ui| show(ui, tall));
    h.settle(1, &mut |ui| show(ui, tall));
    assert_eq!(dir.get(), Some(Direction::Down));
    h.settle(1, &mut |ui| show(ui, short));
    assert_eq!(dir.get(), Some(Direction::Up));
}

#[test]
fn empty_list_shows_a_message_and_emits_nothing() {
    let mut h = Harness::new();
    let mut fx = Fx::new(control_at(40.0));
    fx.options.clear();
    open(&mut h, &mut fx);
    let out = h.frame(Vec::new(), &mut |ui| fx.show(ui));
    assert!(texts(&out).iter().any(|(t, _)| t == "No options"));
    assert!(fx.picked.is_empty());
}

#[test]
fn long_lists_scroll_after_ten_rows_and_still_open_down_where_ten_rows_fit() {
    let mut h = Harness::new();
    let many: Vec<String> = (0..60).map(|i| format!("Option {i}")).collect();
    let seen = std::cell::Cell::new((None, None));
    let mut show = |ui: &mut Ui| {
        let out = Dropdown::new("long")
            .options(many.clone())
            .selected(Some(0))
            .at_rect(control_at(300.0))
            .show(ui);
        seen.set((out.popup_rect.map(|r| r.height()), out.direction));
    };
    h.settle(3, &mut show);
    h.click(control_at(300.0).center(), &mut show);
    h.settle(1, &mut show);
    let (height, dir) = seen.get();
    // Control row + ten rows + the bottom margin, not sixty rows.
    assert!(height.unwrap() < 28.0 + 11.0 * 30.0, "{height:?}");
    assert_eq!(
        dir,
        Some(Direction::Down),
        "ten rows fit below, so it stays down"
    );
}

#[test]
fn a_list_that_fits_nowhere_scrolls_within_the_roomier_side() {
    let mut h = Harness::new();
    let many: Vec<String> = (0..60).map(|i| format!("Option {i}")).collect();
    let seen = std::cell::Cell::new(None);
    let mut show = |ui: &mut Ui| {
        let out = Dropdown::new("long2")
            .options(many.clone())
            .selected(Some(0))
            .max_popup_height(2000.0)
            .at_rect(control_at(100.0))
            .show(ui);
        seen.set(out.popup_rect.map(|r| r.height()));
    };
    h.settle(3, &mut show);
    h.click(control_at(100.0).center(), &mut show);
    h.settle(1, &mut show);
    assert!(
        seen.get().unwrap() <= 700.0 - 100.0 + 0.5,
        "{:?}",
        seen.get()
    );
}

#[test]
fn grid_layout_emits_the_clicked_tile_and_highlights_the_current() {
    let mut h = Harness::new();
    let picked = std::cell::RefCell::new(Vec::new());
    let thumbs = std::cell::RefCell::new(Vec::new());
    let mut show = |ui: &mut Ui| {
        let mut d = Dropdown::new("grid")
            .selected_text("Type: Brick")
            .selected(Some(1))
            .at_rect(control_at(40.0))
            .grid(3, Vec2::new(80.0, 64.0));
        for name in ["Default", "Brick", "Concrete", "Stone"] {
            let thumbs = &thumbs;
            d = d.option(DropdownOption::new(name).with_thumb(move |_ui, r| {
                thumbs.borrow_mut().push((name, r));
            }));
        }
        if let Some(i) = d.show(ui).chosen {
            picked.borrow_mut().push(i);
        }
    };
    h.settle(3, &mut show);
    h.click(control_at(40.0).center(), &mut show);
    thumbs.borrow_mut().clear();
    h.settle(1, &mut show);
    let t = thumbs.borrow().clone();
    let r = |n: &str| t.iter().rev().find(|(x, _)| *x == n).unwrap().1;
    assert_eq!(
        r("Default").width(),
        80.0 - 6.0,
        "thumbnail fills the tile width less its inset"
    );
    assert!(
        r("Brick").left() > r("Default").left(),
        "same row, next column"
    );
    assert_eq!(r("Brick").top(), r("Default").top());
    assert!(
        r("Stone").top() > r("Default").top(),
        "fourth tile wraps to row two"
    );
    assert!(
        r("Default").top() > control_at(40.0).bottom(),
        "tiles are under the control row"
    );
    h.click(r("Concrete").center(), &mut show);
    assert_eq!(*picked.borrow(), vec![2]);
}

#[test]
fn grid_keyboard_moves_by_column_and_row() {
    let mut h = Harness::new();
    let picked = std::cell::RefCell::new(Vec::new());
    let mut show = |ui: &mut Ui| {
        let out = Dropdown::new("grid")
            .options(["a", "b", "c", "d"])
            .selected(Some(0))
            .at_rect(control_at(40.0))
            .grid(2, Vec2::new(60.0, 40.0))
            .show(ui);
        if let Some(i) = out.chosen {
            picked.borrow_mut().push(i);
        }
    };
    h.settle(3, &mut show);
    h.key(Key::Tab, &mut show);
    h.key(Key::Enter, &mut show);
    h.key(Key::ArrowDown, &mut show); // no cursor yet -> first tile (a)
    h.key(Key::ArrowDown, &mut show); // a -> c
    h.key(Key::ArrowRight, &mut show); // c -> d
    h.key(Key::Enter, &mut show);
    assert_eq!(*picked.borrow(), vec![3]);
}

#[test]
fn control_and_rows_announce_themselves_to_accessibility() {
    let mut h = Harness::new();
    h.ctx.enable_accesskit();
    let mut fx = Fx::new(control_at(40.0));
    open(&mut h, &mut fx);
    let out = h.frame(Vec::new(), &mut |ui| fx.show(ui));
    let update = out.platform_output.accesskit_update.expect("accesskit on");
    let labelled: Vec<(egui::accesskit::Role, String)> = update
        .nodes
        .iter()
        .filter_map(|(_, n)| n.label().map(|l| (n.role(), l.to_owned())))
        .collect();
    assert!(
        labelled
            .iter()
            .any(|(r, l)| *r == egui::accesskit::Role::ComboBox && l == "Hatch"),
        "{labelled:?}"
    );
    for name in ["Solid", "Gradient", "None"] {
        assert!(
            labelled.iter().any(|(_, l)| l == name),
            "row {name}: {labelled:?}"
        );
    }
}

#[test]
fn style_derives_monochrome_colours_from_visuals_and_palette() {
    let v = egui::Visuals::dark();
    let s = DropdownStyle::from_visuals(&v);
    assert_ne!(s.selected_fill, s.box_fill, "current is distinguishable");
    assert_ne!(s.hover_fill, s.selected_fill);
    for c in [s.selected_fill, s.hover_fill] {
        // grey: channels within a few steps of each other
        let [r, g, b, _] = c.to_array();
        assert!(r.abs_diff(g) < 12 && g.abs_diff(b) < 12, "{c:?}");
    }
    let p = ThemePalette::dark();
    let s2 = DropdownStyle::from_palette(&p, &v);
    assert_eq!(s2.box_fill, p.surface.background.color32().unwrap());
    assert_eq!(s2.border, p.control.cap_border.color32().unwrap());
}

/// Every filled rect in the frame, in paint order, with its layer.
fn filled_rects(out: &egui::FullOutput) -> Vec<(Rect, Color32)> {
    fn walk(shape: &egui::Shape, out: &mut Vec<(Rect, Color32)>) {
        match shape {
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            egui::Shape::Rect(r) => out.push((r.rect, r.fill)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for c in &out.shapes {
        walk(&c.shape, &mut v);
    }
    v
}

/// Regression (Text Style Manager, 2026-10-19): a dropdown inside a row whose
/// local visuals make widget chrome transparent -- how SDB's managers embed a
/// `DragValue` in a field frame -- drew its open list with a transparent box, so
/// the rows beneath showed through. The list must paint an opaque box under
/// its rows, and the closed control keeps its theme fill, wherever it sits.
#[test]
fn the_open_list_has_an_opaque_box_even_inside_transparent_chrome() {
    let mut h = Harness::new();
    let control = control_at(40.0);
    let mut show = |ui: &mut Ui| {
        ui.scope(|ui| {
            let v = &mut ui.style_mut().visuals;
            v.extreme_bg_color = Color32::TRANSPARENT;
            for w in [
                &mut v.widgets.inactive,
                &mut v.widgets.hovered,
                &mut v.widgets.active,
            ] {
                w.bg_fill = Color32::TRANSPARENT;
                w.weak_bg_fill = Color32::TRANSPARENT;
            }
            let _ = Dropdown::new("transparent_row")
                .options(NAMES.to_vec())
                .selected(Some(0))
                .at_rect(control)
                .show(ui);
        });
    };
    h.settle(3, &mut show);
    let closed = h.frame(Vec::new(), &mut show);
    assert!(
        filled_rects(&closed)
            .iter()
            .any(|(r, fill)| *r == control && fill.a() == 255),
        "the closed control keeps the theme's opaque fill: {:?}",
        filled_rects(&closed)
    );
    h.click(control.center(), &mut show);
    // Past the popup's fade-in, which multiplies everything in it by its opacity.
    h.settle(30, &mut show);
    let out = h.frame(Vec::new(), &mut show);
    let rects = filled_rects(&out);
    let (box_rect, fill) = rects
        .iter()
        .find(|(r, _)| r.min == control.min && r.height() > control.height() * 2.0)
        .copied()
        .expect("the open list paints its box");
    assert_eq!(fill.a(), 255, "the box is opaque: {fill:?} {rects:?}");
    // Every option's text sits inside that box.
    for name in &NAMES[1..] {
        let p = text_pos(&out, name);
        assert!(box_rect.contains(p), "{name} at {p:?} outside {box_rect:?}");
    }
}

#[test]
fn opaque_keeps_opaque_colours_and_fills_in_transparent_ones() {
    assert_eq!(opaque(Color32::RED), Color32::RED);
    assert_eq!(opaque(Color32::TRANSPARENT).a(), 255);
    assert_eq!(
        opaque(Color32::from_rgba_unmultiplied(40, 40, 40, 128)).a(),
        255
    );
}

#[test]
fn show_items_returns_the_chosen_item_and_marks_the_current() {
    let mut h = Harness::new();
    let picked = std::cell::RefCell::new(Vec::new());
    let mut show = |ui: &mut Ui| {
        let got = Dropdown::new("items").at_rect(control_at(40.0)).show_items(
            ui,
            [10_u32, 20, 30],
            |v| format!("{v} mm"),
            |v| *v == 20,
        );
        if let Some(v) = got {
            picked.borrow_mut().push(v);
        }
    };
    h.settle(3, &mut show);
    h.click(control_at(40.0).center(), &mut show);
    let out = h.frame(Vec::new(), &mut show);
    assert!(
        control_at(40.0).contains(text_pos(&out, "20 mm")),
        "the current item is the top row"
    );
    h.click(text_pos(&out, "30 mm"), &mut show);
    assert_eq!(*picked.borrow(), vec![30]);
}

#[test]
fn show_value_edits_in_place_and_reports_the_change() {
    let mut h = Harness::new();
    let mut value = 20_u32;
    let mut changes = 0;
    let mut show = |ui: &mut Ui| {
        if Dropdown::new("val").at_rect(control_at(40.0)).show_value(
            ui,
            &mut value,
            [10_u32, 20, 30],
            |v| format!("{v} mm"),
        ) {
            changes += 1;
        }
    };
    h.settle(3, &mut show);
    h.click(control_at(40.0).center(), &mut show);
    let out = h.frame(Vec::new(), &mut show);
    h.click(text_pos(&out, "10 mm"), &mut show);
    assert_eq!((value, changes), (10, 1));
}

#[test]
fn heading_rows_are_shown_but_never_selected() {
    let mut h = Harness::new();
    let picked = std::cell::RefCell::new(Vec::new());
    let mut show = |ui: &mut Ui| {
        let out = Dropdown::new("sec")
            .at_rect(control_at(40.0))
            .option("Continuous")
            .option(DropdownOption::heading("Other"))
            .option("Gearwheel")
            .selected(Some(0))
            .show(ui);
        if let Some(i) = out.chosen {
            picked.borrow_mut().push(i);
        }
    };
    h.settle(3, &mut show);
    h.click(control_at(40.0).center(), &mut show);
    let out = h.frame(Vec::new(), &mut show);
    h.click(text_pos(&out, "Other"), &mut show);
    assert!(picked.borrow().is_empty(), "a heading is not a choice");
    h.key(Key::ArrowDown, &mut show);
    h.key(Key::Enter, &mut show);
    assert_eq!(*picked.borrow(), vec![2], "the keyboard skips the heading");
}

#[test]
fn an_open_dropdown_owns_escape_for_handlers_that_run_before_it_draws() {
    let mut h = Harness::new();
    let mut fx = Fx::new(control_at(40.0));
    let sibling = std::cell::Cell::new(0);
    let mut show = |ui: &mut Ui| {
        // A sibling Escape handler that runs before the dropdown (global
        // cancel, window close) and respects the stamp.
        if ui.input(|i| i.key_pressed(Key::Escape)) && !dropdown_owns_escape(ui.ctx()) {
            sibling.set(sibling.get() + 1);
        }
        fx.show(ui);
    };
    h.settle(3, &mut show);
    h.click(control_at(40.0).center(), &mut show);
    h.settle(1, &mut show);
    h.key(Key::Escape, &mut show);
    assert_eq!(sibling.get(), 0, "Esc closed the dropdown, nothing else");
    // Next Esc (dropdown closed) reaches the sibling.
    h.key(Key::Escape, &mut show);
    assert_eq!(sibling.get(), 1);
}

#[test]
fn keys_do_nothing_once_focus_has_moved_away_and_the_list_closes() {
    let mut h = Harness::new();
    let mut fx = Fx::new(control_at(40.0));
    let mut show = |ui: &mut Ui| {
        fx.show(ui);
        ui.put(
            Rect::from_min_size(Pos2::new(40.0, 400.0), Vec2::new(60.0, 20.0)),
            egui::Button::new("next"),
        );
    };
    h.settle(3, &mut show);
    h.key(Key::Tab, &mut show);
    h.key(Key::Enter, &mut show);
    h.key(Key::Tab, &mut show); // focus moves to the button
    drop(show);
    assert!(
        fx.last.as_ref().unwrap().popup_rect.is_none(),
        "closed on focus loss"
    );
    assert!(fx.picked.is_empty());
}

#[test]
fn a_stale_cursor_after_the_options_shrink_does_not_panic_or_emit() {
    let mut h = Harness::new();
    let mut fx = Fx::new(control_at(40.0));
    h.settle(3, &mut |ui| fx.show(ui));
    h.key(Key::Tab, &mut |ui| fx.show(ui));
    h.key(Key::Enter, &mut |ui| fx.show(ui));
    h.key(Key::End, &mut |ui| fx.show(ui)); // cursor on the last option (3)
    fx.options.truncate(2);
    fx.value = 0;
    h.key(Key::Enter, &mut |ui| fx.show(ui));
    assert!(fx.picked.iter().all(|&i| i < 2), "{:?}", fx.picked);
}

#[test]
fn a_stale_cursor_in_a_grid_is_dropped_too() {
    let mut h = Harness::new();
    let n = std::cell::Cell::new(4usize);
    let picked = std::cell::RefCell::new(Vec::new());
    let mut show = |ui: &mut Ui| {
        let opts: Vec<String> = (0..n.get()).map(|i| format!("t{i}")).collect();
        let out = Dropdown::new("g")
            .options(opts)
            .at_rect(control_at(40.0))
            .grid(2, Vec2::new(60.0, 40.0))
            .show(ui);
        if let Some(i) = out.chosen {
            picked.borrow_mut().push(i);
        }
    };
    h.settle(3, &mut show);
    h.key(Key::Tab, &mut show);
    h.key(Key::Enter, &mut show);
    h.key(Key::End, &mut show);
    n.set(2);
    h.key(Key::Enter, &mut show);
    assert!(picked.borrow().iter().all(|&i| i < 2));
}

#[test]
fn lazy_options_are_built_only_while_open() {
    let mut h = Harness::new();
    let built = std::cell::Cell::new(0);
    let picked = std::cell::RefCell::new(Vec::new());
    let mut show = |ui: &mut Ui| {
        let (_, got) = Dropdown::new("lazy")
            .selected_text("b")
            .at_rect(control_at(40.0))
            .show_items_lazy(
                ui,
                || {
                    built.set(built.get() + 1);
                    vec!["a", "b", "c"]
                },
                |s| s.to_string(),
                |s| *s == "b",
            );
        if let Some(v) = got {
            picked.borrow_mut().push(v);
        }
    };
    h.settle(5, &mut show);
    assert_eq!(built.get(), 0, "closed: nothing built");
    h.click(control_at(40.0).center(), &mut show);
    let out = h.frame(Vec::new(), &mut show);
    assert!(built.get() > 0);
    let c = text_pos(&out, "c");
    h.click(c, &mut show);
    assert_eq!(*picked.borrow(), vec!["c"]);
    let after = built.get();
    h.settle(4, &mut show);
    assert_eq!(built.get(), after, "closed again: no more building");
}

#[test]
fn a_disabled_dropdown_does_not_open() {
    let mut h = Harness::new();
    let mut fx = Fx::new(control_at(40.0));
    let mut show = |ui: &mut Ui| {
        ui.add_enabled_ui(false, |ui| fx.show(ui));
    };
    h.settle(3, &mut show);
    h.click(control_at(40.0).center(), &mut show);
    drop(show);
    assert!(fx.last.as_ref().unwrap().popup_rect.is_none());
}

#[test]
fn the_list_grows_to_the_longest_option_and_the_control_stays_narrow() {
    let mut h = Harness::new();
    let mut rect = None;
    let mut show = |ui: &mut Ui| {
        let out = Dropdown::new("wide")
            .options(["Short", "Concrete (stipple, large aggregate pattern)"])
            .selected(Some(0))
            .at_rect(control_at(40.0))
            .show(ui);
        rect = out.popup_rect;
    };
    h.settle(3, &mut show);
    h.click(control_at(40.0).center(), &mut show);
    h.settle(1, &mut show);
    assert!(rect.unwrap().width() > 250.0, "{rect:?}");
    assert_eq!(rect.unwrap().left(), control_at(40.0).left());
}

#[test]
fn an_open_list_closes_when_its_control_stops_being_drawn() {
    let mut h = Harness::new();
    let mut fx = Fx::new(control_at(40.0));
    open(&mut h, &mut fx);
    assert!(fx.last.as_ref().unwrap().popup_rect.is_some());
    h.settle(4, &mut |_ui| {}); // the panel is closed
    h.settle(2, &mut |ui| fx.show(ui));
    assert!(fx.last.as_ref().unwrap().popup_rect.is_none());
}
