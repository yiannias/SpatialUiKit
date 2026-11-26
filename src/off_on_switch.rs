//! A compact **Off | On** switch for on/off settings (SDB TASK-418).
//!
//! Chris, 2026-11-26/27, from sketches of four styles: two segments, Off and
//! On, with the active one filled, about the size of the title bar's small
//! slider, the text at the 11 px minimum. The state reads by **position and
//! text** (which segment is filled, and the word in it) as well as colour: On
//! is filled with the theme's highlight, Off with a neutral chip grey.
//! One-shot actions ("Unhide all", "End temporary") stay buttons.
//!
//! Colours come from the theme ([`crate::interface_tokens::theme_colors`]);
//! the built-in Light look falls back to its own visuals.

use egui::{Color32, Response, Sense, Stroke, StrokeKind, Ui, Vec2};

/// Text size of the segment labels before the text scale (the 11 px minimum).
const LABEL_SIZE: f32 = 11.0;
/// Track height before the text scale.
const HEIGHT: f32 = 16.0;
/// Horizontal padding inside a segment, each side, before the text scale.
const SEGMENT_PAD: f32 = 4.0;
/// Inset of the filled segment inside the track.
const INSET: f32 = 1.5;

struct Palette {
    track: Color32,
    track_stroke: Stroke,
    on_fill: Color32,
    on_ink: Color32,
    off_fill: Color32,
    off_ink: Color32,
    idle_ink: Color32,
    focus: Color32,
}

fn palette(ui: &Ui) -> Palette {
    match crate::interface_tokens::theme_colors(ui) {
        Some(c) => Palette {
            track: c.well,
            track_stroke: Stroke::new(1.0, c.outline),
            on_fill: c.highlight,
            // Dark ink on the bright highlight; the well is the darkest grey.
            on_ink: c.well,
            off_fill: c.chip_hover,
            off_ink: c.text_strong,
            idle_ink: c.text_muted,
            focus: c.text_strong,
        },
        None => {
            let v = ui.visuals();
            Palette {
                track: v.extreme_bg_color,
                track_stroke: v.widgets.inactive.bg_stroke,
                on_fill: v.selection.bg_fill,
                on_ink: v.strong_text_color(),
                off_fill: v.widgets.hovered.weak_bg_fill,
                off_ink: v.strong_text_color(),
                idle_ink: v.weak_text_color(),
                focus: v.strong_text_color(),
            }
        }
    }
}

/// The switch's size at the current text scale, for layouts that need it
/// before drawing.
pub fn size(ui: &Ui) -> Vec2 {
    let s = crate::theme::current_text_scale(ui);
    let font = egui::FontId::proportional(LABEL_SIZE * s);
    let widest = ["Off", "On"]
        .iter()
        .map(|t| {
            ui.painter()
                .layout_no_wrap((*t).to_owned(), font.clone(), Color32::PLACEHOLDER)
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    let segment = widest + 2.0 * SEGMENT_PAD * s;
    egui::vec2(segment * 2.0 + 2.0 * INSET * s, HEIGHT * s)
}

/// Draws the switch for `on`; a click, Space or Enter flips it, Left sets Off
/// and Right sets On while it has focus. `label` is what screen readers hear
/// (the setting's name). The response is `changed()` on the frame it flipped.
pub fn off_on_switch(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let s = crate::theme::current_text_scale(ui);
    let size = size(ui);
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    let enabled = ui.is_enabled();
    let before = *on;
    if enabled {
        if response.clicked() {
            *on = !*on;
        }
        if response.has_focus() {
            ui.input(|i| {
                if i.key_pressed(egui::Key::ArrowLeft) {
                    *on = false;
                }
                if i.key_pressed(egui::Key::ArrowRight) {
                    *on = true;
                }
            });
        }
    }
    if *on != before {
        response.mark_changed();
    }
    let value = *on;
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, value, label)
    });

    if ui.is_rect_visible(rect) {
        let p = palette(ui);
        let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.45) };
        let radius = (size.y * 0.3).round() as u8;
        let painter = ui.painter();
        painter.rect(
            rect,
            radius,
            fade(p.track),
            p.track_stroke,
            StrokeKind::Inside,
        );
        let inner = rect.shrink(INSET * s);
        let half = inner.width() / 2.0;
        let off_rect = egui::Rect::from_min_size(inner.min, egui::vec2(half, inner.height()));
        let on_rect = egui::Rect::from_min_size(
            egui::pos2(inner.min.x + half, inner.min.y),
            egui::vec2(half, inner.height()),
        );
        let (active, fill) = if value {
            (on_rect, p.on_fill)
        } else {
            (off_rect, p.off_fill)
        };
        painter.rect_filled(active, radius.saturating_sub(1), fade(fill));
        let font = egui::FontId::proportional(LABEL_SIZE * s);
        for (text, seg, is_active, ink) in [
            ("Off", off_rect, !value, p.off_ink),
            ("On", on_rect, value, p.on_ink),
        ] {
            let color = if is_active { ink } else { p.idle_ink };
            painter.text(
                seg.center(),
                egui::Align2::CENTER_CENTER,
                text,
                font.clone(),
                fade(color),
            );
        }
        if response.has_focus() {
            painter.rect_stroke(
                rect.expand(1.5),
                radius + 1,
                Stroke::new(1.0, p.focus),
                StrokeKind::Outside,
            );
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(on: &mut bool, events: Vec<egui::Event>, ctx: &egui::Context) -> (egui::Rect, bool) {
        let mut out = (egui::Rect::NOTHING, false);
        let _ = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                let r = off_on_switch(ui, on, "Reveal hidden");
                out = (r.rect, r.changed());
            },
        );
        out
    }

    /// Small, as Chris asked: about the title bar slider's size, text at the
    /// 11 px minimum.
    #[test]
    fn the_switch_is_compact() {
        let ctx = egui::Context::default();
        let mut on = false;
        let (rect, _) = run(&mut on, Vec::new(), &ctx);
        // `HEIGHT` at the kit's default text scale (a little over 1).
        assert!(
            (HEIGHT..HEIGHT * 1.2).contains(&rect.height()),
            "{}",
            rect.height()
        );
        assert!(
            rect.width() > 30.0 && rect.width() < 64.0,
            "{}",
            rect.width()
        );
    }

    /// A click flips it and reports the change.
    #[test]
    fn a_click_flips_it() {
        let ctx = egui::Context::default();
        let mut on = false;
        let (rect, _) = run(&mut on, Vec::new(), &ctx);
        let at = rect.center();
        let press = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        run(
            &mut on,
            vec![egui::Event::PointerMoved(at), press(true)],
            &ctx,
        );
        let (_, changed) = run(&mut on, vec![press(false)], &ctx);
        assert!(on && changed);
    }
}
