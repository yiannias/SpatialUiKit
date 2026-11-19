//! Virtualized list: draws only the rows that are visible, for lists of any
//! length and rows of any (differing) height.
//!
//! `egui::ScrollArea::show_rows` needs one fixed row height. Real lists here
//! have rows of different heights (a name plus a summary, an extra line for
//! the selected row), so this helper measures each row as it is drawn, keeps
//! the heights, and stands in for the rows it does not draw with empty space
//! (using the average measured height for rows not yet seen). The scroll
//! position is the ScrollArea's own, keyed by `id_salt`, so it survives like
//! any other scroll area.
//!
//! Use [`show`] in place of `ScrollArea::vertical().show(..)` plus a loop over
//! every row. Ask for a row to be brought into view with `scroll_to`.

use egui::{ScrollArea, Ui};
use std::hash::Hash;

/// Row height assumed before any row has been measured.
const FIRST_GUESS: f32 = 24.0;

/// Remembered between frames: measured row heights (0 = not measured) and the
/// viewport last drawn, to scroll the minimum distance.
#[derive(Clone, Default)]
struct State {
    heights: Vec<f32>,
    offset: f32,
    viewport_height: f32,
}

/// Draws `count` rows in a vertical scroll area, calling `row` only for rows
/// near the visible part. `max_height` caps the area (None = fill what is
/// available). `scroll_to` brings that row into view, moving the least that
/// does it. Returns the indices of the rows that were drawn.
pub fn show(
    ui: &mut Ui,
    id_salt: impl Hash,
    count: usize,
    max_height: Option<f32>,
    scroll_to: Option<usize>,
    mut row: impl FnMut(&mut Ui, usize),
) -> std::ops::Range<usize> {
    let id = {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        id_salt.hash(&mut hasher);
        ui.id().with(("sdb_virtual_list", hasher.finish()))
    };
    let mut state = ui
        .ctx()
        .memory(|m| m.data.get_temp::<State>(id))
        .unwrap_or_default();
    state.heights.resize(count, 0.0);
    let spacing = ui.spacing().item_spacing.y;

    let mut area = ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false]);
    if let Some(max) = max_height {
        area = area.max_height(max);
    }
    if let Some(target) = scroll_to.filter(|t| *t < count) {
        let (top, bottom) = row_span(&state.heights, target, spacing);
        if top < state.offset {
            area = area.vertical_scroll_offset(top);
        } else if bottom > state.offset + state.viewport_height && state.viewport_height > 0.0 {
            area = area.vertical_scroll_offset(bottom - state.viewport_height);
        }
    }

    let mut drawn = 0..0;
    let mut changed = false;
    let output = area.show_viewport(ui, |ui, viewport| {
        (drawn, changed) = draw_rows(
            ui,
            &mut state.heights,
            viewport.min.y,
            viewport.max.y,
            &mut row,
        );
    });
    state.offset = output.state.offset.y;
    state.viewport_height = output.inner_rect.height();
    ui.ctx().memory_mut(|m| m.data.insert_temp(id, state));
    if changed {
        ui.ctx().request_repaint();
    }
    drawn
}

/// The average of the measured heights, or a first guess.
fn average(heights: &[f32]) -> f32 {
    let (sum, n) = heights
        .iter()
        .filter(|h| **h > 0.0)
        .fold((0.0, 0), |(s, n), h| (s + h, n + 1));
    if n == 0 {
        FIRST_GUESS
    } else {
        sum / n as f32
    }
}

/// Top and bottom (without the trailing spacing) of row `i` in list space.
fn row_span(heights: &[f32], i: usize, spacing: f32) -> (f32, f32) {
    let avg = average(heights);
    let h = |k: usize| if heights[k] > 0.0 { heights[k] } else { avg };
    let top: f32 = (0..i).map(|k| h(k) + spacing).sum();
    (top, top + h(i))
}

