//! A segmented "choice toggle": a small fixed set of mutually exclusive
//! options drawn as one recessed track with the chosen option raised as a chip.
//!
//! It replaces a row of radio buttons or selectable labels (Chris, 2026-11-09
//! panel-theming pass). Selection is shown by brightness only, with no accent
//! colour, so panel chrome never competes with the drawing.

use egui::{
    Align2, Color32, CornerRadius, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
    WidgetInfo, WidgetType,
};

/// Padding between the track edge and the segments, in points.
const INNER_PAD: f32 = 2.0;
/// Horizontal text padding inside one segment, per side, in points.
const SEGMENT_PAD: f32 = 10.0;

/// How `count` equal segments are split into rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowPlan {
    /// Segments per row (the last row may hold fewer).
    pub per_row: usize,
    /// Number of rows.
    pub rows: usize,
}

/// Plans the rows for `count` segments that each need `segment_w` points, in a
/// track `available` points wide (including its inner padding).
///
/// If everything fits on one row it is one row; otherwise the options wrap into
/// the fewest rows that fit, with the segments spread evenly so the rows are
/// as equal as possible. Always at least one segment per row, so a very narrow
/// panel degrades to a vertical stack rather than overflowing.
pub fn plan_rows(count: usize, segment_w: f32, available: f32) -> RowPlan {
    if count == 0 {
        return RowPlan {
            per_row: 1,
            rows: 0,
        };
    }
    let usable = (available - 2.0 * INNER_PAD).max(0.0);
    let fit = if segment_w <= 0.0 {
        count
    } else {
        ((usable / segment_w).floor() as usize).clamp(1, count)
    };
    let rows = count.div_ceil(fit);
    RowPlan {
        per_row: count.div_ceil(rows),
        rows,
    }
}

