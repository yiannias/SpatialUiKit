//! Declarative settings/preferences tree, generic over an app-supplied
//! action type `A` and context type `Ctx` -- same shape as [`crate::menu`].
//!
//! Concept and layout notes: `docs/Sketches/Prefs Panel.md` (and the sketch
//! it transcribes). Key requirements from that sketch this module is built
//! around:
//!
//! - One tree of data drives **two** renderings: a left-hand navigation
//!   outline (labels only) and a right-hand pane that renders every
//!   section's fields in one continuous scroll, grouped under headers.
//!   Clicking a nav entry scrolls the content pane to that section rather
//!   than switching to an isolated sub-view -- see [`render_nav`] /
//!   [`render_content`].
//! - The three settings tiers (Application-Wide, User-Specific, Project) are
//!   just top-level [`SettingsNode::Section`] entries built by each app; this
//!   crate has no opinion on tier names or which fields belong where.
//! - Each leaf field reads its live value from `Ctx` and, on edit, produces
//!   an app-supplied action `A` -- the app owns the actual state mutation,
//!   mirroring how [`crate::menu::MenuNode::Item`] fires an action rather
//!   than mutating anything itself.
//!
//! What stays app-specific: native file/folder picker dialogs for path
//! fields, via [`SettingsHost::browse_path`] -- the same reasoning
//! `menu::MenuHost` and `ribbon::RibbonHost` use for icon lookup.
//!
//! Deferred (per the sketch's own "to be investigated later" note): a
//! dedicated presentation for long tabular lists (e.g. a variable-length set
//! of library search paths) as a popup/expandable list instead of inline
//! rows. Not modeled here yet -- build repeated `Path` fields under a
//! `Group` in the meantime.

/// Reads a field's current value out of the app's context each frame.
pub type ReadFn<Ctx, T> = Box<dyn Fn(&Ctx) -> T + Send + Sync>;

pub enum SettingsNode<A, Ctx> {
    /// Top-level tier or major subsection (e.g. "Application-Wide Settings",
    /// "Auto Save & Directories"). Appears as a branch in the left-hand nav
    /// tree and a header in the right-hand content pane.
    Section {
        id: &'static str,
        label: &'static str,
        children: Vec<SettingsNode<A, Ctx>>,
    },
    /// A lighter-weight header within a `Section` that doesn't get its own
    /// nav-tree entry (e.g. "Support Directories & Paths" nested under
    /// "Auto Save & Directories") -- content-pane grouping only.
    Group {
        label: &'static str,
        children: Vec<SettingsNode<A, Ctx>>,
    },
    Field(Field<A, Ctx>),
    /// Escape hatch for a control shape the declarative `Field` kinds don't
    /// cover -- a variable-length list with per-row remove + an "add" capture
    /// flow (e.g. SDB's pan/zoom trigger chip-lists). Renders as a raw
    /// closure over the same `ui`/`ctx`/`actions` every `Field` gets; owns no
    /// state itself (apps needing capture/edit state across frames should use
    /// `ui.memory_mut` the way `Field`-based rows never need to).
    Custom {
        label: &'static str,
        render: Box<dyn Fn(&mut egui::Ui, &Ctx, &mut Vec<A>) + Send + Sync>,
    },
}

impl<A, Ctx> SettingsNode<A, Ctx> {
    pub fn custom(
        label: &'static str,
        render: impl Fn(&mut egui::Ui, &Ctx, &mut Vec<A>) + Send + Sync + 'static,
    ) -> Self {
        SettingsNode::Custom {
            label,
            render: Box::new(render),
        }
    }
    pub fn section(
        id: &'static str,
        label: &'static str,
        children: Vec<SettingsNode<A, Ctx>>,
    ) -> Self {
        SettingsNode::Section {
            id,
            label,
            children,
        }
    }

    pub fn group(label: &'static str, children: Vec<SettingsNode<A, Ctx>>) -> Self {
        SettingsNode::Group { label, children }
    }
}

