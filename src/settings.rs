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
}

impl<A, Ctx> SettingsNode<A, Ctx> {
    pub fn section(
        id: &'static str,
        label: &'static str,
        children: Vec<SettingsNode<A, Ctx>>,
    ) -> Self {
        SettingsNode::Section { id, label, children }
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

/// Render the left-hand navigation outline for a settings tree: one row per
/// `Section` (nested sections indent), skipping `Group`/`Field` entries so
/// the nav stays a table of contents rather than mirroring every control.
/// Returns the `id` of the section the user clicked this frame, if any --
/// the caller feeds that into [`render_content`]'s `scroll_to` to jump the
/// content pane there.
pub fn render_nav<A, Ctx>(
    ui: &mut egui::Ui,
    tree: &[SettingsNode<A, Ctx>],
) -> Option<&'static str> {
    let mut clicked = None;
    for node in tree {
        if let SettingsNode::Section { id, label, children } = node {
            if ui.selectable_label(false, *label).clicked() {
                clicked = Some(*id);
            }
            ui.indent(*id, |ui| {
                if let Some(child_clicked) = render_nav(ui, children) {
                    clicked = Some(child_clicked);
                }
            });
        }
    }
    clicked
}

/// Render the single continuously-scrolling content pane: every `Section`
/// and `Group` as a header followed by its fields, all in one column. When
/// `scroll_to` names a `Section` id, that header's rect is scrolled into
/// view this frame (see the sketch's "spatial continuity" rationale in
/// `docs/Sketches/Prefs Panel.md` -- clicking the nav walks you to a
/// section, it doesn't isolate it).
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
            SettingsNode::Section { id, label, children } => {
                let resp = ui.heading(*label);
                if scroll_to == Some(*id) {
                    ui.scroll_to_rect(resp.rect, Some(egui::Align::TOP));
                }
                render_content(ui, ctx, host, children, scroll_to, actions);
                ui.add_space(12.0);
            }
            SettingsNode::Group { label, children } => {
                ui.strong(*label);
                render_content(ui, ctx, host, children, scroll_to, actions);
                ui.add_space(6.0);
            }
            SettingsNode::Field(field) => render_field(ui, ctx, host, field, actions),
        }
    }
}

fn render_field<A: Clone, Ctx>(
    ui: &mut egui::Ui,
    ctx: &Ctx,
    host: &impl SettingsHost,
    field: &Field<A, Ctx>,
    actions: &mut Vec<A>,
) {
    ui.horizontal(|ui| {
        ui.add_enabled_ui(field.enabled, |ui| {
            ui.label(field.label);
            match &field.control {
                FieldControl::Toggle { value, on_change } => {
                    let mut val = value(ctx);
                    if ui.checkbox(&mut val, "").changed() {
                        actions.push(on_change(val));
                    }
                }
                FieldControl::Text { value, on_change } => {
                    let mut val = value(ctx);
                    if ui.text_edit_singleline(&mut val).changed() {
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
                    let mut val = value(ctx);
                    if ui.text_edit_singleline(&mut val).changed() {
                        actions.push(on_change(val));
                    }
                    if let Some(picked) = host.browse_path(ui, &value(ctx)) {
                        actions.push(on_browse(picked));
                    }
                }
            }
        });
    });

    if let Some(hint) = field.disabled_hint {
        if !field.enabled {
            ui.label(hint);
        }
    } else if let Some(text) = field.hover {
        ui.label(text);
    }
}
