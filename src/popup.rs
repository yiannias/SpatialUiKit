//! Popup lifecycle: the one place that decides when a popup closes and where
//! focus goes afterwards.
//!
//! A popup draws its own chrome (a card, a menu, a window). What every popup
//! shares is its lifecycle, which used to be hand-rolled per family:
//!
//! * **Escape closes it**, unless a dropdown above it owns that Escape.
//! * **A press outside closes it**, but never the press that opened it.
//! * **Focus comes back**: the widget that had focus when the popup opened is
//!   focused again when it closes.
//! * While open it **stamps ownership of Escape**, so a global Escape handler
//!   that runs before the popup draws leaves the key to it
//!   ([`popup_owns_escape`], the same idea as
//!   [`crate::dropdown::dropdown_owns_escape`]).
//!
//! Use: call [`dismissal`] every frame the popup is drawn, with the rectangle
//! it occupies; close the popup when it returns a reason; call [`finish`] once
//! when it is closed (or [`finish`] from wherever the open flag is cleared) so
//! the next opening starts fresh and focus returns.

use egui::{Context, Id, Rect};

/// Why a popup should close.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    /// Escape was pressed.
    Escape,
    /// The pointer was pressed outside the popup.
    OutsideClick,
}

/// Per-popup remembered state: the focus owner at open, and that the popup has
/// drawn at least once.
#[derive(Clone, Copy, Default)]
struct PopupState {
    previous_focus: Option<Id>,
}

fn state_id(key: Id) -> Id {
    key.with("sdb_popup_state")
}

/// Whether a popup of this window is open (or was last frame). A global Escape
/// handler that runs before popups draw must not act while this is true: the
/// popup closes itself on that Escape. Expires after one frame.
pub fn popup_owns_escape(ctx: &Context) -> bool {
    let now = ctx.cumulative_frame_nr();
    ctx.memory(|m| m.data.get_temp::<u64>(Id::new("sdb_popup_open_stamp")))
        .is_some_and(|stamp| stamp.saturating_add(1) >= now)
}

fn stamp_open(ctx: &Context) {
    let now = ctx.cumulative_frame_nr();
    ctx.memory_mut(|m| m.data.insert_temp(Id::new("sdb_popup_open_stamp"), now));
}

/// Call once per frame the popup is drawn, with the rectangle it occupies.
/// Returns the reason it should close, if any. The frame it first appears is
/// never an outside click (that press is the click that opened it), and the
/// focus owner at that moment is remembered for [`finish`].
pub fn dismissal(ctx: &Context, key: Id, rect: Rect) -> Option<CloseReason> {
    let id = state_id(key);
    let first = ctx.memory(|m| m.data.get_temp::<PopupState>(id)).is_none();
    if first {
        let previous_focus = ctx.memory(|m| m.focused());
        ctx.memory_mut(|m| m.data.insert_temp(id, PopupState { previous_focus }));
        ctx.request_repaint();
    }
    stamp_open(ctx);
    if first {
        return None;
    }
    // A dropdown open above the popup closes itself on this Escape first.
    let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape))
        && !crate::dropdown::dropdown_owns_escape(ctx);
    if escape {
        return Some(CloseReason::Escape);
    }
    let outside = ctx.input(|i| {
        i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !rect.contains(p))
    });
    outside.then_some(CloseReason::OutsideClick)
}

