//! Interface tokens (SDB TASK-384, 2026-11-26): the colours a theme file could
//! not reach before -- popup and menu surfaces, the menu bar, the control-state
//! fills and outlines `os_style` used to hard-code, the text tiers, and the
//! tool/state highlight.
//!
//! [`InterfaceTokens`] is the serialized (DTCG) form stored in
//! [`crate::theme::ThemePalette::interface`]; [`InterfaceColors`] is the
//! resolved, `Copy` form the paint path reads, so nothing parses hex strings
//! per frame. [`InterfaceColors::DARK`] is exactly the set of literals
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
//! The canvas is deliberately **not** here: its backdrop (egui `panel_fill`,
//! which these tokens never touch) and the instruments drawn on it (snap
//! marks, axes, grips, previews) are the drawing's, and decorative colour must
//! never change how the designer perceives the drawing. The host app names
//! those as *locked* canvas tokens -- built-in only, never in a theme file
//! (Chris, 2026-11-27; SDB's `canvas_tokens`).

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
///
/// Field docs are on [`InterfaceColors`].
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct InterfaceTokens {
    pub menu_bar: ColorToken,
    pub popup_background: ColorToken,
    pub popup_border: ColorToken,
    pub well: ColorToken,
    pub well_hover: ColorToken,
    pub field: ColorToken,
    pub tile: ColorToken,
    pub tile_hover: ColorToken,
    pub capsule: ColorToken,
    pub scrim: ColorToken,
    pub chip: ColorToken,
    pub chip_hover: ColorToken,
    pub chip_pressed: ColorToken,
    pub dropdown_selected: ColorToken,
    pub row_hover: ColorToken,
    pub selection: ColorToken,
    pub selection_text: ColorToken,
    pub primary: ColorToken,
    pub pill: ColorToken,
    pub pill_edge: ColorToken,
    pub outline: ColorToken,
    pub outline_hover: ColorToken,
    pub separator: ColorToken,
    pub frame: ColorToken,
    pub field_border: ColorToken,
    pub row_rule: ColorToken,
    pub text: ColorToken,
    pub text_strong: ColorToken,
    pub text_hover: ColorToken,
    pub text_emphasis: ColorToken,
    pub text_label: ColorToken,
    pub text_muted: ColorToken,
    pub text_quiet: ColorToken,
    pub text_disabled: ColorToken,
    pub scrim_text: ColorToken,
    pub drop_target: ColorToken,
    pub drop_target_border: ColorToken,
    pub drop_target_text: ColorToken,
    pub grip: ColorToken,
    pub grip_hover: ColorToken,
    pub context_menu: ColorToken,
    pub context_menu_border: ColorToken,
    pub context_hover: ColorToken,
    pub context_hover_border: ColorToken,
    pub card_shadow: ColorToken,
    pub dialog_scrim: ColorToken,
    pub dialog_shadow: ColorToken,
    pub prompt_scrim: ColorToken,
    pub divider: ColorToken,
    pub pill_wash: ColorToken,
    pub status_text: ColorToken,
    pub primary_button: ColorToken,
    pub row_selected: ColorToken,
    pub pill_active: ColorToken,
    pub recovery_text: ColorToken,
    pub recovery_fill: ColorToken,
    pub close_hover: ColorToken,
    pub close_hover_text: ColorToken,
    pub highlight: ColorToken,
    pub attention: ColorToken,
    pub confirm: ColorToken,
    pub feed_info: ColorToken,
    pub feed_success: ColorToken,
    pub feed_error: ColorToken,
    pub feed_echo: ColorToken,
    pub error_text: ColorToken,
    pub danger_text: ColorToken,
}

