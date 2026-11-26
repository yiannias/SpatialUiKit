//! The one place a hover hint is attached (SDB TASK-168, behaviour: tooltips).
//!
//! Panels used to call `on_hover_text` directly, so there was no single point
//! where an app-wide "tooltips on/off" switch could act. A hint now goes through
//! [`ResponseTooltipExt::hover_hint`], which does nothing while the app has
//! tooltips switched off with [`set_enabled`]. Hints are on by default, so an
//! app that never calls [`set_enabled`] behaves exactly as before.
//!
//! The same module holds **stable anchors** (SDB TASK-373, guided onboarding):
//! a widget that a guide may point at records its rectangle under an English
//! key such as `"ribbon.line"` with [`mark_anchor`] (or [`ResponseTooltipExt::
//! anchored_hint`], which also attaches its hint). egui's own widget ids are
//! hashes of where a widget is built, stable between frames but not readable
//! and not stable across releases, so a guide names keys instead. The app can
//! ask for one key to be ringed ([`set_anchor_highlight`]); the ring is painted
//! where the widget is drawn, so it follows the widget into any window. And a
//! hint whose key a guide covers gains a "Show me" link ([`set_show_me_keys`],
//! [`take_show_me`]).

use std::collections::HashMap;

use egui::{Color32, Context, Id, Order, Rect, Response, ViewportId, WidgetText};

fn enabled_id() -> Id {
    Id::new("spatial_ui_kit_tooltips_enabled")
}

/// Switches hover hints on or off for the whole app. Held in egui's temporary
/// memory, so the app sets it from its own saved setting each frame (or when it
/// changes); a fresh context starts with hints on.
pub fn set_enabled(ctx: &Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(enabled_id(), on));
}

/// Whether hover hints are currently shown. `true` until [`set_enabled`] says
/// otherwise.
pub fn enabled(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(enabled_id()))
        .unwrap_or(true)
}

/// Hover hints that respect the app-wide switch. Implemented for [`Response`];
/// each method is the same as the egui method of the matching name when hints
/// are on, and returns the response untouched when they are off.
pub trait ResponseTooltipExt: Sized {
    /// `on_hover_text`, subject to the switch.
    fn hover_hint(self, text: impl Into<WidgetText>) -> Self;
    /// `on_disabled_hover_text`, subject to the switch.
    fn disabled_hover_hint(self, text: impl Into<WidgetText>) -> Self;
    /// [`hover_hint`](Self::hover_hint) for a widget with a stable anchor key:
    /// records the anchor ([`mark_anchor`]) and, when the app has listed the
    /// key with [`set_show_me_keys`], adds a "Show me" link under the text.
    fn anchored_hint(self, key: &str, text: impl Into<WidgetText>) -> Self;
}

impl ResponseTooltipExt for Response {
    fn hover_hint(self, text: impl Into<WidgetText>) -> Self {
        if enabled(&self.ctx) {
            self.on_hover_text(text)
        } else {
            self
        }
    }

    fn disabled_hover_hint(self, text: impl Into<WidgetText>) -> Self {
        if enabled(&self.ctx) {
            self.on_disabled_hover_text(text)
        } else {
            self
        }
    }

    fn anchored_hint(self, key: &str, text: impl Into<WidgetText>) -> Self {
        mark_anchor(&self.ctx, key, self.rect);
        if !enabled(&self.ctx) {
            return self;
        }
        if !show_me_covers(&self.ctx, key) {
            return self.on_hover_text(text);
        }
        let ctx = self.ctx.clone();
        let key = key.to_owned();
        let text = text.into();
        // A tooltip holding a link is interactive in egui: it stays open while
        // the pointer travels into it.
        self.on_hover_ui(move |ui| {
            ui.label(text);
            if ui.link("Show me").clicked() {
                ctx.data_mut(|d| d.insert_temp(show_me_request_id(), key.clone()));
            }
        })
    }
}

