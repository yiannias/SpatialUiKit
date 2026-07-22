//! Passive Panel mechanics shared between SpatialSketchPad and
//! SpatialDrawingBoard: the floating translucent "squircle" that anchors to
//! the drawing viewport's edges, its auto-fading notification feed, cockpit-
//! style annunciator push-buttons, and its resize/context-menu chrome.
//!
//! Investigating both apps' current `chrome::passive_panel.rs` (2026-07-20,
//! ahead of this extraction) found them already nearly identical in
//! mechanics -- `fit_to_viewport`, the annunciator painting, the feed
//! fade-out math, the resize grip, and the right-click menu were essentially
//! copy-identical between the two, differing only in which annunciators each
//! app wires up and small policy choices (SDB gates margin recapture on an
//! actual drag and edge-snaps; SSP recaptures every valid frame). This module
//! is a **utils-style extraction**, not a generic-trait one like `menu`: none
//! of these functions touch either app's `Action`/`UiContext` type at all --
//! they operate on plain geometry, `FeedEntry`/`FeedKind`, and closures for
//! the two callback points (reset, hide) that do need app state.
//!
//! What deliberately stays app-specific: the actual list of annunciators
//! wired up (which toggle, which icon, which tooltip -- inherently tied to
//! each app's own settings/`Action` set), the annunciator icon glyph *data*
//! (visually similar but conceptually app-owned), the tool-prompt/command-line
//! row, and the margin-recapture *policy* (when to call [`capture_margins`],
//! not the capture math itself).

use std::collections::VecDeque;
use std::time::Instant;

/// What kind of feed entry -- controls the text color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedKind {
    /// Transient app status ("Fuse needs 2+ selected solids").
    Info,
    /// A command's success response ("OK").
    Success,
    /// A command's error response ("ERR: ...").
    Error,
    /// The echo of a typed command line ("> create.line ...").
    Echo,
}

/// Text color for a feed entry's `FeedKind`.
pub fn feed_color(kind: FeedKind) -> egui::Color32 {
    match kind {
        FeedKind::Info => egui::Color32::from_rgb(205, 205, 210),
        FeedKind::Success => egui::Color32::from_rgb(150, 210, 150),
        FeedKind::Error => egui::Color32::from_rgb(235, 130, 118),
        FeedKind::Echo => egui::Color32::from_rgb(150, 175, 225),
    }
}

pub struct FeedEntry {
    pub kind: FeedKind,
    pub text: String,
    pub at: Instant,
}

/// How long a feed entry stays fully visible, and how long it fades.
pub const FEED_HOLD_SECS: f32 = 5.0;
pub const FEED_FADE_SECS: f32 = 3.0;

/// The window frame's fixed contribution to the outer size: inner margins
/// (14, 10) on each side plus a 1px hairline -- exact, so margin math never
/// drifts. Identical in both apps.
pub const FRAME_PAD: egui::Vec2 = egui::vec2(30.0, 22.0);

/// Fit the panel's outer rect to `vp` from its stored margins: both
/// horizontal margins preserved (width stretches); vertically, only the
/// `anchor_bottom`-selected edge's margin is honored (that edge stays fixed,
/// the other floats) -- an explicit, stable choice rather than a per-call
/// "nearer edge" guess, which visibly jumps when floating-point-close
/// top/bottom margins flip which one counts as nearer across a resize.
/// `min_outer` is the app's own minimum outer size (SSP and SDB use
/// different values).
pub fn fit_to_viewport(
    margins: (f32, f32, f32, f32),
    anchor_bottom: bool,
    height: f32,
    vp: egui::Rect,
    min_outer: egui::Vec2,
) -> egui::Rect {
    let (l, t, r, b) = margins;
    let w = (vp.width() - l - r).max(min_outer.x);
    let h = height.clamp(min_outer.y, (vp.height() - 24.0).max(min_outer.y));
    // Nearer horizontal edge wins if the width had to be clamped.
    let x = if r <= l { vp.right() - r - w } else { vp.left() + l };
    let y = if anchor_bottom { vp.bottom() - b - h } else { vp.top() + t };
    egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(w, h))
}

