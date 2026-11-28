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
    pub well: ColorToken,
    pub well_hover: ColorToken,
    pub field: ColorToken,
    pub capsule: ColorToken,
    pub scrim: ColorToken,
    pub prompt_scrim: ColorToken,
    pub card_shadow: ColorToken,
    pub dialog_shadow: ColorToken,
    pub chip: ColorToken,
    pub chip_hover: ColorToken,
    pub chip_pressed: ColorToken,
    pub row_hover: ColorToken,
    pub selection: ColorToken,
    pub primary: ColorToken,
    pub row_selected: ColorToken,
    pub pill_active: ColorToken,
    pub switch_on: ColorToken,
    pub switch_knob: ColorToken,
    pub pill: ColorToken,
    pub pill_edge: ColorToken,
    pub outline: ColorToken,
    pub outline_hover: ColorToken,
    pub frame: ColorToken,
    pub field_border: ColorToken,
    pub grip: ColorToken,
    pub text: ColorToken,
    pub text_strong: ColorToken,
    pub text_label: ColorToken,
    pub text_muted: ColorToken,
    pub text_disabled: ColorToken,
    pub scrim_text: ColorToken,
    pub drop_target: ColorToken,
    pub drop_target_border: ColorToken,
    pub drop_target_text: ColorToken,
    pub context_menu: ColorToken,
    pub context_menu_border: ColorToken,
    pub context_hover: ColorToken,
    pub context_hover_border: ColorToken,
    pub close_hover: ColorToken,
    pub highlight: ColorToken,
    pub attention: ColorToken,
    pub recovery_fill: ColorToken,
    pub feed_echo: ColorToken,
    pub error: ColorToken,
}

