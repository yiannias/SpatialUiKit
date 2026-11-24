//! The one place a hover hint is attached (SDB TASK-168, behaviour: tooltips).
//!
//! Panels used to call `on_hover_text` directly, so there was no single point
//! where an app-wide "tooltips on/off" switch could act. A hint now goes through
//! [`ResponseTooltipExt::hover_hint`], which does nothing while the app has
//! tooltips switched off with [`set_enabled`]. Hints are on by default, so an
//! app that never calls [`set_enabled`] behaves exactly as before.

use egui::{Context, Id, Response, WidgetText};

fn enabled_id() -> Id {
    Id::new("spatial_ui_kit_tooltips_enabled")
}

/// Switches hover hints on or off for the whole app. Held in egui's temporary
/// memory, so the app sets it from its own saved setting each frame (or when it
/// changes); a fresh context starts with hints on.
pub fn set_enabled(ctx: &Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(enabled_id(), on));
}

/// Whether hover hints are currently shown. `true` until [`set_enabled`] says
/// otherwise.
pub fn enabled(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(enabled_id()))
        .unwrap_or(true)
}

/// Hover hints that respect the app-wide switch. Implemented for [`Response`];
/// each method is the same as the egui method of the matching name when hints
/// are on, and returns the response untouched when they are off.
pub trait ResponseTooltipExt: Sized {
    /// `on_hover_text`, subject to the switch.
    fn hover_hint(self, text: impl Into<WidgetText>) -> Self;
    /// `on_disabled_hover_text`, subject to the switch.
    fn disabled_hover_hint(self, text: impl Into<WidgetText>) -> Self;
}

impl ResponseTooltipExt for Response {
    fn hover_hint(self, text: impl Into<WidgetText>) -> Self {
        if enabled(&self.ctx) {
            self.on_hover_text(text)
        } else {
            self
        }
    }

    fn disabled_hover_hint(self, text: impl Into<WidgetText>) -> Self {
        if enabled(&self.ctx) {
            self.on_disabled_hover_text(text)
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_are_on_until_switched_off_and_back_on() {
        let ctx = Context::default();
        assert!(enabled(&ctx));
        set_enabled(&ctx, false);
        assert!(!enabled(&ctx));
        set_enabled(&ctx, true);
        assert!(enabled(&ctx));
    }

    /// A hint can be attached with the switch on or off without panicking, and
    /// the response it returns is the same widget's.
    #[test]
    fn the_response_is_returned_either_way() {
        let ctx = Context::default();
        for on in [true, false] {
            set_enabled(&ctx, on);
            let _ = ctx.run_ui(Default::default(), |ui| {
                let plain = ui.label("x");
                let id = plain.id;
                let hinted = plain.hover_hint("tip");
                assert_eq!(hinted.id, id);
            });
        }
    }
}