/// Draws a choice toggle and writes the chosen value to `current`.
///
/// `options` pairs each value with its label. The response is `changed()` on
/// the frame the value changed. Honours `ui.is_enabled()`. Each segment
/// reports itself to accessibility tools as a selectable radio button, takes
/// keyboard focus (a 1px ring) and is chosen with Space or Enter.
pub fn choice_toggle<T: PartialEq + Clone>(
    ui: &mut Ui,
    current: &mut T,
    options: &[(T, &str)],
) -> Response {
    let enabled = ui.is_enabled();
    let dark = ui.visuals().dark_mode;
    let height = ui.spacing().interact_size.y;
    let font = egui::TextStyle::Button.resolve(ui.style());

    let (track_fill, chip_fill, chip_stroke, chosen_ink, idle_ink, hover_ink, ring) = if dark {
        (
            Color32::from_rgb(0x14, 0x14, 0x18),
            Color32::from_rgb(0x3A, 0x3B, 0x42),
            Stroke::NONE,
            Color32::from_rgb(0xF0, 0xF0, 0xF2),
            Color32::from_rgb(0xB4, 0xB5, 0xBC),
            Color32::from_rgb(0xE8, 0xE8, 0xEC),
            Color32::from_rgb(0xF0, 0xF0, 0xF2),
        )
    } else {
        (
            Color32::from_rgb(0xE2, 0xE3, 0xE8),
            Color32::WHITE,
            Stroke::new(1.0, Color32::from_black_alpha(24)),
            Color32::from_rgb(0x20, 0x20, 0x24),
            Color32::from_rgb(0x4A, 0x4C, 0x54),
            Color32::from_rgb(0x20, 0x20, 0x24),
            Color32::from_rgb(0x20, 0x20, 0x24),
        )
    };

    let widest = options
        .iter()
        .map(|(_, label)| {
            ui.painter()
                .layout_no_wrap(label.to_string(), font.clone(), Color32::WHITE)
                .size()
                .x
        })
        .fold(0.0_f32, f32::max);
    let segment_w = widest + 2.0 * SEGMENT_PAD;
    let natural = segment_w * options.len() as f32 + 2.0 * INNER_PAD;
    let available = ui.available_width();
    let track_w = if available.is_finite() {
        natural.min(available)
    } else {
        natural
    };
    let plan = plan_rows(options.len(), segment_w, track_w);
    let rows = plan.rows.max(1);
    let track_h = height * rows as f32 + 2.0 * INNER_PAD;

    let (track, mut response) = ui.allocate_exact_size(Vec2::new(track_w, track_h), Sense::hover());
    let seg_w = (track.width() - 2.0 * INNER_PAD) / plan.per_row as f32;
    let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.45) };

    ui.painter()
        .rect_filled(track, CornerRadius::same(5), fade(track_fill));

    let mut chosen = None;
    for (index, (value, label)) in options.iter().enumerate() {
        let (row, col) = (index / plan.per_row, index % plan.per_row);
        let rect = Rect::from_min_size(
            Pos2::new(
                track.left() + INNER_PAD + seg_w * col as f32,
                track.top() + INNER_PAD + height * row as f32,
            ),
            Vec2::new(seg_w, height),
        );
        let sense = if enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let seg = ui.interact(rect, response.id.with(("choice_segment", index)), sense);
        let selected = value == current;
        seg.widget_info(|| {
            WidgetInfo::selected(WidgetType::RadioButton, enabled, selected, *label)
        });
        if enabled && seg.clicked() {
            chosen = Some(value.clone());
        }
        if selected {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(4), fade(chip_fill));
            if chip_stroke.width > 0.0 {
                ui.painter().rect_stroke(
                    rect,
                    CornerRadius::same(4),
                    chip_stroke,
                    StrokeKind::Inside,
                );
            }
        }
        let ink = if selected {
            chosen_ink
        } else if enabled && seg.hovered() {
            hover_ink
        } else {
            idle_ink
        };
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            font.clone(),
            fade(ink),
        );
        if enabled && seg.has_focus() {
            ui.painter().rect_stroke(
                rect,
                CornerRadius::same(4),
                Stroke::new(1.0, ring),
                StrokeKind::Inside,
            );
        }
    }
    if let Some(value) = chosen {
        if *current != value {
            *current = value;
            response.mark_changed();
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn everything_fits_on_one_row_when_there_is_room() {
        assert_eq!(
            plan_rows(3, 60.0, 300.0),
            RowPlan {
                per_row: 3,
                rows: 1
            }
        );
    }

    #[test]
    fn too_wide_options_wrap_into_equal_rows() {
        // Four 60pt segments in 130pt: two fit a row, so two rows of two.
        assert_eq!(
            plan_rows(4, 60.0, 130.0),
            RowPlan {
                per_row: 2,
                rows: 2
            }
        );
        // Five in 130pt: two fit, so three rows, spread as evenly as possible.
        let p = plan_rows(5, 60.0, 130.0);
        assert_eq!(p.rows, 3);
        assert_eq!(p.per_row, 2);
    }

    #[test]
    fn a_hopelessly_narrow_track_stacks_one_per_row() {
        assert_eq!(
            plan_rows(3, 100.0, 20.0),
            RowPlan {
                per_row: 1,
                rows: 3
            }
        );
    }

    #[test]
    fn no_options_means_no_rows() {
        assert_eq!(plan_rows(0, 50.0, 200.0).rows, 0);
    }

    fn frame(ctx: &egui::Context, value: &mut u8, events: Vec<egui::Event>) -> Response {
        let mut out = None;
        let _ = ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 100.0))),
                ..Default::default()
            },
            |ui| {
                out = Some(choice_toggle(
                    ui,
                    value,
                    &[(0u8, "One"), (1u8, "Two"), (2u8, "Three")],
                ));
            },
        );
        out.expect("ran")
    }

    #[test]
    fn a_click_on_another_segment_changes_the_value() {
        let ctx = egui::Context::default();
        let mut value = 0u8;
        // Learn the layout, then click the middle of the track: the second segment.
        let probe = frame(&ctx, &mut value, vec![]);
        let at = probe.rect.center();
        frame(&ctx, &mut value, vec![egui::Event::PointerMoved(at)]);
        let mut changed = false;
        for pressed in [true, false] {
            let r = frame(
                &ctx,
                &mut value,
                vec![egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
            changed |= r.changed();
        }
        assert_eq!(value, 1);
        assert!(changed);
    }
}
