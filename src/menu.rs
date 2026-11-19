//! Declarative menu tree, generic over an app-supplied action type `A` and
//! context type `Ctx`.
//!
//! Ported from the host application's `ssp-ui::menu_tree` + `chrome::menu_bar`
//! (the two apps' menu trees were already structurally identical — same
//! `Heading`/`Separator`/`Row`/`Item`/`Submenu` shape, same builder-style
//! construction — just independently typed to each app's own `Action`/
//! `UiContext`). Genericizing over `A`/`Ctx` here lets both apps walk one
//! tree implementation instead of two.
//!
//! What stays app-specific: icon rendering and command-registry-based
//! shortcut-label lookup. Those need each app's own icon atlas / key-binding
//! override machinery, so [`render_menu_bar`] takes a [`MenuHost`]
//! implementation supplied by the caller rather than importing either app's
//! icon/command-registry crate.

pub type CheckedFn<Ctx> = Box<dyn Fn(&Ctx) -> bool + Send + Sync>;

/// Cosmetic label color for a top-level or nested submenu button.
pub enum MenuColor {
    Default,
    Yellow,
    Blue,
}

pub enum MenuNode<A, Ctx> {
    /// Non-interactive label row (section heading, or a whole-menu
    /// placeholder like "(Placeholder)").
    Heading(&'static str),
    Separator,
    /// A handful of items render side by side in one row rather than
    /// stacked -- native menus have no such concept, so a native menu
    /// builder just flattens a `Row`'s children into ordinary sequential
    /// items.
    Row(Vec<MenuNode<A, Ctx>>),
    Item {
        /// Stable id, unique across the tree. Reuses a command-registry id
        /// where one exists so shortcut labels stay in sync automatically;
        /// otherwise a made-up dotted id.
        id: &'static str,
        label: &'static str,
        action: A,
        /// Looked up via the app's own command registry (through
        /// [`MenuHost::shortcut_label`]) for a live, override-aware shortcut
        /// label.
        command_id: Option<&'static str>,
        /// A literal "Ctrl+Shift+X"-style placeholder shortcut, reformatted
        /// per-platform by [`MenuHost::shortcut_label`] -- used only by
        /// not-yet-implemented commands with no real registry entry.
        literal_shortcut: Option<&'static str>,
        icon: Option<&'static str>,
        hover: Option<&'static str>,
        enabled: bool,
        disabled_hint: Option<&'static str>,
        /// Present for toggle/radio-style items -- both renderers evaluate
        /// this fresh every frame against the real `Ctx` to show a live
        /// checkmark.
        checked: Option<CheckedFn<Ctx>>,
        /// When true (only meaningful alongside `checked`), a click only
        /// fires `action` if the item is *not* currently checked -- for
        /// items where two labels drive one shared flip action and clicking
        /// the already-active label must be a no-op rather than toggling
        /// away.
        guard_unchecked: bool,
        /// Cosmetic right-aligned glyph for a disabled row that looks like
        /// it opens a submenu but doesn't.
        right_text: Option<&'static str>,
        /// Force-close the whole open submenu chain after this item is
        /// clicked (egui-only; native menus close on click regardless).
        close_menu: bool,
    },
    Submenu {
        label: &'static str,
        color: MenuColor,
        children: Vec<MenuNode<A, Ctx>>,
    },
}

impl<A, Ctx> MenuNode<A, Ctx> {
    /// Convenience constructor for the common case: a plain enabled item
    /// with no icon, hover text, shortcut, or checkmark.
    pub fn item(id: &'static str, label: &'static str, action: A) -> MenuNode<A, Ctx> {
        MenuNode::Item {
            id,
            label,
            action,
            command_id: None,
            literal_shortcut: None,
            icon: None,
            hover: None,
            enabled: true,
            disabled_hint: None,
            checked: None,
            guard_unchecked: false,
            right_text: None,
            close_menu: false,
        }
    }

    pub fn icon(mut self, icon: &'static str) -> MenuNode<A, Ctx> {
        if let MenuNode::Item { icon: slot, .. } = &mut self {
            *slot = Some(icon);
        }
        self
    }

    pub fn command(mut self, cmd_id: &'static str) -> MenuNode<A, Ctx> {
        if let MenuNode::Item { command_id, .. } = &mut self {
            *command_id = Some(cmd_id);
        }
        self
    }

