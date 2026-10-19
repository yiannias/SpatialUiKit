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
/// its OS (the host app reads the live Windows accent color; a fallback constant
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

    ensure_readable_text_selection(&mut visuals);
    visuals
}

/// WCAG relative luminance of an opaque sRGB colour.
fn relative_luminance(c: egui::Color32) -> f32 {
    let channel = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
}

/// WCAG contrast ratio between two opaque colours, 1.0 (none) to 21.0.
pub fn contrast_ratio(a: egui::Color32, b: egui::Color32) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Minimum contrast between selected text and the selection highlight.
pub const MIN_SELECTED_TEXT_CONTRAST: f32 = 4.0;

/// Minimum contrast between the selection highlight and the field it sits on.
pub const MIN_HIGHLIGHT_CONTRAST: f32 = 1.6;

/// The highlight as it actually appears: `selection.bg_fill` blended over the
/// text-edit field fill.
pub fn effective_selection_fill(visuals: &egui::Visuals) -> egui::Color32 {
    effective_selection_fill_over(visuals.selection.bg_fill, visuals.text_edit_bg_color())
}

/// Opaque `base` mixed `t` (0..=1) of the way toward `toward`.
fn mixed(base: egui::Color32, toward: egui::Color32, t: f32) -> egui::Color32 {
    let mix = |a: u8, b: u8| (a as f32 * (1.0 - t) + b as f32 * t).round() as u8;
    egui::Color32::from_rgb(
        mix(base.r(), toward.r()),
        mix(base.g(), toward.g()),
        mix(base.b(), toward.b()),
    )
}

/// egui paints selected text in `selection.stroke.color`, and the same stroke is
/// the focus outline of a focused `TextEdit`, so it cannot be recoloured without
/// changing the focus ring. With a dark OS accent as that colour a fully selected
/// field value -- every number or rename field opens with its text selected --
/// was nearly invisible: dark text on a dark highlight (2026-10-18).
///
/// So the stroke is left alone and the *highlight* (`selection.bg_fill`) is moved
/// instead, the smallest step toward white or black that makes the stroke colour
/// read on it ([`MIN_SELECTED_TEXT_CONTRAST`]), preferring a step that also keeps
/// the highlight visible against the text field it sits on
/// ([`MIN_HIGHLIGHT_CONTRAST`]). The field fill is `visuals.text_edit_bg_color()`,
/// so call this on the visuals actually used where the field is drawn (the dock
/// overrides `extreme_bg_color`). Idempotent; call after setting
/// `selection.bg_fill`.
pub fn ensure_readable_text_selection(visuals: &mut egui::Visuals) {
    let field = visuals.text_edit_bg_color();
    let text = visuals.selection.stroke.color;
    let base = effective_selection_fill_over(visuals.selection.bg_fill, field);
    let readable = |bg: egui::Color32| contrast_ratio(text, bg) >= MIN_SELECTED_TEXT_CONTRAST;
    let visible = |bg: egui::Color32| contrast_ratio(bg, field) >= MIN_HIGHLIGHT_CONTRAST;
    if readable(base) && visible(base) {
        return;
    }
    // (readable and visible) first, then readable only; smallest step wins.
    let mut best_readable: Option<egui::Color32> = None;
    for step in 0..=20 {
        let t = step as f32 / 20.0;
        for toward in [egui::Color32::WHITE, egui::Color32::BLACK] {
            let bg = mixed(base, toward, t);
            if !readable(bg) {
                continue;
            }
            if visible(bg) {
                visuals.selection.bg_fill = bg;
                return;
            }
            best_readable.get_or_insert(bg);
        }
    }
    if let Some(bg) = best_readable {
        visuals.selection.bg_fill = bg;
    }
}

/// `fill` blended over `field`, opaque.
fn effective_selection_fill_over(fill: egui::Color32, field: egui::Color32) -> egui::Color32 {
    let a = fill.a() as f32 / 255.0;
    let mix = |top: u8, under: u8| (top as f32 * a + under as f32 * (1.0 - a)).round() as u8;
    egui::Color32::from_rgb(
        mix(fill.r(), field.r()),
        mix(fill.g(), field.g()),
        mix(fill.b(), field.b()),
    )
}