/// Derives fresh margins (left, top, right, bottom) from the panel's
/// rendered `rect` against the viewport `vp` -- the pure math half of margin
/// bookkeeping. Callers decide *when* to call this (every valid frame, or
/// only on an actual drag -- see this module's doc comment) and may run the
/// result through [`snap_edge`] first.
pub fn capture_margins(rect: egui::Rect, vp: egui::Rect) -> (f32, f32, f32, f32) {
    (
        (rect.left() - vp.left()).clamp(0.0, vp.width()),
        (rect.top() - vp.top()).clamp(0.0, vp.height()),
        (vp.right() - rect.right()).clamp(0.0, vp.width()),
        (vp.bottom() - rect.bottom()).clamp(0.0, vp.height()),
    )
}

/// True if the panel's rendered `rect` now sits in the bottom half of the
/// viewport `vp` -- the raw signal `anchor_bottom` gets updated from,
/// **only** on a frame where the panel actually moved for a reason other
/// than the module's own re-fit (see each app's `passive_panel` for the
/// `refit_this_frame` gate this must be paired with).
pub fn capture_anchor_bottom(rect: egui::Rect, vp: egui::Rect) -> bool {
    rect.center().y - vp.top() > vp.height() * 0.5
}

/// Rounds `raw` to `edge_margin` if it's within `threshold` of the edge, so a
/// drag that ends up "roughly" near an edge consistently lands at exactly the
/// same gap instead of whatever pixel the user happened to release on.
/// Optional -- SDB uses this on its drag-captured margins, SSP doesn't call
/// it at all.
pub fn snap_edge(raw: f32, edge_margin: f32, threshold: f32) -> f32 {
    if raw <= threshold {
        edge_margin
    } else {
        raw
    }
}

// ---------------------------------------------------------------------------
// Annunciator icon painting -- tiny vector glyphs in a unit square (y down),
// painted directly with the egui painter rather than a rasterized icon
// texture: the caps need per-state tinting and a painted glow halo, which a
// baked single-color texture can't do. The glyph *data* (which shapes make
// up e.g. an OSNAP icon) stays app-specific; only the enum shape and the
// painter are shared.
// ---------------------------------------------------------------------------

pub enum Glyph {
    /// Line segment (x0, y0) -> (x1, y1).
    Seg(f32, f32, f32, f32),
    /// Small filled dot at (x, y).
    Dot(f32, f32),
    /// Circular arc: center, radius, start/end angle in degrees (screen
    /// space, y down -- so negative angles sweep upward).
    Arc(f32, f32, f32, f32, f32),
}

pub fn paint_icon(painter: &egui::Painter, rect: egui::Rect, glyphs: &[Glyph], color: egui::Color32, width: f32) {
    let map = |x: f32, y: f32| egui::pos2(rect.min.x + x * rect.width(), rect.min.y + y * rect.height());
    let stroke = egui::Stroke::new(width, color);
    for g in glyphs {
        match *g {
            Glyph::Seg(x0, y0, x1, y1) => {
                painter.line_segment([map(x0, y0), map(x1, y1)], stroke);
            }
            Glyph::Dot(x, y) => {
                painter.circle_filled(map(x, y), width * 0.9, color);
            }
            Glyph::Arc(cx, cy, r, a0, a1) => {
                let n = 12;
                let mut prev: Option<egui::Pos2> = None;
                for i in 0..=n {
                    let t = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
                    let p = map(cx + r * t.cos(), cy + r * t.sin());
                    if let Some(q) = prev {
                        painter.line_segment([q, p], stroke);
                    }
                    prev = Some(p);
                }
            }
        }
    }
}