    pub fn shortcut(mut self, literal: &'static str) -> MenuNode<A, Ctx> {
        if let MenuNode::Item {
            literal_shortcut, ..
        } = &mut self
        {
            *literal_shortcut = Some(literal);
        }
        self
    }

    pub fn hover(mut self, text: &'static str) -> MenuNode<A, Ctx> {
        if let MenuNode::Item { hover: slot, .. } = &mut self {
            *slot = Some(text);
        }
        self
    }

    pub fn disabled(mut self, hint: &'static str) -> MenuNode<A, Ctx> {
        if let MenuNode::Item {
            enabled,
            disabled_hint,
            ..
        } = &mut self
        {
            *enabled = false;
            *disabled_hint = Some(hint);
        }
        self
    }

    pub fn checked(mut self, f: impl Fn(&Ctx) -> bool + Send + Sync + 'static) -> MenuNode<A, Ctx> {
        if let MenuNode::Item { checked, .. } = &mut self {
            *checked = Some(Box::new(f));
        }
        self
    }

    pub fn guard_unchecked(mut self) -> MenuNode<A, Ctx> {
        if let MenuNode::Item {
            guard_unchecked, ..
        } = &mut self
        {
            *guard_unchecked = true;
        }
        self
    }

    pub fn right_text(mut self, text: &'static str) -> MenuNode<A, Ctx> {
        if let MenuNode::Item {
            right_text: slot, ..
        } = &mut self
        {
            *slot = Some(text);
        }
        self
    }

    pub fn close_menu(mut self) -> MenuNode<A, Ctx> {
        if let MenuNode::Item { close_menu, .. } = &mut self {
            *close_menu = true;
        }
        self
    }
}

/// App-specific rendering hooks a [`MenuNode`] tree needs but that this crate
/// can't provide generically: icon lookup/drawing and shortcut-label
/// resolution (command-registry override lookups, platform-specific
/// reformatting like macOS's ⌘/⌥/⇧ glyphs).
///
/// Deliberately **not** generic over `Ctx`: a `MenuHost` is built fresh each
/// frame by the caller (so it can freely borrow that frame's icon atlas /
/// key-override table), whereas `Ctx` itself must stay an owned, lifetime-free
/// type so a `Vec<MenuNode<A, Ctx>>` can be cached once (e.g. behind a
/// `LazyLock`) across frames -- see each consuming app's `menu_tree` module
/// for why that caching matters (id/label leaking happens once, not per
/// frame).
pub trait MenuHost {
    /// Resolve the display string for an item's shortcut, given its
    /// `command_id` (registry-backed, override-aware) or `literal_shortcut`
    /// (placeholder string) -- exactly one is normally `Some`.
    fn shortcut_label(&self, command_id: Option<&str>, literal_shortcut: Option<&str>) -> String;

    /// Draw one menu item's row (icon + label + shortcut, left-to-right) and
    /// return its `Response`, respecting `enabled`.
    fn item_button(
        &self,
        ui: &mut egui::Ui,
        enabled: bool,
        icon: Option<&str>,
        label: &str,
        shortcut: &str,
    ) -> egui::Response;
}

const MENU_BAR_USED_ID: &str = "spatial_ui_kit_menu_bar_used_until_leave";
const MENU_BAR_NOTICE_ID: &str = "spatial_ui_kit_menu_bar_hide_notice_sent";
const MENU_BAR_REVEAL_EDGE: f32 = 4.0;

/// Whether a menu bar should be drawn and whether this is the first frame it
/// became hidden in this egui session. Hosts that combine the menu with
/// other chrome can use the same policy without adopting [`render_menu_bar`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuBarVisibility {
    pub visible: bool,
    pub first_hidden_notice: bool,
}

