//! Minimal shared light/dark theme concept (Tier-1 "Application-Wide"
//! setting in both the host app's three-tier settings model). Wraps egui's
//! own built-in `Visuals::dark()`/`light()` -- this is deliberately not a
//! design-token system (see the host app's `docs/ui-refinements-todo.md` "Tier 2"
//! note: a full color/token system is a separate, larger discussion). Each
//! app owns persistence (its own `ApplicationSettings`-equivalent) and
//! applies `visuals()` to its `egui::Context` on change.

use crate::tokens::{ColorToken, DimensionToken};
use serde::{Deserialize, Deserializer, Serialize};

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

/// UI text-size scale: continuous percent value (75–175%, clamped).
/// Defaults to 100. Serializes/deserializes with backward compatibility:
/// accepts old enum names (Small→90, Default→100, Large→115, ExtraLarge→130)
/// or a plain number.
#[derive(Clone, Copy, PartialEq, Debug, Eq, PartialOrd, Ord)]
pub struct TextScale(pub u16);

impl TextScale {
    /// Minimum percent, clamped.
    pub const MIN: u16 = 75;
    /// Maximum percent, clamped.
    pub const MAX: u16 = 175;
    /// Default percent (100).
    pub const DEFAULT: u16 = 100;

    /// Clamps the given percent to the valid range.
    pub fn new(percent: u16) -> Self {
        TextScale(percent.clamp(Self::MIN, Self::MAX))
    }

    pub fn multiplier(&self) -> f32 {
        self.0 as f32 / 100.0
    }

    pub fn label(&self) -> String {
        format!("{}%", self.0)
    }

    /// Returns the next larger value, stepping by 5%, clamped at MAX.
    pub fn larger(self) -> TextScale {
        TextScale::new(self.0.saturating_add(5).min(Self::MAX))
    }

    /// Returns the next smaller value, stepping by 5%, clamped at MIN.
    pub fn smaller(self) -> TextScale {
        TextScale::new(self.0.saturating_sub(5).max(Self::MIN))
    }
}

impl Default for TextScale {
    fn default() -> Self {
        TextScale(Self::DEFAULT)
    }
}

impl Serialize for TextScale {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u16(self.0)
    }
}

// Helper enum for deserialization to accept both old enum names and new numbers
#[derive(Deserialize)]
#[serde(untagged)]
enum TextScaleRepr {
    Number(u16),
    String(String),
}

impl<'de> Deserialize<'de> for TextScale {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let repr = TextScaleRepr::deserialize(deserializer)?;
        match repr {
            TextScaleRepr::Number(percent) => Ok(TextScale::new(percent)),
            TextScaleRepr::String(s) => {
                // Backward compatibility: accept old enum names
                match s.as_str() {
                    "Small" => Ok(TextScale::new(90)),
                    "Default" => Ok(TextScale::new(100)),
                    "Large" => Ok(TextScale::new(115)),
                    "ExtraLarge" => Ok(TextScale::new(130)),
                    _ => Err(D::Error::custom(&format!("unknown TextScale: {}", s))),
                }
            }
        }
    }
}

/// Whole-UI scale: continuous percent value (75–200%, clamped).
/// Makes the ribbon, panels, icons, spacing *and* text larger or smaller together,
/// like Lunacy's View > Interface Scale. Applied as egui's zoom factor, which multiplies
/// the OS DPI scale, so [`TextScale`] stays a separate refinement that multiplies on top:
/// Interface 115% + Text 115% draws text at ~132%.
/// Defaults to 100. Serializes/deserializes with backward compatibility:
/// accepts old enum names (P80→80, P90→90, …, P150→150) or a plain number.
#[derive(Clone, Copy, PartialEq, Debug, Eq, PartialOrd, Ord)]
pub struct InterfaceScale(pub u16);

impl InterfaceScale {
    /// Minimum percent, clamped.
    pub const MIN: u16 = 75;
    /// Maximum percent, clamped.
    pub const MAX: u16 = 200;
    /// Default percent (100).
    pub const DEFAULT: u16 = 100;

    /// Clamps the given percent to the valid range.
    pub fn new(percent: u16) -> Self {
        InterfaceScale(percent.clamp(Self::MIN, Self::MAX))
    }