/// Call when the popup has closed (or is not open): forgets its state so the
/// next opening gets its first-frame guard again, and gives focus back to the
/// widget that had it when the popup opened. Safe to call every frame while
/// closed: it only acts on a popup that was open.
pub fn finish(ctx: &Context, key: Id) {
    let id = state_id(key);
    let state = ctx.memory(|m| m.data.get_temp::<PopupState>(id));
    if let Some(state) = state {
        ctx.memory_mut(|m| m.data.remove::<PopupState>(id));
        if let Some(previous) = state.previous_focus {
            ctx.memory_mut(|m| m.request_focus(previous));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Key, Modifiers, PointerButton, Pos2, RawInput, Vec2};

    fn frame(ctx: &Context, time: f64, events: Vec<Event>, mut run: impl FnMut(&Context)) {
        let _ = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| run(ui.ctx()),
        );
    }

    fn press(pos: Pos2) -> Vec<Event> {
        vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ]
    }

    fn escape() -> Vec<Event> {
        vec![Event::Key {
            key: Key::Escape,
            physical_key: Some(Key::Escape),
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]
    }

    const KEY: &str = "test_popup";
    fn rect() -> Rect {
        Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(100.0, 60.0))
    }

    /// The frame a popup first appears never closes it, even with a press
    /// outside (that press opened it).
    #[test]
    fn the_opening_press_does_not_close_the_popup() {
        let ctx = Context::default();
        let mut reason = Some(CloseReason::Escape);
        frame(&ctx, 1.0, press(Pos2::new(10.0, 10.0)), |ctx| {
            reason = dismissal(ctx, Id::new(KEY), rect());
        });
        assert_eq!(reason, None);
    }

    #[test]
    fn escape_closes_and_a_press_inside_does_not() {
        let ctx = Context::default();
        frame(&ctx, 1.0, vec![], |ctx| {
            dismissal(ctx, Id::new(KEY), rect());
        });
        let mut inside = Some(CloseReason::Escape);
        frame(&ctx, 2.0, press(Pos2::new(120.0, 120.0)), |ctx| {
            inside = dismissal(ctx, Id::new(KEY), rect());
        });
        assert_eq!(inside, None, "a press inside stays open");
        let mut esc = None;
        frame(&ctx, 3.0, escape(), |ctx| {
            esc = dismissal(ctx, Id::new(KEY), rect());
        });
        assert_eq!(esc, Some(CloseReason::Escape));
    }

    #[test]
    fn a_press_outside_closes() {
        let ctx = Context::default();
        frame(&ctx, 1.0, vec![], |ctx| {
            dismissal(ctx, Id::new(KEY), rect());
        });
        let mut reason = None;
        frame(&ctx, 2.0, press(Pos2::new(350.0, 250.0)), |ctx| {
            reason = dismissal(ctx, Id::new(KEY), rect());
        });
        assert_eq!(reason, Some(CloseReason::OutsideClick));
    }

    /// An open popup owns Escape for handlers that run before it draws.
    #[test]
    fn an_open_popup_owns_escape() {
        let ctx = Context::default();
        let mut before = true;
        frame(&ctx, 1.0, vec![], |ctx| {
            before = popup_owns_escape(ctx);
            dismissal(ctx, Id::new(KEY), rect());
        });
        assert!(!before, "nothing open yet");
        let mut owned = false;
        frame(&ctx, 2.0, vec![], |ctx| {
            owned = popup_owns_escape(ctx);
        });
        assert!(owned);
    }

    /// Focus goes back to the widget that had it when the popup opened, and
    /// the next opening starts with its first-frame guard again.
    #[test]
    fn finish_returns_focus_and_resets() {
        let ctx = Context::default();
        let trigger = Id::new("trigger");
        frame(&ctx, 1.0, vec![], |ctx| {
            ctx.memory_mut(|m| m.request_focus(trigger));
        });
        frame(&ctx, 2.0, vec![], |ctx| {
            dismissal(ctx, Id::new(KEY), rect());
            // Something inside the popup takes focus.
            ctx.memory_mut(|m| m.request_focus(Id::new("inside")));
        });
        frame(&ctx, 3.0, vec![], |ctx| {
            finish(ctx, Id::new(KEY));
        });
        let mut focused = None;
        frame(&ctx, 4.0, vec![], |ctx| {
            focused = ctx.memory(|m| m.focused());
        });
        assert_eq!(focused, Some(trigger), "focus returned to the trigger");

        // Reopened: the first frame is guarded again.
        let mut reason = Some(CloseReason::Escape);
        frame(&ctx, 5.0, press(Pos2::new(10.0, 10.0)), |ctx| {
            reason = dismissal(ctx, Id::new(KEY), rect());
        });
        assert_eq!(reason, None);
    }
}