/// Where a stable anchor was last drawn, and in which window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub rect: Rect,
    pub viewport: ViewportId,
}

/// Every anchor recorded so far: key -> (rect, window, that window's frame).
#[derive(Clone, Default)]
struct Anchors(HashMap<String, (Rect, ViewportId, u64)>);

fn anchors_id() -> Id {
    Id::new("spatial_ui_kit_anchors")
}

fn highlight_id() -> Id {
    Id::new("spatial_ui_kit_anchor_highlight")
}

fn show_me_keys_id() -> Id {
    Id::new("spatial_ui_kit_show_me_keys")
}

fn show_me_request_id() -> Id {
    Id::new("spatial_ui_kit_show_me_request")
}

/// Records that the widget known by `key` is drawn at `rect` in the window
/// being drawn now. Call it every frame the widget is drawn. If the app has
/// asked for this key to be ringed, the ring is painted here.
pub fn mark_anchor(ctx: &Context, key: &str, rect: Rect) {
    let viewport = ctx.viewport_id();
    let frame = ctx.cumulative_frame_nr_for(viewport);
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Anchors>(anchors_id())
            .0
            .insert(key.to_owned(), (rect, viewport, frame));
    });
    let highlight = ctx.data(|d| d.get_temp::<Option<AnchorHighlight>>(highlight_id()));
    if let Some(h) = highlight.flatten().filter(|h| h.key == key) {
        paint_ring(ctx, rect, &h);
    }
}

/// Where `key` was drawn this frame or the one before, if it was. An anchor
/// its window stopped drawing goes stale after one frame of that window.
pub fn anchor(ctx: &Context, key: &str) -> Option<Anchor> {
    let (rect, viewport, frame) = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Anchors>(anchors_id())
            .0
            .get(key)
            .copied()
    })?;
    (ctx.cumulative_frame_nr_for(viewport) <= frame + 1).then_some(Anchor { rect, viewport })
}

/// The ring a guide puts round its target: a stroke of `color` with a small
/// numbered badge, so the target reads by outline and number, not colour.
#[derive(Clone, Debug, PartialEq)]
pub struct AnchorHighlight {
    pub key: String,
    /// The badge text (the step number); empty for no badge.
    pub badge: String,
    pub color: Color32,
    /// The badge text colour on `color`.
    pub on_color: Color32,
    /// The layer order the ring draws in; the app decides (above its target).
    pub order: Order,
}

/// Rings the anchor named in `highlight` wherever it is drawn; `None` clears.
pub fn set_anchor_highlight(ctx: &Context, highlight: Option<AnchorHighlight>) {
    ctx.data_mut(|d| d.insert_temp(highlight_id(), highlight));
}

fn paint_ring(ctx: &Context, rect: Rect, h: &AnchorHighlight) {
    let painter = ctx.layer_painter(egui::LayerId::new(
        h.order,
        Id::new("spatial_ui_kit_anchor_ring"),
    ));
    let ring = rect.expand(2.0);
    painter.rect_stroke(
        ring,
        4.0,
        egui::Stroke::new(2.0, h.color),
        egui::StrokeKind::Outside,
    );
    if !h.badge.is_empty() {
        let center = ring.left_top();
        painter.circle_filled(center, 8.0, h.color);
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            &h.badge,
            egui::FontId::proportional(10.0),
            h.on_color,
        );
    }
}

/// The anchor keys whose hints carry a "Show me" link (empty: none). The app
/// sets this every frame from its guides, and empties it when guides are off.
pub fn set_show_me_keys(ctx: &Context, keys: Vec<String>) {
    ctx.data_mut(|d| d.insert_temp(show_me_keys_id(), keys));
}

fn show_me_covers(ctx: &Context, key: &str) -> bool {
    ctx.data(|d| {
        d.get_temp::<Vec<String>>(show_me_keys_id())
            .is_some_and(|keys| keys.iter().any(|k| k == key))
    })
}

