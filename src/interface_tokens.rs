//! Interface tokens (SDB TASK-384, 2026-11-26): the colours a theme file could
//! not reach before -- popup and menu surfaces, the menu bar, the control-state
//! fills and outlines `os_style` used to hard-code, the text tiers, and the
//! tool/state highlight.
//!
//! [`InterfaceTokens`] is the serialized (DTCG) form stored in
//! [`crate::theme::ThemePalette::interface`]; [`InterfaceColors`] is the
//! resolved, `Copy` form the paint path reads, so nothing parses hex strings
//! per frame. [`InterfaceColors::dark_default`] is exactly the set of literals
//! these tokens replaced, so the built-in Dark theme draws pixel-for-pixel as
//! before.
//!
//! **Why `Option` on the palette, and only for Dark:** the light theme's
//! control look is egui's own light `Visuals` plus the OS accent (its
//! selection colour *is* the accent), which a fixed token cannot reproduce.
//! `None` means "the platform look, untouched", which is what Light gets, and
//! what a theme file saved before these tokens existed gets for a Light base.
//! A Dark-based file saved before them gets [`InterfaceTokens::dark`], i.e.
//! today's look (see `ThemePalette`'s deserializer).
//!
//! The canvas backdrop (egui `panel_fill`) is deliberately **not** a token: it
//! is the drawing's background, and decorative colour must never change how
//! the designer perceives the drawing.

use crate::tokens::ColorToken;
use serde::{Deserialize, Serialize};

/// The serialized interface tokens. JSON keys are by name.
///
/// **Every field defaults, per field, to the built-in Dark value**
/// (`#[serde(default)]` on the struct, [`Default`] = [`InterfaceTokens::dark`]).
/// That is what lets this set grow: a file saved before a token existed
/// loads with that one token at today's value instead of failing, and a
/// hand-written theme may set only the tokens it changes. (The other
/// `ThemePalette` sections stay all-or-nothing; see `ThemePalette`.)
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct InterfaceTokens {
    /// The combined title/menu bar row. Applied by the host app, which owns
    /// that row.
    pub menu_bar: ColorToken,
    /// Menus, submenus, dropdown lists, tooltips and other popups (egui
    /// `window_fill`), and their 1px outline (`window_stroke`). One step of
    /// elevation above the surfaces, which is the point: without its own
    /// token a lighter surface made every menu look sunk.
    pub popup_background: ColorToken,
    pub popup_border: ColorToken,
    /// A resting button chip, hovered, and pressed.
    pub chip: ColorToken,
    pub chip_hover: ColorToken,
    pub chip_pressed: ColorToken,
    /// Recessed wells: dropdowns, toggle tracks, egui's `extreme_bg_color`.
    pub well: ColorToken,
    /// A hovered closed dropdown (a well brightening one step).
    pub well_hover: ColorToken,
    /// Text-entry fields.
    pub field: ColorToken,
    /// Control outlines at rest and while hovered/open.
    pub outline: ColorToken,
    pub outline_hover: ColorToken,
    /// Separators and the outline of non-interactive frames.
    pub separator: ColorToken,
    /// The selected-text / selected-row highlight, and the colour drawn on it
    /// (also the focus ring of a text field).
    pub selection: ColorToken,
    pub selection_text: ColorToken,
    /// Text tiers: body text, hovered body text, quiet labels, and muted
    /// glyphs (a dropdown's chevron).
    pub text: ColorToken,
    pub text_strong: ColorToken,
    pub text_label: ColorToken,
    pub text_muted: ColorToken,
    /// The selected row of an open dropdown list, and the chosen segment of
    /// a choice toggle.
    pub dropdown_selected: ColorToken,
    /// The ink of a hovered, unchosen choice-toggle segment.
    pub text_hover: ColorToken,
    /// The tool/state highlight: the armed tool, lit annunciators, the
    /// command prompt. The host app's own "interface highlight" setting, when
    /// the user has picked one, overrides it.
    pub highlight: ColorToken,
}