/// Platform-gated entry point: applies [`windows11_visuals`] on Windows,
/// otherwise returns `base`'s plain `egui::Visuals` untouched.
///
/// `windows11_visuals` itself stays unconditional and untouched (its own
/// tests assert WinUI3-specific radii/strokes regardless of the host OS this
/// crate happens to build on) -- the platform decision belongs at the call
/// site, which is what this wrapper is for. Added 2026-09-25: the two the host app
/// call sites (`sdb_app::frame`, `sdb_app::settings_actions`) were calling
/// `windows11_visuals` unconditionally, so a WinUI3 4px/8px corner-radius and
/// flat-button-border reskin was landing on macOS too, with no AppKit
/// equivalent -- see `docs/design/2026-09-11_visual-consistency-audit.md`
/// §5. Callers should switch to this function; `windows11_visuals` itself is
/// left public for direct/tested use.
pub fn platform_visuals(base: Theme, accent: egui::Color32) -> egui::Visuals {
    #[cfg(target_os = "windows")]
    {
        windows11_visuals(base, accent)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = accent;
        let mut visuals = base.visuals();
        ensure_readable_text_selection(&mut visuals);
        visuals
    }
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
        // The stroke is also the focus ring: it stays the accent.
        assert_eq!(v.selection.stroke.color, accent);
    }

    const ACCENTS: [egui::Color32; 6] = [
        egui::Color32::from_rgb(0, 120, 215),
        egui::Color32::from_rgb(16, 24, 64),
        egui::Color32::from_rgb(10, 10, 40),
        egui::Color32::from_rgb(20, 20, 20),
        egui::Color32::from_rgb(250, 250, 200),
        egui::Color32::from_rgb(200, 30, 30),
    ];

    /// Visuals as the dock draws fields: the dock overrides `extreme_bg_color`
    /// with its surface colour, then re-ensures the selection against it.
    fn dock_visuals(theme: Theme, accent: egui::Color32, surface: egui::Color32) -> egui::Visuals {
        let mut v = windows11_visuals(theme, accent);
        v.extreme_bg_color = surface;
        ensure_readable_text_selection(&mut v);
        v
    }

    /// Selected text (the rename field, a number being edited) stays readable on
    /// the highlight for any accent, in both themes and on the dock's own field
    /// fill, and the focus ring (the selection stroke) is never rewritten.
    #[test]
    fn selected_text_reads_on_the_highlight_and_the_focus_ring_is_kept() {
        let surfaces = [
            egui::Color32::from_rgb(30, 30, 35),
            egui::Color32::from_rgb(12, 12, 14),
            egui::Color32::from_rgb(246, 246, 248),
        ];
        for theme in Theme::ALL {
            for accent in ACCENTS {
                for surface in surfaces {
                    let v = dock_visuals(theme, accent, surface);
                    let fill = effective_selection_fill(&v);
                    assert_eq!(v.selection.stroke.color, accent, "focus ring kept");
                    assert!(
                        contrast_ratio(v.selection.stroke.color, fill)
                            >= MIN_SELECTED_TEXT_CONTRAST,
                        "{theme:?} accent {accent:?} on {surface:?}: highlight {fill:?}"
                    );
                }
            }
        }
    }

    /// TYPED (unselected) text in an edit field -- what a rename or a Properties
    /// value field shows while typing -- contrasts with the field fill, with the
    /// dock's real overridden fill.
    #[test]
    fn typed_text_contrasts_with_the_field_fill_in_both_themes() {
        let dock_surface = |theme: Theme| match theme {
            Theme::Dark => egui::Color32::from_rgb(30, 30, 35),
            Theme::Light => egui::Color32::from_rgb(246, 246, 248),
        };
        for theme in Theme::ALL {
            for accent in ACCENTS {
                let v = dock_visuals(theme, accent, dock_surface(theme));
                assert!(
                    contrast_ratio(v.widgets.inactive.text_color(), v.text_edit_bg_color()) >= 4.5,
                    "{theme:?} {accent:?}"
                );
            }
        }
    }

    /// The highlight stays visible against the field when it can.
    #[test]
    fn the_highlight_is_visible_against_the_field() {
        for theme in Theme::ALL {
            for accent in [ACCENTS[1], ACCENTS[0]] {
                let v = windows11_visuals(theme, accent);
                assert!(
                    contrast_ratio(effective_selection_fill(&v), v.text_edit_bg_color())
                        >= MIN_HIGHLIGHT_CONTRAST,
                    "{theme:?} {accent:?}"
                );
            }
        }
    }

    #[test]
    fn an_already_readable_selection_is_left_alone() {
        let mut v = Theme::Dark.visuals();
        v.selection.bg_fill = egui::Color32::from_rgb(0, 60, 130);
        v.selection.stroke.color = egui::Color32::WHITE;
        let before = v.selection.bg_fill;
        ensure_readable_text_selection(&mut v);
        assert_eq!(v.selection.bg_fill, before);
        assert_eq!(v.selection.stroke.color, egui::Color32::WHITE);
    }
}