/// One editable control, e.g. "AutoSave Enabled" or "Template Location".
pub struct Field<A, Ctx> {
    /// Stable id, unique across the tree (used as the egui widget id salt).
    pub id: &'static str,
    pub label: &'static str,
    pub hover: Option<&'static str>,
    pub enabled: bool,
    pub disabled_hint: Option<&'static str>,
    pub control: FieldControl<A, Ctx>,
}

/// The control kinds shown in the sketch: a toggle (AutoSave Enabled), a
/// free-text value (AutoSave Schedule, "30 min"), a dropdown (Notification
/// Method), and a path field with a browse ("...") button (AutoSave
/// Location, Template Location, Library Search Paths).
pub enum FieldControl<A, Ctx> {
    Toggle {
        value: ReadFn<Ctx, bool>,
        on_change: Box<dyn Fn(bool) -> A + Send + Sync>,
    },
    Text {
        value: ReadFn<Ctx, String>,
        on_change: Box<dyn Fn(String) -> A + Send + Sync>,
    },
    Dropdown {
        value: ReadFn<Ctx, &'static str>,
        options: &'static [&'static str],
        on_change: Box<dyn Fn(&'static str) -> A + Send + Sync>,
    },
    /// A text field plus a browse button. `on_change` handles the field
    /// being typed/pasted into directly; `on_browse` fires when the user
    /// clicks "..." and [`SettingsHost::browse_path`] returns a chosen path.
    Path {
        value: ReadFn<Ctx, String>,
        on_change: Box<dyn Fn(String) -> A + Send + Sync>,
        on_browse: Box<dyn Fn(String) -> A + Send + Sync>,
    },
    /// A single button with no editable value of its own -- for a field row
    /// that navigates elsewhere (e.g. "open the full keybindings panel")
    /// rather than reading/writing a setting.
    Action {
        button_label: &'static str,
        on_click: Box<dyn Fn(&Ctx) -> A + Send + Sync>,
    },
}

impl<A, Ctx> Field<A, Ctx> {
    fn base(id: &'static str, label: &'static str, control: FieldControl<A, Ctx>) -> Self {
        Field {
            id,
            label,
            hover: None,
            enabled: true,
            disabled_hint: None,
            control,
        }
    }

    pub fn toggle(
        id: &'static str,
        label: &'static str,
        value: impl Fn(&Ctx) -> bool + Send + Sync + 'static,
        on_change: impl Fn(bool) -> A + Send + Sync + 'static,
    ) -> SettingsNode<A, Ctx> {
        SettingsNode::Field(Self::base(
            id,
            label,
            FieldControl::Toggle {
                value: Box::new(value),
                on_change: Box::new(on_change),
            },
        ))
    }

    pub fn text(
        id: &'static str,
        label: &'static str,
        value: impl Fn(&Ctx) -> String + Send + Sync + 'static,
        on_change: impl Fn(String) -> A + Send + Sync + 'static,
    ) -> SettingsNode<A, Ctx> {
        SettingsNode::Field(Self::base(
            id,
            label,
            FieldControl::Text {
                value: Box::new(value),
                on_change: Box::new(on_change),
            },
        ))
    }

    pub fn dropdown(
        id: &'static str,
        label: &'static str,
        value: impl Fn(&Ctx) -> &'static str + Send + Sync + 'static,
        options: &'static [&'static str],
        on_change: impl Fn(&'static str) -> A + Send + Sync + 'static,
    ) -> SettingsNode<A, Ctx> {
        SettingsNode::Field(Self::base(
            id,
            label,
            FieldControl::Dropdown {
                value: Box::new(value),
                options,
                on_change: Box::new(on_change),
            },
        ))
    }

    pub fn action(
        id: &'static str,
        label: &'static str,
        button_label: &'static str,
        on_click: impl Fn(&Ctx) -> A + Send + Sync + 'static,
    ) -> SettingsNode<A, Ctx> {
        SettingsNode::Field(Self::base(
            id,
            label,
            FieldControl::Action {
                button_label,
                on_click: Box::new(on_click),
            },
        ))
    }