/// Cockpit-style annunciator push-button: dark rounded cap, icon + caption
/// back-illuminated in `accent` when `lit` (layered wide-to-narrow strokes
/// approximate the bloom of a light behind the lens), dim gray when not.
/// Returns the raw `Response` so callers needing more than a plain
/// left-click (e.g. a right-click submenu) can attach their own handling --
/// see [`annunciator`] for the common "just want a bool" case.
pub fn annunciator_response(
    ui: &mut egui::Ui,
    label: &str,
    glyphs: &[Glyph],
    lit: bool,
    accent: egui::Color32,
    tip: &str,
) -> egui::Response {
    let size = egui::vec2(58.0, 40.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let hovered = resp.hovered();
    let fill = if lit {
        egui::Color32::from_rgb(
            24 + (accent.r() as u32 * 26 / 255) as u8,
            24 + (accent.g() as u32 * 26 / 255) as u8,
            27 + (accent.b() as u32 * 26 / 255) as u8,
        )
    } else {
        egui::Color32::from_rgb(24, 24, 27)
    };
    let border = if hovered {
        egui::Color32::from_rgb(110, 110, 118)
    } else if lit {
        egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 70)
    } else {
        egui::Color32::from_rgb(58, 58, 64)
    };
    let painter = ui.painter();
    painter.rect_filled(rect, 7.0, fill);
    painter.rect_stroke(rect, 7.0, egui::Stroke::new(1.0, border), egui::StrokeKind::Inside);

    let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.min.y + 15.0), egui::vec2(15.0, 15.0));
    let caption_pos = egui::pos2(rect.center().x, rect.max.y - 8.5);
    // Bypasses egui's style system entirely (raw `painter.text`), so scale
    // it manually against the app's text-size setting -- see
    // `crate::theme::current_text_scale`'s doc comment.
    let font = egui::FontId::monospace(crate::theme::PASSIVE_PANEL_TEXT_SIZE * crate::theme::current_text_scale(ui));
    if lit {
        // Back-illumination: wide translucent underpaints beneath the crisp
        // stroke, and a soft halo behind the caption.
        paint_icon(painter, icon_rect, glyphs, accent.gamma_multiply(0.16), 4.2);
        paint_icon(painter, icon_rect, glyphs, accent.gamma_multiply(0.38), 2.6);
        paint_icon(painter, icon_rect, glyphs, accent, 1.3);
        let halo = accent.gamma_multiply(0.3);
        for off in [egui::vec2(-1.0, 0.0), egui::vec2(1.0, 0.0), egui::vec2(0.0, -1.0), egui::vec2(0.0, 1.0)] {
            painter.text(caption_pos + off, egui::Align2::CENTER_CENTER, label, font.clone(), halo);
        }
        painter.text(caption_pos, egui::Align2::CENTER_CENTER, label, font, accent);
    } else {
        let dim = egui::Color32::from_rgb(104, 104, 112);
        paint_icon(painter, icon_rect, glyphs, dim, 1.3);
        painter.text(caption_pos, egui::Align2::CENTER_CENTER, label, font, dim);
    }
    resp.on_hover_text(tip)
}

/// Plain-bool convenience wrapper over [`annunciator_response`], for the
/// common case where only a left-click matters.
pub fn annunciator(ui: &mut egui::Ui, label: &str, glyphs: &[Glyph], lit: bool, accent: egui::Color32, tip: &str) -> bool {
    annunciator_response(ui, label, glyphs, lit, accent, tip).clicked()
}

/// Renders the panel's translucent rounded-rect frame at the given opacity
/// (0.25-1.0, clamped). Identical styling in both apps.
pub fn panel_frame(opacity: f32) -> egui::Frame {
    let alpha = (opacity.clamp(0.25, 1.0) * 255.0) as u8;
    egui::Frame::new()
        .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 35, alpha))
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(90, 90, 100, alpha.max(120))))
        .corner_radius(14)
        .inner_margin(egui::Margin::symmetric(14, 10))
}

/// Builds the fixed-size, title-bar-less, movable panel window. Size is
/// fully owned by the margin model above (egui's own resize memory can't be
/// driven from viewport changes), so the window is fixed-size and resizing
/// goes through [`resize_grip`] instead.
pub fn panel_window(content_size: egui::Vec2, frame: egui::Frame, pending_pos: Option<(f32, f32)>) -> egui::Window<'static> {
    let mut window = egui::Window::new("passive_panel")
        .id(egui::Id::new("passive_panel"))
        .title_bar(false)
        .resizable(false)
        .movable(true)
        .frame(frame)
        .fixed_size(content_size);
    if let Some((x, y)) = pending_pos {
        window = window.current_pos([x, y]);
    }
    window
}