    pub fn multiplier(&self) -> f32 {
        self.0 as f32 / 100.0
    }

    pub fn label(&self) -> String {
        format!("{}%", self.0)
    }

    /// Returns the next larger value, stepping by 5%, clamped at MAX.
    pub fn larger(self) -> InterfaceScale {
        InterfaceScale::new(self.0.saturating_add(5).min(Self::MAX))
    }

    /// Returns the next smaller value, stepping by 5%, clamped at MIN.
    pub fn smaller(self) -> InterfaceScale {
        InterfaceScale::new(self.0.saturating_sub(5).max(Self::MIN))
    }
}

impl Default for InterfaceScale {
    fn default() -> Self {
        InterfaceScale(Self::DEFAULT)
    }
}

impl Serialize for InterfaceScale {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u16(self.0)
    }
}

// Helper enum for deserialization to accept both old enum names and new numbers
#[derive(Deserialize)]
#[serde(untagged)]
enum InterfaceScaleRepr {
    Number(u16),
    String(String),
}

impl<'de> Deserialize<'de> for InterfaceScale {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        let repr = InterfaceScaleRepr::deserialize(deserializer)?;
        match repr {
            InterfaceScaleRepr::Number(percent) => Ok(InterfaceScale::new(percent)),
            InterfaceScaleRepr::String(s) => {
                // Backward compatibility: accept old enum names (P80, P90, etc.)
                match s.as_str() {
                    "P80" => Ok(InterfaceScale::new(80)),
                    "P90" => Ok(InterfaceScale::new(90)),
                    "P100" => Ok(InterfaceScale::new(100)),
                    "P110" => Ok(InterfaceScale::new(110)),
                    "P115" => Ok(InterfaceScale::new(115)),
                    "P125" => Ok(InterfaceScale::new(125)),
                    "P150" => Ok(InterfaceScale::new(150)),
                    _ => Err(D::Error::custom(&format!("unknown InterfaceScale: {}", s))),
                }
            }
        }
    }
}

/// Applies `scale` as egui's zoom factor. Also turns off egui's built-in
/// Ctrl +/-/0 UI zoom: left on, it silently changes the same zoom factor
/// behind the saved setting's back, and those keys belong to the app.
pub fn apply_interface_scale(ctx: &egui::Context, scale: InterfaceScale) {
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
    ctx.set_zoom_factor(scale.multiplier());
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
/// Renamed from `PASSIVE_PANEL_TEXT_SIZE` on 2026-08-10, when the host app retired
/// "passive" as product vocabulary (the panel takes command input and sets
/// the drafting scale, so the word was wrong as well as vague). the host app agreed
/// to the same rename on 2026-08-21, so the `passive_panel` module in this
/// crate followed too, becoming [`crate::command_status_panel`], and the interim
/// `PASSIVE_PANEL_TEXT_SIZE` alias this const briefly kept is gone.
pub const COMMAND_STATUS_PANEL_TEXT_SIZE: f32 = 10.0;

/// Header bar tokens for a themed in-app `egui::Window` (see
/// `crate::window_chrome`). Docs/design/2026-08-30_theme-system-spec.md
/// (the host app repo) is the source spec; values below come from the Lunacy mockup
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

/// Background for a "floating surface" -- a ribbon module capsule, a docked
/// side panel's card, or the Command & Status Panel's own frame. Split out
/// from `WindowChromeTokens` (which is the four themed *dialogs'* chrome)
/// because Chris asked, 2026-09-05, for these three surfaces -- previously
/// two different colors (the OS-accent-driven `Visuals::window_fill`/
/// `panel_fill` for the ribbon/dock, a hardcoded near-black for the CSP) --
/// to share one settable color, after noticing the CSP's own distinct
/// background "helps a lot" for legibility against the viewport.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct SurfaceTokens {
    pub background: ColorToken,
    /// The 1px outline drawn around a floating surface. Added 2026-09-13:
    /// the ribbon's module capsules and the Command & Status Panel each had
    /// their *own* border literal -- an opaque `rgb(58, 59, 64)` in
    /// `ribbon::FRAME_STROKE` and a lighter, panel-opacity-modulated
    /// `rgba(90, 90, 100, alpha.max(120))` in
    /// [`crate::command_status_panel::panel_frame_themed`] -- so two things
    /// meant to read as the same kind of floating card were outlined
    /// differently. Chris, 2026-09-13: they should match, "for both light
    /// and dark mode." One token now feeds both.
    ///
    /// Unlike the fill above, this is **not** faded with the CSP's own
    /// opacity slider: the ribbon's border has always been fully opaque, and
    /// "same width, same color" only holds if the CSP's stops varying with a
    /// setting the ribbon has no equivalent of.
    pub border_color: ColorToken,
    pub border_width: DimensionToken,
}