    pub fn path(
        id: &'static str,
        label: &'static str,
        value: impl Fn(&Ctx) -> String + Send + Sync + 'static,
        on_change: impl Fn(String) -> A + Send + Sync + 'static,
        on_browse: impl Fn(String) -> A + Send + Sync + 'static,
    ) -> SettingsNode<A, Ctx> {
        SettingsNode::Field(Self::base(
            id,
            label,
            FieldControl::Path {
                value: Box::new(value),
                on_change: Box::new(on_change),
                on_browse: Box::new(on_browse),
            },
        ))
    }
}

impl<A, Ctx> SettingsNode<A, Ctx> {
    pub fn hover(mut self, text: &'static str) -> Self {
        if let SettingsNode::Field(f) = &mut self {
            f.hover = Some(text);
        }
        self
    }

    pub fn disabled(mut self, hint: &'static str) -> Self {
        if let SettingsNode::Field(f) = &mut self {
            f.enabled = false;
            f.disabled_hint = Some(hint);
        }
        self
    }
}

/// App-specific rendering hook a settings tree needs but this crate can't
/// provide generically: the native file/folder picker behind a `Path`
/// field's "..." button. Mirrors `menu::MenuHost` / `ribbon::RibbonHost`.
pub trait SettingsHost {
    /// Draw the "..." browse button next to a path field and, if clicked,
    /// run the app's native picker. Returns `Some(new_path)` the frame the
    /// user confirms a choice.
    fn browse_path(&self, ui: &mut egui::Ui, current: &str) -> Option<String>;
}

/// Search box for the nav pane, pinned above the tree per the sketch's "🔍
/// Search" box. The app owns `query`'s storage (e.g. `egui::Memory`'s
/// per-frame temp data, keyed by a stable `Id` -- no persistence to disk
/// needed, it's ephemeral UI state); this just draws the widget. Feed the
/// same string into [`render_nav`]'s `filter` param.
pub fn render_search(ui: &mut egui::Ui, query: &mut String) {
    ui.horizontal(|ui| {
        ui.label("\u{1f50d}");
        ui.add(
            egui::TextEdit::singleline(query)
                .hint_text("Search")
                .desired_width(f32::INFINITY),
        );
    });
}

fn label_contains(label: &str, filter_lower: &str) -> bool {
    label.to_ascii_lowercase().contains(filter_lower)
}

/// Whether `node` or any of its descendants (by label) matches `filter_lower`
/// (already-lowercased) -- used by [`render_nav`] to decide whether a
/// `Section` row (or one of its ancestors) should still show while filtering.
fn node_matches<A, Ctx>(node: &SettingsNode<A, Ctx>, filter_lower: &str) -> bool {
    match node {
        SettingsNode::Section {
            label, children, ..
        }
        | SettingsNode::Group { label, children } => {
            label_contains(label, filter_lower)
                || children.iter().any(|c| node_matches(c, filter_lower))
        }
        SettingsNode::Field(f) => label_contains(f.label, filter_lower),
        SettingsNode::Custom { label, .. } => label_contains(label, filter_lower),
    }
}

/// Render the left-hand navigation outline for a settings tree: one row per
/// `Section` (nested sections indent), skipping `Group`/`Field` entries so
/// the nav stays a table of contents rather than mirroring every control.
/// Returns the `id` of the section the user clicked this frame, if any --
/// the caller feeds that into [`render_content`]'s `scroll_to` to jump the
/// content pane there.
///
/// `filter`: when non-empty, a `Section` row (and its subtree) only shows if
/// its own label or some descendant's label contains it (case-insensitive).
/// The content pane is deliberately *not* filtered the same way -- it stays
/// the sketch's single continuous document; search only narrows which nav
/// rows you can jump from, matching the sketch's own "search box narrows
/// the tree" framing rather than hiding content.
pub fn render_nav<A, Ctx>(
    ui: &mut egui::Ui,
    tree: &[SettingsNode<A, Ctx>],
    filter: &str,
) -> Option<&'static str> {
    let filter_lower = filter.to_ascii_lowercase();
    render_nav_filtered(ui, tree, &filter_lower)
}