/// [`InterfaceTokens`] resolved to colours: what the paint path reads.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct InterfaceColors {
    // Layers.
    /// The combined title/menu bar row. Applied by the host app, which owns
    /// that row.
    pub menu_bar: egui::Color32,
    /// Menus, submenus, dropdown lists, tooltips and other popups (egui
    /// `window_fill`). One step of elevation above the surfaces.
    pub popup_background: egui::Color32,
    /// Recessed wells: dropdowns, toggle tracks, preview tiles, sketch
    /// checkboxes, egui's `extreme_bg_color`.
    pub well: egui::Color32,
    /// A hovered closed dropdown (a well brightening one step).
    pub well_hover: egui::Color32,
    /// Text-entry fields.
    pub field: egui::Color32,
    /// The idle-input capsule over the canvas (`30,30,35` at alpha 235).
    pub capsule: egui::Color32,
    /// The dimming behind a dialog, or over a window waiting on a child window.
    pub scrim: egui::Color32,
    /// The lighter dimming behind a prompt window's card.
    pub prompt_scrim: egui::Color32,
    /// The drop shadow under ribbon pods and docked cards.
    pub card_shadow: egui::Color32,
    /// An in-app dialog's drop shadow.
    pub dialog_shadow: egui::Color32,
    // Control states.
    /// A resting button chip.
    pub chip: egui::Color32,
    /// A hovered chip, and a selected row in a dropdown or style list.
    pub chip_hover: egui::Color32,
    /// A pressed chip, a hovered sketch checkbox, and the faint rule
    /// between Settings rows.
    pub chip_pressed: egui::Color32,
    /// A hovered list row (a faint white wash).
    pub row_hover: egui::Color32,
    /// The selected-text highlight (its text is `text_strong`).
    pub selection: egui::Color32,
    /// A window's one primary action (Open, Save, Print, Apply), and a
    /// selected preview tile's ring.
    pub primary: egui::Color32,
    /// A selected row in a host list (file windows).
    pub row_selected: egui::Color32,
    /// The active half of a two-state toggle (List / Thumbnails).
    pub pill_active: egui::Color32,
    /// The track of an on/off switch that is on (the approved Properties
    /// mock-up's bright blue).
    pub switch_on: egui::Color32,
    /// The knob of an on/off switch (white in the approved mock-up).
    pub switch_knob: egui::Color32,
    /// The hover-action pill on a preview tile.
    pub pill: egui::Color32,
    /// The pill's near-black outline.
    pub pill_edge: egui::Color32,
    // Outlines.
    /// Control outlines at rest, separators, and an unlit pill's wash.
    pub outline: egui::Color32,
    /// Control outlines hovered or open.
    pub outline_hover: egui::Color32,
    /// Popup outlines (egui `window_stroke`) and framed regions.
    pub frame: egui::Color32,
    /// Field and tile outlines, and the Command & Status Panel's divider.
    pub field_border: egui::Color32,
    /// A resize grip (hovered: `text_label`).
    pub grip: egui::Color32,
    // Text.
    /// Body text, and ordinary feed lines.
    pub text: egui::Color32,
    /// Strong text: hovered text, values, headings, field labels, text on the
    /// selection and the close glyph.
    pub text_strong: egui::Color32,
    /// Quiet labels (form labels, non-interactive text), a hovered grip.
    pub text_label: egui::Color32,
    /// Muted text: section labels, help text, pod labels' tier, a dropdown's
    /// chevron, the pointer coordinates.
    pub text_muted: egui::Color32,
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
    // Right-click menu.
    /// A right-click menu: mostly transparent so the drawing shows through
    /// (`18,18,22` at alpha 200).
    pub context_menu: egui::Color32,
    /// The right-click menu's hairline.
    pub context_menu_border: egui::Color32,
    /// A hovered right-click menu row's soft glow (`255,200,120` at alpha 28).
    pub context_hover: egui::Color32,
    /// That row's glow outline (`255,180,90` at alpha 110).
    pub context_hover_border: egui::Color32,
    /// The window close button hovered (Windows' own close red).
    pub close_hover: egui::Color32,
    // State (Chris 2026-11-27: one mint for done/on, one amber for attention).
    /// Done / on: the armed tool, lit annunciators, the command prompt, success
    /// lines, the idle-input flash, the Recover badge. The host app's own
    /// "interface highlight" setting, when the user has picked one, overrides it.
    pub highlight: egui::Color32,
    /// Attention: a ribbon button's flash, the idle-input capsule while typing,
    /// warnings.
    pub attention: egui::Color32,
    /// The Recover badge's fill.
    pub recovery_fill: egui::Color32,
    /// The echo of a typed command in the Command & Status Panel feed.
    pub feed_echo: egui::Color32,
    /// Errors: error feed lines (marked ✕ as well, so they never rely on
    /// colour), inline field errors, a destructive button's label. The hue is
    /// still to be re-picked away from red (Chris).
    pub error: egui::Color32,

    // Merged-away names (slice 7, 2026-11-27). Not theme tokens and never in
    // a file: each always holds the colour of the token it was merged into
    // ([`InterfaceColors::with_aliases`]). They exist only so code written
    // against the 67-token set keeps compiling while hosts move to the merged
    // names; they will be removed once no caller uses them.
    /// Alias of `text_muted` (merged 2026-11-27); kept so older callers build.
    pub text_quiet: egui::Color32,
    /// Alias of `text_strong` (merged 2026-11-27); kept so older callers build.
    pub text_hover: egui::Color32,
    /// Alias of `outline` (merged 2026-11-27); kept so older callers build.
    pub separator: egui::Color32,
    /// Alias of `text_strong` (merged 2026-11-27); kept so older callers build.
    pub selection_text: egui::Color32,
    /// Alias of `text_strong` (merged 2026-11-27); kept so older callers build.
    pub close_hover_text: egui::Color32,
    /// Alias of `outline` (merged 2026-11-27); kept so older callers build.
    pub pill_wash: egui::Color32,
    /// Alias of `well` (merged 2026-11-27); kept so older callers build.
    pub tile: egui::Color32,
    /// Alias of `chip_pressed` (merged 2026-11-27); kept so older callers build.
    pub tile_hover: egui::Color32,
    /// Alias of `frame` (merged 2026-11-27); kept so older callers build.
    pub popup_border: egui::Color32,
    /// Alias of `chip_hover` (merged 2026-11-27); kept so older callers build.
    pub dropdown_selected: egui::Color32,
    /// Alias of `chip_pressed` (merged 2026-11-27); kept so older callers build.
    pub row_rule: egui::Color32,
    /// Alias of `text_muted` (merged 2026-11-27); kept so older callers build.
    pub status_text: egui::Color32,
    /// Alias of `text_label` (merged 2026-11-27); kept so older callers build.
    pub grip_hover: egui::Color32,
    /// Alias of `field_border` (merged 2026-11-27); kept so older callers build.
    pub divider: egui::Color32,
    /// Alias of `scrim` (merged 2026-11-27); kept so older callers build.
    pub dialog_scrim: egui::Color32,
    /// Alias of `highlight` (merged 2026-11-27); kept so older callers build.
    pub confirm: egui::Color32,
    /// Alias of `highlight` (merged 2026-11-27); kept so older callers build.
    pub feed_success: egui::Color32,
    /// Alias of `highlight` (merged 2026-11-27); kept so older callers build.
    pub recovery_text: egui::Color32,
    /// Alias of `text` (merged 2026-11-27); kept so older callers build.
    pub feed_info: egui::Color32,
    /// Alias of `error` (merged 2026-11-27); kept so older callers build.
    pub error_text: egui::Color32,
    /// Alias of `error` (merged 2026-11-27); kept so older callers build.
    pub danger_text: egui::Color32,
    /// Alias of `error` (merged 2026-11-27); kept so older callers build.
    pub feed_error: egui::Color32,
    /// Alias of `text_strong` (merged 2026-11-27); kept so older callers build.
    pub text_emphasis: egui::Color32,
    /// Alias of `primary` (merged 2026-11-27); kept so older callers build.
    pub primary_button: egui::Color32,
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
            well: Color32::from_rgb(0x14, 0x14, 0x18),
            well_hover: Color32::from_rgb(0x1A, 0x1A, 0x1F),
            field: Color32::from_rgb(0x2A, 0x2B, 0x30),
            capsule: Color32::from_rgba_premultiplied(28, 28, 32, 235),
            scrim: Color32::from_black_alpha(150),
            prompt_scrim: Color32::from_black_alpha(90),
            card_shadow: Color32::from_black_alpha(77),
            dialog_shadow: Color32::from_black_alpha(180),
            chip: Color32::from_rgb(0x2E, 0x2F, 0x35),
            chip_hover: Color32::from_rgb(0x38, 0x39, 0x40),
            chip_pressed: Color32::from_rgb(0x26, 0x27, 0x2C),
            row_hover: Color32::from_rgba_premultiplied(12, 12, 12, 12),
            selection: Color32::from_rgb(0x44, 0x46, 0x4F),
            primary: Color32::from_rgb(0x55, 0x7E, 0xAF),
            row_selected: Color32::from_rgb(52, 74, 96),
            pill_active: Color32::from_rgb(61, 103, 133),
            switch_on: Color32::from_rgb(96, 204, 254),
            switch_knob: Color32::WHITE,
            pill: Color32::from_rgb(0x6B, 0x6B, 0x6B),
            pill_edge: Color32::from_rgb(0x05, 0x05, 0x05),
            outline: Color32::from_rgba_premultiplied(20, 20, 20, 20),
            outline_hover: Color32::from_rgba_premultiplied(36, 36, 36, 36),
            frame: Color32::from_gray(60),
            field_border: Color32::from_rgb(0x4A, 0x4B, 0x52),
            grip: Color32::from_rgb(105, 105, 115),
            text: Color32::from_rgb(0xDC, 0xDC, 0xE0),
            text_strong: Color32::from_rgb(0xF0, 0xF0, 0xF2),
            text_label: Color32::from_rgb(0xB4, 0xB5, 0xBC),
            text_muted: Color32::from_rgb(0x8A, 0x8B, 0x93),
            text_disabled: Color32::from_rgb(0x5A, 0x5B, 0x60),
            scrim_text: Color32::from_rgba_premultiplied(230, 230, 230, 230),
            drop_target: Color32::from_rgba_premultiplied(9, 13, 17, 20),
            drop_target_border: Color32::from_rgba_premultiplied(118, 141, 180, 200),
            drop_target_text: Color32::from_rgba_premultiplied(180, 194, 216, 230),
            context_menu: Color32::from_rgba_premultiplied(14, 14, 17, 200),
            context_menu_border: Color32::from_rgba_premultiplied(60, 60, 60, 60),
            context_hover: Color32::from_rgba_premultiplied(28, 22, 13, 28),
            context_hover_border: Color32::from_rgba_premultiplied(110, 78, 39, 110),
            close_hover: Color32::from_rgb(0xC4, 0x2B, 0x1C),
            highlight: Color32::from_rgb(104, 222, 158),
            attention: Color32::from_rgb(255, 178, 82),
            recovery_fill: Color32::from_rgb(30, 76, 48),
            feed_echo: Color32::from_rgb(150, 175, 225),
            error: Color32::from_rgb(235, 130, 118),
            text_quiet: Color32::TRANSPARENT,
            text_hover: Color32::TRANSPARENT,
            separator: Color32::TRANSPARENT,
            selection_text: Color32::TRANSPARENT,
            close_hover_text: Color32::TRANSPARENT,
            pill_wash: Color32::TRANSPARENT,
            tile: Color32::TRANSPARENT,
            tile_hover: Color32::TRANSPARENT,
            popup_border: Color32::TRANSPARENT,
            dropdown_selected: Color32::TRANSPARENT,
            row_rule: Color32::TRANSPARENT,
            status_text: Color32::TRANSPARENT,
            grip_hover: Color32::TRANSPARENT,
            divider: Color32::TRANSPARENT,
            dialog_scrim: Color32::TRANSPARENT,
            confirm: Color32::TRANSPARENT,
            feed_success: Color32::TRANSPARENT,
            recovery_text: Color32::TRANSPARENT,
            feed_info: Color32::TRANSPARENT,
            error_text: Color32::TRANSPARENT,
            danger_text: Color32::TRANSPARENT,
            feed_error: Color32::TRANSPARENT,
            text_emphasis: Color32::TRANSPARENT,
            primary_button: Color32::TRANSPARENT,
        }
        .with_aliases()
    };

    /// Sets every merged-away alias field to its merged token's colour.
    pub const fn with_aliases(self) -> Self {
        Self {
            text_quiet: self.text_muted,
            text_hover: self.text_strong,
            separator: self.outline,
            selection_text: self.text_strong,
            close_hover_text: self.text_strong,
            pill_wash: self.outline,
            tile: self.well,
            tile_hover: self.chip_pressed,
            popup_border: self.frame,
            dropdown_selected: self.chip_hover,
            row_rule: self.chip_pressed,
            status_text: self.text_muted,
            grip_hover: self.text_label,
            divider: self.field_border,
            dialog_scrim: self.scrim,
            confirm: self.highlight,
            feed_success: self.highlight,
            recovery_text: self.highlight,
            feed_info: self.text,
            error_text: self.error,
            danger_text: self.error,
            feed_error: self.error,
            text_emphasis: self.text_strong,
            primary_button: self.primary,
            ..self
        }
    }

    /// [`Self::DARK`].
    pub fn dark_default() -> Self {
        Self::DARK
    }

    /// These colours with `error` (and its merged aliases `error_text`,
    /// `danger_text`, `feed_error`) set to `error`. For a host adopting a
    /// different error hue, such as [`ERROR_ROSE`].
    pub const fn with_error(self, error: egui::Color32) -> Self {
        Self {
            error,
            error_text: error,
            danger_text: error,
            feed_error: error,
            ..self
        }
    }
}