/// Colors for the small chrome *controls* that sit on a surface -- a ribbon
/// module capsule's vertical label pill, the Command & Status Panel's
/// annunciator caps and its scale capsule, and the accent those light up in.
///
/// Split out from [`SurfaceTokens`] (which is the card these sit *on*)
/// because Chris, 2026-09-13, asked for exactly this group to be themeable
/// per-mode: the label pill and annunciator caps are near-black constants
/// carried over from the dark-only "graphical novel"/cockpit look, and read
/// as jarring dark blocks once the app is in light mode. Dark mode keeps
/// every one of its existing literals (see [`ThemePalette::dark`]); light
/// mode is where these actually differ.
///
/// The pill and the cap get *separate* background/foreground pairs rather
/// than one shared pair, because the dark theme's two are genuinely
/// different colors today (`rgb(43, 44, 49)` pill vs. `rgb(24, 24, 27)` cap)
/// and collapsing them would silently change dark mode. Light mode is free
/// to give them the same value, and does.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct ControlTokens {
    /// The "active"/highlight color: a lit annunciator's icon and caption,
    /// the CSP's command prompt and its prompt pills, the scale capsule's
    /// text. Chris, 2026-09-13: "The highlight color should be definable,
    /// but a mid-tone BLUE would be good" -- for light mode; dark mode keeps
    /// the app's long-standing amber.
    pub accent: ColorToken,
    /// A ribbon module capsule's vertical label pill (FILE / DRAW / DIM).
    pub label_background: ColorToken,
    pub label_foreground: ColorToken,
    /// An annunciator cap (GRID/OSNAP/ORTHO/POLAR) and the scale capsule --
    /// their unlit fill and their unlit icon/caption color.
    pub cap_background: ColorToken,
    pub cap_foreground: ColorToken,
    /// The cap's own 1px outline at rest, and while hovered.
    pub cap_border: ColorToken,
    pub cap_border_hovered: ColorToken,
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
    pub surface: SurfaceTokens,
    pub control: ControlTokens,
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
            // Matches the Command & Status Panel's long-standing hardcoded
            // frame color (`command_status_panel::panel_frame`'s prior
            // literal `rgb(30, 30, 35)`) -- picked as the shared default
            // specifically because Chris singled that panel's background
            // out as the one to match everything else to.
            // `border_color`/`border_width` are `ribbon::FRAME_STROKE`'s
            // exact previous literal and its 1.0 width, so the ribbon's
            // capsules are pixel-identical to before; it is the Command &
            // Status Panel's own lighter, opacity-modulated border that
            // moves onto this value (which is the point -- see
            // `SurfaceTokens::border_color`).
            // 2026-09-24, ribbon-pods visual-fidelity pass (`docs/design/
            // 2026-09-24_ribbon-pods-spec.md`): border color/width moved to
            // Chris's sketch spec (`#2B2C31`, ~0.031*H at H=56px -> ~1.7px)
            // -- previously `ribbon::FRAME_STROKE`'s `rgb(58,59,64)` at 1.0px.
            surface: SurfaceTokens {
                background: ColorToken::new(egui::Color32::from_rgb(30, 30, 35)),
                border_color: ColorToken::new(egui::Color32::from_rgb(0x2B, 0x2C, 0x31)),
                border_width: DimensionToken::new(1.7),
            },
            // Every value here is the dark-only literal it replaces, so dark
            // mode renders exactly as it did before these tokens existed:
            // `sdb_ui::appearance::ACCENT`'s amber, `ribbon::
            // MODULE_LABEL_BG`/`MODULE_LABEL_FG`, and
            // `command_status_panel::annunciator_response_sized`'s cap fill,
            // dim glyph color and two border colors.
            control: ControlTokens {
                accent: ColorToken::new(egui::Color32::from_rgb(255, 178, 82)),
                label_background: ColorToken::new(egui::Color32::from_rgb(0x2B, 0x2C, 0x31)),
                label_foreground: ColorToken::new(egui::Color32::from_rgb(0x7F, 0x80, 0x88)),
                cap_background: ColorToken::new(egui::Color32::from_rgb(24, 24, 27)),
                cap_foreground: ColorToken::new(egui::Color32::from_rgb(104, 104, 112)),
                cap_border: ColorToken::new(egui::Color32::from_rgb(58, 58, 64)),
                cap_border_hovered: ColorToken::new(egui::Color32::from_rgb(110, 110, 118)),
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
            // A touch darker than the window background (0xFA) so the
            // surface still reads as a distinct layer, mirroring the dark
            // theme's surface being a touch different from its own window
            // background.
            // The border is a mid grey rather than the dark theme's near-
            // black `rgb(58, 59, 64)`: a near-black hairline around every
            // capsule is what makes the light-mode chrome read as a set of
            // stickers cut out of the dark theme. Dark enough to still be a
            // real edge against the `0xF0` surface, light enough not to
            // fight the content inside it.
            surface: SurfaceTokens {
                background: ColorToken::new(egui::Color32::from_rgb(0xF0, 0xF0, 0xF2)),
                border_color: ColorToken::new(egui::Color32::from_rgb(150, 151, 158)),
                border_width: DimensionToken::new(1.0),
            },
            // Chris, 2026-09-13, on the ribbon capsule titles and the CSP's
            // assist buttons in light mode: "The button/capsule 'labels'
            // should have a very light grey background with a fairly dark
            // grey text overlay on it." The pill and the cap share one pair
            // here (unlike dark, where they differ) -- in light mode there is
            // no lamp-behind-a-lens conceit for the cap to be darker for.
            //
            // The accent is a mid-tone blue, per Chris ("a mid-tone BLUE
            // would be good"), picked for legibility rather than saturation:
            // `rgb(40, 100, 185)` clears 5:1 contrast against the light-grey
            // chip below, where a brighter blue like `rgb(70, 130, 220)`
            // lands nearer 3:1 and reads washed out on a lit caption.
            control: ControlTokens {
                accent: ColorToken::new(egui::Color32::from_rgb(40, 100, 185)),
                label_background: ColorToken::new(egui::Color32::from_rgb(226, 227, 232)),
                label_foreground: ColorToken::new(egui::Color32::from_rgb(74, 76, 84)),
                cap_background: ColorToken::new(egui::Color32::from_rgb(226, 227, 232)),
                cap_foreground: ColorToken::new(egui::Color32::from_rgb(74, 76, 84)),
                cap_border: ColorToken::new(egui::Color32::from_rgb(190, 191, 198)),
                cap_border_hovered: ColorToken::new(egui::Color32::from_rgb(120, 122, 130)),
            },
        }
    }
}

