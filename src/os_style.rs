//! A "Visuals-level" reskin of `egui::Visuals` toward the host OS's native
//! widget look, layered on top of [`crate::theme::Theme`]'s plain
//! `dark()`/`light()` base. Windows 11 is the first (and, for now, only)
//! target -- Chris, 2026-08-30: "since we are on windows that is the place
//! to start."
//!
//! Deliberately a *reskin*, not a rewrite: this only tunes colors, corner
//! radii, stroke widths, and the accent color on the existing `Visuals`
//! struct, which every built-in egui widget already reads from. It does
//! **not** replace how any widget paints itself (e.g. a checkbox's tick
//! mark is still egui's own shape, and a checked checkbox's box still
//! doesn't get an accent fill the way native Windows checkboxes do --
//! `egui::widgets::checkbox` doesn't consult `Visuals` for "checked", only
//! for hover/press state). Chris chose this scope explicitly over a full
//! custom-painted WinUI3 widget set, trading fidelity for touching one
//! central style object instead of every widget call site app-wide.

use crate::theme::Theme;

/// Windows 11 / WinUI3's small-control corner radius. Buttons, checkboxes,
/// combo boxes, text fields.
const CONTROL_CORNER_RADIUS: u8 = 4;

/// WinUI3's larger corner radius, for windows, menus/flyouts, and popups.
const SURFACE_CORNER_RADIUS: u8 = 8;

/// Applies a Windows-11-flavored reskin to `base`'s `egui::Visuals`:
/// smaller corner radii on controls, larger ones on windows/menus, subtle
/// 1px borders on controls (WinUI3's flat-button look, vs. egui's default
/// borderless buttons), and `accent` driving selection highlight,
/// hyperlinks, and a focus-ring-ish stroke on the active/open widget
/// states.
///
/// `accent` is a plain parameter, not read here, so this stays platform-
/// agnostic -- the caller supplies whatever accent color makes sense for
/// its OS (SDB reads the live Windows accent color; a fallback constant
/// when that read fails, or on a future non-Windows target, is the
/// caller's call, not this function's).
pub fn windows11_visuals(base: Theme, accent: egui::Color32) -> egui::Visuals {
    let mut visuals = base.visuals();

    visuals.window_corner_radius = SURFACE_CORNER_RADIUS.into();
    visuals.menu_corner_radius = SURFACE_CORNER_RADIUS.into();

    let border = if base == Theme::Dark {
        egui::Color32::from_white_alpha(24)
    } else {
        egui::Color32::from_black_alpha(24)
    };

    for widgets in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widgets.corner_radius = CONTROL_CORNER_RADIUS.into();
    }
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, border);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, border);

    // Approximates WinUI3's accent-colored focus rectangle: egui has no
    // separate "focused" bucket (`Widgets::style` folds `has_focus()` into
    // `active`), so this is what a keyboard-focused or pressed control gets.
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.5, accent);
    visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, accent);

    visuals.selection.bg_fill = accent.gamma_multiply(0.35);
    visuals.selection.stroke = egui::Stroke::new(1.0, accent);
    visuals.hyperlink_color = accent;

    visuals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_and_surface_radii_differ() {
        let v = windows11_visuals(Theme::Dark, egui::Color32::from_rgb(0, 120, 215));
        assert_eq!(
            v.widgets.inactive.corner_radius,
            CONTROL_CORNER_RADIUS.into()
        );
        assert_eq!(v.window_corner_radius, SURFACE_CORNER_RADIUS.into());
    }

    #[test]
    fn accent_drives_selection_and_hyperlinks() {
        let accent = egui::Color32::from_rgb(0, 120, 215);
        let v = windows11_visuals(Theme::Light, accent);
        assert_eq!(v.hyperlink_color, accent);
        assert_eq!(v.selection.stroke.color, accent);
    }

    #[test]
    fn dark_and_light_get_differently_toned_borders() {
        let accent = egui::Color32::from_rgb(0, 120, 215);
        let dark = windows11_visuals(Theme::Dark, accent);
        let light = windows11_visuals(Theme::Light, accent);
        assert_ne!(
            dark.widgets.inactive.bg_stroke.color,
            light.widgets.inactive.bg_stroke.color
        );
    }
}