/// Shared hide/reveal policy for menu bars. Full-screen status comes from the
/// native viewport, so this does not invent an application full-screen mode.
/// The menu returns when Alt is held or the pointer reaches the top edge; a
/// selected menu item hides it until the pointer leaves that edge.
pub fn menu_bar_visibility(ctx: &egui::Context, hide_in_full_screen: bool) -> MenuBarVisibility {
    let fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
    if !fullscreen || !hide_in_full_screen {
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(MENU_BAR_USED_ID), false));
        return MenuBarVisibility {
            visible: true,
            first_hidden_notice: false,
        };
    }

    let (edge_top, alt_down) = ctx.input(|i| {
        let pointer = i.pointer.hover_pos();
        let top = i.viewport().inner_rect.map_or(0.0, |rect| rect.top());
        (
            pointer.is_some_and(|p| p.y <= top + MENU_BAR_REVEAL_EDGE),
            i.modifiers.alt,
        )
    });
    let popup_open = egui::Popup::is_any_open(ctx);
    let used_id = egui::Id::new(MENU_BAR_USED_ID);
    let used: bool = ctx.data(|d| d.get_temp(used_id)).unwrap_or(false);
    let used = if !edge_top && !alt_down && !popup_open {
        false
    } else {
        used
    };
    ctx.data_mut(|d| d.insert_temp(used_id, used));
    let visible = !used && (edge_top || alt_down) || popup_open;
    let notice_id = egui::Id::new(MENU_BAR_NOTICE_ID);
    let notified: bool = ctx.data(|d| d.get_temp(notice_id)).unwrap_or(false);
    let first_hidden_notice = !visible && !notified;
    if first_hidden_notice {
        ctx.data_mut(|d| d.insert_temp(notice_id, true));
    }
    MenuBarVisibility {
        visible,
        first_hidden_notice,
    }
}

/// Call after a menu selection to hide a revealed bar until the pointer
/// leaves the top edge (or the user reveals it again with Alt).
pub fn note_menu_bar_item_used(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(MENU_BAR_USED_ID), true));
}

fn colorize(label: &str, color: &MenuColor) -> egui::RichText {
    match color {
        MenuColor::Default => egui::RichText::new(label),
        MenuColor::Yellow => egui::RichText::new(label).color(egui::Color32::from_rgb(255, 255, 0)),
        MenuColor::Blue => egui::RichText::new(label).color(egui::Color32::from_rgb(100, 150, 255)),
    }
}

fn render_node<A: Clone, Ctx>(
    ui: &mut egui::Ui,
    ctx: &Ctx,
    host: &impl MenuHost,
    node: &MenuNode<A, Ctx>,
    actions: &mut Vec<A>,
) {
    match node {
        MenuNode::Heading(text) => {
            ui.label(*text);
        }
        MenuNode::Separator => {
            ui.separator();
        }
        MenuNode::Row(children) => {
            ui.horizontal(|ui| {
                for child in children {
                    render_node(ui, ctx, host, child, actions);
                }
            });
        }
        MenuNode::Submenu {
            label,
            color,
            children,
        } => {
            ui.menu_button(colorize(label, color), |ui| {
                for child in children {
                    render_node(ui, ctx, host, child, actions);
                }
            });
        }
        MenuNode::Item {
            label,
            action,
            command_id,
            literal_shortcut,
            icon,
            hover,
            enabled,
            disabled_hint,
            checked,
            guard_unchecked,
            right_text,
            close_menu,
            ..
        } => {
            if let Some(rt) = right_text {
                // Cosmetic disabled row that looks like it opens a submenu
                // (e.g. an "Undo History" placeholder) -- not expressible
                // via `MenuHost::item_button`.
                let resp = ui.add_enabled(*enabled, egui::Button::new(*label).right_text(*rt));
                if let Some(hint) = disabled_hint {
                    resp.on_disabled_hover_text(*hint);
                }
                return;
            }

            if let Some(is_checked) = checked {
                let mut val = is_checked(ctx);
                let resp = ui.checkbox(&mut val, *label);
                if resp.clicked() && (!*guard_unchecked || !is_checked(ctx)) {
                    actions.push(action.clone());
                }
                return;
            }

            let shortcut = host.shortcut_label(*command_id, *literal_shortcut);
            let resp = host.item_button(ui, *enabled, *icon, label, &shortcut);
            let resp = if let Some(hint) = disabled_hint {
                resp.on_disabled_hover_text(*hint)
            } else if let Some(text) = hover {
                resp.on_hover_text(*text)
            } else {
                resp
            };
            if resp.clicked() {
                actions.push(action.clone());
                if *close_menu {
                    ui.close();
                }
            }
        }
    }
}

// ---- Open menu joins its menu-bar title (TASK-224) --------------------------

/// Corner radius of the title tab's top corners.
const TAB_RADIUS: f32 = 6.0;
/// Radius of the small inverse curve where the tab meets the wider card.
const TAB_FILLET: f32 = 5.0;