/// [`InterfaceTokens`] resolved to colours: what the paint path reads.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct InterfaceColors {
    // Layers.
    /// The combined title/menu bar row. Applied by the host app, which owns
    /// that row.
    pub menu_bar: egui::Color32,
    /// Menus, submenus, dropdown lists, tooltips and other popups (egui
    /// `window_fill`). One step of elevation above the surfaces, which is the
    /// point: without its own token a lighter surface made every menu look
    /// sunk. Default: egui's dark `window_fill`.
    pub popup_background: egui::Color32,
    /// The popups' 1px outline (egui `window_stroke`).
    pub popup_border: egui::Color32,
    /// Recessed wells: dropdowns, toggle tracks, egui's `extreme_bg_color`.
    pub well: egui::Color32,
    /// A hovered closed dropdown (a well brightening one step).
    pub well_hover: egui::Color32,
    /// Text-entry fields.
    pub field: egui::Color32,
    /// Preview tiles, sketch-checkbox boxes and error bubbles in the
    /// sketch-space manager windows.
    pub tile: egui::Color32,
    /// A hovered sketch checkbox box.
    pub tile_hover: egui::Color32,
    /// The idle-input capsule over the canvas (`30,30,35` at alpha 235).
    pub capsule: egui::Color32,
    /// The dimming laid over a window that waits on a child window.
    pub scrim: egui::Color32,
    // Control states.
    /// A resting button chip.
    pub chip: egui::Color32,
    /// A hovered button chip.
    pub chip_hover: egui::Color32,
    /// A pressed button chip.
    pub chip_pressed: egui::Color32,
    /// The selected row of an open dropdown list or a style list, and the
    /// chosen segment of a choice toggle.
    pub dropdown_selected: egui::Color32,
    /// A hovered list row (a faint white wash).
    pub row_hover: egui::Color32,
    /// The selected-text highlight.
    pub selection: egui::Color32,
    /// The text drawn on the selection, also a text field's focus ring.
    pub selection_text: egui::Color32,
    /// A primary (default-action) button in the sketch-space windows, and a
    /// selected preview tile's ring.
    pub primary: egui::Color32,
    /// The hover-action pill on a preview tile.
    pub pill: egui::Color32,
    /// The pill's near-black outline.
    pub pill_edge: egui::Color32,
    // Outlines.
    /// Control outlines at rest.
    pub outline: egui::Color32,
    /// Control outlines hovered or open.
    pub outline_hover: egui::Color32,
    /// Separators and the outline of non-interactive frames.
    pub separator: egui::Color32,
    /// Framed-region outlines in the sketch-space windows.
    pub frame: egui::Color32,
    /// Field and tile outlines in the sketch-space windows.
    pub field_border: egui::Color32,
    /// The faint rule between Settings rows.
    pub row_rule: egui::Color32,
    // Text.
    /// Body text.
    pub text: egui::Color32,
    /// Hovered body text, chosen choice-toggle text.
    pub text_strong: egui::Color32,
    /// Values and names in the sketch-space windows, and a hovered unchosen
    /// choice-toggle segment.
    pub text_hover: egui::Color32,
    /// Settings field labels: the panel's brightest text.
    pub text_emphasis: egui::Color32,
    /// Quiet labels (form labels, non-interactive text).
    pub text_label: egui::Color32,
    /// Muted text: section labels, headings in the sketch-space windows, a
    /// dropdown's chevron.
    pub text_muted: egui::Color32,
    /// Settings help text (the ribbon pod-label grey).
    pub text_quiet: egui::Color32,
    /// A disabled Settings row's hint.
    pub text_disabled: egui::Color32,
    /// Text on the scrim.
    pub scrim_text: egui::Color32,
    // Docking.
    /// The detached-tab ghost's fill (`120,160,220` at alpha 20).
    pub drop_target: egui::Color32,
    /// The ghost's outline (`150,180,230` at alpha 200).
    pub drop_target_border: egui::Color32,
    /// The ghost's title (`200,215,240` at alpha 230).
    pub drop_target_text: egui::Color32,
    // Command & Status Panel.
    /// The resize grip.
    pub grip: egui::Color32,
    /// The resize grip hovered or dragged.
    pub grip_hover: egui::Color32,
    // Host-app chrome (SDB's menu bar, Command & Status Panel, dialogs, cards).
    /// A right-click menu: mostly transparent so the drawing shows through
    /// (`18,18,22` at alpha 200).
    pub context_menu: egui::Color32,
    /// The right-click menu's hairline.
    pub context_menu_border: egui::Color32,
    /// A hovered right-click menu row's soft glow (`255,200,120` at alpha 28).
    pub context_hover: egui::Color32,
    /// That row's glow outline (`255,180,90` at alpha 110).
    pub context_hover_border: egui::Color32,
    /// The drop shadow under ribbon pods and docked cards.
    pub card_shadow: egui::Color32,
    /// The dimming behind an in-app dialog.
    pub dialog_scrim: egui::Color32,
    /// An in-app dialog's drop shadow.
    pub dialog_shadow: egui::Color32,
    /// The dimming behind a prompt window's card.
    pub prompt_scrim: egui::Color32,
    /// The Command & Status Panel's column divider (hovered: `grip_hover`).
    pub divider: egui::Color32,
    /// An unlit pill's faint wash (the Command & Status Panel's scale pill).
    pub pill_wash: egui::Color32,
    /// The pointer coordinates in the Command & Status Panel.
    pub status_text: egui::Color32,
    /// The one primary action of a host window (Open, Save, Print).
    pub primary_button: egui::Color32,
    /// A selected row in a host list (file windows).
    pub row_selected: egui::Color32,
    /// The active half of a two-state toggle (List / Thumbnails).
    pub pill_active: egui::Color32,
    /// The Recover badge's text in Open Recent.
    pub recovery_text: egui::Color32,
    /// The Recover badge's fill.
    pub recovery_fill: egui::Color32,
    /// The window close button hovered (Windows' own close red).
    pub close_hover: egui::Color32,
    /// The close glyph on `close_hover`.
    pub close_hover_text: egui::Color32,
    // State (Chris's decision 3 may unify these; today each keeps its own value).
    /// The tool highlight: the armed tool, lit annunciators, the command
    /// prompt. The host app's own "interface highlight" setting, when the user
    /// has picked one, overrides it.
    pub highlight: egui::Color32,
    /// Attention amber: a ribbon button's flash, the idle-input capsule while
    /// typing.
    pub attention: egui::Color32,
    /// The idle-input capsule's flash when a typed command runs.
    pub confirm: egui::Color32,
    /// Command & Status Panel feed: an ordinary line.
    pub feed_info: egui::Color32,
    /// Feed: a success line.
    pub feed_success: egui::Color32,
    /// Feed: an error line.
    pub feed_error: egui::Color32,
    /// Feed: the echo of a typed command.
    pub feed_echo: egui::Color32,
    /// An inline field error (the sketch-space error bubble; egui's
    /// `LIGHT_RED`).
    pub error_text: egui::Color32,
    /// The label of a destructive button (Delete).
    pub danger_text: egui::Color32,
}

