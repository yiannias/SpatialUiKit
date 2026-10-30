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
    /// flow (e.g. the host app's pan/zoom trigger chip-lists). Renders as a raw
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
    /// Overrides the label's colour (the host colour-codes rows this way);
    /// `None` keeps the panel's standard label colour.
    pub label_color: Option<egui::Color32>,
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
    /// App-drawn control for a row the kit has no kind for (a colour swatch,
    /// say). It draws in the control column of the shared grid, so the row
    /// gets the same label colour, column alignment, separator and help text
    /// as every other field -- unlike a `SettingsNode::Custom`, which is
    /// drawn outside the grid.
    Custom {
        render: Box<dyn Fn(&mut egui::Ui, &Ctx, &mut Vec<A>) + Send + Sync>,
    },
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
            label_color: None,
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

    /// A field whose control is drawn by the app; see [`FieldControl::Custom`].
    pub fn custom(
        id: &'static str,
        label: &'static str,
        render: impl Fn(&mut egui::Ui, &Ctx, &mut Vec<A>) + Send + Sync + 'static,
    ) -> SettingsNode<A, Ctx> {
        SettingsNode::Field(Self::base(
            id,
            label,
            FieldControl::Custom {
                render: Box::new(render),
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

    /// Colours every field label under this node with `color_of(field id)`
    /// (`None` leaves a label at the standard colour).
    pub fn tint_labels(&mut self, color_of: &dyn Fn(&'static str) -> Option<egui::Color32>) {
        match self {
            SettingsNode::Field(f) => f.label_color = color_of(f.id),
            SettingsNode::Section { children, .. } | SettingsNode::Group { children, .. } => {
                for c in children {
                    c.tint_labels(color_of);
                }
            }
            SettingsNode::Custom { .. } => {}
        }
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

    /// Whether field names carry a right-click menu this frame (default: no).
    fn label_menu_enabled(&self, _ctx: &egui::Context) -> bool {
        false
    }

    /// Contents of the right-click menu on the field named `field_id`; only
    /// called while [`Self::label_menu_enabled`] is true.
    fn label_context_menu(&self, _ui: &mut egui::Ui, _field_id: &'static str) {}

    /// A fixed label-column width, in body-text heights, shared by every tier
    /// (default: `None`, each tier sizes the column to its widest label).
    /// With `Some`, the column does not change width when the reader moves
    /// between tiers, and a label wider than it wraps onto further lines.
    fn label_column_units(&self) -> Option<f32> {
        None
    }
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
pub fn render_search_sized(ui: &mut egui::Ui, query: &mut String, height: f32) {
    // A fixed-height pill (no dependence on available height, so no feedback
    // loop): painted by hand, with the field laid out inside it.
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let v = ui.visuals();
    let radius = height / 2.0;
    ui.painter().rect_filled(rect, radius, v.extreme_bg_color);
    ui.painter().rect_stroke(
        rect,
        radius,
        v.widgets.noninteractive.bg_stroke,
        egui::StrokeKind::Inside,
    );
    let weak = v.weak_text_color();
    let mut inner = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(12.0, 0.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    inner.label(egui::RichText::new("\u{1f50d}").size(13.0).color(weak));
    inner.add(
        egui::TextEdit::singleline(query)
            .hint_text("Search")
            .desired_width(f32::INFINITY)
            .margin(egui::Margin::ZERO)
            .frame(egui::Frame::NONE)
            .vertical_align(egui::Align::Center),
    );
}

/// [`render_search_sized`] with the original, taller padding-driven height.
pub fn render_search(ui: &mut egui::Ui, query: &mut String) {
    // Deliberately no `set_min_height`/`horizontal_centered` -- either
    // stretches to whatever height the surrounding container currently
    // offers, and since that container (a `Panel::top` in the host app's settings
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

/// Fixed width of an actual control widget (dropdown/text field/slider) --
/// Chris, round 2 of the restyle: the *column* should take the content
/// pane's remaining width (see [`ColumnWidths::control`]), but a control
/// widget stretched to that width would look like a pile of full-bleed bars
/// rather than the ribbon pods' consistent control sizing language
/// (docs/design/2026-09-24_ribbon-pods-spec.md). So the column is wide but
/// each widget inside it stays this width, left-aligned in the space.
const CONTROL_WIDGET_WIDTH: f32 = 280.0;

/// The three widths [`render_field_row`] needs, computed once per tier by
/// [`measure_column_widths`] and threaded down through
/// [`render_section_body`]/[`render_content`] rather than recomputed (or
/// hardcoded) per row:
/// - `label`: the widest label in the tier, so every row's label column
///   lines up (see that function's doc comment on why this isn't done via
///   `Grid::min_col_width`).
/// - `control`: the *column's* width -- the content pane's remaining width
///   after the label column, minus a right margin -- which is what the
///   help text wraps to. Individual control widgets stay at
///   [`CONTROL_WIDGET_WIDTH`] regardless; only help text (and the
///   `Static`/read-only value) actually uses the full column.
/// - `help_cap`: a comfortable reading measure (~70 characters) that further
///   caps help text's wrap width -- a very wide content pane would otherwise
///   stretch a one-line hint into a hard-to-track full-bleed sentence.
#[derive(Clone, Copy)]
struct ColumnWidths {
    label: f32,
    control: f32,
    help_cap: f32,
}

/// One "row unit" derived from the current body text size rather than a bare
/// pixel constant, so every space/gap in the panel keeps its proportions as
/// Interface Scale or Text Size change (Text Size only touches font metrics,
/// not spacing -- see `theme::apply_text_scale` -- so anything that should
/// track it has to be computed from a text height, not hardcoded).
fn row_unit(ui: &egui::Ui) -> f32 {
    ui.text_style_height(&egui::TextStyle::Body)
}

fn density_key() -> egui::Id {
    egui::Id::new("spatial_ui_kit_settings_density")
}

/// Sets how tightly the settings content page packs its *vertical* gaps
/// (above and below headers, between rows, under a field's help text):
/// `1.0` is the shipped spacing, `0.78` removes about 22% of it. Font sizes
/// and column widths are never affected -- density is not zoom. Stored in
/// `ctx` memory like [`crate::motion::set_reduce_motion`]; an app that never
/// calls this gets exactly the old spacing, so other hosts are unchanged.
/// Clamped to `0.5..=1.5` so a bad value cannot collapse rows onto each other.
pub fn set_density(ctx: &egui::Context, density: f32) {
    ctx.data_mut(|d| d.insert_temp(density_key(), density.clamp(0.5, 1.5)));
}

fn density(ui: &egui::Ui) -> f32 {
    ui.ctx().data(|d| d.get_temp(density_key())).unwrap_or(1.0)
}

/// [`row_unit`] scaled by the density: the base of every vertical gap.
/// Font sizes keep using [`row_unit`] directly.
fn space_unit(ui: &egui::Ui) -> f32 {
    row_unit(ui) * density(ui)
}

/// The label's actual font size (as opposed to [`row_unit`], a *line
/// height*, always somewhat taller than the font itself) -- the base every
/// other text size in a row is stated as a fraction of, per Chris's round-2
/// note that help text should be "one step smaller than the labels."
fn label_font_size(ui: &egui::Ui) -> f32 {
    egui::TextStyle::Body.resolve(ui.style()).size
}

/// Help/hint text size: 0.85x the label's own font size (egui's built-in
/// `TextStyle::Small` is a fixed 10.0 regardless of body size, which
/// wouldn't track Text Size scaling the way every other size in this module
/// does).
fn help_font_size(ui: &egui::Ui) -> f32 {
    label_font_size(ui) * 0.85
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

/// Between-row separator color: deliberately very low contrast against the
/// panel surface -- a guide the eye can use to track label -> control across
/// a row, not a divider meant to be noticed on its own (that's what the
/// tier rule and section/group headers are for). Dark is `#26272C` on the
/// ribbon pods' `#1E1E23`-ish surface; light is the equivalent few-percent
/// step off that theme's surface tone.
fn row_rule_color(ui: &egui::Ui) -> egui::Color32 {
    if ui.visuals().dark_mode {
        egui::Color32::from_rgb(0x26, 0x27, 0x2C)
    } else {
        egui::Color32::from_rgb(0xE3, 0xE4, 0xE8)
    }
}

/// Tier header: "Application Settings" / "User-Specific Settings" / "Project
/// Settings" -- the top level of the settings tree's three-level hierarchy
/// (tier / section / group). Large, bold, no box, generous space above, and
/// a thin full-width rule below it, so a tier reads as a hard break rather
/// than another box the same weight as everything under it.
fn tier_header(ui: &mut egui::Ui, label: &str, open: bool) -> (egui::Response, bool) {
    let unit = row_unit(ui);
    let gap = space_unit(ui);
    ui.add_space(gap * 1.6);
    let (resp, toggled) = collapsible_label(
        ui,
        egui::RichText::new(label)
            .size(unit * 1.3)
            .strong()
            .color(label_color(ui)),
        open,
    );
    ui.add_space(gap * 0.5);
    let rule_rect = ui.available_rect_before_wrap();
    ui.painter().hline(
        rule_rect.x_range(),
        rule_rect.top(),
        ui.visuals().widgets.noninteractive.bg_stroke,
    );
    ui.add_space(gap * 0.7);
    (resp, toggled)
}

/// A heading with a leading chevron; clicking either folds or unfolds what is
/// under it. Returns the heading's response and whether it was clicked.
fn collapsible_label(
    ui: &mut egui::Ui,
    text: egui::RichText,
    open: bool,
) -> (egui::Response, bool) {
    let side = row_unit(ui) * 0.9;
    let color = ui.visuals().weak_text_color();
    let inner = ui.horizontal(|ui| {
        let (rect, chevron) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click());
        let c = rect.center();
        let s = side * 0.22;
        let pts = if open {
            [
                c + egui::vec2(-s, -s * 0.5),
                c + egui::vec2(0.0, s * 0.5),
                c + egui::vec2(s, -s * 0.5),
            ]
        } else {
            [
                c + egui::vec2(-s * 0.5, -s),
                c + egui::vec2(s * 0.5, 0.0),
                c + egui::vec2(-s * 0.5, s),
            ]
        };
        ui.painter().add(egui::Shape::line(
            pts.to_vec(),
            egui::Stroke::new(1.5, color),
        ));
        let label = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
        if chevron.hovered() || label.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        (label.clone(), chevron.clicked() || label.clicked())
    });
    inner.inner
}

/// Section header: "Themes", "Visual Fidelity", "GPU & Rendering" -- medium
/// semibold, no box, one level down from a tier.
fn section_header(ui: &mut egui::Ui, label: &str, open: bool) -> (egui::Response, bool) {
    let unit = row_unit(ui);
    let gap = space_unit(ui);
    ui.add_space(gap * 1.0);
    let out = collapsible_label(
        ui,
        egui::RichText::new(label)
            .size(unit * 1.02)
            .strong()
            .color(label_color(ui)),
        open,
    );
    ui.add_space(gap * 0.45);
    out
}

type Collapsed = std::collections::HashSet<&'static str>;

fn collapsed_key() -> egui::Id {
    egui::Id::new("spatial_ui_kit_settings_collapsed")
}

/// Unfolds every section on the path to `target`, so a jump to it is visible.
fn expand_path<A, Ctx>(
    nodes: &[SettingsNode<A, Ctx>],
    target: &str,
    collapsed: &mut Collapsed,
) -> bool {
    let mut found = false;
    for n in nodes {
        match n {
            SettingsNode::Section { id, children, .. } => {
                if *id == target || expand_path(children, target, collapsed) {
                    collapsed.remove(id);
                    found = true;
                }
            }
            SettingsNode::Group { children, .. } => {
                found |= expand_path(children, target, collapsed);
            }
            _ => {}
        }
    }
    found
}

/// Group header: "Selection Highlight", "Spacing" -- smaller semibold, in
/// the help-text color rather than the label color, so it reads as a quiet
/// subdivision of the section above it instead of competing with it.
fn group_header(ui: &mut egui::Ui, label: &str) {
    let unit = row_unit(ui);
    let gap = space_unit(ui);
    ui.add_space(gap * 0.6);
    ui.label(
        egui::RichText::new(label)
            .size(unit * 0.85)
            .strong()
            .color(help_text_color(ui)),
    );
    ui.add_space(gap * 0.3);
}

/// A comfortable reading measure, in points, approximating ~70 characters
/// of body text -- measured against a real mixed-case pangram fragment
/// rather than guessed from a per-character average, so it tracks the
/// actual font/Text Size rather than an assumed average glyph width.
/// Chris, round 2: help text should wrap to this even when the control
/// column itself is much wider (a very wide content pane would otherwise
/// stretch a one-line hint into a hard-to-track full-bleed sentence).
fn comfortable_reading_width(ui: &egui::Ui) -> f32 {
    // ~70 characters, mixed-case with a realistic mix of narrow/wide glyphs
    // -- close enough to a body sentence's actual character mix that
    // measuring it beats guessing a single "average character width".
    const SAMPLE_70_CHARS: &str =
        "Sphinx of black quartz, judge my vows: pack my boxes with five dozen";
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    ui.painter()
        .layout_no_wrap(SAMPLE_70_CHARS.to_string(), font_id, egui::Color32::WHITE)
        .size()
        .x
}

/// The widest a tier's `Field` labels get, in points, measured with the
/// current body font -- nested fields (whose label renders indented) count
/// their indent as part of their width. [`render_field_row`] pins the label
/// cell to exactly this width (`ui.set_width`, not a `Grid`-level
/// `min_col_width`) -- a `Grid`'s own column-width memory is keyed by its
/// id, and each run of consecutive `Field`s gets its own id (see
/// [`render_section_body`]) rather than sharing one id across the whole
/// tier, since reusing one `Grid` id for several `Grid::show` calls at
/// different rects in the same frame is exactly egui's own "ID clash"
/// antipattern (`Context::warn_on_id_clash`) and did in fact paint its red
/// "First use of Grid ID" debug overlay all over this panel before this
/// measurement approach replaced it. Per-cell fixed widths sidestep that
/// entirely: the `Grid` just measures whatever width each cell's content
/// claims, which is this value every time.
///
/// `control`, the column's width, is the content pane's remaining width
/// after the label column and inter-column spacing, minus a right margin of
/// one more such gap (so the column doesn't run flush to the scrollbar) --
/// Chris, round 2: "the control/help column should take the remaining width
/// of the content pane," not the previous fixed 240pt. Individual control
/// widgets stay narrow inside it (see [`CONTROL_WIDGET_WIDTH`]); only help
/// text and a `Static` read-only value actually use the full column, and
/// even help text further caps at [`comfortable_reading_width`].
fn measure_column_widths<A, Ctx>(
    ui: &egui::Ui,
    host: &impl SettingsHost,
    tree: &[SettingsNode<A, Ctx>],
) -> ColumnWidths {
    fn walk<A, Ctx>(
        ui: &egui::Ui,
        font_id: &egui::FontId,
        tree: &[SettingsNode<A, Ctx>],
        max_w: &mut f32,
    ) {
        for node in tree {
            match node {
                SettingsNode::Section { children, .. } | SettingsNode::Group { children, .. } => {
                    walk(ui, font_id, children, max_w);
                }
                SettingsNode::Field(f) => {
                    let w = ui
                        .painter()
                        .layout_no_wrap(f.label.to_string(), font_id.clone(), egui::Color32::WHITE)
                        .size()
                        .x;
                    let w = if f.nested { w + row_unit(ui) * 1.1 } else { w };
                    *max_w = max_w.max(w);
                }
                SettingsNode::Custom { .. } => {}
            }
        }
    }
    let font_id = egui::TextStyle::Body.resolve(ui.style());
    let mut max_w: f32 = 0.0;
    walk(ui, &font_id, tree, &mut max_w);
    let unit = row_unit(ui);
    let label = match host.label_column_units() {
        // Half the pane at most, so a narrow window keeps room for controls.
        Some(units) => (units * unit).min(ui.available_width() * 0.5),
        None => max_w + 4.0,
    };

    let col_spacing = unit * 1.3;
    let right_margin = col_spacing;
    let control =
        (ui.available_width() - label - col_spacing - right_margin).max(CONTROL_WIDGET_WIDTH);

    ColumnWidths {
        label,
        control,
        help_cap: comfortable_reading_width(ui),
    }
}

/// Render the single continuously-scrolling content pane: every `Section`
/// and `Group` as a heading followed by its fields, all in one column. When
/// `scroll_to` names a `Section` id, that heading's rect is scrolled into
/// view this frame (see the sketch's "spatial continuity" rationale in
/// `docs/Sketches/Prefs Panel.md` -- clicking the nav walks you to a
/// section, it doesn't isolate it).
///
/// Three heading weights: a tier (Application-Wide/User/Project) gets
/// [`tier_header`]; a nested `Section` (Themes, GPU & Rendering, ...) gets
/// [`section_header`]; a `Group` (Selection Highlight, Spacing, ...) gets
/// [`group_header`]. Every `Field` row anywhere under one tier -- across
/// every nested `Section` and `Group` -- lines up its label/control columns
/// with every other one in that tier, via [`measure_column_widths`] rather
/// than a shared `Grid` id (see that function's doc comment for why).
pub fn render_content<A: Clone, Ctx>(
    ui: &mut egui::Ui,
    ctx: &Ctx,
    host: &impl SettingsHost,
    tree: &[SettingsNode<A, Ctx>],
    scroll_to: Option<&'static str>,
    actions: &mut Vec<A>,
) {
    // Fold state lives in ctx memory (session-only), keyed by section id.
    let mut collapsed: Collapsed = ui
        .ctx()
        .memory(|m| m.data.get_temp(collapsed_key()))
        .unwrap_or_default();
    if let Some(target) = scroll_to {
        expand_path(tree, target, &mut collapsed);
    }
    for node in tree {
        match node {
            SettingsNode::Section {
                id,
                label,
                children,
            } => {
                let open = !collapsed.contains(id);
                let (resp, toggled) = tier_header(ui, label, open);
                if toggled && open {
                    collapsed.insert(id);
                } else if toggled {
                    collapsed.remove(id);
                }
                if scroll_to == Some(*id) {
                    ui.scroll_to_rect(resp.rect, Some(egui::Align::TOP));
                }
                if open {
                    let widths = measure_column_widths(ui, host, children);
                    render_section_body(
                        ui,
                        ctx,
                        host,
                        widths,
                        children,
                        scroll_to,
                        &mut collapsed,
                        actions,
                    );
                }
            }
            // Each app's tree root is a flat list of tier `Section`s, so
            // `Group`/`Field`/`Custom` shouldn't appear here in practice --
            // handled via the same body renderer for robustness.
            _ => {
                let single = std::slice::from_ref(node);
                let widths = measure_column_widths(ui, host, single);
                render_section_body(
                    ui,
                    ctx,
                    host,
                    widths,
                    single,
                    scroll_to,
                    &mut collapsed,
                    actions,
                );
            }
        }
    }
    ui.ctx()
        .memory_mut(|m| m.data.insert_temp(collapsed_key(), collapsed));
}

/// Walks one tier's subtree, rendering nested `Section`/`Group` headings
/// inline and feeding every `Field` run into its own `egui::Grid` (a unique
/// id per run, per egui's own id-clash rule -- see [`measure_column_widths`]),
/// each cell pinned to `widths` so every run's columns line up regardless of
/// which run it's in.
#[allow(clippy::too_many_arguments)]
fn render_section_body<A: Clone, Ctx>(
    ui: &mut egui::Ui,
    ctx: &Ctx,
    host: &impl SettingsHost,
    widths: ColumnWidths,
    tree: &[SettingsNode<A, Ctx>],
    scroll_to: Option<&'static str>,
    collapsed: &mut Collapsed,
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
                let open = !collapsed.contains(id);
                let (resp, toggled) = section_header(ui, label, open);
                if toggled && open {
                    collapsed.insert(id);
                } else if toggled {
                    collapsed.remove(id);
                }
                if scroll_to == Some(*id) {
                    ui.scroll_to_rect(resp.rect, Some(egui::Align::TOP));
                }
                if open {
                    render_section_body(
                        ui, ctx, host, widths, children, scroll_to, collapsed, actions,
                    );
                }
                i += 1;
            }
            SettingsNode::Group { label, children } => {
                group_header(ui, label);
                render_section_body(
                    ui, ctx, host, widths, children, scroll_to, collapsed, actions,
                );
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
                // Unique id per run (the first field's own stable id) --
                // never a shared per-tier id; see `measure_column_widths`'s
                // doc comment for why that's an egui id-clash bug, not
                // harmless reuse. No `min_col_width` here either: each cell
                // claims its exact width itself (`render_field_row`), which
                // is what the `Grid` actually measures.
                let run_id = match &tree[start] {
                    SettingsNode::Field(f) => f.id,
                    _ => unreachable!(),
                };
                let mut row_rects: Vec<egui::Rect> = Vec::new();
                let grid = egui::Grid::new(run_id)
                    .num_columns(2)
                    .spacing([unit * 1.3, space_unit(ui) * 0.5])
                    .show(ui, |ui| {
                        for node in &tree[start..i] {
                            if let SettingsNode::Field(field) = node {
                                row_rects
                                    .push(render_field_row(ui, ctx, host, field, widths, actions));
                            }
                        }
                    });
                // Subdued between-row separators -- a guide for the eye to
                // track label -> control across a row, never above a run's
                // first row or below its last, never a background stripe (see
                // `row_rule_color`). Painted after the Grid from each row's
                // *measured* rect, midway through the gap between rows: the
                // earlier in-Grid version read the Grid's cursor after
                // `end_row()`, which isn't reliable for rows whose wrapped
                // help text changes height, so some rows got a rule and
                // others didn't (Chris, 2026-09-24 review).
                let left = grid.response.rect.left();
                let right = left + widths.label + unit * 1.3 + widths.control;
                for pair in row_rects.windows(2) {
                    let y = ((pair[0].bottom() + pair[1].top()) * 0.5).round() + 0.5;
                    ui.painter()
                        .hline(left..=right, y, egui::Stroke::new(1.0, row_rule_color(ui)));
                }
            }
        }
    }
}

fn render_field_row<A: Clone, Ctx>(
    ui: &mut egui::Ui,
    ctx: &Ctx,
    host: &impl SettingsHost,
    field: &Field<A, Ctx>,
    widths: ColumnWidths,
    actions: &mut Vec<A>,
) -> egui::Rect {
    let label_col = label_color(ui);
    let name_col = field.label_color.unwrap_or(label_col);
    let label_rect = ui
        .add_enabled_ui(field.enabled, |ui| {
            ui.scope(|ui| {
                // Exact width, not just a minimum -- this is what the `Grid`
                // actually measures for the column (see `measure_column_widths`).
                ui.set_width(widths.label);
                let text = egui::RichText::new(field.label).color(name_col);
                let menu = host.label_menu_enabled(ui.ctx());
                let name = |ui: &mut egui::Ui| {
                    if menu {
                        let r = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                        r.context_menu(|ui| host.label_context_menu(ui, field.id));
                    } else {
                        ui.label(text);
                    }
                };
                if field.nested {
                    ui.horizontal(|ui| {
                        ui.add_space(row_unit(ui) * 1.1);
                        name(ui);
                    });
                } else {
                    name(ui);
                }
            });
        })
        .response
        .rect;

    let control_rect = ui
        .add_enabled_ui(field.enabled, |ui| {
            ui.vertical(|ui| {
                // The *column* takes the remaining content-pane width; each
                // widget below stays at `CONTROL_WIDGET_WIDTH` regardless (see
                // `ColumnWidths::control`'s doc comment) -- only help text and a
                // `Static` value actually use the full column.
                ui.set_width(widths.control);
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
                            .add(
                                egui::TextEdit::singleline(&mut val)
                                    .desired_width(CONTROL_WIDGET_WIDTH),
                            )
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
                        if let Some(opt) = crate::dropdown::Dropdown::new(field.id)
                            .selected_text(current)
                            .width(CONTROL_WIDGET_WIDTH)
                            .show_items(
                                ui,
                                options.iter().copied(),
                                |o| o.to_string(),
                                |o| current == *o,
                            )
                        {
                            actions.push(on_change(opt));
                        }
                    }
                    FieldControl::DynamicDropdown {
                        value,
                        options,
                        on_change,
                    } => {
                        let current = value(ctx);
                        let opts = options(ctx);
                        if let Some(opt) = crate::dropdown::Dropdown::new(field.id)
                            .selected_text(current.clone())
                            .width(CONTROL_WIDGET_WIDTH)
                            .show_items(ui, opts.iter().cloned(), |o| o.clone(), |o| &current == o)
                        {
                            actions.push(on_change(&opt, ctx));
                        }
                    }
                    FieldControl::Static { value } => {
                        // Unlike an editable control, a read-only value is
                        // allowed to actually use the full (wide) column -- it's
                        // exactly the kind of content [`ColumnWidths::control`]
                        // exists for.
                        ui.add(
                            egui::Label::new(egui::RichText::new(value(ctx)).color(label_col))
                                .wrap(),
                        );
                    }
                    FieldControl::Custom { render } => render(ui, ctx, actions),
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
                                        .desired_width(CONTROL_WIDGET_WIDTH - 28.0),
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
                        // egui's `Slider` has no `desired_width` of its own --
                        // its track width comes from `Spacing::slider_width`,
                        // scoped here (rather than set globally) so it matches
                        // every other control's `CONTROL_WIDGET_WIDTH` without
                        // affecting sliders anywhere else in the app. The value
                        // readout suffix (e.g. "110%") draws past the track, so
                        // the track itself is narrower than the full width.
                        ui.spacing_mut().slider_width = CONTROL_WIDGET_WIDTH - 70.0;
                        let response = ui.add(
                            egui::Slider::new(&mut val, range.clone())
                                .step_by(*step as f64)
                                // Whole-number steps read "110%", not "110.0%".
                                .fixed_decimals(if step.fract() == 0.0 { 0 } else { 2 })
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

                // Help/disabled-hint text wraps to the narrower of the column's
                // own width and a comfortable reading measure -- the column can
                // be much wider than one sentence wants to be (see
                // `ColumnWidths::help_cap`'s doc comment).
                let help_width = widths.control.min(widths.help_cap);
                if let Some(hint) = field.disabled_hint {
                    if !field.enabled {
                        ui.add_space(space_unit(ui) * 0.15);
                        ui.scope(|ui| {
                            ui.set_max_width(help_width);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(hint)
                                        .size(help_font_size(ui))
                                        .color(disabled_hint_color(ui)),
                                )
                                .wrap(),
                            );
                        });
                    }
                } else if let Some(text) = field.hover {
                    ui.add_space(space_unit(ui) * 0.15);
                    ui.scope(|ui| {
                        ui.set_max_width(help_width);
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(text)
                                    .size(help_font_size(ui))
                                    .color(help_text_color(ui)),
                            )
                            .wrap(),
                        );
                    });
                }
            });
        })
        .response
        .rect;

    ui.end_row();
    label_rect.union(control_rect)
}

#[cfg(test)]
mod density_tests {
    use super::*;

    #[test]
    fn density_defaults_to_one_and_is_clamped() {
        let ctx = egui::Context::default();
        let read = || -> f32 { ctx.data(|d| d.get_temp(density_key())).unwrap_or(1.0) };
        assert_eq!(read(), 1.0);
        set_density(&ctx, 0.78);
        assert_eq!(read(), 0.78);
        set_density(&ctx, 0.0);
        assert_eq!(read(), 0.5);
        set_density(&ctx, 9.0);
        assert_eq!(read(), 1.5);
    }
}

#[cfg(test)]
mod label_column_tests {
    use super::*;

    struct Fixed(Option<f32>);
    impl SettingsHost for Fixed {
        fn browse_path(&self, _ui: &mut egui::Ui, _current: &str) -> Option<String> {
            None
        }
        fn label_column_units(&self) -> Option<f32> {
            self.0
        }
    }

    fn tier(label: &'static str) -> Vec<SettingsNode<(), ()>> {
        vec![SettingsNode::section(
            "a",
            "Alpha",
            vec![Field::static_text("f", label, |_: &()| String::new())],
        )]
    }

    fn label_width(host: &Fixed, label: &'static str) -> f32 {
        let ctx = egui::Context::default();
        let mut out = 0.0;
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1000.0, 800.0),
                )),
                ..Default::default()
            },
            |ui| {
                out = measure_column_widths(ui, host, &tier(label)).label;
            },
        );
        out
    }

    #[test]
    fn fixed_label_column_is_the_same_for_every_tier() {
        let host = Fixed(Some(16.0));
        let short = label_width(&host, "Name");
        let long = label_width(
            &host,
            "A very long setting label that would otherwise widen the column",
        );
        assert_eq!(short, long);
    }

    #[test]
    fn default_label_column_still_follows_the_widest_label() {
        let host = Fixed(None);
        assert!(label_width(&host, "A much longer label") > label_width(&host, "Hi"));
    }
}