/// The outline of an open menu's tab, from the card's left edge up round the
/// tab and down to the card's top edge, with the inverse curve where it meets
/// the wider card: bottom-left, up, rounded top-left, across, rounded
/// top-right, down, then the fillet out onto the card top. `card_top` is the
/// y of the card's top edge (the bottom of the bar).
pub fn tab_outline(tab: egui::Rect, card_top: f32) -> Vec<egui::Pos2> {
    let mut points = vec![egui::pos2(tab.left(), card_top)];
    let r = TAB_RADIUS
        .min(tab.width() / 2.0)
        .min((card_top - tab.top()).max(0.0));
    let arc = |center: egui::Pos2, from: f32, to: f32, out: &mut Vec<egui::Pos2>| {
        const STEPS: usize = 6;
        for i in 0..=STEPS {
            let a = from + (to - from) * i as f32 / STEPS as f32;
            out.push(center + egui::vec2(a.cos(), a.sin()) * r);
        }
    };
    // Up the left edge, then round the top-left corner.
    points.push(egui::pos2(tab.left(), tab.top() + r));
    arc(
        egui::pos2(tab.left() + r, tab.top() + r),
        std::f32::consts::PI,
        std::f32::consts::PI * 1.5,
        &mut points,
    );
    points.push(egui::pos2(tab.right() - r, tab.top()));
    arc(
        egui::pos2(tab.right() - r, tab.top() + r),
        std::f32::consts::PI * 1.5,
        std::f32::consts::TAU,
        &mut points,
    );
    // Down the right edge to the fillet, then out onto the card's top edge.
    let f = TAB_FILLET.min((card_top - tab.top()).max(0.0));
    points.push(egui::pos2(tab.right(), card_top - f));
    let center = egui::pos2(tab.right() + f, card_top - f);
    for i in 0..=6 {
        let a = std::f32::consts::PI - std::f32::consts::FRAC_PI_2 * i as f32 / 6.0;
        points.push(center + egui::vec2(a.cos(), a.sin()) * f);
    }
    points
}

/// A top-level menu-bar title whose open card joins it as one outline (TASK-224):
/// the open title becomes the card's tab (bold, drawn in the card's fill), the
/// card's top-left corner is square under the tab, and the card starts at the
/// bottom of the bar, never overlapping it. `bar` is the menu bar's rectangle
/// (the title spans its full height so the card has no gap); `bar_fill` is the
/// bar's background. Closed, it hovers like any title. Behaves as
/// `egui::containers::menu::MenuButton` otherwise (hover switches between
/// open titles, same close rules).
pub fn title_menu<R>(
    ui: &mut egui::Ui,
    label: &str,
    bar: egui::Rect,
    bar_fill: egui::Color32,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> (egui::Response, Option<egui::InnerResponse<R>>) {
    // The title is as tall as the bar, so the card opens at the bar's bottom edge.
    // Placed on the bar explicitly (not wherever the row's layout would centre
    // it), so it is exactly as tall as the bar whatever hosts it.
    let slot = egui::Rect::from_min_max(
        egui::pos2(ui.cursor().min.x, bar.top()),
        egui::pos2(ui.max_rect().right().max(ui.cursor().min.x), bar.bottom()),
    );
    let (open, response, inner) = ui
        .scope_builder(
            egui::UiBuilder::new()
                .max_rect(slot)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
            |ui| {
                let id = ui.next_auto_id();
                let open = egui::Popup::is_id_open(ui.ctx(), id.with("popup"));
                let rich = if open {
                    egui::RichText::new(label).strong()
                } else {
                    egui::RichText::new(label)
                };
                let button = egui::Button::new(rich)
                    .frame(false)
                    .min_size(egui::vec2(0.0, bar.height()));
                let (response, inner) =
                    egui::containers::menu::MenuButton::from_button(button).ui(ui, content);
                (open, response, inner)
            },
        )
        .inner;

    let visuals = ui.visuals().clone();
    let font = egui::TextStyle::Button.resolve(ui.style());
    if !open && response.hovered() {
        // The ordinary hover pill, inset from the bar's top and bottom.
        let pill = response.rect.shrink2(egui::vec2(0.0, 5.0));
        ui.painter()
            .rect_filled(pill, 5.0, visuals.widgets.hovered.weak_bg_fill);
        ui.painter().text(
            response.rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            font.clone(),
            visuals.widgets.hovered.fg_stroke.color,
        );
    }
    if let Some(inner) = &inner {
        let card = inner.response.rect;
        let fill = visuals.window_fill();
        let stroke = visuals.window_stroke();
        let painter = ui.ctx().layer_painter(inner.response.layer_id);
        let card_top = card.top();
        let tab = egui::Rect::from_min_max(
            egui::pos2(card.left(), bar.top() + 3.0),
            egui::pos2(response.rect.right().max(card.left() + 8.0), card_top),
        );
        // Square the card's top-left corner under the tab.
        let corner = egui::Rect::from_min_size(card.left_top(), egui::vec2(10.0, 10.0));
        painter.rect_filled(corner, 0.0, fill);
        // The card's left edge, straight from the tab down.
        painter.line_segment(
            [card.left_top(), card.left_bottom() - egui::vec2(0.0, 8.0)],
            stroke,
        );
        // The tab itself, in the card's fill.
        painter.rect_filled(
            tab,
            egui::CornerRadius {
                nw: TAB_RADIUS as u8,
                ne: TAB_RADIUS as u8,
                sw: 0,
                se: 0,
            },
            fill,
        );
        // Open the seam: cover the card's top stroke under the tab...
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(tab.left() + stroke.width, card_top - 1.0),
                egui::pos2(tab.right() - stroke.width, card_top + stroke.width + 0.5),
            ),
            0.0,
            fill,
        );
        // ...and the stretch the inverse curve replaces, with the bar's own colour.
        painter.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(tab.right() + stroke.width, card_top - 1.0),
                egui::pos2(tab.right() + TAB_FILLET, card_top + stroke.width * 0.5),
            ),
            0.0,
            bar_fill,
        );
        painter.add(egui::Shape::line(tab_outline(tab, card_top), stroke));
        painter.text(
            egui::pos2(tab.center().x, (tab.top() + card_top) / 2.0),
            egui::Align2::CENTER_CENTER,
            label,
            font,
            visuals.strong_text_color(),
        );
    }
    (response, inner)
}