fn render_nav_filtered<A, Ctx>(
    ui: &mut egui::Ui,
    tree: &[SettingsNode<A, Ctx>],
    filter_lower: &str,
) -> Option<&'static str> {
    let mut clicked = None;
    for node in tree {
        if let SettingsNode::Section {
            id,
            label,
            children,
        } = node
        {
            if !filter_lower.is_empty() && !node_matches(node, filter_lower) {
                continue;
            }
            if ui.selectable_label(false, *label).clicked() {
                clicked = Some(*id);
            }
            ui.indent(*id, |ui| {
                if let Some(child_clicked) = render_nav_filtered(ui, children, filter_lower) {
                    clicked = Some(child_clicked);
                }
            });
        }
    }
    clicked
}

/// A bordered, content-sized header box, matching the sketch's rounded-rect
/// boxes around each section/group header in the content pane (as opposed
/// to a plain heading label spanning the full width). Returns the frame's
/// response so callers can `scroll_to_rect` it.
fn framed_header(ui: &mut egui::Ui, label: &str, text_size: f32) -> egui::Response {
    egui::Frame::new()
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(label).strong().size(text_size));
        })
        .response
}

/// Draws a rounded "elbow" tree connector from the vertical trunk line at
/// `trunk_x` out to a child header vertically centered at `target_y`, then a
/// straight run over to `target_x` -- a small quarter-circle-ish bend
/// (cubic Bézier, control points at the standard ~0.5523*r circle-arc
/// offset) instead of a sharp right angle, per the "little bend" look Chris
/// asked for (2026-07-21) over egui's default straight `indent_has_left_vline`.
fn draw_elbow(
    painter: &egui::Painter,
    trunk_x: f32,
    trunk_top: f32,
    target_y: f32,
    target_x: f32,
    stroke: egui::Stroke,
) {
    const K: f32 = 0.5523;
    let r = 8.0_f32
        .min((target_x - trunk_x - 2.0).max(1.0))
        .min((target_y - trunk_top).max(1.0));
    let bend_start = egui::pos2(trunk_x, target_y - r);
    let bend_end = egui::pos2(trunk_x + r, target_y);
    let bezier = egui::epaint::CubicBezierShape::from_points_stroke(
        [
            bend_start,
            egui::pos2(trunk_x, target_y - r + r * K),
            egui::pos2(trunk_x + r - r * K, target_y),
            bend_end,
        ],
        false,
        egui::Color32::TRANSPARENT,
        stroke,
    );
    painter.add(bezier);
    painter.line_segment([bend_end, egui::pos2(target_x, target_y)], stroke);
}

/// Indents `children` under a parent Section/Group header, one nesting
/// level, drawing a vertical trunk line down to the last immediate child
/// Section/Group header and a curved elbow connector out to each one --
/// replaces egui's plain `Ui::indent` (straight vline, sharp corner) with
/// the tree-outline look. Fields don't get their own branch (too busy --
/// they're already visually grouped by `render_content`'s shared grid).
fn render_indented<A: Clone, Ctx>(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    ctx: &Ctx,
    host: &impl SettingsHost,
    children: &[SettingsNode<A, Ctx>],
    scroll_to: Option<&'static str>,
    actions: &mut Vec<A>,
) {
    let indent = ui.spacing().indent;
    let mut child_rect = ui.available_rect_before_wrap();
    child_rect.min.x += indent;
    let top_y = child_rect.min.y;
    let trunk_x = child_rect.min.x - indent * 0.5;

    let mut child_ui = ui.new_child(egui::UiBuilder::new().id_salt(id_salt).max_rect(child_rect));
    let mut branch_ys: Vec<f32> = Vec::new();
    render_content_impl(
        &mut child_ui,
        ctx,
        host,
        children,
        scroll_to,
        actions,
        &mut branch_ys,
    );
    let child_min_rect = child_ui.min_rect();

    if let Some(&last) = branch_ys.last() {
        let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        let painter = ui.painter();
        painter.line_segment(
            [egui::pos2(trunk_x, top_y), egui::pos2(trunk_x, last)],
            stroke,
        );
        for &y in &branch_ys {
            draw_elbow(painter, trunk_x, top_y, y, child_rect.min.x, stroke);
        }
    }

    ui.advance_cursor_after_rect(child_min_rect);
}

