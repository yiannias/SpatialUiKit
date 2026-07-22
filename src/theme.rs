//! Minimal shared light/dark theme concept (Tier-1 "Application-Wide"
//! setting in both SDB and SSP's three-tier settings model). Wraps egui's
//! own built-in `Visuals::dark()`/`light()` -- this is deliberately not a
//! design-token system (see SDB's `docs/ui-refinements-todo.md` "Tier 2"
//! note: a full color/token system is a separate, larger discussion). Each
//! app owns persistence (its own `ApplicationSettings`-equivalent) and
//! applies `visuals()` to its `egui::Context` on change.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub fn visuals(&self) -> egui::Visuals {
        match self {
            Theme::Dark => egui::Visuals::dark(),
            Theme::Light => egui::Visuals::light(),
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Theme::Dark => "Dark",
            Theme::Light => "Light",
        }
    }

    pub const ALL: [Theme; 2] = [Theme::Dark, Theme::Light];
}

/// UI text-size preset (Tier-1 "Application-Wide" setting, alongside
/// `Theme`) -- Chris, 2026-07-21: "lots of the interface text is too small
/// or inconsistent." A discrete multiplier over egui's default font sizes,
/// not a continuous slider -- matches the rest of this settings tree's
/// dropdown-of-presets style (see `Theme`) rather than adding a new slider
/// `FieldControl` variant for one setting.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug, Default)]
pub enum TextScale {
    Small,
    #[default]
    Default,
    Large,
    ExtraLarge,
}

impl TextScale {
    pub fn multiplier(&self) -> f32 {
        match self {
            TextScale::Small => 0.9,
            TextScale::Default => 1.0,
            TextScale::Large => 1.15,
            TextScale::ExtraLarge => 1.3,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            TextScale::Small => "Small (90%)",
            TextScale::Default => "Default (100%)",
            TextScale::Large => "Large (115%)",
            TextScale::ExtraLarge => "Extra Large (130%)",
        }
    }

    pub const ALL: [TextScale; 4] =
        [TextScale::Small, TextScale::Default, TextScale::Large, TextScale::ExtraLarge];
}

/// Applies `scale` to every named `egui::TextStyle`'s font size, scaled from
/// egui's own default baseline sizes each time (not compounding on whatever
/// the live style currently holds) -- so switching presets back and forth
/// is exact rather than drifting, and doesn't require the caller to track a
/// "previous scale" to undo first. Only touches font sizes, not spacing/
/// padding, so this is a text-legibility knob, not a general UI-zoom one.
pub fn apply_text_scale(ctx: &egui::Context, scale: TextScale) {
    let baseline = egui::Style::default().text_styles;
    ctx.all_styles_mut(|style| {
        for (text_style, font_id) in &baseline {
            let mut font_id = font_id.clone();
            font_id.size *= scale.multiplier();
            style.text_styles.insert(text_style.clone(), font_id);
        }
    });
}

/// Single text size shared by every label in the passive panel (feed
/// entries, undo/redo summary, cursor readout, annunciator captions) --
/// Chris, 2026-07-22: "there look to be various different text sizes in the
/// passive window... make them all the same." Multiply by
/// [`current_text_scale`] at each call site.
pub const PASSIVE_PANEL_TEXT_SIZE: f32 = 10.0;

/// Current text-scale ratio in effect, derived from how far the live
/// `TextStyle::Body` size has diverged from egui's own default -- lets code
/// that still needs a raw pixel size (a custom `painter.text` call, or a
/// `RichText` size not backed by a named `TextStyle`) respond to
/// [`apply_text_scale`] without threading a scale value through every call
/// site's signature. Multiply a hardcoded literal by this.
pub fn current_text_scale(ui: &egui::Ui) -> f32 {
    let default_body = egui::Style::default()
        .text_styles
        .get(&egui::TextStyle::Body)
        .map(|f| f.size)
        .unwrap_or(14.0);
    if default_body <= 0.0 {
        return 1.0;
    }
    let live_body = ui.style().text_styles.get(&egui::TextStyle::Body).map(|f| f.size).unwrap_or(default_body);
    live_body / default_body
}
