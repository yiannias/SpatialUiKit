//! Shared tree rows for navigators whose contents come from app-owned data.
//!
//! The look follows Chris's Symbol Editor mock-up (2026-11-17, chosen for
//! every tree on 2026-11-27, SDB TASK-409): only the groups leading to the open
//! item start expanded, thin guide lines join each row to its parent, the open
//! item is underlined in the accent, and a dot after a name marks an unapplied
//! change. Colours come from the theme (`interface_tokens`), never literals.
//!
//! Settings has a domain-specific declarative tree in [`crate::settings`],
//! while editors often build trees from live project data. This small renderer
//! shares the indentation, counts, selection rows, and row context menus
//! without making the kit depend on either data model.

/// The action reported by [`render_tree`] for a selectable tree row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NavigationEvent<K> {
    /// The user selected an item row.
    Select(K),
    /// The user chose a context-menu action on an item row.
    Context { key: K, action: &'static str },
}

/// One row in an app-built navigation tree.
///
/// Use [`branch`](Self::branch) and [`group`](Self::group) for headings and
/// [`item`](Self::item) for selectable leaves. Counts belong to headings;
/// context actions belong to item rows.
pub struct NavigationNode<K> {
    label: String,
    count: Option<usize>,
    kind: RowKind,
    key: Option<K>,
    enabled: bool,
    selected: bool,
    /// The item has unapplied changes: a dot after its name.
    modified: bool,
    context_actions: Vec<ContextAction>,
    children: Vec<Self>,
}

#[derive(Clone, Copy)]
enum RowKind {
    /// A boxed top-level row, such as "This drawing" or a library.
    Root,
    Branch,
    Group,
    Item,
    Hint,
}

struct ContextAction {
    label: &'static str,
    enabled: bool,
}

impl<K> NavigationNode<K> {
    /// Builds a boxed top-level row (a document or a loaded library) holding
    /// branches.
    pub fn root(label: impl Into<String>, count: usize, children: Vec<Self>) -> Self {
        Self {
            kind: RowKind::Root,
            ..Self::branch(label, count, children)
        }
    }

    /// Builds a major branch row, such as a document or symbol category.
    pub fn branch(label: impl Into<String>, count: usize, children: Vec<Self>) -> Self {
        Self {
            label: label.into(),
            count: Some(count),
            kind: RowKind::Branch,
            key: None,
            enabled: false,
            selected: false,
            modified: false,
            context_actions: Vec::new(),
            children,
        }
    }

    /// Builds a grouping row beneath a branch.
    pub fn group(label: impl Into<String>, count: usize, children: Vec<Self>) -> Self {
        Self {
            label: label.into(),
            count: Some(count),
            kind: RowKind::Group,
            key: None,
            enabled: false,
            selected: false,
            modified: false,
            context_actions: Vec::new(),
            children,
        }
    }