/// Render the single continuously-scrolling content pane: every `Section`
/// and `Group` as a header followed by its fields, all in one column. When
/// `scroll_to` names a `Section` id, that header's rect is scrolled into
/// view this frame (see the sketch's "spatial continuity" rationale in
/// `docs/Sketches/Prefs Panel.md` -- clicking the nav walks you to a
/// section, it doesn't isolate it).
///
/// Formatting matters here as much as content: a tree meant to hold dozens
/// to hundreds of fields is unusable if every field is its own free-floating
/// `label + widget` line with no shared alignment. So runs of consecutive
/// sibling [`SettingsNode::Field`]s are batched into a single two-column
/// `egui::Grid` (label column, control column) -- every field in that run
/// lines up, rather than each one picking its own label width. A `Group` or
/// `Section` breaks the run (and starts its own grid for its own fields).
///
/// Top-level entries (the three tiers -- Application-Wide/User/Project) each
/// get a full grey bordered frame around header+content, not just the
/// header box every nested Section/Group gets -- makes each tier read as
/// its own card.
pub fn render_content<A: Clone, Ctx>(
    ui: &mut egui::Ui,
    ctx: &Ctx,
    host: &impl SettingsHost,
    tree: &[SettingsNode<A, Ctx>],
    scroll_to: Option<&'static str>,
    actions: &mut Vec<A>,
) {
    for node in tree {
        // Fixed width for every top-level tier card, computed once from the
        // content pane's available width (rather than each `Frame` shrink-
        // wrapping its own content) so all three tiers line up, and pulled
        // in a bit from the scroll area's right edge so the card doesn't
        // crowd/underlap the scrollbar.
        const SCROLLBAR_GUTTER: f32 = 14.0;
        let card_width = (ui.available_width() - SCROLLBAR_GUTTER).max(200.0);
        match node {
            SettingsNode::Section {
                id,
                label,
                children,
            } => {
                ui.add_space(10.0);
                egui::Frame::new()
                    .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                    .corner_radius(6)
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_width(card_width - 24.0); // minus the inner_margin above
                        let resp = framed_header(ui, label, 16.0);
                        if scroll_to == Some(*id) {
                            ui.scroll_to_rect(resp.rect, Some(egui::Align::TOP));
                        }
                        ui.add_space(8.0);
                        render_indented(ui, *id, ctx, host, children, scroll_to, actions);
                    });
                ui.add_space(18.0);
            }
            // Each app's tree root is a flat list of tier `Section`s, so
            // `Group`/`Field` shouldn't appear here in practice -- handled
            // via the plain (unframed) renderer for robustness.
            _ => render_content_impl(
                ui,
                ctx,
                host,
                std::slice::from_ref(node),
                scroll_to,
                actions,
                &mut Vec::new(),
            ),
        }
    }
}

