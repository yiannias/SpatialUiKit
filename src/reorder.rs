//! Row reorder: the one way a list's rows are moved, by dragging or from the
//! keyboard.
//!
//! * **Drag**: the caller draws each row and gives the drag handle (or whole
//!   row) `Sense::drag()`. [`drag`] watches those responses, shows a line
//!   where the row would land, and returns the move when the pointer is
//!   released.
//! * **Keyboard**: with a row focused, **Alt+Up / Alt+Down** moves it one
//!   place ([`key_move`]).
//!
//! Both produce a [`Move`]; the list applies it to its own data (with its
//! own undo) or uses [`apply`] for a plain `Vec`.

use egui::{Id, Key, Modifiers, Response, Stroke, Ui};

/// Move the row at `from` so it ends at index `to` (both indices into the
/// list as it is now, `to` being its place after the move).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Move {
    pub from: usize,
    pub to: usize,
}

/// Moves one item of `items` as `mv` says.
pub fn apply<T>(items: &mut Vec<T>, mv: Move) {
    if mv.from < items.len() && mv.to < items.len() && mv.from != mv.to {
        let item = items.remove(mv.from);
        items.insert(mv.to, item);
    }
}

/// The index the dragged row ends at when dropped in `slot` (0 = above the
/// first row, `len` = below the last). `None` when that leaves it in place.
pub fn drop_move(from: usize, slot: usize) -> Option<Move> {
    let to = if slot > from { slot - 1 } else { slot };
    (to != from).then_some(Move { from, to })
}

/// The slot a pointer at height `y` falls in, given each row's rectangle
/// top to bottom: the number of rows whose middle is above it.
pub fn slot_at(rows: &[egui::Rect], y: f32) -> usize {
    rows.iter().filter(|r| r.center().y < y).count()
}

/// Alt+Up / Alt+Down: -1 (up) or +1 (down), consuming the keys. For a list
/// that works out the neighbour itself (rows grouped by kind, say).
pub fn key_step(ui: &mut Ui) -> Option<isize> {
    let (up, down) = ui.input_mut(|i| {
        (
            i.consume_key(Modifiers::ALT, Key::ArrowUp),
            i.consume_key(Modifiers::ALT, Key::ArrowDown),
        )
    });
    if up {
        Some(-1)
    } else if down {
        Some(1)
    } else {
        None
    }
}

/// Alt+Up / Alt+Down on the row at `focused` (of `len`): the move one place
/// up or down, consuming the keys. `None` with no key or at the end.
pub fn key_move(ui: &mut Ui, focused: usize, len: usize) -> Option<Move> {
    match key_step(ui)? {
        -1 if focused > 0 => Some(Move {
            from: focused,
            to: focused - 1,
        }),
        1 if focused + 1 < len => Some(Move {
            from: focused,
            to: focused + 1,
        }),
        _ => None,
    }
}

/// Drag reorder over `handles` (one response per row, top to bottom, each
/// with `Sense::drag()`), `rows` the rectangles of whole rows. Paints the
/// drop line while dragging and returns the move on release. `id` names the
/// list.
pub fn drag(ui: &mut Ui, id: Id, handles: &[Response], rows: &[egui::Rect]) -> Option<Move> {
    let state_id = id.with("sdb_reorder_drag");
    let dragging: Option<usize> = ui.memory(|m| m.data.get_temp(state_id));
    if let Some(from) = handles.iter().position(|h| h.drag_started()) {
        ui.memory_mut(|m| m.data.insert_temp(state_id, from));
        return None;
    }
    let from = dragging?;
    let pointer = ui.input(|i| i.pointer.interact_pos().or(i.pointer.hover_pos()));
    let Some(y) = pointer.map(|p| p.y) else {
        ui.memory_mut(|m| m.data.remove::<usize>(state_id));
        return None;
    };
    let slot = slot_at(rows, y);
    let released = handles.iter().any(|h| h.drag_stopped());
    if released {
        ui.memory_mut(|m| m.data.remove::<usize>(state_id));
        return drop_move(from, slot);
    }
    if handles.iter().any(|h| h.dragged()) && !rows.is_empty() {
        let line_y = if slot < rows.len() {
            rows[slot].top()
        } else {
            rows[rows.len() - 1].bottom()
        };
        let span = rows[0].x_range();
        // A line (not colour alone) marks where the row will land.
        ui.painter().hline(
            span,
            line_y,
            Stroke::new(2.0, ui.visuals().selection.stroke.color),
        );
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Pos2, Rect, Vec2};

    #[test]
    fn apply_moves_one_item() {
        let mut v = vec!['a', 'b', 'c', 'd'];
        apply(&mut v, Move { from: 0, to: 2 });
        assert_eq!(v, vec!['b', 'c', 'a', 'd']);
        apply(&mut v, Move { from: 3, to: 0 });
        assert_eq!(v, vec!['d', 'b', 'c', 'a']);
        apply(&mut v, Move { from: 1, to: 9 });
        assert_eq!(v, vec!['d', 'b', 'c', 'a'], "out of range is ignored");
    }

    #[test]
    fn dropping_in_a_slot_gives_the_final_index() {
        assert_eq!(drop_move(0, 3), Some(Move { from: 0, to: 2 }));
        assert_eq!(drop_move(3, 1), Some(Move { from: 3, to: 1 }));
        assert_eq!(drop_move(2, 2), None, "just above itself");
        assert_eq!(drop_move(2, 3), None, "just below itself");
    }

    #[test]
    fn the_slot_follows_the_row_middles() {
        let rows: Vec<Rect> = (0..3)
            .map(|i| Rect::from_min_size(Pos2::new(0.0, i as f32 * 20.0), Vec2::new(100.0, 20.0)))
            .collect();
        assert_eq!(slot_at(&rows, 2.0), 0);
        assert_eq!(slot_at(&rows, 12.0), 1);
        assert_eq!(slot_at(&rows, 59.0), 3);
    }

    #[test]
    fn alt_arrows_move_the_focused_row_one_place() {
        let ctx = egui::Context::default();
        let key = |k: Key| {
            vec![egui::Event::Key {
                key: k,
                physical_key: Some(k),
                pressed: true,
                repeat: false,
                modifiers: Modifiers::ALT,
            }]
        };
        let mut out = None;
        let mut run = |events: Vec<egui::Event>, focused: usize| {
            let _ = ctx.run_ui(
                egui::RawInput {
                    modifiers: Modifiers::ALT,
                    events,
                    ..Default::default()
                },
                |ui| out = key_move(ui, focused, 3),
            );
            out
        };
        assert_eq!(run(key(Key::ArrowUp), 1), Some(Move { from: 1, to: 0 }));
        assert_eq!(run(key(Key::ArrowDown), 1), Some(Move { from: 1, to: 2 }));
        assert_eq!(run(key(Key::ArrowUp), 0), None, "already first");
        assert_eq!(run(key(Key::ArrowDown), 2), None, "already last");
    }
}