/// Draws the rows that overlap `[from, to]` (list space, y down from the top
/// of the list), spacers standing in for the rest. Returns the drawn range
/// and whether any measured height changed.
fn draw_rows(
    ui: &mut Ui,
    heights: &mut [f32],
    from: f32,
    to: f32,
    row: &mut impl FnMut(&mut Ui, usize),
) -> (std::ops::Range<usize>, bool) {
    let count = heights.len();
    let spacing = ui.spacing().item_spacing.y;
    let row_spacing = ui.spacing().clone();
    let avg = average(heights);
    let height_of =
        |heights: &[f32], i: usize| (if heights[i] > 0.0 { heights[i] } else { avg }) + spacing;
    ui.spacing_mut().item_spacing.y = 0.0;
    let mut top = 0.0;
    let mut first = 0;
    while first < count {
        let h = height_of(heights, first);
        if top + h > from {
            break;
        }
        top += h;
        first += 1;
    }
    ui.add_space(top);
    let mut index = first;
    let mut y = top;
    let mut changed = false;
    while index < count && y < to {
        let response = ui.scope(|ui| {
            *ui.spacing_mut() = row_spacing.clone();
            row(ui, index);
        });
        let h = response.response.rect.height();
        if (heights[index] - h).abs() > 0.5 {
            heights[index] = h;
            changed = true;
        }
        ui.add_space(spacing);
        y += h + spacing;
        index += 1;
    }
    let rest: f32 = (index..count).map(|i| height_of(heights, i)).sum();
    ui.add_space(rest);
    ui.spacing_mut().item_spacing.y = row_spacing.item_spacing.y;
    (first..index, changed)
}

/// The same list drawn straight into `ui`, which sits inside a scroll area
/// of the caller's (a panel with a header above the rows): rows outside the
/// visible clip are skipped. Use [`show`] when the list is its own scroll
/// area. Returns the indices drawn.
pub fn show_inline(
    ui: &mut Ui,
    id_salt: impl Hash,
    count: usize,
    mut row: impl FnMut(&mut Ui, usize),
) -> std::ops::Range<usize> {
    let id = {
        use std::hash::Hasher;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        id_salt.hash(&mut hasher);
        ui.id().with(("sdb_virtual_inline", hasher.finish()))
    };
    let mut heights = ui
        .ctx()
        .memory(|m| m.data.get_temp::<Vec<f32>>(id))
        .unwrap_or_default();
    heights.resize(count, 0.0);
    let origin = ui.cursor().min.y;
    let clip = ui.clip_rect();
    let (drawn, changed) = draw_rows(
        ui,
        &mut heights,
        clip.min.y - origin,
        clip.max.y - origin,
        &mut row,
    );
    ui.ctx().memory_mut(|m| m.data.insert_temp(id, heights));
    if changed {
        ui.ctx().request_repaint();
    }
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Context, Pos2, RawInput, Rect, Vec2};

    /// Rows alternate between one and two lines tall; a 10 000-row list draws
    /// only what is visible, and scroll_to reaches a far row.
    #[test]
    fn only_visible_rows_are_drawn_and_scroll_to_reaches_a_far_row() {
        let ctx = Context::default();
        let mut drawn_rows = Vec::new();
        let frame = |time: f64, scroll_to: Option<usize>, out: &mut Vec<usize>| {
            let _ = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 300.0))),
                    time: Some(time),
                    ..Default::default()
                },
                |ui| {
                    out.clear();
                    show(ui, "list", 10_000, Some(200.0), scroll_to, |ui, i| {
                        out.push(i);
                        ui.label(format!("row {i}"));
                        if i % 2 == 1 {
                            ui.label("second line");
                        }
                    });
                },
            );
        };
        for t in 1..5 {
            frame(t as f64, None, &mut drawn_rows);
        }
        assert!(
            drawn_rows.len() < 40,
            "only visible rows: drew {}",
            drawn_rows.len()
        );
        assert_eq!(drawn_rows[0], 0);
        // Jump to a far row (it must be drawn once the offset settles).
        let mut far = Vec::new();
        for t in 5..14 {
            frame(t as f64, Some(9000), &mut far);
        }
        assert!(far.contains(&9000), "row 9000 in view: {far:?}");
        assert!(far.len() < 40);
    }

    /// Inside a caller's scroll area only the rows in the visible clip draw.
    #[test]
    fn the_inline_list_draws_only_rows_in_the_clip() {
        let ctx = Context::default();
        let mut out = Vec::new();
        for t in 1..5 {
            let _ = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 300.0))),
                    time: Some(t as f64),
                    ..Default::default()
                },
                |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            ui.label("header");
                            out.clear();
                            show_inline(ui, "inline", 5_000, |ui, i| {
                                out.push(i);
                                ui.label(format!("row {i}"));
                            });
                        });
                },
            );
        }
        assert!(!out.is_empty() && out.len() < 40, "drew {}", out.len());
    }
}
