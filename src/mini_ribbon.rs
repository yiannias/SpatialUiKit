//! A small, self-contained ribbon of icon buttons that floats over a pane's
//! content, for tools that belong to one pane rather than to the whole app.
//!
//! Prior art, reused rather than re-drawn: the capsule frame, its rotated
//! vertical group label and the button row all come from
//! [`crate::ribbon::ribbon_panel`], the same shell a full-width ribbon uses,
//! so a mini-ribbon reads as a smaller sibling of it.
//!
//! The host supplies the look and the art. Colors arrive in
//! [`MiniRibbonTheme`]; icons arrive through a [`GlyphPainter`] keyed by
//! [`MiniRibbonItem::glyph`]. A host with no art for a key returns `false`,
//! and the kit then draws the label's first letter in its place, so a missing
//! icon is visible rather than silently blank.
//!
//! Button ids are separate from glyph keys on purpose. The kit keys its
//! click-flash and hold state by the button key in context-wide temp data, so
//! two ribbons that shared a key would flash each other's buttons too.
//! [`MiniRibbonItem::id`] is the kit key; [`MiniRibbonItem::glyph`] only picks
//! the art.

use crate::ribbon::{ModuleFrameStyle, RibbonButton, RibbonGroup, RibbonHost};

/// Colors for a mini-ribbon. The host fills these from its own palette, so the
/// kit carries no palette of its own and the ribbon follows the host's theme.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MiniRibbonTheme {
    /// Fill behind the capsules. Applied as the window fill for the ribbon's
    /// own scope, so the caller's visuals are left alone.
    pub surface: egui::Color32,
    /// Border drawn around each capsule.
    pub border: egui::Color32,
    /// Width of the capsule border, in points.
    pub border_width: f32,
    /// Text color of each capsule's rotated group label.
    pub label_foreground: egui::Color32,
    /// Fill behind each capsule's rotated group label.
    pub label_background: egui::Color32,
    /// Color of the armed (selected) button's glyph, caption and highlight.
    /// Color marks state only, so unselected buttons never use it.
    pub tool_accent: egui::Color32,
}

impl MiniRibbonTheme {
    /// The capsule frame in the shape [`crate::ribbon`]'s host trait wants.
    fn module_frame_style(&self) -> ModuleFrameStyle {
        ModuleFrameStyle {
            border: self.border,
            border_width: self.border_width,
            label_foreground: self.label_foreground,
            label_background: self.label_background,
        }
    }
}

/// Paints the monochrome glyph for `key` into `rect` in `color`. Returns
/// `false` without drawing anything when the key is unknown; the kit then
/// draws the label's first letter instead.
///
/// A plain function reference is enough for most hosts. It is a parameter
/// rather than a trait so a host can pass its icon lookup without defining a
/// type for it.
pub type GlyphPainter<'a> =
    &'a dyn Fn(&egui::Painter, &egui::Context, egui::Rect, &str, egui::Color32) -> bool;

/// One button: a glyph above a lowercase caption.
#[derive(Clone, Debug)]
pub struct MiniRibbonItem<A> {
    /// Unique, stable key for this button across the whole app (e.g.
    /// `"editor.move"`). Not the glyph: see the module doc for why.
    pub id: &'static str,
    /// Glyph key, passed to the host's [`GlyphPainter`].
    pub glyph: &'static str,
    /// Caption under the glyph, drawn lowercase. Also the letter fallback's
    /// source when the glyph is missing.
    pub label: &'static str,
    /// Hover text. The caption is used when this is empty.
    pub tooltip: &'static str,
    /// Value sent back through [`MiniRibbonOutput::actions`] when the button
    /// is clicked.
    pub action: A,
    /// Lit in the accent color, marking the armed tool.
    pub selected: bool,
    /// A disabled button is drawn dimmed and does not respond to clicks.
    pub enabled: bool,
    /// A hold-past-threshold flyout on this button, the same one a full-width
    /// ribbon's buttons carry ([`crate::ribbon::RibbonFlyout`]). Empty for a
    /// plain button.
    pub flyout: Vec<MiniFlyoutItem<A>>,
}