/// The anchor key whose "Show me" link was clicked since the last call.
pub fn take_show_me(ctx: &Context) -> Option<String> {
    ctx.data_mut(|d| d.remove_temp::<String>(show_me_request_id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_are_on_until_switched_off_and_back_on() {
        let ctx = Context::default();
        assert!(enabled(&ctx));
        set_enabled(&ctx, false);
        assert!(!enabled(&ctx));
        set_enabled(&ctx, true);
        assert!(enabled(&ctx));
    }

    /// A hint can be attached with the switch on or off without panicking, and
    /// the response it returns is the same widget's.
    #[test]
    fn the_response_is_returned_either_way() {
        let ctx = Context::default();
        for on in [true, false] {
            set_enabled(&ctx, on);
            let _ = ctx.run_ui(Default::default(), |ui| {
                let plain = ui.label("x");
                let id = plain.id;
                let hinted = plain.hover_hint("tip");
                assert_eq!(hinted.id, id);
            });
        }
    }

    /// An anchor is found while its widget is drawn and goes stale once it
    /// is not, and an anchored hint records its widget's rectangle.
    #[test]
    fn anchors_are_fresh_while_drawn_and_stale_after() {
        let ctx = Context::default();
        let mut rect = Rect::NOTHING;
        let _ = ctx.run_ui(Default::default(), |ui| {
            let r = ui.button("Line").anchored_hint("ribbon.line", "Line");
            rect = r.rect;
        });
        // Read inside later frames, as an app does.
        let mut seen = Vec::new();
        for _ in 0..2 {
            let _ = ctx.run_ui(Default::default(), |ui| {
                seen.push((
                    anchor(ui.ctx(), "ribbon.line"),
                    anchor(ui.ctx(), "ribbon.arc"),
                ));
            });
        }
        // The next frame still knows the last place; one more and it is stale.
        let found = seen[0].0.expect("recorded");
        assert_eq!(found.rect, rect);
        assert_eq!(found.viewport, ViewportId::ROOT);
        assert!(seen[0].1.is_none());
        assert!(seen[1].0.is_none());
    }

    /// The highlight paints a ring and badge only for its own key.
    #[test]
    fn the_highlight_rings_only_its_own_key() {
        let ctx = Context::default();
        let ring_shapes = |key: &str| {
            set_anchor_highlight(
                &ctx,
                Some(AnchorHighlight {
                    key: key.into(),
                    badge: "1".into(),
                    color: Color32::LIGHT_BLUE,
                    on_color: Color32::BLACK,
                    order: Order::Tooltip,
                }),
            );
            let out = ctx.run_ui(Default::default(), |ui| {
                let r = ui.button("Line");
                mark_anchor(ui.ctx(), "ribbon.line", r.rect);
            });
            out.shapes
                .iter()
                .filter(
                    |s| matches!(&s.shape, egui::Shape::Circle(c) if c.fill == Color32::LIGHT_BLUE),
                )
                .count()
        };
        assert!(ring_shapes("ribbon.line") > 0);
        assert_eq!(ring_shapes("ribbon.arc"), 0);
        set_anchor_highlight(&ctx, None);
    }

    /// Show me keys and the request round-trip; the request is taken once.
    #[test]
    fn show_me_keys_and_request_round_trip() {
        let ctx = Context::default();
        assert!(!show_me_covers(&ctx, "ribbon.line"));
        set_show_me_keys(&ctx, vec!["ribbon.line".into()]);
        assert!(show_me_covers(&ctx, "ribbon.line"));
        assert!(!show_me_covers(&ctx, "ribbon.arc"));
        ctx.data_mut(|d| d.insert_temp(show_me_request_id(), "ribbon.line".to_string()));
        assert_eq!(take_show_me(&ctx).as_deref(), Some("ribbon.line"));
        assert_eq!(take_show_me(&ctx), None);
    }
}
