//! Idle-timeout command-alias buffer, generic over an app-supplied action
//! type `A` -- ported from SpatialDrawingBoard's `chrome::alias_buffer`
//! (`crates/sdb_ui/src/chrome/alias_buffer.rs`), ADR 0010's alias-resolving
//! command line had kept this app-specific; this module is the reversal for
//! that one piece, following the same generic-over-`A` shape as
//! [`crate::menu`] and [`crate::ribbon`].
//!
//! What it is: the AutoCAD-style "just start typing" command path. The user
//! types a command alias (e.g. `L`, `CO`, `M`) *anywhere*, without first
//! clicking into a command-line text field; after a short pause with no new
//! keystrokes, the accumulated buffer is resolved against the app's alias
//! table and dispatched. It's the idle-timeout complement to an
//! always-available, Enter-driven command line -- both can coexist (SDB's
//! passive panel command line keeps its own unconditional Enter-driven
//! resolution regardless of this module's timeout mode).
//!
//! Unlike [`crate::menu::MenuNode`], this module has **no `Ctx` type
//! parameter**. The menu tree is built once and cached across many frames
//! (hence needing `Fn(&Ctx) -> bool` closures to re-evaluate live state each
//! frame), whereas [`update`] runs fresh every frame and simply takes
//! whatever state it needs (`enter_only`, `tool_active`, `timeout_ms`) as
//! plain arguments the caller reads from its own context that frame --
//! there's nothing to cache, so there's nothing to genericize over.
//!
//! What stays app-specific: the alias table itself (what strings resolve to
//! what command, and their display names) -- supplied through the
//! [`AliasResolver`] trait -- and turning a resolved command id into the
//! app's own action `A`, supplied as a closure by the caller.

use std::time::{Duration, Instant};

/// How long a resolved alias's flash confirmation stays visible in
/// [`capsule`]. Matches SDB's original constant.
pub const DEFAULT_FLASH_DURATION: Duration = Duration::from_millis(450);

/// Per-frame accumulator state, owned by the caller (one instance per app,
/// analogous to SDB's `App::alias_buffer` field).
#[derive(Default)]
pub struct IdleInputState {
    buffer: String,
    last_key_time: Option<Instant>,
    flash: Option<(String, Instant)>,
}

impl IdleInputState {
    /// Discard any in-progress buffer and pending flash immediately (called
    /// on `Escape`, and when the app switches into "Enter Required" mode).
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.last_key_time = None;
    }

    /// The buffer's current contents, for a caller that wants to draw its
    /// own capsule instead of using [`capsule`].
    pub fn text(&self) -> &str {
        &self.buffer
    }
}

/// One alias match: the resolved command's stable id (handed to the
/// caller's action-constructing closure) and its display name (shown in the
/// flash confirmation).
pub struct AliasMatch {
    pub id: &'static str,
    pub display_name: &'static str,
}

/// The app's own alias table lookup -- mirrors SDB's
/// `keybindings::AliasOverrides::resolve` + `keybindings::find` pair. Kept
/// as a trait rather than a closure since a real implementation typically
/// needs two related lookups (resolve the alias, then look up the resolved
/// command's display name); a single `resolve` call returning both avoids
/// forcing the caller to search twice.
pub trait AliasResolver {
    fn resolve(&self, text: &str) -> Option<AliasMatch>;
}