/// One row of a mini-ribbon button's hold-flyout.
#[derive(Clone, Debug)]
pub struct MiniFlyoutItem<A> {
    /// Glyph key, as for [`MiniRibbonItem::glyph`].
    pub key: &'static str,
    /// Caption for the flyout row.
    pub label: &'static str,
    /// Fired, in order, when this row is picked.
    pub actions: Vec<A>,
}

/// One capsule: a rotated all-caps label on its left, then its buttons.
#[derive(Clone, Debug)]
pub struct MiniRibbonGroup<A> {
    /// Text of the rotated label on the capsule's left edge.
    pub label: &'static str,
    /// The capsule's buttons, left to right.
    pub items: Vec<MiniRibbonItem<A>>,
}

/// Sizes, in points. The defaults suit a ribbon tucked into a pane's corner;
/// a pod height of `button_height + 2 * spatial_ui_kit::ribbon::GAP`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MiniRibbonStyle {
    /// Height of each button, excluding the capsule's gaps.
    pub button_height: f32,
    /// Side length of the square the glyph is drawn into.
    pub icon_size: f32,
    /// Glyph top, from the button's own top.
    pub icon_top: f32,
    /// Font size of the caption, before the host's text scale.
    pub caption_pt: f32,
    /// Vertical gap between the glyph and the caption.
    pub caption_gap: f32,
    /// Extra width either side of a button's glyph/caption, so the armed
    /// highlight does not touch the caption.
    pub side_pad: f32,
}

impl Default for MiniRibbonStyle {
    fn default() -> Self {
        Self {
            button_height: 40.0,
            icon_size: 20.0,
            icon_top: 4.0,
            caption_pt: 9.0,
            caption_gap: 3.0,
            side_pad: 2.0,
        }
    }
}

/// What a frame of the mini-ribbon produced.
pub struct MiniRibbonOutput<A> {
    /// Actions of the buttons clicked this frame, in click order.
    pub actions: Vec<A>,
    /// The area the capsules actually cover, so a caller can keep content it
    /// wants visible (a canvas, a drawing) out from under them.
    pub rect: egui::Rect,
}

/// Draws `groups` as floating capsules whose row starts at `top_left` (the
/// kit insets the first capsule by [`crate::ribbon::GAP`]), on top of whatever
/// `ui` has already painted there, without taking any of `ui`'s layout space.
/// Widgets drawn later win hover and click over earlier ones in egui, so
/// drawing this *after* a canvas makes the buttons take the pointer and the
/// canvas see nothing beneath them.
#[allow(clippy::too_many_arguments)]
pub fn show_floating<A: Clone>(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    top_left: egui::Pos2,
    max_width: f32,
    groups: &[MiniRibbonGroup<A>],
    style: &MiniRibbonStyle,
    theme: &MiniRibbonTheme,
    glyphs: GlyphPainter<'_>,
) -> MiniRibbonOutput<A> {
    let row_h = style.button_height + 2.0 * crate::ribbon::GAP;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(id_salt)
            .max_rect(egui::Rect::from_min_size(
                top_left,
                egui::vec2(max_width.max(0.0), row_h),
            ))
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    // Through `show`, not `show_inner`: `show` applies the surface fill that
    // the capsules are drawn with. Skipping it lightens every capsule.
    let actions = show(&mut child, groups, style, theme, glyphs);
    MiniRibbonOutput {
        actions,
        rect: child.min_rect(),
    }
}

/// Draws `groups` in `ui`'s normal layout flow and returns clicked actions.
pub fn show<A: Clone>(
    ui: &mut egui::Ui,
    groups: &[MiniRibbonGroup<A>],
    style: &MiniRibbonStyle,
    theme: &MiniRibbonTheme,
    glyphs: GlyphPainter<'_>,
) -> Vec<A> {
    // The kit fills each capsule with `window_fill`. The host's surface color
    // is what every other floating capsule uses, so the mini-ribbon matches
    // them in both themes. Scoped so the caller's own visuals are untouched.
    ui.scope(|ui| {
        ui.visuals_mut().window_fill = theme.surface;
        show_inner(ui, groups, style, theme, glyphs)
    })
    .inner
}