impl InterfaceColors {
    /// The built-in Dark values: exactly the literals these tokens replaced,
    /// so the built-in Dark theme draws pixel-for-pixel as before. A `const`,
    /// so the kit's old public colour constants (`sketch::palette`) can alias
    /// it.
    pub const DARK: InterfaceColors = {
        use egui::Color32;
        InterfaceColors {
            menu_bar: Color32::from_rgb(0x20, 0x20, 0x20),
            popup_background: Color32::from_gray(27),
            popup_border: Color32::from_gray(60),
            well: Color32::from_rgb(0x14, 0x14, 0x18),
            well_hover: Color32::from_rgb(0x1A, 0x1A, 0x1F),
            field: Color32::from_rgb(0x2A, 0x2B, 0x30),
            tile: Color32::from_rgb(0x16, 0x16, 0x1A),
            tile_hover: Color32::from_rgb(0x26, 0x26, 0x2C),
            capsule: Color32::from_rgba_premultiplied(28, 28, 32, 235),
            scrim: Color32::from_black_alpha(150),
            chip: Color32::from_rgb(0x2E, 0x2F, 0x35),
            chip_hover: Color32::from_rgb(0x38, 0x39, 0x40),
            chip_pressed: Color32::from_rgb(0x26, 0x27, 0x2C),
            dropdown_selected: Color32::from_rgb(0x3A, 0x3B, 0x42),
            row_hover: Color32::from_rgba_premultiplied(12, 12, 12, 12),
            selection: Color32::from_rgb(0x44, 0x46, 0x4F),
            selection_text: Color32::from_rgb(0xF0, 0xF0, 0xF2),
            primary: Color32::from_rgb(0x55, 0x7E, 0xAF),
            pill: Color32::from_rgb(0x6B, 0x6B, 0x6B),
            pill_edge: Color32::from_rgb(0x05, 0x05, 0x05),
            outline: Color32::from_rgba_premultiplied(20, 20, 20, 20),
            outline_hover: Color32::from_rgba_premultiplied(36, 36, 36, 36),
            separator: Color32::from_rgba_premultiplied(18, 18, 18, 18),
            frame: Color32::from_rgb(0x3C, 0x3C, 0x3D),
            field_border: Color32::from_rgb(0x4A, 0x4B, 0x52),
            row_rule: Color32::from_rgb(0x26, 0x27, 0x2C),
            text: Color32::from_rgb(0xDC, 0xDC, 0xE0),
            text_strong: Color32::from_rgb(0xF0, 0xF0, 0xF2),
            text_hover: Color32::from_rgb(0xE8, 0xE8, 0xEC),
            text_emphasis: Color32::WHITE,
            text_label: Color32::from_rgb(0xB4, 0xB5, 0xBC),
            text_muted: Color32::from_rgb(0x8A, 0x8B, 0x93),
            text_quiet: Color32::from_rgb(0x7F, 0x80, 0x88),
            text_disabled: Color32::from_rgb(0x5A, 0x5B, 0x60),
            scrim_text: Color32::from_rgba_premultiplied(230, 230, 230, 230),
            drop_target: Color32::from_rgba_premultiplied(9, 13, 17, 20),
            drop_target_border: Color32::from_rgba_premultiplied(118, 141, 180, 200),
            drop_target_text: Color32::from_rgba_premultiplied(180, 194, 216, 230),
            grip: Color32::from_rgb(105, 105, 115),
            grip_hover: Color32::from_rgb(180, 180, 190),
            context_menu: Color32::from_rgba_premultiplied(14, 14, 17, 200),
            context_menu_border: Color32::from_rgba_premultiplied(60, 60, 60, 60),
            context_hover: Color32::from_rgba_premultiplied(28, 22, 13, 28),
            context_hover_border: Color32::from_rgba_premultiplied(110, 78, 39, 110),
            card_shadow: Color32::from_black_alpha(77),
            dialog_scrim: Color32::from_black_alpha(140),
            dialog_shadow: Color32::from_black_alpha(180),
            prompt_scrim: Color32::from_black_alpha(90),
            divider: Color32::from_rgb(78, 78, 86),
            pill_wash: Color32::from_rgba_premultiplied(20, 20, 20, 20),
            status_text: Color32::from_rgb(150, 150, 158),
            primary_button: Color32::from_rgb(69, 128, 168),
            row_selected: Color32::from_rgb(52, 74, 96),
            pill_active: Color32::from_rgb(61, 103, 133),
            recovery_text: Color32::from_rgb(115, 205, 145),
            recovery_fill: Color32::from_rgb(30, 76, 48),
            close_hover: Color32::from_rgb(0xC4, 0x2B, 0x1C),
            close_hover_text: Color32::WHITE,
            highlight: Color32::from_rgb(104, 222, 158),
            attention: Color32::from_rgb(255, 178, 82),
            confirm: Color32::from_rgb(120, 230, 150),
            feed_info: Color32::from_rgb(205, 205, 210),
            feed_success: Color32::from_rgb(150, 210, 150),
            feed_error: Color32::from_rgb(235, 130, 118),
            feed_echo: Color32::from_rgb(150, 175, 225),
            error_text: Color32::from_rgb(255, 128, 128),
            danger_text: Color32::from_rgb(0xE0, 0x70, 0x5F),
        }
    };