    /// Builds a selectable leaf. `enabled` lets an editor lock other rows
    /// while a selected item has unapplied changes.
    pub fn item(key: K, label: impl Into<String>, selected: bool, enabled: bool) -> Self {
        Self {
            label: label.into(),
            count: None,
            kind: RowKind::Item,
            key: Some(key),
            enabled,
            selected,
            modified: false,
            context_actions: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Builds a quiet informational row, for example a branch's empty-search
    /// message. It cannot be selected and has no children.
    pub fn hint(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            count: None,
            kind: RowKind::Hint,
            key: None,
            enabled: false,
            selected: false,
            modified: false,
            context_actions: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Marks an item as having unapplied changes: a dot after its name (and
    /// ", unapplied changes" for screen readers).
    pub fn modified(mut self, modified: bool) -> Self {
        self.modified = modified;
        self
    }

    /// Whether this row is, or contains, the selected item.
    fn holds_selection(&self) -> bool {
        self.selected || self.children.iter().any(Self::holds_selection)
    }

    /// Adds an item context-menu command. The returned event carries this
    /// stable action label and the same key as the row.
    pub fn context_action(mut self, label: &'static str, enabled: bool) -> Self {
        self.context_actions.push(ContextAction { label, enabled });
        self
    }
}

/// Draws a generic navigation tree and returns the first action performed
/// this frame. Each nested level is indented; heading rows display their
/// optional counts, while item rows use egui's selectable button style.
pub fn render_tree<K: Clone>(
    ui: &mut egui::Ui,
    nodes: &[NavigationNode<K>],
) -> Option<NavigationEvent<K>> {
    let mut event = None;
    let mut rows = Vec::new();
    let any_selected = nodes.iter().any(NavigationNode::holds_selection);
    render_nodes(ui, nodes, any_selected, &mut event, &mut rows);
    // Arrow keys (TASK-168 row 1): with one of the tree's rows focused,
    // Up / Down / PageUp / PageDown / Home / End move the selection through
    // the enabled item rows, and focus follows it.
    if event.is_none() && rows.iter().any(|row| row.focused) {
        let keys = crate::list_nav::consume_keys(ui);
        if keys.any() {
            let nav: Vec<usize> = (0..rows.len()).filter(|&i| rows[i].enabled).collect();
            let current = rows
                .iter()
                .position(|row| row.focused)
                .or_else(|| rows.iter().position(|row| row.selected));
            let page = nav.len().min(8) as isize;
            let next = crate::list_nav::apply(&nav, current, keys, page);
            if let Some(next) = next.filter(|next| Some(*next) != current) {
                ui.memory_mut(|m| m.request_focus(rows[next].id));
                if !rows[next].selected {
                    event = Some(NavigationEvent::Select(rows[next].key.clone()));
                }
            }
        }
    }
    // Typing letters with a row focused jumps to the next row starting with
    // them (shared type-to-find: prefix, pause reset, repeated letter cycles).
    if event.is_none() && rows.iter().any(|row| row.focused) {
        let typed: String = ui.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Text(t) => Some(t.as_str()),
                    _ => None,
                })
                .collect()
        });
        let id = ui.id().with("sdb_tree_type_find");
        let mut find: crate::type_find::TypeFind =
            ui.memory(|m| m.data.get_temp(id)).unwrap_or_default();
        let now = ui.input(|i| i.time);
        if find.feed(&typed, now) {
            let texts: Vec<&str> = rows
                .iter()
                .map(|r| if r.enabled { r.label.as_str() } else { "" })
                .collect();
            let current = rows.iter().position(|r| r.focused);
            if let Some(next) = find.find(&texts, current).filter(|n| Some(*n) != current) {
                ui.memory_mut(|m| m.request_focus(rows[next].id));
                if !rows[next].selected {
                    event = Some(NavigationEvent::Select(rows[next].key.clone()));
                }
            }
        }
        ui.memory_mut(|m| m.data.insert_temp(id, find));
    }
    // egui itself moves focus between rows on the arrow keys; the selection
    // follows whichever row has focus.
    if event.is_none() {
        if let Some(row) = rows
            .iter()
            .find(|row| row.focused && !row.selected && row.enabled)
        {
            event = Some(NavigationEvent::Select(row.key.clone()));
        }
    }
    event
}

/// What the tree remembers about one item row while drawing, for keyboard
/// navigation.
struct ItemRow<K> {
    key: K,
    id: egui::Id,
    enabled: bool,
    selected: bool,
    focused: bool,
    label: String,
}

/// The chevron-and-label heading of a branch or group. Returns whether the
/// user asked to toggle it, and the row's rect (for the guide lines).
fn disclosure_row(
    ui: &mut egui::Ui,
    label: egui::RichText,
    count: Option<usize>,
    open: bool,
    boxed: bool,
) -> (bool, egui::Rect) {
    let draw = |ui: &mut egui::Ui| {
        ui.horizontal(|ui| {
            let side = ui.spacing().interact_size.y;
            let (rect, chevron) =
                ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click());
            crate::glyphs::chevron(
                ui.painter(),
                egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(side * 0.786)),
                if open {
                    crate::glyphs::Dir::Down
                } else {
                    crate::glyphs::Dir::Right
                },
                ui.visuals().weak_text_color(),
            );
            let text = label.text().to_owned();
            let name = ui.add(egui::Label::new(label).sense(egui::Sense::click()));
            chevron.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::Button, true, open, text.clone())
            });
            let mut toggled = chevron.clicked() || name.clicked();
            if chevron.has_focus() {
                let key = |k| ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, k));
                toggled |=
                    (!open && key(egui::Key::ArrowRight)) || (open && key(egui::Key::ArrowLeft));
            }
            // The count is a plain weak number at the row's right edge.
            if let Some(count) = count {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(count.to_string()).weak());
                });
            }
            toggled
        })
    };
    if boxed {
        let shown = egui::Frame::new()
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(4, 2))
            .show(ui, draw);
        (shown.inner.inner, shown.response.rect)
    } else {
        let row = draw(ui);
        (row.inner, row.response.rect)
    }
}

