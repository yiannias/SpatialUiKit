//! Declarative menu tree, generic over an app-supplied action type `A` and
//! context type `Ctx`.
//!
//! Ported from SpatialSketchPad's `ssp-ui::menu_tree` + `chrome::menu_bar`
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