    /// [`Self::DARK`].
    pub fn dark_default() -> Self {
        Self::DARK
    }
}

impl Default for InterfaceTokens {
    fn default() -> Self {
        Self::dark()
    }
}

impl InterfaceTokens {
    /// The built-in Dark theme's interface tokens ([`InterfaceColors::DARK`]).
    pub fn dark() -> Self {
        Self::from_colors(&InterfaceColors::DARK)
    }

    pub fn from_colors(c: &InterfaceColors) -> Self {
        let t = ColorToken::new;
        Self {
            menu_bar: t(c.menu_bar),
            popup_background: t(c.popup_background),
            popup_border: t(c.popup_border),
            well: t(c.well),
            well_hover: t(c.well_hover),
            field: t(c.field),
            tile: t(c.tile),
            tile_hover: t(c.tile_hover),
            capsule: t(c.capsule),
            scrim: t(c.scrim),
            chip: t(c.chip),
            chip_hover: t(c.chip_hover),
            chip_pressed: t(c.chip_pressed),
            dropdown_selected: t(c.dropdown_selected),
            row_hover: t(c.row_hover),
            selection: t(c.selection),
            selection_text: t(c.selection_text),
            primary: t(c.primary),
            pill: t(c.pill),
            pill_edge: t(c.pill_edge),
            outline: t(c.outline),
            outline_hover: t(c.outline_hover),
            separator: t(c.separator),
            frame: t(c.frame),
            field_border: t(c.field_border),
            row_rule: t(c.row_rule),
            text: t(c.text),
            text_strong: t(c.text_strong),
            text_hover: t(c.text_hover),
            text_emphasis: t(c.text_emphasis),
            text_label: t(c.text_label),
            text_muted: t(c.text_muted),
            text_quiet: t(c.text_quiet),
            text_disabled: t(c.text_disabled),
            scrim_text: t(c.scrim_text),
            drop_target: t(c.drop_target),
            drop_target_border: t(c.drop_target_border),
            drop_target_text: t(c.drop_target_text),
            grip: t(c.grip),
            grip_hover: t(c.grip_hover),
            context_menu: t(c.context_menu),
            context_menu_border: t(c.context_menu_border),
            context_hover: t(c.context_hover),
            context_hover_border: t(c.context_hover_border),
            card_shadow: t(c.card_shadow),
            dialog_scrim: t(c.dialog_scrim),
            dialog_shadow: t(c.dialog_shadow),
            prompt_scrim: t(c.prompt_scrim),
            divider: t(c.divider),
            pill_wash: t(c.pill_wash),
            status_text: t(c.status_text),
            primary_button: t(c.primary_button),
            row_selected: t(c.row_selected),
            pill_active: t(c.pill_active),
            recovery_text: t(c.recovery_text),
            recovery_fill: t(c.recovery_fill),
            close_hover: t(c.close_hover),
            close_hover_text: t(c.close_hover_text),
            highlight: t(c.highlight),
            attention: t(c.attention),
            confirm: t(c.confirm),
            feed_info: t(c.feed_info),
            feed_success: t(c.feed_success),
            feed_error: t(c.feed_error),
            feed_echo: t(c.feed_echo),
            error_text: t(c.error_text),
            danger_text: t(c.danger_text),
        }
    }

