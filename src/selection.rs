//! Multi-select rows: the one implementation of how a click changes a set of
//! selected rows.
//!
//! * **Click** selects just that row (or, for a list whose set is a separate
//!   tick-box column, leaves the set alone: [`PlainClick::Keep`]).
//! * **Ctrl/Cmd+click** toggles that row and keeps the rest.
//! * **Shift+click** selects the range from the anchor (the last row clicked
//!   without Shift) to this row. With Ctrl held too the range is added to the
//!   set instead of replacing it.
//!
//! The model is pure: a list passes its rows in display order, the set, the
//! anchor and the click, and draws from the set afterwards.

use egui::Modifiers;
use std::collections::HashSet;
use std::hash::Hash;

/// What a click without Ctrl or Shift does to the set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlainClick {
    /// The set becomes just the clicked row (a standard list).
    Replace,
    /// The set is untouched (the set is driven by tick boxes; a plain click
    /// only moves the anchor and the list's own highlight).
    Keep,
}

/// Applies one click on `clicked` to `set`. `order` is every row in display
/// order; `anchor` is the list's remembered anchor row. Returns whether the
/// set changed.
pub fn click<K: Clone + Eq + Hash>(
    set: &mut HashSet<K>,
    anchor: &mut Option<K>,
    order: &[K],
    clicked: &K,
    mods: Modifiers,
    plain: PlainClick,
) -> bool {
    let before = set.clone();
    let toggle = mods.command || mods.ctrl;
    if mods.shift {
        let from = anchor
            .as_ref()
            .and_then(|a| order.iter().position(|k| k == a));
        let to = order.iter().position(|k| k == clicked);
        if let (Some(from), Some(to)) = (from, to) {
            if !toggle {
                set.clear();
            }
            let (lo, hi) = (from.min(to), from.max(to));
            set.extend(order[lo..=hi].iter().cloned());
            // The anchor stays: a second Shift+click re-ranges from it.
            return *set != before;
        }
        // No usable anchor: a Shift+click behaves like a plain click.
    }
    if toggle {
        if !set.remove(clicked) {
            set.insert(clicked.clone());
        }
    } else if plain == PlainClick::Replace {
        set.clear();
        set.insert(clicked.clone());
    }
    *anchor = Some(clicked.clone());
    *set != before
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER: [u32; 6] = [10, 20, 30, 40, 50, 60];
    fn shift() -> Modifiers {
        Modifiers::SHIFT
    }
    fn ctrl() -> Modifiers {
        Modifiers::CTRL
    }
    fn go(
        set: &mut HashSet<u32>,
        anchor: &mut Option<u32>,
        row: u32,
        mods: Modifiers,
        plain: PlainClick,
    ) {
        click(set, anchor, &ORDER, &row, mods, plain);
    }
    fn sorted(set: &HashSet<u32>) -> Vec<u32> {
        let mut v: Vec<u32> = set.iter().copied().collect();
        v.sort();
        v
    }

    #[test]
    fn a_plain_click_selects_only_that_row() {
        let (mut set, mut anchor) = (HashSet::from([10, 20]), None);
        go(
            &mut set,
            &mut anchor,
            30,
            Modifiers::NONE,
            PlainClick::Replace,
        );
        assert_eq!(sorted(&set), vec![30]);
        assert_eq!(anchor, Some(30));
    }

    #[test]
    fn a_plain_click_can_leave_a_tick_box_set_alone() {
        let (mut set, mut anchor) = (HashSet::from([10, 20]), None);
        go(&mut set, &mut anchor, 30, Modifiers::NONE, PlainClick::Keep);
        assert_eq!(sorted(&set), vec![10, 20]);
        assert_eq!(anchor, Some(30), "but it still moves the anchor");
    }

    #[test]
    fn ctrl_click_toggles_one_row() {
        let (mut set, mut anchor) = (HashSet::from([10]), None);
        go(&mut set, &mut anchor, 30, ctrl(), PlainClick::Replace);
        assert_eq!(sorted(&set), vec![10, 30]);
        go(&mut set, &mut anchor, 10, ctrl(), PlainClick::Replace);
        assert_eq!(sorted(&set), vec![30]);
    }

    #[test]
    fn shift_click_selects_the_range_from_the_anchor_either_way() {
        let (mut set, mut anchor) = (HashSet::new(), None);
        go(
            &mut set,
            &mut anchor,
            20,
            Modifiers::NONE,
            PlainClick::Replace,
        );
        go(&mut set, &mut anchor, 50, shift(), PlainClick::Replace);
        assert_eq!(sorted(&set), vec![20, 30, 40, 50]);
        // A second Shift+click re-ranges from the same anchor.
        go(&mut set, &mut anchor, 10, shift(), PlainClick::Replace);
        assert_eq!(sorted(&set), vec![10, 20]);
    }

    #[test]
    fn ctrl_shift_adds_the_range_to_what_is_selected() {
        let (mut set, mut anchor) = (HashSet::new(), None);
        go(
            &mut set,
            &mut anchor,
            10,
            Modifiers::NONE,
            PlainClick::Replace,
        );
        go(&mut set, &mut anchor, 40, ctrl(), PlainClick::Replace);
        go(
            &mut set,
            &mut anchor,
            60,
            Modifiers::CTRL | Modifiers::SHIFT,
            PlainClick::Replace,
        );
        assert_eq!(sorted(&set), vec![10, 40, 50, 60]);
    }

    #[test]
    fn shift_without_an_anchor_acts_like_a_plain_click() {
        let (mut set, mut anchor) = (HashSet::new(), None);
        go(&mut set, &mut anchor, 30, shift(), PlainClick::Replace);
        assert_eq!(sorted(&set), vec![30]);
        assert_eq!(anchor, Some(30));
    }
}
