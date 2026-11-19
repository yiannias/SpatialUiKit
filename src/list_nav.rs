//! Arrow-key list navigation (roving highlight): the one implementation of how
//! Up / Down / PageUp / PageDown / Home / End move a highlight through a list.
//!
//! The dropdown and the font picker each had their own copy; this is the
//! shared one. A list passes the indices that can be highlighted (headings and
//! disabled rows left out), the current highlight and the keys pressed, and
//! gets the next highlight back. Movement clamps at the ends and, with no
//! highlight yet, forward lands on the first entry and backward on the last.

use egui::{Key, Modifiers};

/// Which navigation keys were pressed this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NavKeys {
    pub down: bool,
    pub up: bool,
    pub page_down: bool,
    pub page_up: bool,
    pub home: bool,
    pub end: bool,
}

impl NavKeys {
    /// Whether any navigation key was pressed.
    pub fn any(&self) -> bool {
        self.down || self.up || self.page_down || self.page_up || self.home || self.end
    }
}

/// Consumes the navigation keys (no modifiers) so nothing else, such as a text
/// field above the list, sees them.
pub fn consume_keys(ui: &mut egui::Ui) -> NavKeys {
    ui.input_mut(|i| {
        let m = Modifiers::NONE;
        NavKeys {
            down: i.consume_key(m, Key::ArrowDown),
            up: i.consume_key(m, Key::ArrowUp),
            page_down: i.consume_key(m, Key::PageDown),
            page_up: i.consume_key(m, Key::PageUp),
            home: i.consume_key(m, Key::Home),
            end: i.consume_key(m, Key::End),
        }
    })
}

/// Moves a highlight by `delta` through `nav` (the highlightable indices, top
/// to bottom), clamped to the ends. With no highlight, forward lands on the
/// first entry and backward on the last.
pub fn step(nav: &[usize], cursor: Option<usize>, delta: isize) -> Option<usize> {
    if nav.is_empty() {
        return None;
    }
    match cursor.and_then(|c| nav.iter().position(|&i| i == c)) {
        Some(p) => Some(nav[(p as isize + delta).clamp(0, nav.len() as isize - 1) as usize]),
        None if delta >= 0 => Some(nav[0]),
        None => Some(nav[nav.len() - 1]),
    }
}

/// The highlight after the keys in `keys`, applied in the order Down, Up,
/// PageDown, PageUp, Home, End (so a later key wins). `page` is how many
/// entries a page moves.
pub fn apply(nav: &[usize], cursor: Option<usize>, keys: NavKeys, page: isize) -> Option<usize> {
    let mut cursor = cursor;
    if keys.down {
        cursor = step(nav, cursor, 1);
    }
    if keys.up {
        cursor = step(nav, cursor, -1);
    }
    if keys.page_down {
        cursor = step(nav, cursor, page.max(1));
    }
    if keys.page_up {
        cursor = step(nav, cursor, -page.max(1));
    }
    if keys.home {
        cursor = nav.first().copied();
    }
    if keys.end {
        cursor = nav.last().copied();
    }
    cursor
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAV: [usize; 4] = [1, 3, 4, 7];

    #[test]
    fn steps_clamp_and_skip_unlistable_rows() {
        assert_eq!(step(&NAV, Some(1), 1), Some(3));
        assert_eq!(step(&NAV, Some(3), 1), Some(4));
        assert_eq!(step(&NAV, Some(7), 1), Some(7), "clamps at the end");
        assert_eq!(step(&NAV, Some(1), -1), Some(1), "clamps at the start");
        assert_eq!(step(&NAV, Some(4), -1), Some(3));
        assert_eq!(step(&[], Some(1), 1), None);
    }

    /// With no highlight (or one that is not listable), forward starts at the
    /// first entry and backward at the last.
    #[test]
    fn no_highlight_starts_at_the_ends() {
        assert_eq!(step(&NAV, None, 1), Some(1));
        assert_eq!(step(&NAV, None, -1), Some(7));
        assert_eq!(step(&NAV, Some(2), 1), Some(1), "row 2 is not in the list");
    }

    #[test]
    fn home_end_and_pages_move_through_the_list() {
        let keys = |f: fn(&mut NavKeys)| {
            let mut k = NavKeys::default();
            f(&mut k);
            k
        };
        assert_eq!(apply(&NAV, Some(4), keys(|k| k.home = true), 3), Some(1));
        assert_eq!(apply(&NAV, Some(3), keys(|k| k.end = true), 3), Some(7));
        assert_eq!(
            apply(&NAV, Some(1), keys(|k| k.page_down = true), 2),
            Some(4)
        );
        assert_eq!(apply(&NAV, Some(7), keys(|k| k.page_up = true), 2), Some(3));
        assert_eq!(apply(&NAV, Some(3), NavKeys::default(), 2), Some(3));
        assert!(!NavKeys::default().any());
    }
}