    /// Resolves every token; a value that fails to parse falls back to the
    /// dark default for that one field rather than failing the whole theme.
    pub fn resolve(&self) -> InterfaceColors {
        let d = InterfaceColors::DARK;
        let c = |t: &ColorToken, fallback: egui::Color32| t.color32().unwrap_or(fallback);
        InterfaceColors {
            menu_bar: c(&self.menu_bar, d.menu_bar),
            popup_background: c(&self.popup_background, d.popup_background),
            popup_border: c(&self.popup_border, d.popup_border),
            well: c(&self.well, d.well),
            well_hover: c(&self.well_hover, d.well_hover),
            field: c(&self.field, d.field),
            tile: c(&self.tile, d.tile),
            tile_hover: c(&self.tile_hover, d.tile_hover),
            capsule: c(&self.capsule, d.capsule),
            scrim: c(&self.scrim, d.scrim),
            chip: c(&self.chip, d.chip),
            chip_hover: c(&self.chip_hover, d.chip_hover),
            chip_pressed: c(&self.chip_pressed, d.chip_pressed),
            dropdown_selected: c(&self.dropdown_selected, d.dropdown_selected),
            row_hover: c(&self.row_hover, d.row_hover),
            selection: c(&self.selection, d.selection),
            selection_text: c(&self.selection_text, d.selection_text),
            primary: c(&self.primary, d.primary),
            pill: c(&self.pill, d.pill),
            pill_edge: c(&self.pill_edge, d.pill_edge),
            outline: c(&self.outline, d.outline),
            outline_hover: c(&self.outline_hover, d.outline_hover),
            separator: c(&self.separator, d.separator),
            frame: c(&self.frame, d.frame),
            field_border: c(&self.field_border, d.field_border),
            row_rule: c(&self.row_rule, d.row_rule),
            text: c(&self.text, d.text),
            text_strong: c(&self.text_strong, d.text_strong),
            text_hover: c(&self.text_hover, d.text_hover),
            text_emphasis: c(&self.text_emphasis, d.text_emphasis),
            text_label: c(&self.text_label, d.text_label),
            text_muted: c(&self.text_muted, d.text_muted),
            text_quiet: c(&self.text_quiet, d.text_quiet),
            text_disabled: c(&self.text_disabled, d.text_disabled),
            scrim_text: c(&self.scrim_text, d.scrim_text),
            drop_target: c(&self.drop_target, d.drop_target),
            drop_target_border: c(&self.drop_target_border, d.drop_target_border),
            drop_target_text: c(&self.drop_target_text, d.drop_target_text),
            grip: c(&self.grip, d.grip),
            grip_hover: c(&self.grip_hover, d.grip_hover),
            context_menu: c(&self.context_menu, d.context_menu),
            context_menu_border: c(&self.context_menu_border, d.context_menu_border),
            context_hover: c(&self.context_hover, d.context_hover),
            context_hover_border: c(&self.context_hover_border, d.context_hover_border),
            card_shadow: c(&self.card_shadow, d.card_shadow),
            dialog_scrim: c(&self.dialog_scrim, d.dialog_scrim),
            dialog_shadow: c(&self.dialog_shadow, d.dialog_shadow),
            prompt_scrim: c(&self.prompt_scrim, d.prompt_scrim),
            divider: c(&self.divider, d.divider),
            pill_wash: c(&self.pill_wash, d.pill_wash),
            status_text: c(&self.status_text, d.status_text),
            primary_button: c(&self.primary_button, d.primary_button),
            row_selected: c(&self.row_selected, d.row_selected),
            pill_active: c(&self.pill_active, d.pill_active),
            recovery_text: c(&self.recovery_text, d.recovery_text),
            recovery_fill: c(&self.recovery_fill, d.recovery_fill),
            close_hover: c(&self.close_hover, d.close_hover),
            close_hover_text: c(&self.close_hover_text, d.close_hover_text),
            highlight: c(&self.highlight, d.highlight),
            attention: c(&self.attention, d.attention),
            confirm: c(&self.confirm, d.confirm),
            feed_info: c(&self.feed_info, d.feed_info),
            feed_success: c(&self.feed_success, d.feed_success),
            feed_error: c(&self.feed_error, d.feed_error),
            feed_echo: c(&self.feed_echo, d.feed_echo),
            error_text: c(&self.error_text, d.error_text),
            danger_text: c(&self.danger_text, d.danger_text),
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
    published_interface_colors(ctx).unwrap_or(InterfaceColors::DARK)
}

/// The interface colours published with [`set_interface_colors`], if any.
pub fn published_interface_colors(ctx: &egui::Context) -> Option<InterfaceColors> {
    ctx.data(|d| d.get_temp::<InterfaceColors>(store_id()))
}

/// The interface colours a widget drawn in `ui` should use, or `None` for the
/// platform look: the published colours (any theme with an `interface`
/// section, dark or light), else the built-in Dark ones on a dark `Ui`, else
/// `None` -- the built-in Light theme, whose widgets keep their own light
/// values. Widgets that have a separate light look ask this rather than
/// checking `dark_mode`, so a light theme file can restyle them too.
pub fn theme_colors(ui: &egui::Ui) -> Option<InterfaceColors> {
    published_interface_colors(ui.ctx())
        .or_else(|| ui.visuals().dark_mode.then_some(InterfaceColors::DARK))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `DARK` is written with `const` constructors; these are the
    /// (non-`const`) constructions the literals it replaced used.
    #[test]
    fn dark_matches_the_literals_it_replaced() {
        use egui::Color32;
        let d = InterfaceColors::DARK;
        let v = egui::Visuals::dark();
        assert_eq!(d.popup_background, v.window_fill);
        assert_eq!(d.popup_border, v.window_stroke.color);
        assert_eq!(d.outline, Color32::from_white_alpha(20));
        assert_eq!(d.outline_hover, Color32::from_white_alpha(36));
        assert_eq!(d.separator, Color32::from_white_alpha(18));
        assert_eq!(d.row_hover, Color32::from_white_alpha(12));
        assert_eq!(d.scrim_text, Color32::from_white_alpha(230));
        assert_eq!(d.capsule, Color32::from_rgba_unmultiplied(30, 30, 35, 235));
        assert_eq!(
            d.drop_target,
            Color32::from_rgba_unmultiplied(120, 160, 220, 20)
        );
        assert_eq!(
            d.drop_target_border,
            Color32::from_rgba_unmultiplied(150, 180, 230, 200)
        );
        assert_eq!(
            d.drop_target_text,
            Color32::from_rgba_unmultiplied(200, 215, 240, 230)
        );
        assert_eq!(d.error_text, Color32::LIGHT_RED);
        assert_eq!(
            d.context_menu,
            Color32::from_rgba_unmultiplied(18, 18, 22, 200)
        );
        assert_eq!(
            d.context_menu_border,
            Color32::from_rgba_unmultiplied(255, 255, 255, 60)
        );
        assert_eq!(
            d.context_hover,
            Color32::from_rgba_unmultiplied(255, 200, 120, 28)
        );
        assert_eq!(
            d.context_hover_border,
            Color32::from_rgba_unmultiplied(255, 180, 90, 110)
        );
        assert_eq!(
            d.pill_wash,
            Color32::from_rgba_unmultiplied(255, 255, 255, 20)
        );
    }

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