fn show_inner<A: Clone>(
    ui: &mut egui::Ui,
    groups: &[MiniRibbonGroup<A>],
    style: &MiniRibbonStyle,
    theme: &MiniRibbonTheme,
    glyphs: GlyphPainter<'_>,
) -> Vec<A> {
    let entries: Vec<_> = groups
        .iter()
        .flat_map(|group| group.items.iter())
        .map(|item| Entry {
            id: item.id,
            glyph: item.glyph,
            tooltip: item.tooltip,
        })
        .collect();
    let host = MiniHost {
        entries: &entries,
        style: *style,
        theme: *theme,
        glyphs,
    };
    let kit_groups: Vec<RibbonGroup<A>> = groups
        .iter()
        .map(|group| RibbonGroup {
            label: group.label,
            buttons: group
                .items
                .iter()
                .map(|item| RibbonButton {
                    key: item.id,
                    label: item.label,
                    action: item.action.clone(),
                    selected: item.selected,
                    enabled: item.enabled,
                    disabled_hint: "",
                    flyout: (!item.flyout.is_empty()).then(|| crate::ribbon::RibbonFlyout {
                        kind: crate::ribbon::FlyoutKind::Alternatives,
                        items: item
                            .flyout
                            .iter()
                            .map(|row| crate::ribbon::FlyoutItem {
                                key: row.key,
                                label: row.label,
                                actions: row.actions.clone(),
                                enabled: true,
                                disabled_hint: "",
                            })
                            .collect(),
                    }),
                })
                .collect(),
        })
        .collect();
    // Tighten the button spacing before resorting to the kit's horizontal
    // scroll: a pane is often narrower than the ribbon's natural width, and
    // losing the last button to a scroll bar is worse than tighter spacing.
    let available = ui.available_width();
    for spacing in [ui.spacing().item_spacing.x, 5.0, 3.0] {
        ui.spacing_mut().item_spacing.x = spacing;
        if natural_width(ui, groups, &host, spacing) <= available {
            break;
        }
    }
    crate::ribbon::ribbon_panel(ui, &kit_groups, &host)
}

/// The row's width as `ribbon_panel` lays it out with `spacing` between
/// buttons and between capsules: a leading `GAP`, then per capsule its label
/// strip, a `GAP`, the buttons, and a closing `GAP`.
fn natural_width<A>(
    ui: &egui::Ui,
    groups: &[MiniRibbonGroup<A>],
    host: &MiniHost<'_>,
    spacing: f32,
) -> f32 {
    use crate::ribbon::GAP;
    /// `crate::ribbon`'s private `LABEL_STRIP_W`.
    const LABEL_STRIP_W: f32 = 16.0;
    let capsules: f32 = groups
        .iter()
        .map(|group| {
            let buttons: f32 = group
                .items
                .iter()
                .map(|item| host.button_width(ui, item.id, item.label))
                .sum();
            LABEL_STRIP_W
                + 2.0 * GAP
                + buttons
                + group.items.len().saturating_sub(1) as f32 * spacing
        })
        .sum();
    GAP + capsules + groups.len().saturating_sub(1) as f32 * spacing
}

struct Entry {
    id: &'static str,
    glyph: &'static str,
    tooltip: &'static str,
}

struct MiniHost<'a> {
    entries: &'a [Entry],
    style: MiniRibbonStyle,
    theme: MiniRibbonTheme,
    glyphs: GlyphPainter<'a>,
}

impl MiniHost<'_> {
    fn caption_font(&self, ui: &egui::Ui) -> egui::FontId {
        egui::FontId::proportional(self.style.caption_pt * crate::theme::current_text_scale(ui))
    }

    fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }
}