/// [`InterfaceTokens`] resolved to colours.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct InterfaceColors {
    pub menu_bar: egui::Color32,
    pub popup_background: egui::Color32,
    pub popup_border: egui::Color32,
    pub chip: egui::Color32,
    pub chip_hover: egui::Color32,
    pub chip_pressed: egui::Color32,
    pub well: egui::Color32,
    pub well_hover: egui::Color32,
    pub field: egui::Color32,
    pub outline: egui::Color32,
    pub outline_hover: egui::Color32,
    pub separator: egui::Color32,
    pub selection: egui::Color32,
    pub selection_text: egui::Color32,
    pub text: egui::Color32,
    pub text_strong: egui::Color32,
    pub text_label: egui::Color32,
    pub text_muted: egui::Color32,
    pub dropdown_selected: egui::Color32,
    pub text_hover: egui::Color32,
    pub highlight: egui::Color32,
}

impl InterfaceColors {
    /// The dark-theme values these tokens replaced, unchanged: the
    /// 2026-11-09 panel-theming greys from `os_style`, egui's own dark popup
    /// fill and outline, Windows 11's unaccented dark caption grey, and the
    /// mint tool highlight.
    pub fn dark_default() -> Self {
        use egui::Color32;
        let egui_dark = egui::Visuals::dark();
        Self {
            menu_bar: Color32::from_rgb(0x20, 0x20, 0x20),
            popup_background: egui_dark.window_fill,
            popup_border: egui_dark.window_stroke.color,
            chip: Color32::from_rgb(0x2E, 0x2F, 0x35),
            chip_hover: Color32::from_rgb(0x38, 0x39, 0x40),
            chip_pressed: Color32::from_rgb(0x26, 0x27, 0x2C),
            well: Color32::from_rgb(0x14, 0x14, 0x18),
            well_hover: Color32::from_rgb(0x1A, 0x1A, 0x1F),
            field: Color32::from_rgb(0x2A, 0x2B, 0x30),
            outline: Color32::from_white_alpha(20),
            outline_hover: Color32::from_white_alpha(36),
            separator: Color32::from_white_alpha(18),
            selection: Color32::from_rgb(0x44, 0x46, 0x4F),
            selection_text: Color32::from_rgb(0xF0, 0xF0, 0xF2),
            text: Color32::from_rgb(0xDC, 0xDC, 0xE0),
            text_strong: Color32::from_rgb(0xF0, 0xF0, 0xF2),
            text_label: Color32::from_rgb(0xB4, 0xB5, 0xBC),
            text_muted: Color32::from_rgb(0x8A, 0x8B, 0x93),
            dropdown_selected: Color32::from_rgb(0x3A, 0x3B, 0x42),
            text_hover: Color32::from_rgb(0xE8, 0xE8, 0xEC),
            highlight: Color32::from_rgb(104, 222, 158),
        }
    }
}

impl Default for InterfaceTokens {
    fn default() -> Self {
        Self::dark()
    }
}

impl InterfaceTokens {
    /// The built-in Dark theme's interface tokens ([`InterfaceColors::dark_default`]).
    pub fn dark() -> Self {
        Self::from_colors(&InterfaceColors::dark_default())
    }

    pub fn from_colors(c: &InterfaceColors) -> Self {
        let t = ColorToken::new;
        Self {
            menu_bar: t(c.menu_bar),
            popup_background: t(c.popup_background),
            popup_border: t(c.popup_border),
            chip: t(c.chip),
            chip_hover: t(c.chip_hover),
            chip_pressed: t(c.chip_pressed),
            well: t(c.well),
            well_hover: t(c.well_hover),
            field: t(c.field),
            outline: t(c.outline),
            outline_hover: t(c.outline_hover),
            separator: t(c.separator),
            selection: t(c.selection),
            selection_text: t(c.selection_text),
            text: t(c.text),
            text_strong: t(c.text_strong),
            text_label: t(c.text_label),
            text_muted: t(c.text_muted),
            dropdown_selected: t(c.dropdown_selected),
            text_hover: t(c.text_hover),
            highlight: t(c.highlight),
        }
    }

