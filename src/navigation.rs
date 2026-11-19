//! Shared tree rows for navigators whose contents come from app-owned data.
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
    context_actions: Vec<ContextAction>,
    children: Vec<Self>,
}

#[derive(Clone, Copy)]
enum RowKind {
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
    /// Builds a major branch row, such as a document or symbol category.
    pub fn branch(label: impl Into<String>, count: usize, children: Vec<Self>) -> Self {
        Self {
            label: label.into(),
            count: Some(count),
            kind: RowKind::Branch,
            key: None,
            enabled: false,
            selected: false,
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
            context_actions: Vec::new(),
            children: Vec::new(),
        }
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
    render_nodes(ui, nodes, &mut event, &mut rows);
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
/// user asked to toggle it.
fn disclosure_row(ui: &mut egui::Ui, label: egui::RichText, open: bool) -> bool {
    ui.horizontal(|ui| {
        let side = ui.spacing().interact_size.y;
        let (rect, chevron) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click());
        let size = side * 0.22;
        let c = rect.center();
        let points = if open {
            [
                c + egui::vec2(-size, -size * 0.5),
                c + egui::vec2(0.0, size * 0.5),
                c + egui::vec2(size, -size * 0.5),
            ]
        } else {
            [
                c + egui::vec2(-size * 0.5, -size),
                c + egui::vec2(size * 0.5, 0.0),
                c + egui::vec2(-size * 0.5, size),
            ]
        };
        ui.painter().add(egui::Shape::line(
            points.to_vec(),
            egui::Stroke::new(1.5, ui.visuals().weak_text_color()),
        ));
        let text = label.text().to_owned();
        let name = ui.add(egui::Label::new(label).sense(egui::Sense::click()));
        chevron.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::Button, true, open, text.clone())
        });
        let mut toggled = chevron.clicked() || name.clicked();
        if chevron.has_focus() {
            let key = |k| ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, k));
            toggled |= (!open && key(egui::Key::ArrowRight)) || (open && key(egui::Key::ArrowLeft));
        }
        toggled
    })
    .inner
}

fn render_nodes<K: Clone>(
    ui: &mut egui::Ui,
    nodes: &[NavigationNode<K>],
    event: &mut Option<NavigationEvent<K>>,
    rows: &mut Vec<ItemRow<K>>,
) {
    for (index, node) in nodes.iter().enumerate() {
        ui.push_id(index, |ui| match node.kind {
            RowKind::Branch | RowKind::Group => {
                let label = match (node.kind, node.count) {
                    (RowKind::Branch, Some(count)) => {
                        egui::RichText::new(format!("{}  ({count})", node.label)).strong()
                    }
                    (_, Some(count)) => egui::RichText::new(format!("{}  ({count})", node.label)),
                    _ => egui::RichText::new(node.label.clone()),
                };
                // A disclosure row: chevron + label. Click, Enter or Space
                // toggles; Left folds and Right opens while it has focus.
                let open_id = ui.id().with("sdb_nav_open");
                let mut open = ui
                    .memory(|m| m.data.get_temp::<bool>(open_id))
                    .unwrap_or(true);
                let toggled = disclosure_row(ui, label, open);
                if toggled {
                    open = !open;
                    ui.memory_mut(|m| m.data.insert_temp(open_id, open));
                }
                if open {
                    ui.indent(&node.label, |ui| {
                        render_nodes(ui, &node.children, event, rows)
                    });
                }
            }
            RowKind::Item => {
                let Some(key) = node.key.as_ref() else {
                    return;
                };
                let response = ui.add_enabled(
                    node.enabled,
                    egui::Button::selectable(node.selected, &node.label).frame_when_inactive(false),
                );
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
            }
            RowKind::Hint => {
                ui.weak(&node.label);
            }
        });
    }
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
        let tree = vec![NavigationNode::branch(
            "Marks",
            1,
            vec![NavigationNode::item(1, "Closed arrow", false, true)],
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
}
