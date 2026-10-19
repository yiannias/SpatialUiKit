use super::*;
use egui::{Event, PointerButton, RawInput};
use std::cell::{Cell, RefCell};

const INTER: &[u8] = include_bytes!("../../assets/Inter.ttf");

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
    fn frame(&mut self, events: Vec<Event>, f: &mut dyn FnMut(&mut Ui)) -> egui::FullOutput {
        self.frame += 1;
        self.ctx.run_ui(
            RawInput {
                time: Some(self.frame as f64 / 60.0),
                events,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(900.0, 700.0))),
                ..Default::default()
            },
            f,
        )
    }
    fn settle(&mut self, n: u32, f: &mut dyn FnMut(&mut Ui)) {
        for _ in 0..n {
            self.frame(Vec::new(), f);
        }
    }
    fn click(&mut self, pos: Pos2, f: &mut dyn FnMut(&mut Ui)) {
        self.frame(vec![Event::PointerMoved(pos)], f);
        for pressed in [true, false] {
            self.frame(
                vec![Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                }],
                f,
            );
        }
        self.settle(2, f);
    }
    fn key(&mut self, key: Key, f: &mut dyn FnMut(&mut Ui)) {
        for pressed in [true, false] {
            self.frame(
                vec![Event::Key {
                    key,
                    physical_key: Some(key),
                    pressed,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
                f,
            );
        }
        self.settle(1, f);
    }
    fn type_text(&mut self, text: &str, f: &mut dyn FnMut(&mut Ui)) {
        self.frame(vec![Event::Text(text.to_owned())], f);
        self.settle(1, f);
    }
}

fn control() -> Rect {
    Rect::from_min_size(Pos2::new(40.0, 40.0), Vec2::new(240.0, 24.0))
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

fn choices(n: usize) -> Vec<FontChoice> {
    let mut v = vec![FontChoice::special("inherit", "Each text's own")];
    v.push(FontChoice::face("Inter Doc", FontGroup::Document));
    v.push(FontChoice::face("Built One", FontGroup::BuiltIn));
    for i in 0..n {
        v.push(FontChoice::face(
            format!("System {i:03}"),
            FontGroup::System,
        ));
    }
    v
}

/// One picker over `n` system faces; records picks, builder calls and loads.
struct Fx {
    n: usize,
    current: String,
    picked: Vec<String>,
    built: Cell<usize>,
    loads: RefCell<Vec<String>>,
    last: Option<Rect>,
}

impl Fx {
    fn new(n: usize) -> Self {
        Self {
            n,
            current: "Built One".to_owned(),
            picked: Vec::new(),
            built: Cell::new(0),
            loads: RefCell::new(Vec::new()),
            last: None,
        }
    }
    fn show(&mut self, ui: &mut Ui) {
        let loads = &self.loads;
        let load = |name: &str| -> Option<FontBytes> {
            loads.borrow_mut().push(name.to_owned());
            Some((std::borrow::Cow::Borrowed(INTER), 0))
        };
        let built = &self.built;
        let n = self.n;
        let out = FontPicker::new("fp", &load)
            .selected(&self.current, &self.current, Some(&self.current))
            .at_rect(control())
            .choices(move || {
                built.set(built.get() + 1);
                choices(n)
            })
            .show(ui);
        self.last = out.popup_rect;
        if let Some(k) = out.chosen {
            self.current = k.clone();
            self.picked.push(k);
        }
    }
}

#[test]
fn rows_are_grouped_under_plain_headings_and_a_query_drops_empty_groups() {
    let c = choices(3);
    let lower: Vec<String> = c.iter().map(|c| c.label.to_lowercase()).collect();
    let rows = build_rows(&c, &lower, "", &[]);
    assert_eq!(rows[0], Row::Choice(0), "special choices first, no heading");
    assert_eq!(rows[1], Row::Heading(Some(FontGroup::Document)));
    assert!(rows.contains(&Row::Heading(Some(FontGroup::System))));
    let rows = build_rows(&c, &lower, "SYSTEM 001", &[]);
    assert_eq!(
        rows,
        vec![Row::Heading(Some(FontGroup::System)), Row::Choice(4)],
        "case-insensitive, and groups with no match lose their heading"
    );
    // Recent faces lead (only while not searching).
    let rows = build_rows(&c, &lower, "", &["System 002".to_owned()]);
    assert_eq!(rows[1], Row::Heading(None));
    assert_eq!(rows[2], Row::Choice(5));
    assert!(
        !build_rows(&c, &lower, "sys", &["System 002".to_owned()]).contains(&Row::Heading(None))
    );
}

#[test]
fn the_highlight_skips_headings_and_stops_at_the_ends() {
    let rows = vec![
        Row::Choice(0),
        Row::Heading(None),
        Row::Choice(1),
        Row::Heading(None),
        Row::Choice(2),
    ];
    assert_eq!(step(&rows, Some(0), 1), Some(2));
    assert_eq!(step(&rows, Some(2), 1), Some(4));
    assert_eq!(step(&rows, Some(4), 5), Some(4));
    assert_eq!(step(&rows, Some(2), -9), Some(0));
    assert_eq!(step(&rows, None, -1), Some(4));
}

#[test]
fn closed_it_builds_no_list_and_opening_focuses_the_search_field() {
    let mut h = Harness::new();
    let mut fx = Fx::new(5);
    h.settle(3, &mut |ui| fx.show(ui));
    assert_eq!(fx.built.get(), 0, "no list is built while closed");
    h.click(control().center(), &mut |ui| fx.show(ui));
    assert_eq!(fx.built.get(), 1, "built once on open, not every frame");
    let out = h.frame(Vec::new(), &mut |ui| fx.show(ui));
    let t = texts(&out);
    for label in [
        "Each text's own",
        "Used in this drawing",
        "Built in",
        "Installed on this computer",
    ] {
        assert!(t.iter().any(|(s, _)| s == label), "{label} missing: {t:?}");
    }
    // Typing goes straight to the search field and filters.
    h.type_text("004", &mut |ui| fx.show(ui));
    let out = h.frame(Vec::new(), &mut |ui| fx.show(ui));
    let t = texts(&out);
    assert!(t.iter().any(|(s, _)| s == "System 004"));
    assert!(!t.iter().any(|(s, _)| s == "System 003"), "{t:?}");
    // Enter picks the (only) match.
    h.key(Key::Enter, &mut |ui| fx.show(ui));
    assert_eq!(fx.picked, vec!["System 004".to_owned()]);
    assert!(fx.last.is_none(), "closed after the pick");
    assert_eq!(recent_faces(&h.ctx), vec!["System 004".to_owned()]);
}

#[test]
fn arrows_move_the_highlight_and_escape_closes_without_picking() {
    let mut h = Harness::new();
    let mut fx = Fx::new(5);
    h.settle(2, &mut |ui| fx.show(ui));
    h.click(control().center(), &mut |ui| fx.show(ui));
    // The highlight starts on the current choice ("Built One"); down twice
    // passes the System heading.
    h.key(Key::ArrowDown, &mut |ui| fx.show(ui));
    h.key(Key::ArrowDown, &mut |ui| fx.show(ui));
    h.key(Key::Escape, &mut |ui| fx.show(ui));
    assert!(fx.picked.is_empty());
    assert!(fx.last.is_none());
    h.click(control().center(), &mut |ui| fx.show(ui));
    h.key(Key::ArrowDown, &mut |ui| fx.show(ui));
    h.key(Key::ArrowDown, &mut |ui| fx.show(ui));
    h.key(Key::Enter, &mut |ui| fx.show(ui));
    assert_eq!(fx.picked, vec!["System 001".to_owned()]);
}

#[test]
fn clicking_a_row_picks_it() {
    let mut h = Harness::new();
    let mut fx = Fx::new(5);
    h.settle(2, &mut |ui| fx.show(ui));
    h.click(control().center(), &mut |ui| fx.show(ui));
    let out = h.frame(Vec::new(), &mut |ui| fx.show(ui));
    let at = texts(&out)
        .into_iter()
        .find(|(s, _)| s == "System 002")
        .unwrap()
        .1;
    h.click(at, &mut |ui| fx.show(ui));
    assert_eq!(fx.picked, vec!["System 002".to_owned()]);
}

#[test]
fn only_visible_rows_load_their_faces() {
    let mut h = Harness::new();
    let mut fx = Fx::new(400);
    h.settle(2, &mut |ui| fx.show(ui));
    h.click(control().center(), &mut |ui| fx.show(ui));
    h.settle(10, &mut |ui| fx.show(ui));
    let loads = fx.loads.borrow().len();
    assert!(
        loads <= 20,
        "{loads} faces loaded for a list showing about a dozen rows"
    );
    assert!(loads >= 5, "the visible rows do load ({loads})");
}

#[test]
fn the_open_box_is_opaque_and_holds_every_painted_row() {
    let mut h = Harness::new();
    let mut fx = Fx::new(30);
    h.settle(2, &mut |ui| fx.show(ui));
    h.click(control().center(), &mut |ui| fx.show(ui));
    h.settle(30, &mut |ui| fx.show(ui));
    let out = h.frame(Vec::new(), &mut |ui| fx.show(ui));
    let popup = fx.last.expect("open");
    let mut fills = Vec::new();
    for c in &out.shapes {
        if let egui::Shape::Rect(r) = &c.shape {
            if r.rect == popup {
                fills.push(r.fill);
            }
        }
    }
    assert!(fills.iter().any(|f| f.a() == 255), "{fills:?}");
    // Every row is drawn clipped to the box (rows half scrolled out are cut
    // at its edge, never drawn over what lies beneath).
    let mut rows = 0;
    for c in &out.shapes {
        if let egui::Shape::Text(t) = &c.shape {
            if t.galley.text().starts_with("System") {
                rows += 1;
                assert!(
                    popup.expand(2.0).contains_rect(c.clip_rect),
                    "{} clipped to {:?}, box {popup:?}",
                    t.galley.text(),
                    c.clip_rect
                );
            }
        }
    }
    assert!(rows > 5);
}

#[test]
fn fitted_size_shrinks_a_wide_sample_to_its_area() {
    let ctx = egui::Context::default();
    let _ = ctx.run_ui(RawInput::default(), |_| {});
    let area = Vec2::new(60.0, 40.0);
    let size = fitted_size(
        &ctx,
        "A very long sample",
        egui::FontFamily::Proportional,
        40.0,
        area,
    );
    assert!(size < 40.0);
    let w = ctx.fonts_mut(|f| {
        f.layout_no_wrap(
            "A very long sample".into(),
            FontId::proportional(size),
            Color32::WHITE,
        )
        .size()
        .x
    });
    assert!(w <= area.x + 0.5, "{w} > {}", area.x);
    assert_eq!(
        fitted_size(&ctx, "Ag", egui::FontFamily::Proportional, 12.0, area),
        12.0,
        "a sample that fits keeps its size"
    );
}

#[test]
fn home_and_end_jump_to_the_first_and_last_choice() {
    let mut h = Harness::new();
    let mut fx = Fx::new(5);
    h.settle(2, &mut |ui| fx.show(ui));
    h.click(control().center(), &mut |ui| fx.show(ui));
    h.key(Key::End, &mut |ui| fx.show(ui));
    h.key(Key::Enter, &mut |ui| fx.show(ui));
    assert_eq!(fx.picked, vec!["System 004".to_owned()]);
    h.click(control().center(), &mut |ui| fx.show(ui));
    h.key(Key::Home, &mut |ui| fx.show(ui));
    h.key(Key::Enter, &mut |ui| fx.show(ui));
    assert_eq!(fx.picked.last().map(String::as_str), Some("inherit"));
}

#[test]
fn the_control_announces_its_label_and_current_choice() {
    let mut h = Harness::new();
    h.ctx.enable_accesskit();
    let mut fx = Fx::new(3);
    h.settle(2, &mut |ui| fx.show(ui));
    let out = h.frame(Vec::new(), &mut |ui| fx.show(ui));
    let update = out
        .platform_output
        .accesskit_update
        .expect("accesskit is on");
    let names: Vec<String> = update
        .nodes
        .iter()
        .filter_map(|(_, n)| n.label().map(str::to_owned))
        .collect();
    assert!(names.iter().any(|n| n == "Typeface"), "{names:?}");
    let values: Vec<String> = update
        .nodes
        .iter()
        .filter_map(|(_, n)| n.value().map(str::to_owned))
        .collect();
    assert!(values.iter().any(|v| v == "Built One"), "{values:?}");
}