/// Advance the idle-input buffer by one frame: gate on the caller's current
/// mode/focus state, accumulate typed characters, and once the idle
/// timeout elapses, resolve and return an action if the buffer matched.
///
/// - `enter_only`: the app's "Enter Required" preference is on -- when
///   true, this function does nothing (mirrors SDB's
///   `command_entry_enter_only`).
/// - `tool_active`: an interactive tool/command is currently running and
///   owns typed keys instead (e.g. dynamic input shorthand like `10<45`) --
///   when true, this function does nothing (mirrors SDB's
///   `!active_tool_is_none`).
/// - `timeout_ms`: idle duration after the last keystroke before the buffer
///   is resolved and dispatched.
///
/// Also does nothing while an egui widget has keyboard focus, so it never
/// steals keystrokes from a real text field -- that check is generic and
/// needs no app input, unlike the two flags above.
pub fn update<A>(
    ui: &mut egui::Ui,
    state: &mut IdleInputState,
    resolver: &impl AliasResolver,
    make_action: impl Fn(&'static str) -> A,
    enter_only: bool,
    tool_active: bool,
    timeout_ms: u64,
) -> Option<A> {
    if enter_only || tool_active {
        state.clear();
        return None;
    }
    // Don't fight typing into an actual widget (a command line, a rename
    // field, a dialog, ...).
    if ui.memory(|m| m.focused().is_some()) {
        return None;
    }

    let egui_ctx = ui.ctx().clone();

    if egui_ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.clear();
        return None;
    }

    let mut appended = false;
    egui_ctx.input(|i| {
        for ev in &i.events {
            if let egui::Event::Text(s) = ev {
                for c in s.chars() {
                    if c.is_ascii_alphanumeric() {
                        state.buffer.push(c.to_ascii_uppercase());
                        appended = true;
                    }
                }
            }
        }
    });
    if appended {
        state.last_key_time = Some(Instant::now());
    }

    if state.buffer.is_empty() {
        return None;
    }
    let Some(last) = state.last_key_time else {
        return None;
    };
    let timeout = Duration::from_millis(timeout_ms);
    if last.elapsed() < timeout {
        // Make sure we get a frame right around the timeout even if
        // nothing else is animating.
        egui_ctx.request_repaint_after(timeout - last.elapsed());
        return None;
    }

    let result = resolver.resolve(&state.buffer);
    state.clear();

    match result {
        Some(AliasMatch { id, display_name }) => {
            state.flash = Some((display_name.to_string(), Instant::now()));
            egui_ctx.request_repaint_after(DEFAULT_FLASH_DURATION);
            Some(make_action(id))
        }
        None => None,
    }
}

/// Floating pill anchored over the bottom-left corner of `viewport_rect`
/// (not the cursor), showing the buffer's live contents while typing, or
/// the resolved command's name briefly flashing green after it fires.
/// No-op while both are empty/expired.
pub fn capsule(ui: &mut egui::Ui, viewport_rect: egui::Rect, state: &IdleInputState) {
    let flashing = state
        .flash
        .as_ref()
        .is_some_and(|(_, at)| at.elapsed() < DEFAULT_FLASH_DURATION);
    if state.buffer.is_empty() && !flashing {
        return;
    }
    let ctx = ui.ctx().clone();

    let margin = egui::vec2(24.0, 24.0);
    let pos = if viewport_rect.width() > 1.0 && viewport_rect.height() > 1.0 {
        egui::pos2(viewport_rect.left() + margin.x, viewport_rect.bottom() - margin.y)
    } else {
        ui.max_rect().left_bottom() + egui::vec2(margin.x, -margin.y)
    };

    let (text, color): (&str, egui::Color32) = if flashing {
        let (name, _) = state.flash.as_ref().unwrap();
        (name.as_str(), egui::Color32::from_rgb(120, 230, 150))
    } else {
        (&state.buffer, egui::Color32::from_rgb(255, 178, 82))
    };

    egui::Area::new(egui::Id::new("idle_input_capsule"))
        .order(egui::Order::Tooltip)
        .fixed_pos(pos)
        .pivot(egui::Align2::LEFT_BOTTOM)
        .interactable(false)
        .show(&ctx, |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 35, 235))
                .stroke(egui::Stroke::new(2.0, color))
                .corner_radius(20)
                .inner_margin(egui::Margin::symmetric(20, 10))
                .show(ui, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(text)
                                .font(egui::FontId::monospace(28.0))
                                .strong()
                                .color(color),
                        )
                        .extend(),
                    );
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_resets_buffer_and_timer() {
        let mut state = IdleInputState {
            buffer: "CO".to_string(),
            last_key_time: Some(Instant::now()),
            flash: None,
        };
        state.clear();
        assert_eq!(state.text(), "");
        assert!(state.last_key_time.is_none());
    }
}