/// Render an in-window egui menu bar from a `MenuNode` tree's top-level
/// entries (each expected to be a `Submenu`, matching how both apps build
/// their trees today) and return the actions triggered this frame.
pub fn render_menu_bar<A: Clone, Ctx>(
    ui: &mut egui::Ui,
    tree: &[MenuNode<A, Ctx>],
    ctx: &Ctx,
    host: &impl MenuHost,
) -> Vec<A> {
    let mut actions: Vec<A> = Vec::new();

    egui::Panel::top("menu_bar").show(ui, |ui| {
        egui::MenuBar::new().ui(ui, |ui| {
            for node in tree {
                if let MenuNode::Submenu {
                    label,
                    color,
                    children,
                } = node
                {
                    ui.menu_button(colorize(label, color), |ui| {
                        for child in children {
                            render_node(ui, ctx, host, child, &mut actions);
                        }
                    });
                }
            }
        });
    });

    actions
}

/// Render [`render_menu_bar`] with the shared full-screen hide/reveal policy.
/// Existing callers keep the old always-visible behavior until they opt in.
pub fn render_menu_bar_with_visibility<A: Clone, Ctx>(
    ui: &mut egui::Ui,
    tree: &[MenuNode<A, Ctx>],
    ctx: &Ctx,
    host: &impl MenuHost,
    hide_in_full_screen: bool,
) -> (MenuBarVisibility, Vec<A>) {
    let visibility = menu_bar_visibility(ui.ctx(), hide_in_full_screen);
    if !visibility.visible {
        return (visibility, Vec::new());
    }
    let actions = render_menu_bar(ui, tree, ctx, host);
    if !actions.is_empty() {
        note_menu_bar_item_used(ui.ctx());
    }
    (visibility, actions)
}

#[cfg(test)]
mod visibility_tests {
    use super::*;
    use egui::{Event, Modifiers, Pos2, Rect, Vec2, ViewportId};