/// The colour of the guide lines: the theme's frame grey, or the plain
/// non-interactive outline for the built-in Light look.
fn guide_color(ui: &egui::Ui) -> egui::Color32 {
    crate::interface_tokens::theme_colors(ui)
        .map_or(ui.visuals().widgets.noninteractive.bg_stroke.color, |c| {
            c.frame
        })
}

/// The accent the open item is drawn in: the theme's highlight (the host
/// publishes the user's own interface highlight into it), or the platform
/// link colour for the built-in Light look.
fn accent_color(ui: &egui::Ui) -> egui::Color32 {
    crate::interface_tokens::theme_colors(ui).map_or(ui.visuals().hyperlink_color, |c| c.highlight)
}

/// Draws the guides from a heading to its open children: one vertical line
/// down from under the heading's chevron, and a short tick into each child.
fn paint_guides(ui: &egui::Ui, heading: egui::Rect, children: &[egui::Rect]) {
    let Some(last) = children.last() else {
        return;
    };
    let side = ui.spacing().interact_size.y;
    let x = (heading.left() + side / 2.0).round() + 0.5;
    let stroke = egui::Stroke::new(1.0, guide_color(ui));
    let painter = ui.painter();
    painter.line_segment(
        [
            egui::pos2(x, heading.bottom()),
            egui::pos2(x, last.center().y),
        ],
        stroke,
    );
    for child in children {
        let y = child.center().y.round() + 0.5;
        painter.line_segment(
            [egui::pos2(x, y), egui::pos2(child.left() - 2.0, y)],
            stroke,
        );
    }
}