/// Draws the feed: either the full scrollback (`show_history`) or the
/// last-few entries fading out with age (the "passive" in Passive Panel).
/// `monospace` matches SDB's styling choice (SSP renders proportional).
/// Requests a repaint while anything is still fading or visible, so the fade
/// animation actually advances frame to frame.
pub fn render_feed(ui: &mut egui::Ui, ctx: &egui::Context, feed: &VecDeque<FeedEntry>, show_history: bool, feed_h: f32, monospace: bool) {
    let style_label = |ui: &mut egui::Ui, text: &str, color: egui::Color32| {
        let mut rt = egui::RichText::new(text).color(color).size(crate::theme::PASSIVE_PANEL_TEXT_SIZE * crate::theme::current_text_scale(ui));
        if monospace {
            rt = rt.monospace();
        }
        ui.label(rt);
    };

    if show_history {
        // `min_height` (not just `max_height`) forces this region to always
        // occupy the full `feed_h` allotted to it -- otherwise a short feed
        // leaves empty space *below* the scroll area, between it and the
        // prompt row the caller renders next, instead of the prompt sitting
        // flush at the bottom with history above it.
        egui::ScrollArea::vertical()
            .id_salt("passive_panel_feed")
            .min_scrolled_height(feed_h)
            .max_height(feed_h)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for entry in feed {
                    style_label(ui, &entry.text, feed_color(entry.kind));
                }
            });
        return;
    }

    let visible_rows = ((feed_h / 16.0).floor() as usize).clamp(1, 3);
    let recent: Vec<(&FeedEntry, f32)> = feed
        .iter()
        .rev()
        .take(visible_rows)
        .filter_map(|e| {
            let age = e.at.elapsed().as_secs_f32();
            if age > FEED_HOLD_SECS + FEED_FADE_SECS {
                return None;
            }
            let a = (1.0 - (age - FEED_HOLD_SECS) / FEED_FADE_SECS).clamp(0.0, 1.0);
            Some((e, a))
        })
        .collect();
    // Bottom-up + fixed-size allocation: the visible rows anchor to the
    // *bottom* of the `feed_h` region (right above the prompt row the caller
    // renders next), with any leftover space pushed above the oldest visible
    // row instead of appearing below everything.
    let any_fading = ui
        .allocate_ui_with_layout(egui::vec2(ui.available_width(), feed_h), egui::Layout::bottom_up(egui::Align::Min), |ui| {
            let mut any_fading = false;
            for (entry, a) in recent.iter() {
                if *a < 1.0 {
                    any_fading = true;
                }
                style_label(ui, &entry.text, feed_color(entry.kind).gamma_multiply(*a));
            }
            any_fading
        })
        .inner;
    if any_fading || !recent.is_empty() {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }
}

/// Draws the bottom-right corner resize grip and applies any drag delta
/// directly to `size` (clamped to `min_outer`). The window itself is
/// fixed-size (see [`panel_window`]) -- this is the only way the user
/// resizes it.
pub fn resize_grip(ui: &mut egui::Ui, size: &mut (f32, f32), min_outer: egui::Vec2) {
    let grip_rect = egui::Rect::from_min_size(ui.max_rect().max - egui::vec2(6.0, 6.0), egui::vec2(16.0, 16.0));
    let grip = ui.interact(grip_rect, ui.id().with("resize_grip"), egui::Sense::drag());
    let grip_color = if grip.hovered() || grip.dragged() {
        egui::Color32::from_rgb(180, 180, 190)
    } else {
        egui::Color32::from_rgb(105, 105, 115)
    };
    let painter = ui.painter();
    let c = grip_rect.min + egui::vec2(8.0, 8.0);
    painter.line_segment([c + egui::vec2(-4.0, 4.0), c + egui::vec2(4.0, -4.0)], egui::Stroke::new(1.2, grip_color));
    painter.line_segment([c + egui::vec2(0.0, 4.0), c + egui::vec2(4.0, 0.0)], egui::Stroke::new(1.2, grip_color));
    if grip.hovered() || grip.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeNwSe);
    }
    if grip.dragged() {
        let d = grip.drag_delta();
        size.0 = (size.0 + d.x).max(min_outer.x);
        size.1 = (size.1 + d.y).max(min_outer.y);
    }
}

