//! Minimal shared light/dark theme concept (Tier-1 "Application-Wide"
//! setting in both SDB and SSP's three-tier settings model). Wraps egui's
//! own built-in `Visuals::dark()`/`light()` -- this is deliberately not a
//! design-token system (see SDB's `docs/ui-refinements-todo.md` "Tier 2"
//! note: a full color/token system is a separate, larger discussion). Each
//! app owns persistence (its own `ApplicationSettings`-equivalent) and
//! applies `visuals()` to its `egui::Context` on change.

use crate::tokens::{ColorToken, DimensionToken};
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

    pub const ALL: [TextScale; 4] = [
        TextScale::Small,
        TextScale::Default,
        TextScale::Large,
        TextScale::ExtraLarge,
    ];
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

/// Single text size shared by every label in the command & status panel
/// (feed entries, undo/redo summary, cursor readout, annunciator captions)
/// -- Chris, 2026-07-22: "there look to be various different text sizes in
/// the passive window... make them all the same." Multiply by
/// [`current_text_scale`] at each call site.
///
/// Renamed from `PASSIVE_PANEL_TEXT_SIZE` on 2026-08-10, when SDB retired
/// "passive" as product vocabulary (the panel takes command input and sets
/// the drafting scale, so the word was wrong as well as vague). SSP agreed
/// to the same rename on 2026-08-21, so the `passive_panel` module in this
/// crate followed too, becoming [`crate::command_status_panel`], and the interim
/// `PASSIVE_PANEL_TEXT_SIZE` alias this const briefly kept is gone.
pub const COMMAND_STATUS_PANEL_TEXT_SIZE: f32 = 10.0;

/// Header bar tokens for a themed in-app `egui::Window` (see
/// `crate::window_chrome`). Docs/design/2026-08-30_theme-system-spec.md
/// (SDB repo) is the source spec; values below come from the Lunacy mockup
/// that spec was extracted from.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct HeaderTokens {
    pub background: ColorToken,
    pub height: DimensionToken,
    pub border_bottom: ColorToken,
    pub title_color: ColorToken,
}

/// Shared by close/minimize/maximize -- flat glyph buttons, no fill at
/// rest, a highlight rect only on hover. Revised 2026-08-30 (was a filled
/// circle matching the Lunacy mockup) to match native Windows chrome once
/// Chris saw the mockup-accurate version rendered next to his real title
/// bar. One size token set covers all three; there's no per-button
/// variation.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct ButtonTokens {
    pub hover_background: ColorToken,
    pub icon_color: ColorToken,
}

/// Full window-chrome token set for one theme.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct WindowChromeTokens {
    pub background: ColorToken,
    pub corner_radius: DimensionToken,
    pub shadow_blur: DimensionToken,
    pub shadow_color: ColorToken,
    /// App-wide themeable window border -- Chris, 2026-08-30, inspired by
    /// Windows' own accent-colored title bar border. Built-ins ship this
    /// at zero width / transparent (no visible change from the
    /// shadow-only look); it exists so the Themes Panel can expose it as
    /// an editable control.
    pub border_width: DimensionToken,
    pub border_color: ColorToken,
    pub header: HeaderTokens,
    pub button: ButtonTokens,
}

/// A named, importable/exportable theme. `base` picks which built-in
/// `egui::Visuals` a theme inherits everything this spec doesn't yet
/// tokenize from (text selection color, hyperlink color, etc.) --
/// see docs/design/2026-08-30_theme-system-spec.md's "Scope boundary"
/// section for why this doesn't yet cover every `Visuals` field.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct ThemePalette {
    pub name: String,
    pub base: Theme,
    pub window: WindowChromeTokens,
}

impl ThemePalette {
    pub fn dark() -> Self {
        Self {
            name: "Dark".to_string(),
            base: Theme::Dark,
            window: WindowChromeTokens {
                background: ColorToken::new(egui::Color32::from_rgb(0x22, 0x22, 0x26)),
                corner_radius: DimensionToken::new(12.0),
                shadow_blur: DimensionToken::new(20.0),
                shadow_color: ColorToken::new(egui::Color32::from_black_alpha(0x40)),
                border_width: DimensionToken::new(0.0),
                border_color: ColorToken::new(egui::Color32::TRANSPARENT),
                header: HeaderTokens {
                    background: ColorToken::new(egui::Color32::from_rgb(0x2E, 0x2E, 0x32)),
                    height: DimensionToken::new(32.0),
                    border_bottom: ColorToken::new(egui::Color32::from_rgba_unmultiplied(
                        0xFF, 0xFF, 0xFF, 0x26,
                    )),
                    title_color: ColorToken::new(egui::Color32::WHITE),
                },
                button: ButtonTokens {
                    hover_background: ColorToken::new(egui::Color32::from_rgb(0x38, 0x38, 0x3C)),
                    icon_color: ColorToken::new(egui::Color32::WHITE),
                },
            },
        }
    }

    pub fn light() -> Self {
        Self {
            name: "Light".to_string(),
            base: Theme::Light,
            window: WindowChromeTokens {
                background: ColorToken::new(egui::Color32::from_rgb(0xFA, 0xFA, 0xFB)),
                corner_radius: DimensionToken::new(12.0),
                shadow_blur: DimensionToken::new(20.0),
                shadow_color: ColorToken::new(egui::Color32::from_black_alpha(0x40)),
                border_width: DimensionToken::new(0.0),
                border_color: ColorToken::new(egui::Color32::TRANSPARENT),
                header: HeaderTokens {
                    background: ColorToken::new(egui::Color32::from_rgb(0xEB, 0xEB, 0xEB)),
                    height: DimensionToken::new(32.0),
                    border_bottom: ColorToken::new(egui::Color32::from_rgba_unmultiplied(
                        0x00, 0x00, 0x00, 0x26,
                    )),
                    title_color: ColorToken::new(egui::Color32::from_rgba_unmultiplied(
                        0x00, 0x00, 0x06, 0xCC,
                    )),
                },
                button: ButtonTokens {
                    hover_background: ColorToken::new(egui::Color32::from_rgba_unmultiplied(
                        0x00, 0x00, 0x06, 0x1F,
                    )),
                    icon_color: ColorToken::new(egui::Color32::from_rgba_unmultiplied(
                        0x00, 0x00, 0x06, 0xCC,
                    )),
                },
            },
        }
    }
}

#[cfg(test)]
mod theme_palette_tests {
    use super::*;

    #[test]
    fn dark_and_light_round_trip_through_json() {
        for palette in [ThemePalette::dark(), ThemePalette::light()] {
            let json = serde_json::to_string(&palette).unwrap();
            let back: ThemePalette = serde_json::from_str(&json).unwrap();
            assert_eq!(palette, back);
        }
    }
}

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
    let live_body = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Body)
        .map(|f| f.size)
        .unwrap_or(default_body);
    live_body / default_body
}