fn render_content_impl<A: Clone, Ctx>(
    ui: &mut egui::Ui,
    ctx: &Ctx,
    host: &impl SettingsHost,
    tree: &[SettingsNode<A, Ctx>],
    scroll_to: Option<&'static str>,
    actions: &mut Vec<A>,
    branch_ys: &mut Vec<f32>,
) {
    let mut i = 0;
    while i < tree.len() {
        match &tree[i] {
            SettingsNode::Section {
                id,
                label,
                children,
            } => {
                ui.add_space(10.0);
                let resp = framed_header(ui, *label, 16.0);
                branch_ys.push(resp.rect.center().y);
                if scroll_to == Some(*id) {
                    ui.scroll_to_rect(resp.rect, Some(egui::Align::TOP));
                }
                ui.add_space(8.0);
                render_indented(ui, *id, ctx, host, children, scroll_to, actions);
                ui.add_space(18.0);
                i += 1;
            }
            SettingsNode::Group { label, children } => {
                ui.add_space(6.0);
                let resp = framed_header(ui, *label, 14.0);
                branch_ys.push(resp.rect.center().y);
                ui.add_space(6.0);
                render_indented(ui, *label, ctx, host, children, scroll_to, actions);
                ui.add_space(12.0);
                i += 1;
            }
            SettingsNode::Custom { render, .. } => {
                render(ui, ctx, actions);
                i += 1;
            }
            SettingsNode::Field(_) => {
                let start = i;
                while i < tree.len() && matches!(tree[i], SettingsNode::Field(_)) {
                    i += 1;
                }
                let grid_id = match &tree[start] {
                    SettingsNode::Field(f) => f.id,
                    _ => unreachable!(),
                };
                egui::Grid::new(grid_id)
                    .num_columns(2)
                    .spacing([18.0, 10.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for node in &tree[start..i] {
                            if let SettingsNode::Field(field) = node {
                                render_field_row(ui, ctx, host, field, actions);
                            }
                        }
                    });
            }
        }
    }
}

fn render_field_row<A: Clone, Ctx>(
    ui: &mut egui::Ui,
    ctx: &Ctx,
    host: &impl SettingsHost,
    field: &Field<A, Ctx>,
    actions: &mut Vec<A>,
) {
    ui.add_enabled_ui(field.enabled, |ui| {
        ui.scope(|ui| {
            ui.set_min_width(190.0);
            ui.label(field.label);
        });
    });

    ui.add_enabled_ui(field.enabled, |ui| {
        ui.vertical(|ui| {
            match &field.control {
                FieldControl::Toggle { value, on_change } => {
                    let mut val = value(ctx);
                    if ui.checkbox(&mut val, "").changed() {
                        actions.push(on_change(val));
                    }
                }
                FieldControl::Text { value, on_change } => {
                    let mut val = value(ctx);
                    if ui
                        .add(egui::TextEdit::singleline(&mut val).desired_width(220.0))
                        .changed()
                    {
                        actions.push(on_change(val));
                    }
                }
                FieldControl::Dropdown {
                    value,
                    options,
                    on_change,
                } => {
                    let current = value(ctx);
                    egui::ComboBox::from_id_salt(field.id)
                        .selected_text(current)
                        .width(220.0)
                        .show_ui(ui, |ui| {
                            for opt in *options {
                                if ui.selectable_label(current == *opt, *opt).clicked() {
                                    actions.push(on_change(opt));
                                }
                            }
                        });
                }
                FieldControl::Path {
                    value,
                    on_change,
                    on_browse,
                } => {
                    ui.horizontal(|ui| {
                        let mut val = value(ctx);
                        if ui
                            .add(egui::TextEdit::singleline(&mut val).desired_width(180.0))
                            .changed()
                        {
                            actions.push(on_change(val));
                        }
                        // `browse_path` opens a blocking native file dialog --
                        // must only fire on an actual "..." button click, not
                        // every frame this field renders (a bug this fixed:
                        // the dialog previously reopened continuously the
                        // instant the field was on screen, since there was no
                        // button here gating the call at all).
                        if ui.button("...").clicked() {
                            if let Some(picked) = host.browse_path(ui, &value(ctx)) {
                                actions.push(on_browse(picked));
                            }
                        }
                    });
                }
                FieldControl::Action {
                    button_label,
                    on_click,
                } => {
                    if ui.button(*button_label).clicked() {
                        actions.push(on_click(ctx));
                    }
                }
            }

            if let Some(hint) = field.disabled_hint {
                if !field.enabled {
                    ui.label(egui::RichText::new(hint).small().weak());
                }
            } else if let Some(text) = field.hover {
                ui.label(egui::RichText::new(text).small().weak());
            }
        });
    });

    ui.end_row();
}