/// Right-click context menu: opacity slider, show-history checkbox, reset
/// position, hide panel. `on_reset`/`on_hide` are called on click (before
/// the menu closes) -- each app wires these to its own `PassivePanelState`
/// methods/fields.
pub fn context_menu(response: &egui::Response, opacity: &mut f32, show_history: &mut bool, on_reset: impl FnOnce(), on_hide: impl FnOnce()) {
    response.context_menu(|ui| {
        ui.add(egui::Slider::new(opacity, 0.25..=1.0).text("Opacity").show_value(false));
        ui.checkbox(show_history, "Show history");
        if ui.button("Reset position").clicked() {
            on_reset();
            ui.close();
        }
        if ui.button("Hide panel").clicked() {
            on_hide();
            ui.close();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vp(w: f32, h: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h))
    }

    const MIN_OUTER: egui::Vec2 = egui::vec2(400.0, 100.0);

    #[test]
    fn width_stretches_to_hold_both_horizontal_margins() {
        let margins = (100.0, 0.0, 50.0, 20.0);
        let wide = fit_to_viewport(margins, true, 150.0, vp(1600.0, 900.0), MIN_OUTER);
        assert_eq!(wide.width(), 1600.0 - 100.0 - 50.0);
        assert_eq!(vp(1600.0, 900.0).right() - wide.right(), 50.0);

        let narrow = fit_to_viewport(margins, true, 150.0, vp(900.0, 900.0), MIN_OUTER);
        assert_eq!(narrow.width(), 900.0 - 100.0 - 50.0);
        assert_eq!(vp(900.0, 900.0).right() - narrow.right(), 50.0);
    }

    #[test]
    fn vertical_anchor_is_stable_across_repeated_resizes() {
        let margins = (20.0, 40.0, 20.0, 20.0);
        let mut last_bottom_gap = None;
        for h in [900.0, 850.0, 920.0, 700.0, 1200.0, 640.0, 1000.0] {
            let rect = fit_to_viewport(margins, true, 150.0, vp(1000.0, h), MIN_OUTER);
            let gap = vp(1000.0, h).bottom() - rect.bottom();
            if let Some(prev) = last_bottom_gap {
                assert_eq!(gap, prev, "bottom margin drifted across a resize");
            }
            last_bottom_gap = Some(gap);
        }
    }

    #[test]
    fn top_anchor_is_honored_independent_of_raw_margin_size() {
        let margins = (20.0, 500.0, 20.0, 5.0);
        let rect = fit_to_viewport(margins, false, 150.0, vp(1000.0, 900.0), MIN_OUTER);
        assert_eq!(rect.top() - vp(1000.0, 900.0).top(), 500.0);
    }

    #[test]
    fn shrinking_viewport_clamps_the_fit_rect_without_needing_state_mutation() {
        let margins = (20.0, 20.0, 20.0, 20.0);
        let rect = fit_to_viewport(margins, true, 900.0, vp(1000.0, 200.0), MIN_OUTER);
        assert!(rect.height() < 900.0);
        assert!(rect.height() <= 200.0 - 24.0);
    }

    #[test]
    fn snap_edge_rounds_near_misses_to_exact_margin() {
        assert_eq!(snap_edge(3.0, 7.0, 24.0), 7.0);
        assert_eq!(snap_edge(24.0, 7.0, 24.0), 7.0);
        assert_eq!(snap_edge(25.0, 7.0, 24.0), 25.0);
    }

    #[test]
    fn capture_margins_clamps_to_viewport_bounds() {
        let vp = vp(1000.0, 800.0);
        // A rect entirely outside vp on the left still clamps to [0, width].
        let rect = egui::Rect::from_min_size(egui::pos2(-500.0, 50.0), egui::vec2(300.0, 100.0));
        let (l, t, r, b) = capture_margins(rect, vp);
        assert_eq!(l, 0.0);
        assert_eq!(t, 50.0);
        assert!(r <= vp.width());
        assert!(b <= vp.height());
    }
}
