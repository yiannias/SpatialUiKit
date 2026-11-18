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
    render_nodes(ui, nodes, &mut event);
    event
}

fn render_nodes<K: Clone>(
    ui: &mut egui::Ui,
    nodes: &[NavigationNode<K>],
    event: &mut Option<NavigationEvent<K>>,
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
                ui.label(label);
                ui.indent(&node.label, |ui| render_nodes(ui, &node.children, event));
            }
            RowKind::Item => {
                let Some(key) = node.key.as_ref() else {
                    return;
                };
                let response = ui.add_enabled(
                    node.enabled,
                    egui::Button::selectable(node.selected, &node.label).frame_when_inactive(false),
                );
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
}
