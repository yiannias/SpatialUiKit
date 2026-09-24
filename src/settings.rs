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
    /// Drawn as a sub-setting of the field above it (label indented) --
    /// e.g. Text Size under Interface Scale, whose value it refines.
    pub nested: bool,
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
    /// A horizontal slider for a numeric range. Emits the action only when the
    /// value changes (`response.changed()`) to avoid setting the same value
    /// repeatedly during a no-op drag. For Interface Scale specifically: to avoid
    /// rescaling the UI under the cursor mid-drag, emits only on drag release or
    /// when changed by keyboard/click, not during the drag itself.
    Slider {
        value: ReadFn<Ctx, f32>,
        range: std::ops::RangeInclusive<f32>,
        step: f32,
        suffix: &'static str,
        on_change: Box<dyn Fn(f32) -> A + Send + Sync>,
    },
    /// A read-only value -- no widget, just text in the control column, for
    /// rows that report something (GPU adapter name, backend, ...) rather
    /// than edit it. Added so those rows can live in the same label/control
    /// grid as everything else instead of a hand-rolled `Custom` row that
    /// couldn't share the grid's column alignment.
    Static { value: ReadFn<Ctx, String> },
    /// Like `Dropdown`, but the option list is computed from `Ctx` each frame
    /// instead of being a fixed `&'static [&'static str]` -- for a picker
    /// whose choices come from runtime data (e.g. the GPU adapters actually
    /// present on this machine). `on_change` gets both the chosen label and
    /// `Ctx`, since recovering the underlying value from the label alone
    /// often needs looking the label back up in that same runtime data.
    DynamicDropdown {
        value: ReadFn<Ctx, String>,
        options: Box<dyn Fn(&Ctx) -> Vec<String> + Send + Sync>,
        on_change: Box<dyn Fn(&str, &Ctx) -> A + Send + Sync>,
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
            nested: false,
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

    pub fn slider(
        id: &'static str,
        label: &'static str,
        value: impl Fn(&Ctx) -> f32 + Send + Sync + 'static,
        range: std::ops::RangeInclusive<f32>,
        step: f32,
        suffix: &'static str,
        on_change: impl Fn(f32) -> A + Send + Sync + 'static,
    ) -> SettingsNode<A, Ctx> {
        SettingsNode::Field(Self::base(
            id,
            label,
            FieldControl::Slider {
                value: Box::new(value),
                range,
                step,
                suffix,
                on_change: Box::new(on_change),
            },
        ))
    }

    pub fn static_text(
        id: &'static str,
        label: &'static str,
        value: impl Fn(&Ctx) -> String + Send + Sync + 'static,
    ) -> SettingsNode<A, Ctx> {
        SettingsNode::Field(Self::base(
            id,
            label,
            FieldControl::Static {
                value: Box::new(value),
            },
        ))
    }

    pub fn dynamic_dropdown(
        id: &'static str,
        label: &'static str,
        value: impl Fn(&Ctx) -> String + Send + Sync + 'static,
        options: impl Fn(&Ctx) -> Vec<String> + Send + Sync + 'static,
        on_change: impl Fn(&str, &Ctx) -> A + Send + Sync + 'static,
    ) -> SettingsNode<A, Ctx> {
        SettingsNode::Field(Self::base(
            id,
            label,
            FieldControl::DynamicDropdown {
                value: Box::new(value),
                options: Box::new(options),
                on_change: Box::new(on_change),
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

    /// Marks this field as a refinement of the field above it; see [`Field::nested`].
    pub fn nested(mut self) -> Self {
        if let SettingsNode::Field(f) = &mut self {
            f.nested = true;
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
///
/// A fully-rounded pill with the icon *inside* the field, rather than a bare
/// `egui::TextEdit` with an emoji glued on beside it -- Chris, 2026-08-30,
/// comparing it unfavorably to 1Password's Windows search field: "much less
/// comfortable and nice." `corner_radius` = half the field height (a true
/// pill, not `os_style`'s flat 4px control radius -- search fields read as
/// a distinct, softer affordance in most native UIs, this one included).
pub fn render_search(ui: &mut egui::Ui, query: &mut String) {
    // Deliberately no `set_min_height`/`horizontal_centered` -- either
    // stretches to whatever height the surrounding container currently
    // offers, and since that container (a `Panel::top` in SDB's settings
    // panel) remembers *this* frame's content height for the *next*
    // frame's available space, the two compound into runaway growth every
    // frame (caught live, 2026-08-30: the field grew to fill the entire
    // nav pane within seconds). Padding via `inner_margin` alone gives the
    // same taller, more comfortable look without depending on available
    // height at all -- the field's size is purely intrinsic (icon + text
    // line + fixed margin), so there's nothing to feed back into.
    egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(ui.visuals().widgets.inactive.bg_stroke)
        .corner_radius(15)
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("\u{1f50d}")
                        .size(13.0)
                        .color(ui.visuals().weak_text_color()),
                );
                ui.add(
                    egui::TextEdit::singleline(query)
                        .hint_text("Search")
                        .desired_width(f32::INFINITY)
                        .frame(egui::Frame::NONE)
                        .vertical_align(egui::Align::Center),
                );
            });
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

/// Fixed width of the control column -- every control (and the help/hint
/// text under it) renders at this width, rather than shrink-wrapping to
/// whatever the widest one happens to be. Matches the ribbon pods' control
/// sizing language (docs/design/2026-09-24_ribbon-pods-spec.md): a
/// consistent control width reads as one system rather than a pile of
/// independently-sized widgets. Also what stops help text from ever running
/// the full width of the window: everything in the control column, wrapped
/// text included, is laid out inside a `Ui` this wide.
const CONTROL_COL_WIDTH: f32 = 240.0;

/// One "row unit" derived from the current body text size rather than a bare
/// pixel constant, so every space/gap in the panel keeps its proportions as
/// Interface Scale or Text Size change (Text Size only touches font metrics,
/// not spacing -- see `theme::apply_text_scale` -- so anything that should
/// track it has to be computed from a text height, not hardcoded).
fn row_unit(ui: &egui::Ui) -> f32 {
    ui.text_style_height(&egui::TextStyle::Body)
}

/// Secondary/help text color with enough contrast against the panel
/// background to stay readable (targets WCAG >= 4.5:1) -- egui's own
/// `weak_text_color()` is tuned for de-emphasis against arbitrary content
/// and skews too faint for a full sentence of help text. Splits dark/light
/// rather than reading `weak_text_color()`, matching the ribbon pods'
/// secondary-text value in dark mode (`#7F8088`) and a comparably-toned
/// mid-grey in light mode.
fn help_text_color(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_rgb(0x7F, 0x80, 0x88)
    } else {
        egui::Color32::from_rgb(0x5B, 0x5D, 0x66)
    }
}

/// A disabled row's hint should read as clearly dimmer than ordinary help
/// text, not just the same weak color reused -- otherwise a disabled field
/// and an enabled one with a hover hint look identically "quiet" and the eye
/// has nothing to tell them apart with.
fn disabled_hint_color(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_rgb(0x5A, 0x5B, 0x60)
    } else {
        egui::Color32::from_rgb(0xA8, 0xA9, 0xAE)
    }
}

/// Primary label color -- brighter than egui's default body text in a few
/// visuals presets, and stated explicitly here (rather than left to
/// `ui.visuals().text_color()`) so field labels are always the panel's
/// brightest, most legible text, per the readability pass this module went
/// through (labels were reading as dim grey against help text that was
/// nearly as bright).
fn label_color(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::WHITE
    } else {
        egui::Color32::from_rgb(0x10, 0x10, 0x14)
    }
}

/// Tier header: "Application Settings" / "User-Specific Settings" / "Project
/// Settings" -- the top level of the settings tree's three-level hierarchy
/// (tier / section / group). Large, bold, no box, generous space above, and
/// a thin full-width rule below it, so a tier reads as a hard break rather
/// than another box the same weight as everything under it.
fn tier_header(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let unit = row_unit(ui);
    ui.add_space(unit * 1.6);
    let resp = ui.label(
        egui::RichText::new(label)
            .size(unit * 1.55)
            .strong()
            .color(label_color(ui)),
    );
    ui.add_space(unit * 0.5);
    let rule_rect = ui.available_rect_before_wrap();
    ui.painter().hline(
        rule_rect.x_range(),
        rule_rect.top(),
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
    ui.add_space(unit * 0.7);
    resp
}

/// Section header: "Themes", "Visual Fidelity", "GPU & Rendering" -- medium
/// semibold, no box, one level down from a tier.
fn section_header(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let unit = row_unit(ui);
    ui.add_space(unit * 1.0);
    let resp = ui.label(
        egui::RichText::new(label)
            .size(unit * 1.15)
            .strong()
            .color(label_color(ui)),
    );
    ui.add_space(unit * 0.45);
    resp
}

/// Group header: "Selection Highlight", "Spacing" -- smaller semibold, in
/// the help-text color rather than the label color, so it reads as a quiet
/// subdivision of the section above it instead of competing with it.
fn group_header(ui: &mut egui::Ui, label: &str) {
    let unit = row_unit(ui);
    ui.add_space(unit * 0.6);
    ui.label(
        egui::RichText::new(label)
            .size(unit * 0.85)
            .strong()
            .color(help_text_color(ui)),
    );
    ui.add_space(unit * 0.3);
}

/// Render the single continuously-scrolling content pane: every `Section`
/// and `Group` as a heading followed by its fields, all in one column. When
/// `scroll_to` names a `Section` id, that heading's rect is scrolled into
/// view this frame (see the sketch's "spatial continuity" rationale in
/// `docs/Sketches/Prefs Panel.md` -- clicking the nav walks you to a
/// section, it doesn't isolate it).
///
/// Three heading weights, one shared grid per tier: a tier
/// (Application-Wide/User/Project) gets [`tier_header`]; a nested `Section`
/// (Themes, GPU & Rendering, ...) gets [`section_header`]; a `Group`
/// (Selection Highlight, Spacing, ...) gets [`group_header`]. All the
/// `Field` rows anywhere under one tier -- across every nested `Section` and
/// `Group` -- share a single `egui::Grid` keyed by that tier's id, so the
/// label column and control column land at the same x for the whole tier,
/// not just within whichever run of fields happened to sit next to each
/// other. (A fresh `Grid::new` call with the same id resumes that id's
/// column widths from whatever the last call already measured, so
/// interleaving headings and `Custom` rows between grid calls doesn't reset
/// alignment.)
pub fn render_content<A: Clone, Ctx>(
    ui: &mut egui::Ui,
    ctx: &Ctx,
    host: &impl SettingsHost,
    tree: &[SettingsNode<A, Ctx>],
    scroll_to: Option<&'static str>,
    actions: &mut Vec<A>,
) {
    for node in tree {
        match node {
            SettingsNode::Section {
                id,
                label,
                children,
            } => {
                let resp = tier_header(ui, label);
                if scroll_to == Some(*id) {
                    ui.scroll_to_rect(resp.rect, Some(egui::Align::TOP));
                }
                render_section_body(ui, ctx, host, id, children, scroll_to, actions);
            }
            // Each app's tree root is a flat list of tier `Section`s, so
            // `Group`/`Field`/`Custom` shouldn't appear here in practice --
            // handled via the same body renderer, keyed to a fallback grid
            // id, for robustness.
            _ => render_section_body(
                ui,
                ctx,
                host,
                "settings_root",
                std::slice::from_ref(node),
                scroll_to,
                actions,
            ),
        }
    }
}

/// Walks one tier's subtree, rendering nested `Section`/`Group` headings
/// inline and feeding every `Field` run into the one `egui::Grid` keyed by
/// `grid_id` (see [`render_content`]'s doc comment).
fn render_section_body<A: Clone, Ctx>(
    ui: &mut egui::Ui,
    ctx: &Ctx,
    host: &impl SettingsHost,
    grid_id: &'static str,
    tree: &[SettingsNode<A, Ctx>],
    scroll_to: Option<&'static str>,
    actions: &mut Vec<A>,
) {
    let mut i = 0;
    while i < tree.len() {
        match &tree[i] {
            SettingsNode::Section {
                id,
                label,
                children,
            } => {
                let resp = section_header(ui, label);
                if scroll_to == Some(*id) {
                    ui.scroll_to_rect(resp.rect, Some(egui::Align::TOP));
                }
                render_section_body(ui, ctx, host, grid_id, children, scroll_to, actions);
                i += 1;
            }
            SettingsNode::Group { label, children } => {
                group_header(ui, label);
                render_section_body(ui, ctx, host, grid_id, children, scroll_to, actions);
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
                let unit = row_unit(ui);
                egui::Grid::new(grid_id)
                    .num_columns(2)
                    .spacing([unit * 1.3, unit * 0.5])
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
    let label_col = label_color(ui);
    ui.add_enabled_ui(field.enabled, |ui| {
        ui.scope(|ui| {
            ui.set_min_width(190.0);
            let text = egui::RichText::new(field.label).color(label_col);
            if field.nested {
                ui.horizontal(|ui| {
                    ui.add_space(row_unit(ui) * 1.1);
                    ui.label(text);
                });
            } else {
                ui.label(text);
            }
        });
    });

    ui.add_enabled_ui(field.enabled, |ui| {
        ui.vertical(|ui| {
            ui.set_width(CONTROL_COL_WIDTH);
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
                        .add(egui::TextEdit::singleline(&mut val).desired_width(CONTROL_COL_WIDTH))
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
                        .width(CONTROL_COL_WIDTH)
                        .show_ui(ui, |ui| {
                            for opt in *options {
                                if ui.selectable_label(current == *opt, *opt).clicked() {
                                    actions.push(on_change(opt));
                                }
                            }
                        });
                }
                FieldControl::DynamicDropdown {
                    value,
                    options,
                    on_change,
                } => {
                    let current = value(ctx);
                    let opts = options(ctx);
                    egui::ComboBox::from_id_salt(field.id)
                        .selected_text(current.clone())
                        .width(CONTROL_COL_WIDTH)
                        .show_ui(ui, |ui| {
                            for opt in &opts {
                                if ui.selectable_label(&current == opt, opt).clicked() {
                                    actions.push(on_change(opt, ctx));
                                }
                            }
                        });
                }
                FieldControl::Static { value } => {
                    ui.add(
                        egui::Label::new(egui::RichText::new(value(ctx)).color(label_col)).wrap(),
                    );
                }
                FieldControl::Path {
                    value,
                    on_change,
                    on_browse,
                } => {
                    ui.horizontal(|ui| {
                        let mut val = value(ctx);
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut val)
                                    .desired_width(CONTROL_COL_WIDTH - 28.0),
                            )
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
                FieldControl::Slider {
                    value,
                    range,
                    step,
                    suffix,
                    on_change,
                } => {
                    // Mid-drag the value lives in egui memory, not in the
                    // app: applying Interface Scale while dragging would
                    // rescale the UI under the cursor. The saved value is
                    // only read when no drag is in progress.
                    let drag_id = egui::Id::new(("settings_slider_drag", field.id));
                    let mut val = ui
                        .data(|d| d.get_temp::<f32>(drag_id))
                        .unwrap_or_else(|| value(ctx));
                    let response = ui.add(
                        egui::Slider::new(&mut val, range.clone())
                            .step_by(*step as f64)
                            .suffix(*suffix),
                    );
                    if response.dragged() {
                        ui.data_mut(|d| d.insert_temp(drag_id, val));
                    } else if response.drag_stopped() {
                        ui.data_mut(|d| d.remove::<f32>(drag_id));
                        actions.push(on_change(val));
                    } else if response.changed() {
                        // Keyboard or a single click on the track.
                        actions.push(on_change(val));
                    }
                }
            }

            if let Some(hint) = field.disabled_hint {
                if !field.enabled {
                    ui.add_space(row_unit(ui) * 0.15);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(hint)
                                .size(row_unit(ui) * 0.85)
                                .color(disabled_hint_color(ui)),
                        )
                        .wrap(),
                    );
                }
            } else if let Some(text) = field.hover {
                ui.add_space(row_unit(ui) * 0.15);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(text)
                            .size(row_unit(ui) * 0.85)
                            .color(help_text_color(ui)),
                    )
                    .wrap(),
                );
            }
        });
    });

    ui.end_row();
}