#[cfg(test)]
mod collapse_tests {
    use super::*;

    struct Host;
    impl SettingsHost for Host {
        fn browse_path(&self, _ui: &mut egui::Ui, _current: &str) -> Option<String> {
            None
        }
    }

    fn tree() -> Vec<SettingsNode<(), ()>> {
        vec![SettingsNode::section(
            "a",
            "Alpha",
            vec![Field::static_text("f", "Field", |_: &()| String::new())],
        )]
    }

    fn frame(ctx: &egui::Context, events: Vec<egui::Event>) -> Collapsed {
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events,
            ..Default::default()
        };
        let _ = ctx.run_ui(raw, |ui| {
            let mut actions: Vec<()> = Vec::new();
            render_content(ui, &(), &Host, &tree(), None, &mut actions);
        });
        ctx.memory(|m| m.data.get_temp::<Collapsed>(collapsed_key()))
            .unwrap_or_default()
    }

    /// Clicking a section heading folds it, clicking again unfolds it.
    #[test]
    fn clicking_a_heading_folds_and_unfolds_its_section() {
        let ctx = egui::Context::default();
        let at = egui::pos2(60.0, 30.0);
        let click = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame(&ctx, vec![]);
        frame(&ctx, vec![egui::Event::PointerMoved(at)]);
        frame(&ctx, vec![click(true)]);
        let folded = frame(&ctx, vec![click(false)]);
        assert!(
            folded.contains("a"),
            "heading click did not fold: {folded:?}"
        );
        frame(&ctx, vec![egui::Event::PointerMoved(at)]);
        frame(&ctx, vec![click(true)]);
        let open = frame(&ctx, vec![click(false)]);
        assert!(!open.contains("a"));
    }
}