/// The error hue Chris approved for SDB on 2026-11-27: rose, soft and clearly
/// neither red nor green, so it never relies on a red/green distinction (the
/// error mark is a drawn ✕ as well). Opt-in: [`InterfaceColors::DARK`] keeps
/// its salmon for hosts that have not chosen; adopt it with
/// [`InterfaceColors::with_error`].
pub const ERROR_ROSE: egui::Color32 = egui::Color32::from_rgb(0xEC, 0x96, 0xC4);

/// [`ERROR_ROSE`] darkened to read on light surfaces (about 8:1 on white):
/// the same hue, for light themes.
pub const ERROR_ROSE_ON_LIGHT: egui::Color32 = egui::Color32::from_rgb(0x96, 0x2C, 0x63);

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
            well: t(c.well),
            well_hover: t(c.well_hover),
            field: t(c.field),
            capsule: t(c.capsule),
            scrim: t(c.scrim),
            prompt_scrim: t(c.prompt_scrim),
            card_shadow: t(c.card_shadow),
            dialog_shadow: t(c.dialog_shadow),
            chip: t(c.chip),
            chip_hover: t(c.chip_hover),
            chip_pressed: t(c.chip_pressed),
            row_hover: t(c.row_hover),
            selection: t(c.selection),
            primary: t(c.primary),
            row_selected: t(c.row_selected),
            pill_active: t(c.pill_active),
            switch_on: t(c.switch_on),
            switch_knob: t(c.switch_knob),
            pill: t(c.pill),
            pill_edge: t(c.pill_edge),
            outline: t(c.outline),
            outline_hover: t(c.outline_hover),
            frame: t(c.frame),
            field_border: t(c.field_border),
            grip: t(c.grip),
            text: t(c.text),
            text_strong: t(c.text_strong),
            text_label: t(c.text_label),
            text_muted: t(c.text_muted),
            text_disabled: t(c.text_disabled),
            scrim_text: t(c.scrim_text),
            drop_target: t(c.drop_target),
            drop_target_border: t(c.drop_target_border),
            drop_target_text: t(c.drop_target_text),
            context_menu: t(c.context_menu),
            context_menu_border: t(c.context_menu_border),
            context_hover: t(c.context_hover),
            context_hover_border: t(c.context_hover_border),
            close_hover: t(c.close_hover),
            highlight: t(c.highlight),
            attention: t(c.attention),
            recovery_fill: t(c.recovery_fill),
            feed_echo: t(c.feed_echo),
            error: t(c.error),
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
            well: c(&self.well, d.well),
            well_hover: c(&self.well_hover, d.well_hover),
            field: c(&self.field, d.field),
            capsule: c(&self.capsule, d.capsule),
            scrim: c(&self.scrim, d.scrim),
            prompt_scrim: c(&self.prompt_scrim, d.prompt_scrim),
            card_shadow: c(&self.card_shadow, d.card_shadow),
            dialog_shadow: c(&self.dialog_shadow, d.dialog_shadow),
            chip: c(&self.chip, d.chip),
            chip_hover: c(&self.chip_hover, d.chip_hover),
            chip_pressed: c(&self.chip_pressed, d.chip_pressed),
            row_hover: c(&self.row_hover, d.row_hover),
            selection: c(&self.selection, d.selection),
            primary: c(&self.primary, d.primary),
            row_selected: c(&self.row_selected, d.row_selected),
            pill_active: c(&self.pill_active, d.pill_active),
            switch_on: c(&self.switch_on, d.switch_on),
            switch_knob: c(&self.switch_knob, d.switch_knob),
            pill: c(&self.pill, d.pill),
            pill_edge: c(&self.pill_edge, d.pill_edge),
            outline: c(&self.outline, d.outline),
            outline_hover: c(&self.outline_hover, d.outline_hover),
            frame: c(&self.frame, d.frame),
            field_border: c(&self.field_border, d.field_border),
            grip: c(&self.grip, d.grip),
            text: c(&self.text, d.text),
            text_strong: c(&self.text_strong, d.text_strong),
            text_label: c(&self.text_label, d.text_label),
            text_muted: c(&self.text_muted, d.text_muted),
            text_disabled: c(&self.text_disabled, d.text_disabled),
            scrim_text: c(&self.scrim_text, d.scrim_text),
            drop_target: c(&self.drop_target, d.drop_target),
            drop_target_border: c(&self.drop_target_border, d.drop_target_border),
            drop_target_text: c(&self.drop_target_text, d.drop_target_text),
            context_menu: c(&self.context_menu, d.context_menu),
            context_menu_border: c(&self.context_menu_border, d.context_menu_border),
            context_hover: c(&self.context_hover, d.context_hover),
            context_hover_border: c(&self.context_hover_border, d.context_hover_border),
            close_hover: c(&self.close_hover, d.close_hover),
            highlight: c(&self.highlight, d.highlight),
            attention: c(&self.attention, d.attention),
            recovery_fill: c(&self.recovery_fill, d.recovery_fill),
            feed_echo: c(&self.feed_echo, d.feed_echo),
            error: c(&self.error, d.error),
            text_quiet: d.text_quiet,
            text_hover: d.text_hover,
            separator: d.separator,
            selection_text: d.selection_text,
            close_hover_text: d.close_hover_text,
            pill_wash: d.pill_wash,
            tile: d.tile,
            tile_hover: d.tile_hover,
            popup_border: d.popup_border,
            dropdown_selected: d.dropdown_selected,
            row_rule: d.row_rule,
            status_text: d.status_text,
            grip_hover: d.grip_hover,
            divider: d.divider,
            dialog_scrim: d.dialog_scrim,
            confirm: d.confirm,
            feed_success: d.feed_success,
            recovery_text: d.recovery_text,
            feed_info: d.feed_info,
            error_text: d.error_text,
            danger_text: d.danger_text,
            feed_error: d.feed_error,
            text_emphasis: d.text_emphasis,
            primary_button: d.primary_button,
        }
        .with_aliases()
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
        assert_eq!(d.frame, v.window_stroke.color);
        assert_eq!(d.outline, Color32::from_white_alpha(20));
        assert_eq!(d.outline_hover, Color32::from_white_alpha(36));
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
        // The unlit pill's wash was `rgba_unmultiplied(255,255,255,20)`;
        // merged into `outline` (slice 7), which is the same colour.
        assert_eq!(
            d.outline,
            Color32::from_rgba_unmultiplied(255, 255, 255, 20)
        );
    }

    /// A theme file saved before slice 7 (2026-11-27) carries tokens that were
    /// merged away (`text_quiet`, `separator`, `feed_success`, ...). They are
    /// ignored, and the file still loads.
    #[test]
    fn a_file_with_merged_away_tokens_still_loads() {
        let mut value = serde_json::to_value(InterfaceTokens::dark()).unwrap();
        let map = value.as_object_mut().unwrap();
        for old in [
            "text_quiet",
            "separator",
            "feed_success",
            "dropdown_selected",
        ] {
            map.insert(
                old.into(),
                serde_json::json!({"$type": "color", "$value": "#010203FF"}),
            );
        }
        let loaded: InterfaceTokens = serde_json::from_value(value).unwrap();
        assert_eq!(loaded.resolve(), InterfaceColors::DARK);
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

    #[test]
    fn with_error_sets_the_error_and_its_aliases_only() {
        let rose = InterfaceColors::DARK.with_error(ERROR_ROSE);
        assert_eq!(
            [
                rose.error,
                rose.error_text,
                rose.danger_text,
                rose.feed_error
            ],
            [ERROR_ROSE; 4]
        );
        assert_eq!(
            rose.with_error(InterfaceColors::DARK.error),
            InterfaceColors::DARK
        );
        assert_eq!(rose.highlight, InterfaceColors::DARK.highlight);
    }
}