    fn frame(
        ctx: &egui::Context,
        pointer: Option<Pos2>,
        alt_down: bool,
        mut draw: impl FnMut(&egui::Context),
    ) -> MenuBarVisibility {
        let mut input = egui::RawInput::default();
        let viewport = input.viewports.get_mut(&ViewportId::ROOT).unwrap();
        viewport.fullscreen = Some(true);
        viewport.inner_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)));
        input.screen_rect = viewport.inner_rect;
        input.modifiers = Modifiers {
            alt: alt_down,
            ..Default::default()
        };
        if let Some(pos) = pointer {
            input.events.push(Event::PointerMoved(pos));
        } else {
            input.events.push(Event::PointerGone);
        }
        let mut visibility = MenuBarVisibility {
            visible: false,
            first_hidden_notice: false,
        };
        let _ = ctx.run_ui(input, |ui| {
            visibility = menu_bar_visibility(ui.ctx(), true);
            draw(ui.ctx());
        });
        visibility
    }

    #[test]
    fn menu_bar_hides_once_reveals_and_hides_after_selection_until_leave() {
        let ctx = egui::Context::default();
        let first = frame(&ctx, Some(Pos2::new(100.0, 20.0)), false, |_| {});
        assert!(!first.visible);
        assert!(first.first_hidden_notice);

        let revealed = frame(&ctx, Some(Pos2::new(100.0, 1.0)), false, |_| {});
        assert!(revealed.visible);
        assert!(!revealed.first_hidden_notice);

        let used = frame(
            &ctx,
            Some(Pos2::new(100.0, 1.0)),
            false,
            note_menu_bar_item_used,
        );
        assert!(used.visible);
        let hidden = frame(&ctx, Some(Pos2::new(100.0, 20.0)), false, |_| {});
        assert!(!hidden.visible);

        let alt_revealed = frame(&ctx, Some(Pos2::new(100.0, 20.0)), true, |_| {});
        assert!(alt_revealed.visible);
    }

    #[test]
    fn menu_bar_is_always_visible_when_setting_is_off_or_view_is_not_fullscreen() {
        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&ViewportId::ROOT)
            .unwrap()
            .fullscreen = Some(false);
        let mut state = MenuBarVisibility {
            visible: false,
            first_hidden_notice: false,
        };
        let _ = ctx.run_ui(input, |ui| state = menu_bar_visibility(ui.ctx(), true));
        assert_eq!(
            state,
            MenuBarVisibility {
                visible: true,
                first_hidden_notice: false,
            }
        );

        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&ViewportId::ROOT)
            .unwrap()
            .fullscreen = Some(true);
        let _ = ctx.run_ui(input, |ui| {
            assert!(menu_bar_visibility(ui.ctx(), false).visible)
        });
    }
}

#[cfg(test)]
mod title_menu_tests {
    use super::*;

    /// The tab outline starts on the card's left edge at the card top, rises
    /// along the tab's left edge, runs across and down its right edge, and ends
    /// on the card top just past the tab (the inverse curve).
    #[test]
    fn the_tab_outline_runs_round_the_tab_and_onto_the_card_top() {
        let tab = egui::Rect::from_min_max(egui::pos2(100.0, 3.0), egui::pos2(160.0, 32.0));
        let points = tab_outline(tab, 32.0);
        assert_eq!(points[0], egui::pos2(100.0, 32.0));
        let top = points.iter().map(|p| p.y).fold(f32::INFINITY, f32::min);
        assert!((top - 3.0).abs() < 1e-3, "reaches the tab top: {top}");
        let last = *points.last().unwrap();
        assert!(
            (last.y - 32.0).abs() < 1e-3,
            "ends on the card top: {last:?}"
        );
        assert!(last.x > 160.0, "and past the tab's right edge");
        assert!(
            points.iter().all(|p| p.x >= 100.0 - 1e-3),
            "never left of the card edge"
        );
    }

    fn click(pos: egui::Pos2) -> Vec<egui::Event> {
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        vec![egui::Event::PointerMoved(pos), button(true), button(false)]
    }

    /// The open card starts at the bottom of the bar, not below a gap.
    #[test]
    fn the_open_card_starts_at_the_bottom_of_the_bar() {
        let ctx = egui::Context::default();
        let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 32.0));
        let mut card_top = None;
        let frames = [
            (1.0, vec![]),
            (2.0, click(egui::pos2(14.0, 16.0))),
            (3.0, vec![]),
            (4.0, vec![]),
        ];
        for (t, events) in frames {
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400.0, 300.0),
                    )),
                    time: Some(t),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::MenuBar::new().ui(ui, |ui| {
                        let (_, inner) = title_menu(ui, "File", bar, egui::Color32::BLACK, |ui| {
                            ui.label("Open")
                        });
                        if let Some(inner) = inner {
                            card_top = Some(inner.response.rect.top());
                        }
                    });
                },
            );
        }
        let top = card_top.expect("the menu opened");
        assert!(
            (top - bar.bottom()).abs() < 1.5,
            "card top {top} vs bar bottom {}",
            bar.bottom()
        );
    }
}