/// Draws `nodes` and returns each one's row rect, for the parent's guides.
/// `any_selected` is whether the whole tree has a selected item: with none,
/// the first top-level row opens so the tree is never all folded.
fn render_nodes<K: Clone>(
    ui: &mut egui::Ui,
    nodes: &[NavigationNode<K>],
    any_selected: bool,
    event: &mut Option<NavigationEvent<K>>,
    rows: &mut Vec<ItemRow<K>>,
) -> Vec<egui::Rect> {
    let mut rects = Vec::new();
    for (index, node) in nodes.iter().enumerate() {
        let rect = ui.push_id(index, |ui| match node.kind {
            RowKind::Root | RowKind::Branch | RowKind::Group => {
                let label = match node.kind {
                    RowKind::Root | RowKind::Branch => {
                        egui::RichText::new(node.label.clone()).strong()
                    }
                    _ => egui::RichText::new(node.label.clone()),
                };
                // A disclosure row: chevron + label. Click, Enter or Space
                // toggles; Left folds and Right opens while it has focus.
                let open_id = ui.id().with("sdb_nav_open");
                // Only the groups leading to the open item start expanded
                // (Chris's Symbol Editor mock-up); the user's own toggles are
                // remembered over that.
                let default_open = node.holds_selection()
                    || (!any_selected && index == 0 && matches!(node.kind, RowKind::Root));
                let mut open = ui
                    .memory(|m| m.data.get_temp::<bool>(open_id))
                    .unwrap_or(default_open);
                let (toggled, heading) = disclosure_row(
                    ui,
                    label,
                    node.count,
                    open,
                    matches!(node.kind, RowKind::Root),
                );
                if toggled {
                    open = !open;
                    ui.memory_mut(|m| m.data.insert_temp(open_id, open));
                }
                if open {
                    let children = ui
                        .indent(&node.label, |ui| {
                            render_nodes(ui, &node.children, any_selected, event, rows)
                        })
                        .inner;
                    paint_guides(ui, heading, &children);
                }
                heading
            }
            RowKind::Item => {
                let Some(key) = node.key.as_ref() else {
                    return egui::Rect::NOTHING;
                };
                // The open item reads by its accent colour *and* an
                // underline (not colour alone); there is no fill.
                let text = if node.selected {
                    egui::RichText::new(&node.label)
                        .color(accent_color(ui))
                        .underline()
                } else {
                    egui::RichText::new(&node.label)
                };
                let row = ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    let response = ui.add_enabled(
                        node.enabled,
                        egui::Button::selectable(false, text).frame_when_inactive(false),
                    );
                    if node.modified {
                        let side = ui.text_style_height(&egui::TextStyle::Body);
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(side * 0.7, side),
                            egui::Sense::hover(),
                        );
                        crate::glyphs::dot(ui.painter(), rect, ui.visuals().text_color());
                    }
                    response
                });
                let response = row.inner;
                let spoken = if node.modified {
                    format!("{}, unapplied changes", node.label)
                } else {
                    node.label.clone()
                };
                let selected = node.selected;
                let enabled = node.enabled;
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        enabled,
                        selected,
                        spoken.clone(),
                    )
                });
                rows.push(ItemRow {
                    key: key.clone(),
                    id: response.id,
                    enabled: node.enabled,
                    selected: node.selected,
                    focused: response.has_focus(),
                    label: node.label.clone(),
                });
                response.context_menu(|ui| {
                    for action in &node.context_actions {
                        if ui
                            .add_enabled(action.enabled, egui::Button::new(action.label))
                            .clicked()
                        {
                            *event = Some(NavigationEvent::Context {
                                key: key.clone(),
                                action: action.label,
                            });
                            ui.close();
                        }
                    }
                });
                if response.clicked() && !node.selected {
                    *event = Some(NavigationEvent::Select(key.clone()));
                }
                row.response.rect
            }
            RowKind::Hint => ui.weak(&node.label).rect,
        });
        rects.push(rect.inner);
    }
    rects
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_nested_generic_rows_without_actions() {
        let tree = vec![NavigationNode::branch(
            "Symbols",
            1,
            vec![NavigationNode::group(
                "Leader heads",
                1,
                vec![NavigationNode::item(7, "Arrow", true, true)
                    .context_action("Revert changes", false)],
            )],
        )];
        let ctx = egui::Context::default();
        let mut event = None;
        let _ = ctx.run_ui(Default::default(), |ui| event = render_tree(ui, &tree));
        assert_eq!(event, None);
    }

    /// A root row nests branches and its label has no parenthesised count.
    #[test]
    fn root_row_holds_branches() {
        let tree = vec![NavigationNode::root(
            "This drawing",
            1,
            vec![NavigationNode::branch(
                "Marks",
                1,
                vec![NavigationNode::item(1, "Arrow", false, true)],
            )],
        )];
        let ctx = egui::Context::default();
        let mut event = None;
        let _ = ctx.run_ui(Default::default(), |ui| event = render_tree(ui, &tree));
        assert_eq!(event, None);
        let mut text = String::new();
        let out = ctx.run_ui(Default::default(), |ui| {
            let _ = render_tree(ui, &tree);
        });
        for clipped in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &clipped.shape {
                text.push_str(&t.galley.text());
            }
        }
        assert!(text.contains("This drawing"));
        assert!(!text.contains("(1)"));
    }

    fn run(
        ctx: &egui::Context,
        time: f64,
        events: Vec<egui::Event>,
        selected: usize,
    ) -> (Option<NavigationEvent<usize>>, Vec<egui::Rect>) {
        let tree: Vec<NavigationNode<usize>> = (0..4)
            .map(|i| NavigationNode::item(i, format!("Row {i}"), i == selected, i != 2))
            .collect();
        let mut event = None;
        let mut rects = Vec::new();
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(300.0, 300.0),
                )),
                time: Some(time),
                events,
                ..Default::default()
            },
            |ui| {
                event = render_tree(ui, &tree);
                rects = (0..4)
                    .map(|i| {
                        ui.min_rect()
                            .with_min_y(ui.min_rect().top() + i as f32 * 24.0)
                    })
                    .collect();
            },
        );
        (event, rects)
    }

    fn key(key: egui::Key) -> Vec<egui::Event> {
        vec![egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }]
    }

    /// With a row focused, Down/Up/Home/End move the selection through the
    /// enabled rows (the disabled row is skipped), and keys do nothing while
    /// no row of the tree has focus.
    #[test]
    fn arrow_keys_move_the_selection_through_enabled_rows() {
        let ctx = egui::Context::default();
        // No row focused yet: Down does nothing.
        let (event, _) = run(&ctx, 1.0, key(egui::Key::ArrowDown), 0);
        assert_eq!(event, None, "keys are ignored until a row has focus");
        // Tab focuses the first row.
        let (_, _) = run(&ctx, 2.0, key(egui::Key::Tab), 0);
        let (_, _) = run(&ctx, 2.5, vec![], 0);
        let (event, _) = run(&ctx, 3.0, key(egui::Key::ArrowDown), 0);
        assert_eq!(event, Some(NavigationEvent::Select(1)));
        let (_, _) = run(&ctx, 3.5, vec![], 1);
        // Down again from row 1 skips the disabled row 2 and lands on 3.
        let (event, _) = run(&ctx, 4.0, key(egui::Key::ArrowDown), 1);
        assert_eq!(event, Some(NavigationEvent::Select(3)));
        let (_, _) = run(&ctx, 4.5, vec![], 3);
        // End and Home reach the ends.
        let (event, _) = run(&ctx, 5.0, key(egui::Key::Home), 3);
        assert_eq!(event, Some(NavigationEvent::Select(0)));
    }

    /// With a row focused, typing letters jumps to the row starting with them.
    #[test]
    fn typing_jumps_to_the_row_starting_with_the_letters() {
        let ctx = egui::Context::default();
        let tree = |selected: usize| -> Vec<NavigationNode<usize>> {
            ["Arc", "Beam", "Bevel", "Circle"]
                .iter()
                .enumerate()
                .map(|(i, l)| NavigationNode::item(i, *l, i == selected, true))
                .collect()
        };
        let frame = |time: f64, events: Vec<egui::Event>, selected: usize| {
            let mut event = None;
            let t = tree(selected);
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(300.0, 300.0),
                    )),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| event = render_tree(ui, &t),
            );
            event
        };
        frame(1.0, key(egui::Key::Tab), 0);
        frame(1.5, vec![], 0);
        assert_eq!(
            frame(1.6, vec![egui::Event::Text("c".into())], 0),
            Some(NavigationEvent::Select(3))
        );
    }

    /// A branch folds and opens: with its heading focused, Left hides the rows
    /// under it (the tree gets shorter) and Right shows them again.
    #[test]
    fn a_branch_folds_and_opens() {
        let ctx = egui::Context::default();
        // Selected, so the branch starts open (TASK-409: only the path to the
        // open item starts expanded).
        let tree = vec![NavigationNode::branch(
            "Marks",
            1,
            vec![NavigationNode::item(1, "Closed arrow", true, true)],
        )];
        let frame = |time: f64, events: Vec<egui::Event>| -> f32 {
            let mut height = 0.0;
            let _ = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(300.0, 300.0),
                    )),
                    time: Some(time),
                    events,
                    ..Default::default()
                },
                |ui| {
                    let _ = render_tree(ui, &tree);
                    height = ui.min_rect().height();
                },
            );
            height
        };
        let open = frame(1.0, vec![]);
        frame(2.0, key(egui::Key::Tab));
        frame(3.0, vec![]);
        frame(4.0, key(egui::Key::ArrowLeft));
        let folded = frame(5.0, vec![]);
        assert!(folded < open, "folded {folded} < open {open}");
        frame(6.0, key(egui::Key::ArrowRight));
        let reopened = frame(7.0, vec![]);
        assert!(
            (reopened - open).abs() < 1.0,
            "reopened {reopened} vs {open}"
        );
    }

    /// TASK-409: only the groups leading to the open item start expanded; a
    /// tree with nothing selected opens its first top-level row only.
    #[test]
    fn only_the_path_to_the_open_item_starts_expanded() {
        let texts = |tree: &[NavigationNode<u32>]| {
            let ctx = egui::Context::default();
            let out = ctx.run_ui(Default::default(), |ui| {
                let _ = render_tree(ui, tree);
            });
            let mut text = String::new();
            for clipped in &out.shapes {
                collect_text(&clipped.shape, &mut text);
            }
            text
        };
        let tree = |selected: bool| {
            vec![NavigationNode::root(
                "This drawing",
                3,
                vec![
                    NavigationNode::group(
                        "Leader heads",
                        2,
                        vec![
                            NavigationNode::item(1, "Closed arrow", selected, true),
                            NavigationNode::item(2, "Open arrow", false, true).modified(true),
                        ],
                    ),
                    NavigationNode::group(
                        "Dimension ticks",
                        1,
                        vec![NavigationNode::item(3, "Oblique", false, true)],
                    ),
                ],
            )]
        };
        let shown = texts(&tree(true));
        assert!(shown.contains("Closed arrow") && shown.contains("Open arrow"));
        assert!(
            !shown.contains("Oblique"),
            "an unrelated group stays folded: {shown}"
        );
        let shown = texts(&tree(false));
        assert!(shown.contains("Leader heads") && !shown.contains("Closed arrow"));
    }

    fn collect_text(shape: &egui::Shape, out: &mut String) {
        match shape {
            egui::Shape::Text(t) => {
                out.push_str(t.galley.text());
                out.push('\n');
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| collect_text(s, out)),
            _ => {}
        }
    }
}