    /// Resolves every token; a value that fails to parse falls back to the
    /// dark default for that one field rather than failing the whole theme.
    pub fn resolve(&self) -> InterfaceColors {
        let d = InterfaceColors::dark_default();
        let c = |t: &ColorToken, fallback: egui::Color32| t.color32().unwrap_or(fallback);
        InterfaceColors {
            menu_bar: c(&self.menu_bar, d.menu_bar),
            popup_background: c(&self.popup_background, d.popup_background),
            popup_border: c(&self.popup_border, d.popup_border),
            chip: c(&self.chip, d.chip),
            chip_hover: c(&self.chip_hover, d.chip_hover),
            chip_pressed: c(&self.chip_pressed, d.chip_pressed),
            well: c(&self.well, d.well),
            well_hover: c(&self.well_hover, d.well_hover),
            field: c(&self.field, d.field),
            outline: c(&self.outline, d.outline),
            outline_hover: c(&self.outline_hover, d.outline_hover),
            separator: c(&self.separator, d.separator),
            selection: c(&self.selection, d.selection),
            selection_text: c(&self.selection_text, d.selection_text),
            text: c(&self.text, d.text),
            text_strong: c(&self.text_strong, d.text_strong),
            text_label: c(&self.text_label, d.text_label),
            text_muted: c(&self.text_muted, d.text_muted),
            dropdown_selected: c(&self.dropdown_selected, d.dropdown_selected),
            text_hover: c(&self.text_hover, d.text_hover),
            highlight: c(&self.highlight, d.highlight),
        }
    }
}

fn store_id() -> egui::Id {
    egui::Id::new("spatial_ui_kit::interface_colors")
}

/// Publishes the interface colours the paint path should use (for helpers
/// such as [`crate::os_style::dropdown_well_scope`] that style a scope rather
/// than the whole context). `None` clears it, which reads back as the dark
/// default.
pub fn set_interface_colors(ctx: &egui::Context, colors: Option<InterfaceColors>) {
    ctx.data_mut(|d| {
        if let Some(c) = colors {
            d.insert_temp(store_id(), c);
        } else {
            d.remove::<InterfaceColors>(store_id());
        }
    });
}

/// The interface colours published with [`set_interface_colors`], or the dark
/// default.
pub fn interface_colors(ctx: &egui::Context) -> InterfaceColors {
    ctx.data(|d| d.get_temp::<InterfaceColors>(store_id()))
        .unwrap_or_else(InterfaceColors::dark_default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_tokens_resolve_to_the_dark_default() {
        assert_eq!(
            InterfaceTokens::dark().resolve(),
            InterfaceColors::dark_default()
        );
    }

    /// A section that sets only some tokens (a hand-written theme, or a file
    /// saved before later tokens were added) loads with the rest at the
    /// built-in Dark values.
    #[test]
    fn a_partial_interface_section_fills_in_the_dark_defaults() {
        let mut value = serde_json::to_value(InterfaceTokens::dark()).unwrap();
        let map = value.as_object_mut().unwrap();
        map.retain(|k, _| k == "chip");
        map.insert(
            "chip".into(),
            serde_json::json!({"$type": "color", "$value": "#010203"}),
        );
        let loaded: InterfaceTokens = serde_json::from_value(value).unwrap();
        let mut expected = InterfaceColors::dark_default();
        expected.chip = egui::Color32::from_rgb(1, 2, 3);
        assert_eq!(loaded.resolve(), expected);
    }

    #[test]
    fn interface_tokens_round_trip_through_json() {
        let t = InterfaceTokens::dark();
        let back: InterfaceTokens =
            serde_json::from_str(&serde_json::to_string(&t).unwrap()).unwrap();
        assert_eq!(t, back);
    }
}
