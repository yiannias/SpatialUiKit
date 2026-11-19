//! Type to find: the one rule for typing letters into a list that has no
//! search box (a pull-down, a tree).
//!
//! * Typed characters build a prefix; the highlight jumps to the first row
//!   that starts with it (case-insensitive), searching from the current row.
//! * The prefix is forgotten after a pause ([`PAUSE`] seconds without typing),
//!   so the next letter starts fresh.
//! * Pressing one letter repeatedly (`b`, `b`, `b`) cycles through the rows
//!   starting with that letter instead of building "bbb".
//!
//! The list owns a [`TypeFind`] between frames, feeds it the frame's typed
//! text, and asks it for the row to highlight.

/// Seconds without a typed character after which the prefix starts over.
pub const PAUSE: f64 = 1.0;

/// The typed prefix and when it was last extended.
#[derive(Clone, Debug, Default)]
pub struct TypeFind {
    buffer: String,
    last: f64,
}

impl TypeFind {
    /// The prefix typed so far.
    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// Forgets the prefix.
    pub fn reset(&mut self) {
        self.buffer.clear();
    }

    /// Adds the characters typed this frame (`now` in seconds). Only letters
    /// and digits count (spaces and control characters are not searches).
    /// Returns whether anything was added.
    pub fn feed(&mut self, typed: &str, now: f64) -> bool {
        let chars: Vec<char> = typed.chars().filter(|c| c.is_alphanumeric()).collect();
        if chars.is_empty() {
            return false;
        }
        if now - self.last > PAUSE {
            self.buffer.clear();
        }
        self.buffer.extend(chars);
        self.last = now;
        true
    }

    /// The row to highlight for the current prefix, among `texts` (a row
    /// that cannot be found, such as a heading, is the empty string), from
    /// the row currently highlighted. `None` when nothing matches.
    pub fn find(&self, texts: &[&str], from: Option<usize>) -> Option<usize> {
        let n = texts.len();
        if n == 0 || self.buffer.is_empty() {
            return None;
        }
        let query: Vec<char> = self.buffer.chars().flat_map(|c| c.to_lowercase()).collect();
        let cycling = query.iter().all(|c| *c == query[0]);
        let needle: Vec<char> = if cycling { vec![query[0]] } else { query };
        // One repeated letter moves on from the current row; a longer prefix
        // may stay on it.
        let start = match from {
            Some(f) if cycling => f + 1,
            Some(f) => f,
            None => 0,
        };
        (0..n).map(|k| (start + k) % n).find(|&i| {
            let text: Vec<char> = texts[i].chars().flat_map(|c| c.to_lowercase()).collect();
            !text.is_empty() && text.starts_with(&needle)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROWS: [&str; 6] = ["Arc", "Beam", "Bevel", "Block", "Circle", "Line"];

    #[test]
    fn letters_build_a_prefix_and_jump_to_it() {
        let mut t = TypeFind::default();
        t.feed("b", 1.0);
        assert_eq!(t.find(&ROWS, None), Some(1));
        t.feed("l", 1.2);
        assert_eq!(t.buffer(), "bl");
        assert_eq!(t.find(&ROWS, Some(1)), Some(3), "Block");
    }

    #[test]
    fn a_pause_starts_the_prefix_over() {
        let mut t = TypeFind::default();
        t.feed("b", 1.0);
        t.feed("l", 3.0);
        assert_eq!(t.buffer(), "l");
        assert_eq!(t.find(&ROWS, None), Some(5));
    }

    #[test]
    fn one_letter_repeated_cycles_and_wraps() {
        let mut t = TypeFind::default();
        t.feed("b", 1.0);
        assert_eq!(t.find(&ROWS, Some(0)), Some(1));
        t.feed("b", 1.1);
        assert_eq!(t.find(&ROWS, Some(1)), Some(2));
        t.feed("b", 1.2);
        assert_eq!(t.find(&ROWS, Some(2)), Some(3));
        t.feed("b", 1.3);
        assert_eq!(t.find(&ROWS, Some(3)), Some(1), "wraps to the first B");
    }

    #[test]
    fn headings_and_misses_are_not_found() {
        let mut t = TypeFind::default();
        t.feed("z", 1.0);
        assert_eq!(t.find(&ROWS, None), None);
        let mut t = TypeFind::default();
        t.feed("a", 1.0);
        assert_eq!(t.find(&["", "Arc"], None), Some(1));
        assert!(!t.feed(" ", 2.0), "a space is not a search");
    }
}