/// Whether `color` is dark enough that light-on-dark chrome (specifically
/// the annunciator's back-illuminated "lamp behind a lens" glow, see
/// [`crate::command_status_panel::AnnunciatorStyle`]) reads correctly on it.
///
/// This is what decides, from the palette alone, whether an annunciator cap
/// glows or paints flat -- rather than a separate "glow: on/off" flag that
/// could disagree with the colors around it. Chris, 2026-09-13, asked for
/// "no glow" in light mode; a soft accent bloom under a crisp stroke is a
/// bloom only against a dark cap, and on a light one it is just a smudge.
/// So the rule is stated once, here, in terms of the cap's own color: recolor
/// a palette's cap and its glow follows automatically.
///
/// Uses sRGB relative luminance (Rec. 709 coefficients on the raw 0-255
/// components, not gamma-expanded -- this is a coarse light/dark bucket, not
/// a contrast calculation, and the cheap form agrees with the expanded one
/// everywhere near the 0.5 boundary that matters here).
pub fn is_dark(color: egui::Color32) -> bool {
    let luma = 0.2126 * color.r() as f32 + 0.7152 * color.g() as f32 + 0.0722 * color.b() as f32;
    luma < 128.0
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

    /// The dark palette must keep reproducing the exact literals that were
    /// hardcoded in `ribbon.rs` and `command_status_panel.rs` before the
    /// 2026-09-13 token pass -- Chris's one hard constraint on that work was
    /// "dark mode is good as it is." A drift here is a silent dark-mode
    /// regression that no other test would catch.
    ///
    /// **2026-09-24 update:** the ribbon capsule border and label pill
    /// colors moved again, this time to Chris's Lunacy-sketch spec
    /// (`docs/design/2026-09-24_ribbon-pods-spec.md`'s proportion table --
    /// `#2B2C31` border/tab fill, `#7F8088` label text, border width
    /// ~0.031*H at H=56px), so this test's literals were updated to match
    /// rather than pinning the pre-sketch look forever.
    #[test]
    fn the_dark_palette_reproduces_the_literals_it_replaced() {
        let d = ThemePalette::dark();
        assert_eq!(
            d.surface.border_color.color32().unwrap(),
            egui::Color32::from_rgb(0x2B, 0x2C, 0x31)
        );
        assert_eq!(d.surface.border_width.px().unwrap(), 1.7);
        // `sdb_ui::appearance::ACCENT`.
        assert_eq!(
            d.control.accent.color32().unwrap(),
            egui::Color32::from_rgb(255, 178, 82)
        );
        assert_eq!(
            d.control.label_background.color32().unwrap(),
            egui::Color32::from_rgb(0x2B, 0x2C, 0x31)
        );
        assert_eq!(
            d.control.label_foreground.color32().unwrap(),
            egui::Color32::from_rgb(0x7F, 0x80, 0x88)
        );
        // `annunciator_response_sized`'s unlit cap fill, dim glyph color and
        // its two border colors.
        assert_eq!(
            d.control.cap_background.color32().unwrap(),
            egui::Color32::from_rgb(24, 24, 27)
        );
        assert_eq!(
            d.control.cap_foreground.color32().unwrap(),
            egui::Color32::from_rgb(104, 104, 112)
        );
        assert_eq!(
            d.control.cap_border.color32().unwrap(),
            egui::Color32::from_rgb(58, 58, 64)
        );
        assert_eq!(
            d.control.cap_border_hovered.color32().unwrap(),
            egui::Color32::from_rgb(110, 110, 118)
        );
    }

    /// The whole point of the light palette's control tokens: light chips
    /// with dark text, and a blue accent that is actually legible on them.
    #[test]
    fn the_light_palette_is_light_chips_with_dark_text_and_no_glow() {
        let l = ThemePalette::light();
        let cap_bg = l.control.cap_background.color32().unwrap();
        let label_bg = l.control.label_background.color32().unwrap();
        assert!(!is_dark(cap_bg), "light-mode cap must be a light chip");
        assert!(!is_dark(label_bg), "light-mode pill must be a light chip");
        // ... which is also what turns the annunciator's back-illumination
        // off, per `is_dark`'s doc comment -- Chris: "No glow either."
        assert!(is_dark(l.control.cap_foreground.color32().unwrap()));
        assert!(is_dark(l.control.label_foreground.color32().unwrap()));
        // A mid-tone blue: blue-dominant, and not so pale it disappears on
        // the chip it is painted over.
        let accent = l.control.accent.color32().unwrap();
        assert!(accent.b() > accent.r() && accent.b() > accent.g());
        assert!(is_dark(accent), "accent must stay legible on a light chip");
    }

    /// Chris's 2026-09-24 example: Interface 115% + Text 115% means text
    /// 15% larger than the already-115% interface. egui's zoom factor
    /// scales everything, so the two multiply rather than add or override.
    #[test]
    fn text_size_multiplies_on_top_of_interface_scale() {
        let ctx = egui::Context::default();
        apply_interface_scale(&ctx, InterfaceScale::new(115));
        apply_text_scale(&ctx, TextScale::new(115));
        // egui adopts a new zoom factor at the start of the next pass.
        let _ = ctx.run_ui(Default::default(), |_| {});
        let body_points = ctx.global_style().text_styles[&egui::TextStyle::Body].size;
        let default_body = egui::Style::default().text_styles[&egui::TextStyle::Body].size;
        let on_screen = body_points * ctx.zoom_factor();
        assert!((on_screen / default_body - 1.15 * 1.15).abs() < 1e-4);
        assert!(!ctx.options(|o| o.zoom_with_keyboard));
    }

    #[test]
    fn interface_scale_larger_steps_correctly() {
        assert_eq!(InterfaceScale::new(80).larger(), InterfaceScale::new(85));
        assert_eq!(InterfaceScale::new(90).larger(), InterfaceScale::new(95));
        assert_eq!(InterfaceScale::new(100).larger(), InterfaceScale::new(105));
        assert_eq!(InterfaceScale::new(110).larger(), InterfaceScale::new(115));
        assert_eq!(InterfaceScale::new(115).larger(), InterfaceScale::new(120));
        assert_eq!(InterfaceScale::new(125).larger(), InterfaceScale::new(130));
        assert_eq!(InterfaceScale::new(200).larger(), InterfaceScale::new(200));
    }

    #[test]
    fn interface_scale_smaller_steps_correctly() {
        assert_eq!(InterfaceScale::new(75).smaller(), InterfaceScale::new(75));
        assert_eq!(InterfaceScale::new(80).smaller(), InterfaceScale::new(75));
        assert_eq!(InterfaceScale::new(100).smaller(), InterfaceScale::new(95));
        assert_eq!(InterfaceScale::new(110).smaller(), InterfaceScale::new(105));
        assert_eq!(InterfaceScale::new(115).smaller(), InterfaceScale::new(110));
        assert_eq!(InterfaceScale::new(125).smaller(), InterfaceScale::new(120));
        assert_eq!(InterfaceScale::new(200).smaller(), InterfaceScale::new(195));
    }

    #[test]
    fn text_scale_larger_steps_correctly() {
        assert_eq!(TextScale::new(75).larger(), TextScale::new(80));
        assert_eq!(TextScale::new(100).larger(), TextScale::new(105));
        assert_eq!(TextScale::new(115).larger(), TextScale::new(120));
        assert_eq!(TextScale::new(175).larger(), TextScale::new(175));
    }

    #[test]
    fn text_scale_smaller_steps_correctly() {
        assert_eq!(TextScale::new(75).smaller(), TextScale::new(75));
        assert_eq!(TextScale::new(100).smaller(), TextScale::new(95));
        assert_eq!(TextScale::new(115).smaller(), TextScale::new(110));
        assert_eq!(TextScale::new(175).smaller(), TextScale::new(170));
    }

    #[test]
    fn scales_clamp_to_valid_ranges() {
        assert_eq!(InterfaceScale::new(50).0, InterfaceScale::MIN);
        assert_eq!(InterfaceScale::new(250).0, InterfaceScale::MAX);
        assert_eq!(TextScale::new(50).0, TextScale::MIN);
        assert_eq!(TextScale::new(200).0, TextScale::MAX);
    }

    #[test]
    fn interface_scale_labels_format_correctly() {
        assert_eq!(InterfaceScale::new(80).label(), "80%");
        assert_eq!(InterfaceScale::new(100).label(), "100%");
        assert_eq!(InterfaceScale::new(125).label(), "125%");
    }

    #[test]
    fn text_scale_labels_format_correctly() {
        assert_eq!(TextScale::new(90).label(), "90%");
        assert_eq!(TextScale::new(100).label(), "100%");
        assert_eq!(TextScale::new(115).label(), "115%");
    }

    #[test]
    fn is_dark_buckets_the_two_palettes_the_way_their_names_claim() {
        assert!(is_dark(
            ThemePalette::dark().surface.background.color32().unwrap()
        ));
        assert!(!is_dark(
            ThemePalette::light().surface.background.color32().unwrap()
        ));
        assert!(is_dark(egui::Color32::BLACK));
        assert!(!is_dark(egui::Color32::WHITE));
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