/// Draws `label`'s first letter, uppercased, centered in `rect`. The stand-in
/// for a glyph the host does not have, so the button stays identifiable.
fn draw_letter_fallback(
    painter: &egui::Painter,
    rect: egui::Rect,
    label: &str,
    color: egui::Color32,
) {
    let Some(first) = label.chars().next() else {
        return;
    };
    // `to_uppercase` can yield several chars (e.g. 'ß' -> "SS"), hence a String.
    let letter: String = first.to_uppercase().collect();
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        letter,
        egui::FontId::proportional(rect.height() * 0.8),
        color,
    );
}

impl RibbonHost for MiniHost<'_> {
    fn module_frame_style(&self) -> ModuleFrameStyle {
        self.theme.module_frame_style()
    }

    fn button_size(&self) -> egui::Vec2 {
        egui::vec2(0.0, self.style.button_height)
    }

    fn button_width(&self, ui: &egui::Ui, _key: &str, label: &str) -> f32 {
        let caption = ui
            .painter()
            .layout_no_wrap(
                label.to_lowercase(),
                self.caption_font(ui),
                egui::Color32::PLACEHOLDER,
            )
            .size()
            .x;
        self.style.icon_size.max(caption) + 2.0 * self.style.side_pad
    }

    fn icon_rect(&self, button_rect: egui::Rect) -> egui::Rect {
        egui::Rect::from_center_size(
            egui::pos2(
                button_rect.center().x,
                button_rect.top() + self.style.icon_top + self.style.icon_size / 2.0,
            ),
            egui::Vec2::splat(self.style.icon_size),
        )
    }

    fn icon_button(
        &self,
        ui: &mut egui::Ui,
        key: &str,
        label: &str,
        selected: bool,
        enabled: bool,
        _disabled_hint: &str,
    ) -> egui::Response {
        let sense = if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        };
        let size = egui::vec2(self.button_width(ui, key, label), self.style.button_height);
        let (rect, response) = ui.allocate_exact_size(size, sense);
        let entry = self.entry(key);
        if ui.is_rect_visible(rect) {
            let accent = self.theme.tool_accent;
            let visuals = ui.visuals();
            let hovered = enabled && response.hovered();
            // Monochrome at rest; color only for state (armed tool).
            let fg = if !enabled {
                visuals.weak_text_color().gamma_multiply(0.6)
            } else if selected {
                accent
            } else if hovered {
                visuals.strong_text_color()
            } else {
                visuals.widgets.inactive.fg_stroke.color
            };
            if selected {
                ui.painter()
                    .rect_filled(rect, 6.0, accent.gamma_multiply(0.16));
                ui.painter().rect_stroke(
                    rect,
                    6.0,
                    egui::Stroke::new(1.0, accent.gamma_multiply(0.7)),
                    egui::StrokeKind::Inside,
                );
            } else if hovered {
                ui.painter()
                    .rect_filled(rect, 6.0, visuals.widgets.hovered.weak_bg_fill);
            }
            let icon_rect = self.icon_rect(rect);
            let glyph = entry.map_or(key, |entry| entry.glyph);
            if !(self.glyphs)(ui.painter(), ui.ctx(), icon_rect, glyph, fg) {
                draw_letter_fallback(ui.painter(), icon_rect, label, fg);
            }
            ui.painter().text(
                egui::pos2(rect.center().x, icon_rect.bottom() + self.style.caption_gap),
                egui::Align2::CENTER_TOP,
                label.to_lowercase(),
                self.caption_font(ui),
                fg,
            );
        }
        let tooltip = entry
            .map(|entry| entry.tooltip)
            .filter(|tooltip| !tooltip.is_empty())
            .unwrap_or(label);
        response.on_hover_text(tooltip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme() -> MiniRibbonTheme {
        MiniRibbonTheme {
            surface: egui::Color32::from_rgb(30, 30, 35),
            border: egui::Color32::from_rgb(0x2B, 0x2C, 0x31),
            border_width: 1.0,
            label_foreground: egui::Color32::from_rgb(0x7F, 0x80, 0x88),
            label_background: egui::Color32::from_rgb(0x2B, 0x2C, 0x31),
            tool_accent: egui::Color32::from_rgb(104, 222, 158),
        }
    }

    /// A host with no glyph art at all: every key is unknown.
    fn no_glyphs(
        _: &egui::Painter,
        _: &egui::Context,
        _: egui::Rect,
        _: &str,
        _: egui::Color32,
    ) -> bool {
        false
    }

    fn groups(selected: &str) -> Vec<MiniRibbonGroup<&'static str>> {
        let item = |id: &'static str, glyph: &'static str, label: &'static str| MiniRibbonItem {
            id,
            glyph,
            label,
            tooltip: "",
            action: id,
            selected: id == selected,
            enabled: true,
            flyout: Vec::new(),
        };
        vec![
            MiniRibbonGroup {
                label: "Create",
                items: vec![item("t.line", "line", "Line"), item("t.arc", "arc", "Arc")],
            },
            MiniRibbonGroup {
                label: "Modify",
                items: vec![item("t.move", "move", "Move")],
            },
        ]
    }

    fn text(output: &egui::FullOutput) -> String {
        fn walk(shape: &egui::Shape, out: &mut String) {
            match shape {
                egui::Shape::Text(t) => {
                    out.push_str(t.galley.text());
                    out.push('\n');
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = String::new();
        for clipped in &output.shapes {
            walk(&clipped.shape, &mut out);
        }
        out
    }

    #[test]
    fn floating_ribbon_draws_captions_and_group_labels_without_taking_layout_space() {
        let ctx = egui::Context::default();
        let mut used = egui::Rect::NOTHING;
        let mut ribbon = egui::Rect::NOTHING;
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let before = ui.cursor();
            let out = show_floating(
                ui,
                "test_mini_ribbon",
                ui.max_rect().min,
                600.0,
                &groups("t.line"),
                &MiniRibbonStyle::default(),
                &theme(),
                &no_glyphs,
            );
            ribbon = out.rect;
            used = egui::Rect::from_min_max(before.min, ui.cursor().min);
        });
        let shown = text(&output);
        for word in ["line", "arc", "move", "CREATE", "MODIFY"] {
            assert!(shown.contains(word), "{word} missing from {shown}");
        }
        assert!(
            ribbon.height() > 40.0 && ribbon.width() > 100.0,
            "{ribbon:?}"
        );
        assert_eq!(
            used.height(),
            0.0,
            "a floating ribbon takes no layout space"
        );
    }

    #[test]
    fn unknown_glyph_falls_back_to_letter() {
        let ctx = egui::Context::default();
        let output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let _ = show_floating(
                ui,
                "test_mini_ribbon_fallback",
                ui.max_rect().min,
                600.0,
                &groups("t.line"),
                &MiniRibbonStyle::default(),
                &theme(),
                &no_glyphs,
            );
        });
        // With no glyph art, each button's first letter is drawn on its own
        // line in place of the icon: Line, Arc and Move.
        let shown = text(&output);
        for letter in ["L", "A", "M"] {
            assert!(
                shown.lines().any(|line| line == letter),
                "fallback {letter} missing from {shown}"
            );
        }
    }

    #[test]
    fn clicking_a_button_returns_its_action() {
        let ctx = egui::Context::default();
        let style = MiniRibbonStyle::default();
        let frame = |events: Vec<egui::Event>| {
            let mut actions = Vec::new();
            let mut rect = egui::Rect::NOTHING;
            let _ = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    let out = show_floating(
                        ui,
                        "test_mini_ribbon",
                        egui::pos2(0.0, 0.0),
                        600.0,
                        &groups(""),
                        &style,
                        &theme(),
                        &no_glyphs,
                    );
                    rect = out.rect;
                    actions = out.actions;
                },
            );
            (actions, rect)
        };
        let (_, rect) = frame(Vec::new());
        // The first capsule's first button: past the kit's leading gap,
        // label strip and inner gap, vertically centered.
        let target = egui::pos2(rect.left() + 8.0 + 16.0 + 8.0 + 6.0, rect.center().y);
        frame(vec![egui::Event::PointerMoved(target)]);
        let press = |pressed| egui::Event::PointerButton {
            pos: target,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(vec![press(true)]);
        let (actions, _) = frame(vec![press(false)]);
        assert_eq!(actions, vec!["t.line"]);
    }
}
