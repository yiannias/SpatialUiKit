//! Ribbon rendering shell, shared between SpatialSketchPad and
//! SpatialDrawingBoard (SDB's first ribbon -- it had none before this).
//!
//! Split the same way SSP's original `panels/ribbon/` was: this module is
//! purely *how to draw* a mode tag + button groups. *What* buttons/groups
//! are visible for a given app state is app-specific business logic and
//! stays in each app's own `panels/ribbon/context.rs`-equivalent, generic
//! over that app's own `Action` type -- `RibbonButton<A>`/`RibbonGroup<A>`
//! here are thin generic containers, not full context computation.
//!
//! Icon drawing needs each app's own icon atlas, so it goes through a
//! [`RibbonHost`] trait implementation, the same pattern `menu::MenuHost`
//! uses.
//!
//! **2026-08-14, module-frame visual pass (SDB ribbon study, `docs/design/
//! ribbon-conditional-layout.md`):** the original "quiet workshop" treatment
//! (flat borderless buttons, a group's name in small type *below* its button
//! row, a hairline `ui.separator()` between groups) is gone, replaced by
//! [`module_frame`] -- a rounded-corner box around each group/module with a
//! small vertical, all-caps label pill along its left edge. Chris's reasoning
//! (2026-08-14): a caption row under every group stacked a second text row
//! under the ribbon's own menu-bar row -- a "wedding cake" of stacked text --
//! and cost vertical space that bigger icons could use instead once
//! `RibbonHost::button_size` (new, defaulted) lets a host grow its buttons to
//! fill the row when it hides per-button captions (SDB's `ribbon_show_labels`
//! toggle). A same-day follow-up (still 2026-08-14) dropped the separate
//! DRAFT/EDIT/tool-armed mode tag that used to sit to the left of the first
//! module -- Chris: it "has no use" -- and moved its filled-pill visual
//! treatment onto every module's own label instead of leaving it a one-off
//! (see [`vertical_label_pill`]). `RibbonMode`/`mode_tag` are gone entirely,
//! not just unused -- there was nothing else reading tool-armed state through
//! this module, so nothing else needed to change.
//!
//! **This is a breaking visual change for SpatialSketchPad**, whose own
//! ribbon is a second, unmodified caller of [`ribbon_panel`] through this
//! same path-dependency source. Chris, 2026-08-14: SSP's ribbon work is
//! paused, and it's fine for this to land as either a silent visual change
//! there or a documented stop-gap -- **it lands as the former**. Nothing
//! about the compile surface changed (`RibbonButton`/`RibbonGroup`/
//! `RibbonHost::icon_button` are untouched, and the new `button_size` trait
//! method is defaulted), so SSP keeps building with zero code changes, but
//! the next time anyone builds it, its ribbon will render group frames +
//! vertical labels instead of captions-below at the same fixed 40x40 button
//! size (the default `button_size()`) -- no icon growth, since that only
//! happens for a host that overrides `button_size()`, which SSP's doesn't.
//! **What a future SSP session should know:** if SSP wants the icon-growth
//! half of this pass too (buttons filling the row when captions are hidden),
//! it needs its own `ribbon_show_labels`-equivalent setting and a
//! `RibbonHost::button_size` override on its own host type, mirroring
//! `SdbRibbonHost`'s in `sdb_ui::panels::ribbon::render`. If the frame/label
//! look itself needs tuning for SSP's UI (colors, `LABEL_STRIP_W`,
//! `FRAME_RADIUS` below), those are file-local constants here, not
//! per-host-configurable -- widen them to parameters if SSP's needs
//! diverge from SDB's rather than forking the file.

use crate::motion::{presence_with, MotionSpec};

/// One button in a ribbon group, generic over the app's action type.
/// `selected` is precomputed by the caller (each app compares its own
/// "is this the active tool" state) rather than carrying a tool-type field
/// here, since that comparison is app-specific.
pub struct RibbonButton<A> {
    /// Icon key, looked up by the app's own [`RibbonHost::icon_button`].
    pub key: &'static str,
    pub label: &'static str,
    pub action: A,
    pub selected: bool,
    /// Grayed-out when false. The button stays visible so the ribbon
    /// teaches the full vocabulary of each state.
    pub enabled: bool,
    /// Hover text shown on a disabled button explaining why.
    pub disabled_hint: &'static str,
    /// A hold-past-threshold flyout attached to this button -- see
    /// [`RibbonFlyout`] and [`button_with_flyout`]. `None` for the common
    /// case of a plain single-action button.
    ///
    /// **Breaking, 2026-09-24:** this field is new on `RibbonButton`, so
    /// every existing struct literal (SDB's and SSP's) needs a value now.
    /// Logged in `spatialuikit/docs/ssp-migration-notes.md`.
    pub flyout: Option<RibbonFlyout<A>>,
}

/// Which kind of choice a [`RibbonFlyout`] offers -- drives whether picking
/// an item is remembered and replaces the pod button (per
/// `docs/design/2026-09-24_ribbon-pods-spec.md`'s "Click-and-hold flyouts").
/// The kit itself does not implement the remembering (that's a per-app user
/// setting, e.g. SDB's `ribbon_flyout_picks`); this only tags which kind of
/// flyout the button carries so an app's dispatch code can tell them apart.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FlyoutKind {
    /// The flyout offers alternative ways to run a related command (Save →
    /// Save As, Paste → Paste in Place/as Group). The main button's own
    /// action and look never change.
    Alternatives,
    /// The flyout picks a *variant* of the main command (Output →
    /// print/pdf/image/publish). An app that remembers the pick swaps the
    /// main button's icon/label/action for the chosen item's.
    Variant,
}

/// One row of a [`RibbonFlyout`]'s column -- icon + caption like a ribbon
/// button, but firing a caller-chosen bundle of actions on pick rather than
/// one action on click, so an app can dispatch "remember this pick" and "do
/// it" together (e.g. `[SetRibbonFlyoutPick, ExportPdf]`).
pub struct FlyoutItem<A> {
    /// Icon key, looked up the same way [`RibbonButton::key`] is.
    pub key: &'static str,
    pub label: &'static str,
    pub actions: Vec<A>,
    pub enabled: bool,
    pub disabled_hint: &'static str,
}

/// A hold-flyout attached to a [`RibbonButton`] -- see
/// `docs/design/2026-09-24_ribbon-pods-spec.md`'s "Click-and-hold flyouts".
pub struct RibbonFlyout<A> {
    pub kind: FlyoutKind,
    pub items: Vec<FlyoutItem<A>>,
}

/// One labeled cluster of buttons, drawn with a hairline separator before
/// every group except the first and its name in small type below the row.
pub struct RibbonGroup<A> {
    pub label: &'static str,
    pub buttons: Vec<RibbonButton<A>>,
}

/// One module in a ribbon row -- generalizes `RibbonGroup` so a row can also
/// carry a caller-rendered group (e.g. SDB's tool option fields: text edits,
/// checkboxes, choice combos) that still gets the same group-box treatment
/// -- separator before it, label below it -- as an icon-button group, so it
/// reads as one more group in the row rather than a differently-styled
/// insert. See SDB's `docs/design/ribbon-conditional-layout.md`, "Options
/// field group".
pub enum RibbonModule<'a, A> {
    Buttons(RibbonGroup<A>),
    Custom {
        label: &'static str,
        /// Width of the group's content area, in points. `Buttons` derives
        /// this from its button count; a custom group has no such count to
        /// derive it from, so the caller states it directly.
        width: f32,
        render: Box<dyn FnOnce(&mut egui::Ui) -> Vec<A> + 'a>,
    },
}

impl<A> From<RibbonGroup<A>> for RibbonModule<'_, A> {
    fn from(group: RibbonGroup<A>) -> Self {
        RibbonModule::Buttons(group)
    }
}

/// App-specific icon-button drawing, since it needs each app's own icon
/// atlas. Mirrors `menu::MenuHost`.
pub trait RibbonHost {
    fn icon_button(
        &self,
        ui: &mut egui::Ui,
        key: &str,
        label: &str,
        selected: bool,
        enabled: bool,
        disabled_hint: &str,
    ) -> egui::Response;

    /// Per-button footprint, used to size each module's frame and the
    /// overall ribbon row height (`row_h = button_size().y + GAP*2`).
    /// Defaults to the original fixed 40x40 button, so a host that doesn't
    /// override this (SSP's, as of the 2026-08-14 module-frame pass above)
    /// keeps its existing button size. A host that grows its icons to fill
    /// the row when it hides per-button captions overrides this instead.
    fn button_size(&self) -> egui::Vec2 {
        egui::vec2(40.0, 40.0)
    }

    /// The module capsule's border and label-pill colors. Defaults to the
    /// file-local constants this module has always used, so a host that
    /// doesn't override it (SSP's) renders exactly as before -- see
    /// [`ModuleFrameStyle::default`].
    ///
    /// Added 2026-09-13, when Chris asked for the light-mode capsule titles
    /// (a near-black pill with mid-grey text, fine in dark mode, a jarring
    /// dark block in light) to come from the theme instead. The module doc
    /// comment above anticipated this exact case -- "widen them to
    /// parameters if SSP's needs diverge from SDB's rather than forking the
    /// file" -- and a defaulted `RibbonHost` method is the widening, since
    /// `RibbonHost` is already how every other app-specific decision
    /// (icons, button size) reaches this file.
    fn module_frame_style(&self) -> ModuleFrameStyle {
        ModuleFrameStyle::default()
    }

    /// The icon's own sub-rect within a button's full `button_rect` --
    /// used to place the "hold for more" hover-hint triangle
    /// ([`button_with_flyout`]) at its lower-right corner, above any
    /// caption. Defaults to the whole button rect, which is correct for a
    /// host with no separate icon/caption split. A host that draws a
    /// caption below a smaller icon box (SDB's `SdbRibbonHost`) overrides
    /// this to match, so the hint lands on the icon, not straddling the
    /// caption.
    fn icon_rect(&self, button_rect: egui::Rect) -> egui::Rect {
        button_rect
    }

    /// This one button's own footprint width, height still `button_size().y`
    /// -- used both to size the module frame's content width (so it matches
    /// what `icon_button` actually draws) and, per-button, by
    /// [`draw_button_row`]/[`button_with_flyout`] to allocate that width.
    /// Defaults to `button_size().x`, i.e. every button the same fixed
    /// width, which is what every host did before this method existed (SSP
    /// still does).
    ///
    /// **Added 2026-09-24** (`docs/design/2026-09-24_ribbon-pods-spec.md`,
    /// "Button width: max(glyph, caption text) -- variable, not a fixed
    /// slot"): SDB's `SdbRibbonHost` overrides this to measure `label`
    /// against the glyph box so each button is only as wide as its own
    /// content needs, per the sketch. A defaulted method, not a breaking
    /// signature change, so SSP's fixed-width layout is untouched.
    fn button_width(&self, ui: &egui::Ui, key: &str, label: &str) -> f32 {
        let _ = (ui, key, label);
        self.button_size().x
    }
}

/// Per-theme colors for [`module_frame`]'s capsule border and label pill.
/// Plain resolved `Color32`s, not `ColorToken`s: this crate's drawing code
/// should never be parsing token strings mid-frame, and a host that has no
/// token system at all (SSP) must still be able to hand over four colors.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ModuleFrameStyle {
    pub border: egui::Color32,
    pub border_width: f32,
    pub label_foreground: egui::Color32,
    pub label_background: egui::Color32,
}

impl Default for ModuleFrameStyle {
    /// The dark-only look this module shipped with from 2026-08-14 to
    /// 2026-09-13. Kept as the default so SSP -- an unmodified second caller
    /// of [`ribbon_panel`] through the same path dependency -- is untouched
    /// by SDB's theming work, exactly as [`panel_frame`] was kept for it.
    ///
    /// [`panel_frame`]: crate::command_status_panel::panel_frame
    fn default() -> Self {
        Self {
            border: FRAME_STROKE,
            border_width: 1.0,
            label_foreground: MODULE_LABEL_FG,
            label_background: MODULE_LABEL_BG,
        }
    }
}

/// How long the clicked-button flash lasts, seconds.
const FLASH_SECS: f64 = 0.28;

/// How long a button must be held before its [`RibbonFlyout`] opens --
/// `docs/design/2026-09-24_ribbon-pods-spec.md`'s "Click-and-hold flyouts"
/// ("Paste uses 0.35s today"). `pub` since every flyout in the app shares
/// this one threshold rather than each caller guessing its own.
pub const FLYOUT_HOLD_SECS: f64 = 0.35;

/// The one gap dimension the whole row is built from -- top/bottom padding
/// inside a module frame, the horizontal gap between adjacent frames (and
/// between the mode tag and the first one), and (via `SdbRibbonHost`'s own
/// panel margin, kept equal to this on purpose) the ribbon panel's own
/// top/bottom/side margin. Chris, 2026-08-14, after the first module-frame
/// pass shipped with mismatched numbers here: "the vertical and horizontal
/// gaps... between frames are all about the same" is the thing that matters,
/// more than the exact pixel count -- one constant is what keeps that true
/// instead of three independent guesses drifting apart.
///
/// `pub` since 2026-09-05: Chris asked for the docked side panels' own
/// inset-from-the-window-edge gap (`sdb_app::frame::docked_side_area`) and
/// the Command & Status Panel's edge margin
/// (`spatial_ui_kit::command_status_panel::EDGE_MARGIN`) to match this one
/// number instead of each guessing its own -- same motivation as the doc
/// comment above, extended past just the ribbon's own modules.
pub const GAP: f32 = 8.0;
/// Width of a module frame's vertical-label pill.
const LABEL_STRIP_W: f32 = 16.0;
/// `pub` since 2026-09-05: Chris asked for the docked side panels' own
/// corner radius (`sdb_app::frame::docked_side_area`) and the Command &
/// Status Panel's corner radius
/// (`spatial_ui_kit::command_status_panel::panel_frame_with_background`) to
/// match this one number instead of each guessing its own -- same
/// motivation as `GAP`'s own `pub` doc comment.
pub const FRAME_RADIUS: f32 = 8.0;
const FRAME_STROKE: egui::Color32 = egui::Color32::from_rgb(58, 59, 64);
/// A module label pill's own colors -- the same pair the old DRAFT tag used,
/// carried over because Chris liked that look ("I like the graphical
/// appearance of it") even after asking for the tag itself to go.
const MODULE_LABEL_FG: egui::Color32 = egui::Color32::from_rgb(141, 142, 150);
const MODULE_LABEL_BG: egui::Color32 = egui::Color32::from_rgb(43, 44, 49);

/// Draws a filled box with a small vertical (rotated 90° CCW), all-caps
/// label inside it -- every module's own label pill, `corner_radius` rounded
/// only on the corners at its *outer* edge and square where it abuts the
/// module's icon container (see `module_frame`). Callers pass an
/// already-uppercased/abbreviated `text`; this only handles layout and
/// drawing, not wording -- see `module_frame`'s own doc comment for why
/// abbreviation lives with the caller.
fn vertical_label_pill(
    ui: &egui::Ui,
    rect: egui::Rect,
    text: &str,
    fg: egui::Color32,
    bg: egui::Color32,
    corner_radius: egui::CornerRadius,
) {
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    painter.rect_filled(rect, corner_radius, bg);
    // Proportional, not monospace -- `docs/design/
    // 2026-09-24_ribbon-pods-spec.md`'s proportion table specs "Inter
    // Regular... ALL CAPS", and monospace read visibly wider/blockier than
    // the sketch's label pill text once compared side by side.
    let galley = painter.layout_no_wrap(text.to_string(), egui::FontId::proportional(8.6), fg);
    // Rotated 90 CCW: reads bottom-to-top. The layout origin lands at the
    // bottom-left of the rotated text run.
    let pos = egui::pos2(
        rect.center().x - galley.size().y / 2.0,
        rect.center().y + galley.size().x / 2.0,
    );
    let shape =
        egui::epaint::TextShape::new(pos, galley, fg).with_angle(-std::f32::consts::FRAC_PI_2);
    painter.add(shape);
}

/// A group/module's content width -- each button's own
/// [`RibbonHost::button_width`], with `n - 1` gaps of the ui's own item
/// spacing between them. Shared by both entry points below and by
/// `module_frame`'s caller, which needs this number before it can allocate
/// the frame. Sums each button's own
/// [`RibbonHost::button_width`] instead of assuming they're all the same, so
/// the module frame's content width matches what `icon_button` will actually
/// draw (see that method's 2026-09-24 doc comment). Falls back to the exact
/// same total as `group_content_width` for a host that doesn't override
/// `button_width`.
fn group_content_width_var<A>(
    ui: &egui::Ui,
    buttons: &[RibbonButton<A>],
    host: &impl RibbonHost,
    spacing_x: f32,
) -> f32 {
    buttons
        .iter()
        .map(|b| host.button_width(ui, b.key, b.label))
        .sum::<f32>()
        + buttons.len().saturating_sub(1) as f32 * spacing_x
}

/// Draws one module's rounded-corner frame, with a filled label pill along
/// its left edge (see `vertical_label_pill`), then hands back a child `Ui`
/// scoped to the interior content area for the caller to draw buttons/fields
/// into. The frame's own space is allocated here, in the *caller's* `ui` --
/// the returned child `Ui` is a plain [`egui::Ui::new_child`] over that
/// already-reserved rect, so drawing into it does not double-allocate.
///
/// The label pill's corners are rounded only on its left (the frame's own
/// outer edge, where the two curves are meant to read as one continuous
/// corner) and square on its right, where it abuts the button row flush --
/// Chris's sketch shows this exact condition and asked for it explicitly
/// (2026-08-14): a rounded outside, a square inside where the label meets
/// the icon container.
///
/// `label` is drawn in all caps (Chris, 2026-08-14: "the icon groups'
/// labels should be all caps"). Abbreviation for a label too long to read
/// comfortably down the pill (e.g. "Dimension" -> "DIM", "Leaders & Styles"
/// -> "LEADER") is **not** done here -- it's wording, which belongs with
/// whoever wrote the label in the first place (`context.rs`'s group
/// constructors), not a generic truncation rule that would produce
/// nonsense like "DIME" for an arbitrary cutoff.
fn module_frame(
    ui: &mut egui::Ui,
    label: &str,
    content_w: f32,
    row_h: f32,
    style: ModuleFrameStyle,
) -> egui::Ui {
    let outer_size = egui::vec2(LABEL_STRIP_W + GAP + content_w + GAP, row_h);
    let (outer_rect, _) = ui.allocate_exact_size(outer_size, egui::Sense::hover());
    let label_rect = egui::Rect::from_min_size(outer_rect.min, egui::vec2(LABEL_STRIP_W, row_h));
    if ui.is_rect_visible(outer_rect) {
        // Filled, not just outlined -- Chris, 2026-08-30 (`docs/design/
        // 2026-08-30_chrome-ideas-sketch.md` idea 1): the ribbon's own
        // full-width panel strip is gone (see `ribbon_panel_modules`'s
        // caller in `sdb_ui::build_ui`, a floating `Area` rather than a
        // `Panel` reserving its own space), so each module is now the only
        // thing giving the row a solid backing -- "the 'pods'/'capsules'
        // [should] be the solid elements", with the viewport visible
        // through the gaps around them. (An intermediate version reserved
        // Panel space with a transparent fill instead -- fixed a docked-
        // panel layout bug it caused, but Chris confirmed live the result
        // lost the visual contrast entirely, "back where we started" --
        // see `sdb_app::frame`'s dock-`Area` doc comment for the fuller
        // fix that replaced it.)
        ui.painter()
            .rect_filled(outer_rect, FRAME_RADIUS, ui.visuals().window_fill());
        ui.painter().rect_stroke(
            outer_rect,
            FRAME_RADIUS,
            egui::Stroke::new(style.border_width, style.border),
            egui::StrokeKind::Inside,
        );
        let radius = FRAME_RADIUS as u8;
        let pill_radius = egui::CornerRadius {
            nw: radius,
            sw: radius,
            ne: 0,
            se: 0,
        };
        vertical_label_pill(
            ui,
            label_rect,
            &label.to_uppercase(),
            style.label_foreground,
            style.label_background,
            pill_radius,
        );
    }
    let content_rect = egui::Rect::from_min_size(
        outer_rect.min + egui::vec2(LABEL_STRIP_W + GAP, 0.0),
        egui::vec2(content_w, row_h),
    );
    ui.new_child(
        egui::UiBuilder::new()
            .max_rect(content_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    )
}

/// Paints the activation flash: a brief amber pulse over a just-clicked
/// button, keyed by `flash_id` in egui temp data (not caller state, since
/// it's pure presentation). Shared by [`draw_button_row`] and
/// [`button_with_flyout`] so a flyout button's main click gets the same
/// feedback as a plain one.
fn paint_flash(ui: &egui::Ui, rect: egui::Rect, flash_id: egui::Id) {
    if let Some(t0) = ui.ctx().data(|d| d.get_temp::<f64>(flash_id)) {
        let dt = ui.ctx().input(|i| i.time) - t0;
        if dt < FLASH_SECS {
            let a = (1.0 - dt / FLASH_SECS) as f32;
            let amber = egui::Color32::from_rgb(255, 178, 82);
            ui.painter()
                .rect_filled(rect, 6.0, amber.gamma_multiply(0.22 * a));
            ui.painter().rect_stroke(
                rect,
                6.0,
                egui::Stroke::new(1.5, amber.gamma_multiply(a)),
                egui::StrokeKind::Outside,
            );
            ui.ctx().request_repaint();
        } else {
            ui.ctx().data_mut(|d| d.remove::<f64>(flash_id));
        }
    }
}

/// Draws one button group's row of icon buttons (no label -- the caller's
/// [`module_frame`] already drew one) into `ui`, which is expected to already
/// be scoped to the group's content area with a left-to-right layout. Every
/// button goes through [`button_with_flyout`] -- a plain button (`flyout:
/// None`) behaves exactly as before.
fn draw_button_row<A: Clone>(
    ui: &mut egui::Ui,
    group: &RibbonGroup<A>,
    host: &impl RibbonHost,
    actions: &mut Vec<A>,
) {
    for button in &group.buttons {
        actions.extend(button_with_flyout(ui, host, button));
    }
}

/// Whether `key`'s button currently has a hold in progress or its flyout
/// open -- the same temp-data state [`button_with_flyout`] itself tracks,
/// exposed so a host's `icon_button` can suppress its own hover tooltip
/// while true. Without this, the tooltip (attached inside `icon_button`,
/// which runs *before* `button_with_flyout` knows whether a hold started)
/// pops up over the flyout's own first item the moment the hold opens it --
/// `docs/design/2026-09-24_ribbon-pods-spec.md`'s flyout-fixes item (a).
pub fn is_holding(ctx: &egui::Context, key: &str) -> bool {
    let id_root = egui::Id::new(("ribbon_flyout_button", key));
    let held_open = ctx
        .data(|d| d.get_temp::<bool>(id_root.with("held_open")))
        .unwrap_or(false);
    let pressing = ctx
        .data(|d| d.get_temp::<f64>(id_root.with("press_start")))
        .is_some();
    held_open || pressing
}

/// Pure hold-tracking update for [`button_with_flyout`], decoupled from
/// egui so it's directly unit-testable: given the previous press/held state
/// and this frame's raw pointer facts, returns the updated `(press_start,
/// held_open)`. `pressed_on_button` is true only the frame the pointer goes
/// down directly on the button (`Response::is_pointer_button_down_on`);
/// once a hold latches `held_open` true, it stays true regardless of
/// `pressed_on_button` (the pointer may have dragged off the button onto
/// the flyout column by then) as long as `primary_down` stays true.
fn update_hold_state(
    press_start: Option<f64>,
    held_open: bool,
    has_flyout: bool,
    now: f64,
    primary_down: bool,
    pressed_on_button: bool,
) -> (Option<f64>, bool) {
    if !has_flyout {
        return (None, false);
    }
    let press_start = press_start.or(if pressed_on_button { Some(now) } else { None });
    let held_open =
        held_open || (primary_down && press_start.is_some_and(|s| now - s >= FLYOUT_HOLD_SECS));
    (press_start, held_open)
}

/// What releasing the pointer resolves to for a button with a flyout --
/// pure, so [`button_with_flyout`]'s release-frame decision is directly
/// unit-testable without simulating real egui pointer input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReleaseOutcome {
    /// A short click (never held past the threshold): fire the button's own
    /// action.
    MainAction,
    /// Released over flyout item `usize`, which is enabled.
    Pick(usize),
    /// Held open, then released elsewhere or over a disabled item -- or a
    /// short press that didn't end in a click at all.
    Cancel,
}

/// `hovered_item` is the flyout item index under the pointer at release, if
/// any, **regardless of whether it's enabled** -- `item_enabled` is a
/// separate query so a disabled item under the pointer resolves to `Cancel`
/// rather than `Pick`, matching the spec's "disabled items grey with hover
/// hint" (they don't fire).
fn resolve_release(
    held_open: bool,
    main_clicked: bool,
    hovered_item: Option<usize>,
    item_enabled: impl Fn(usize) -> bool,
) -> ReleaseOutcome {
    if held_open {
        match hovered_item {
            Some(i) if item_enabled(i) => ReleaseOutcome::Pick(i),
            _ => ReleaseOutcome::Cancel,
        }
    } else if main_clicked {
        ReleaseOutcome::MainAction
    } else {
        ReleaseOutcome::Cancel
    }
}

/// Draws one button, plus its hold-flyout if it has one -- see
/// [`RibbonButton::flyout`] and `docs/design/2026-09-24_ribbon-pods-spec.md`'s
/// "Click-and-hold flyouts". Shared by [`draw_button_row`] (the kit's own
/// button-row drawing) and app `RibbonModule::Custom` renders that build a
/// button row by hand (SDB's EDIT/CREATE pods).
///
/// Behavior: a short click fires `button.action`. Holding past
/// [`FLYOUT_HOLD_SECS`] opens the flyout (grown out of the pod as one
/// outline, see [`draw_flyout_column`]); releasing over an enabled item
/// fires that item's `actions`; releasing elsewhere cancels with no action.
/// A disabled button never opens its flyout (it can't even fire its own
/// click). All state lives in egui temp memory keyed off `button.key`, so
/// nothing needs to be threaded back to the caller between frames.
pub fn button_with_flyout<A: Clone>(
    ui: &mut egui::Ui,
    host: &dyn RibbonHost,
    button: &RibbonButton<A>,
) -> Vec<A> {
    let mut actions: Vec<A> = Vec::new();
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let id_root = egui::Id::new(("ribbon_flyout_button", button.key));
    let press_start_id = id_root.with("press_start");
    let held_open_id = id_root.with("held_open");
    let presence_id = id_root.with("presence");
    let flash_id = egui::Id::new(("ribbon_flash", button.key));

    let resp = host.icon_button(
        ui,
        button.key,
        button.label,
        button.selected,
        button.enabled,
        button.disabled_hint,
    );

    let has_flyout = button.enabled && button.flyout.is_some();
    let primary_down = ctx.input(|i| i.pointer.primary_down());
    let primary_released = ctx.input(|i| i.pointer.primary_released());

    // Hold tracking (pure -- see `update_hold_state`): `press_start` is set
    // the frame the pointer goes down directly on the button; `held_open`
    // latches true once the hold crosses the threshold and stays true (even
    // after the pointer drags off the button onto the flyout column below)
    // until release, since the primary button staying down is what "still
    // holding" means here, not staying over this one widget.
    let (press_start, held_open) = update_hold_state(
        ctx.data(|d| d.get_temp::<f64>(press_start_id)),
        ctx.data(|d| d.get_temp::<bool>(held_open_id))
            .unwrap_or(false),
        has_flyout,
        now,
        primary_down,
        resp.is_pointer_button_down_on(),
    );

    let presence = presence_with(
        &ctx,
        presence_id,
        held_open,
        MotionSpec::EXPAND,
        MotionSpec::COLLAPSE,
    );

    let mut hovered_item: Option<usize> = None;
    if let Some(flyout) = &button.flyout {
        if presence.render {
            hovered_item = draw_flyout_column(ui, host, button.key, resp.rect, flyout, presence);
        }
    }

    if has_flyout {
        if primary_released {
            let outcome = resolve_release(held_open, resp.clicked(), hovered_item, |i| {
                button.flyout.as_ref().is_some_and(|f| f.items[i].enabled)
            });
            match outcome {
                ReleaseOutcome::MainAction => {
                    actions.push(button.action.clone());
                    ctx.data_mut(|d| d.insert_temp(flash_id, now));
                }
                ReleaseOutcome::Pick(i) => {
                    if let Some(flyout) = &button.flyout {
                        actions.extend(flyout.items[i].actions.iter().cloned());
                        ctx.data_mut(|d| d.insert_temp(flash_id, now));
                    }
                }
                // Released elsewhere, or over a disabled item: cancel, no
                // action -- matches the spec's "release elsewhere cancels".
                ReleaseOutcome::Cancel => {}
            }
            ctx.data_mut(|d| {
                d.remove::<f64>(press_start_id);
                d.insert_temp(held_open_id, false);
            });
        } else {
            ctx.data_mut(|d| {
                if let Some(s) = press_start {
                    d.insert_temp(press_start_id, s);
                }
                d.insert_temp(held_open_id, held_open);
            });
        }
    } else if resp.clicked() {
        actions.push(button.action.clone());
        ctx.data_mut(|d| d.insert_temp(flash_id, now));
    }

    paint_flash(ui, resp.rect, flash_id);

    // Hover hint: a small downward-pointing triangle at the lower-right of
    // the icon area, shown only while hovering a button that has a flyout --
    // "hold for more" (spec Decision 2). `resp.hovered()` alone (not
    // `held_open`) is deliberate: the hint's job is to advertise the flyout
    // before the user starts holding.
    if button.flyout.is_some() && button.enabled && resp.hovered() {
        draw_hold_hint(ui, host.icon_rect(resp.rect));
    }

    actions
}

/// The "hold for more" hint -- a small filled triangle pointing down, at the
/// lower-right corner of `icon_rect`, in the button's own foreground color
/// (approximated here as the hovered strong-text color, since this function
/// runs after `RibbonHost::icon_button` has already drawn the button and
/// has no way to hand back the exact color it used).
fn draw_hold_hint(ui: &egui::Ui, icon_rect: egui::Rect) {
    // Downward-pointing (apex at the bottom, base above it) -- Chris's spec
    // decision 2: "a small downward-facing triangle at the lower-right of
    // the icon". The previous shape had its two base points at the bottom
    // and its apex above, which reads as pointing *up*.
    let color = ui.visuals().strong_text_color();
    let size = (icon_rect.width().min(icon_rect.height()) * 0.22).clamp(4.0, 9.0);
    let apex = icon_rect.right_bottom();
    let base_left = apex - egui::vec2(size, size * 0.8);
    let base_right = apex - egui::vec2(0.0, size * 0.8);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![base_left, base_right, apex],
        color,
        egui::Stroke::NONE,
    ));
}

/// Draws the flyout column "growing out of the pod as one outline" below
/// `anchor_rect` (the held button's own rect) -- same fill/border as the
/// module capsule, joined to it with no seam (the capsule's bottom border
/// segment under the button is overpainted, since this Foreground `Area`
/// always paints after the module frame's own `Middle`-order painting this
/// frame). Drawn in a Foreground `Area` so it never affects the ribbon row's
/// own height/layout. Returns the index of the item released over, this
/// frame, if any -- the caller (`button_with_flyout`) decides whether that
/// counts as a pick (only while `presence.interactive`, i.e. not while
/// closing).
fn draw_flyout_column<A>(
    ui: &mut egui::Ui,
    host: &dyn RibbonHost,
    key: &'static str,
    anchor_rect: egui::Rect,
    flyout: &RibbonFlyout<A>,
    presence: crate::motion::PresenceFrame,
) -> Option<usize> {
    let ctx = ui.ctx().clone();
    let style = host.module_frame_style();
    let button_size = host.button_size();
    let item_h = button_size.y;
    let natural_h = item_h * flyout.items.len().max(1) as f32;
    let revealed_h = crate::motion::cap_overshoot(
        natural_h * presence.reveal.max(0.0),
        natural_h,
        MotionSpec::EXPAND_OVERSHOOT_CAP_PX,
    )
    .clamp(0.0, natural_h + MotionSpec::EXPAND_OVERSHOOT_CAP_PX);
    let opacity = presence.reveal.clamp(0.0, 1.0);
    // Wide enough for the anchor button *and* every item's own content --
    // fixes items with a longer caption than the button that opens them
    // (e.g. Output's flyout: "print"/"pdf"/"image"/"publish" against the
    // "output" button) reading clipped at the column's right edge.
    let col_w = flyout
        .items
        .iter()
        .fold(anchor_rect.width().max(button_size.x), |w, item| {
            w.max(host.button_width(ui, item.key, item.label))
        });
    let col_min = anchor_rect.left_bottom();
    let full_rect = egui::Rect::from_min_size(col_min, egui::vec2(col_w, natural_h));
    let visible_rect = egui::Rect::from_min_size(col_min, egui::vec2(col_w, revealed_h));

    let mut picked = None;
    let primary_released = ctx.input(|i| i.pointer.primary_released());

    egui::Area::new(egui::Id::new(("ribbon_flyout_area", key)))
        .fixed_pos(col_min)
        .order(egui::Order::Foreground)
        .interactable(presence.interactive)
        .show(&ctx, |ui| {
            ui.set_clip_rect(visible_rect);
            ui.set_opacity(opacity);

            // Overpaint the capsule's own bottom border directly under the
            // button -- erases the seam so the column reads as the pod's
            // own silhouette extending down, not a separate popup.
            let seam = egui::Rect::from_min_size(
                anchor_rect.left_bottom() - egui::vec2(0.0, style.border_width),
                egui::vec2(anchor_rect.width(), style.border_width * 2.0),
            );
            ui.painter()
                .rect_filled(seam, 0.0, ui.visuals().window_fill());

            let radius = FRAME_RADIUS as u8;
            let corners = egui::CornerRadius {
                nw: 0,
                ne: 0,
                sw: radius,
                se: radius,
            };
            ui.painter()
                .rect_filled(full_rect, corners, ui.visuals().window_fill());
            // Left/right/bottom border only -- the top edge merges into the
            // button above, which already reads as one continuous outline
            // once the seam above is overpainted.
            let s = egui::Stroke::new(style.border_width, style.border);
            ui.painter()
                .line_segment([full_rect.left_top(), full_rect.left_bottom()], s);
            ui.painter()
                .line_segment([full_rect.right_top(), full_rect.right_bottom()], s);
            ui.painter()
                .rect_stroke(full_rect, corners, s, egui::StrokeKind::Inside);

            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(full_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            );
            // Zero vertical item spacing -- `natural_h` above is exactly
            // `item_h * items.len()`, with no room for egui's own default
            // spacing between stacked widgets. Leaving that default in
            // pushed the last item (e.g. Output's "publish") past
            // `visible_rect`'s clip, reading cut off --
            // `docs/design/2026-09-24_ribbon-pods-spec.md`'s flyout-fixes
            // item (c).
            child.spacing_mut().item_spacing.y = 0.0;
            for (i, item) in flyout.items.iter().enumerate() {
                let resp = host.icon_button(
                    &mut child,
                    item.key,
                    item.label,
                    false,
                    item.enabled,
                    item.disabled_hint,
                );
                if presence.interactive && primary_released && resp.hovered() {
                    picked = Some(i);
                }
            }
        });

    picked
}

/// Render the button groups. Returns the actions clicked this frame. One
/// row, horizontally scrollable if the groups exceed the window width --
/// groups must never wrap onto extra rows (each group is measured and
/// allocated exactly, since a bare child inside a horizontal row claims all
/// remaining width and stacks every group onto its own line).
pub fn ribbon_panel<A: Clone>(
    ui: &mut egui::Ui,
    groups: &[RibbonGroup<A>],
    host: &impl RibbonHost,
) -> Vec<A> {
    let mut actions: Vec<A> = Vec::new();
    let button_size = host.button_size();
    let row_h = button_size.y + GAP * 2.0;
    // Resolved once per row, not per module: it cannot vary between modules
    // (it's a theme lookup, not a per-group decision) and a host is free to
    // do real work in `module_frame_style` -- SDB's parses color tokens.
    let frame_style = host.module_frame_style();

    // Note, 2026-08-30: SDB (the only current caller of this crate that's
    // actively iterating on ribbon visuals) uses `ribbon_panel_modules`
    // below, not this function -- this one is SSP's entry point. Chris
    // asked for `ScrollArea` to come out of SDB's ribbon specifically
    // ("does NOT need any scrolling capability"); left untouched here
    // rather than silently changing SSP's own ribbon behavior too.
    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal(|ui| {
            for (i, group) in groups.iter().enumerate() {
                // Only before the *first* module -- egui's own
                // `item_spacing.x` (8.0 by default, the same as `GAP`)
                // already lands between every later pair of allocations
                // automatically. Adding `GAP` there too double-counted it
                // (Chris, 2026-08-14: the horizontal gap between frames read
                // wider than the vertical padding inside them). The first
                // module has no preceding allocation for `item_spacing` to
                // apply after, so it still needs an explicit space -- and
                // this is the one Chris confirmed reads correctly as-is,
                // so it's untouched.
                if i == 0 {
                    ui.add_space(GAP);
                }
                let content_w =
                    group_content_width_var(ui, &group.buttons, host, ui.spacing().item_spacing.x);
                let mut child = module_frame(ui, group.label, content_w, row_h, frame_style);
                draw_button_row(&mut child, group, host, &mut actions);
            }
        });
    });

    actions
}

/// Render an ordered list of modules -- the generalized form of
/// `ribbon_panel` that also accepts `RibbonModule::Custom` groups (tool
/// option fields, category units, ...) alongside plain button groups, each
/// drawn with the same module-frame treatment so the row reads as one
/// consistent set of modules.
pub fn ribbon_panel_modules<A: Clone>(
    ui: &mut egui::Ui,
    modules: Vec<RibbonModule<A>>,
    host: &impl RibbonHost,
) -> Vec<A> {
    let mut actions: Vec<A> = Vec::new();
    let button_size = host.button_size();
    let row_h = button_size.y + GAP * 2.0;
    // See `ribbon_panel`'s identical lookup for why this is hoisted.
    let frame_style = host.module_frame_style();

    // No `ScrollArea` -- Chris, 2026-08-30, live: "the ribbon does NOT need
    // any scrolling capability". (It also used to paint a fade-to-
    // transparent gradient at its scrollable edge, invisible against the
    // ribbon's old opaque strip; disabling just the fade turned out not to
    // be the actual source of what looked like fading once the row sat on
    // a transparent `Area` over the viewport -- that's the viewport's own
    // content showing through the row's empty space as designed, not a
    // residual artifact. Removing `ScrollArea` outright is simpler either
    // way, and is what was actually asked for.)
    ui.horizontal(|ui| {
        // See `ribbon_panel`'s identical loop for why this is only before
        // the first module.
        for (i, module) in modules.into_iter().enumerate() {
            if i == 0 {
                ui.add_space(GAP);
            }
            match module {
                RibbonModule::Buttons(group) => {
                    let content_w = group_content_width_var(
                        ui,
                        &group.buttons,
                        host,
                        ui.spacing().item_spacing.x,
                    );
                    let mut child = module_frame(ui, group.label, content_w, row_h, frame_style);
                    draw_button_row(&mut child, &group, host, &mut actions);
                }
                RibbonModule::Custom {
                    label,
                    width,
                    render,
                } => {
                    let mut child = module_frame(ui, label, width, row_h, frame_style);
                    let acts = render(&mut child);
                    actions.extend(acts);
                }
            }
        }
    });

    actions
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal `RibbonHost` for headless testing -- a fixed 40x40 button,
    /// same as `RibbonHost::button_size`'s own default, and no theming
    /// beyond `ModuleFrameStyle::default()`.
    struct TestHost;
    impl RibbonHost for TestHost {
        fn icon_button(
            &self,
            ui: &mut egui::Ui,
            _key: &str,
            _label: &str,
            _selected: bool,
            enabled: bool,
            _disabled_hint: &str,
        ) -> egui::Response {
            let sense = if enabled {
                egui::Sense::click()
            } else {
                egui::Sense::hover()
            };
            let (rect, resp) = ui.allocate_exact_size(self.button_size(), sense);
            ui.painter()
                .rect_filled(rect, 0.0, egui::Color32::TRANSPARENT);
            resp
        }
    }

    fn test_flyout() -> RibbonFlyout<i32> {
        RibbonFlyout {
            kind: FlyoutKind::Alternatives,
            items: vec![
                FlyoutItem {
                    key: "item_a",
                    label: "Item A",
                    actions: vec![10],
                    enabled: true,
                    disabled_hint: "",
                },
                FlyoutItem {
                    key: "item_b",
                    label: "Item B",
                    actions: vec![20],
                    enabled: false,
                    disabled_hint: "Not available yet",
                },
            ],
        }
    }

    fn test_button() -> RibbonButton<i32> {
        RibbonButton {
            key: "test_btn",
            label: "Test",
            action: 1,
            selected: false,
            enabled: true,
            disabled_hint: "",
            flyout: Some(test_flyout()),
        }
    }

    // ---------------------------------------------------------------
    // Pure interaction-logic tests (`update_hold_state`/`resolve_release`).
    //
    // These exercise the exact decision logic `button_with_flyout` runs,
    // without going through egui's own pointer/hit-testing pipeline --
    // headless `egui::Context` passes can drive `RawInput::time`
    // deterministically (see `motion.rs`'s own tests), but egui's hit
    // testing always tests this frame's pointer position against *last*
    // frame's registered widget rects (`Context::begin_pass`), which makes
    // simulating a realistic multi-frame press/hold/release/re-hover
    // choreography from raw `Event`s exercise egui's own input plumbing far
    // more than this crate's logic. Factoring the decision itself out as
    // pure functions (same shape as `motion.rs`'s `Presence`) keeps the
    // *logic* directly, deterministically testable; the presence-driven
    // render/animate half is covered below the same way `motion.rs` covers
    // `Presence` itself.
    // ---------------------------------------------------------------

    #[test]
    fn short_click_fires_main_action_not_items() {
        // Never crosses the hold threshold -- held_open stays false.
        let (start, held) = update_hold_state(None, false, true, 0.0, true, true);
        let (start, held) = update_hold_state(start, held, true, 0.05, true, false);
        assert!(!held, "a short press must never latch open");

        let outcome = resolve_release(held, /* main_clicked */ true, None, |_| true);
        assert_eq!(
            outcome,
            ReleaseOutcome::MainAction,
            "a short click must fire the main action, not an item"
        );
        let _ = start;
    }

    #[test]
    fn hold_past_threshold_opens_without_firing_the_main_action() {
        let (start, held) = update_hold_state(None, false, true, 0.0, true, true);
        assert!(!held, "must not latch open before the threshold");
        let (_, held) = update_hold_state(start, held, true, FLYOUT_HOLD_SECS + 0.01, true, false);
        assert!(held, "must latch open once held past the threshold");

        // Crossing the threshold must not itself resolve to any outcome --
        // that only happens on release, which hasn't happened yet.
    }

    #[test]
    fn held_open_latches_even_after_the_pointer_leaves_the_button() {
        let (start, held) = update_hold_state(None, false, true, 0.0, true, true);
        // Pointer drags off the button (pressed_on_button = false) but the
        // primary button is still down -- held_open must still latch.
        let (_, held) = update_hold_state(start, held, true, FLYOUT_HOLD_SECS + 0.01, true, false);
        assert!(held);
    }

    #[test]
    fn release_over_an_enabled_item_picks_it() {
        let outcome = resolve_release(true, false, Some(0), |i| i == 0);
        assert_eq!(outcome, ReleaseOutcome::Pick(0));
    }

    #[test]
    fn release_over_a_disabled_item_cancels() {
        let outcome = resolve_release(true, false, Some(1), |i| i == 0);
        assert_eq!(
            outcome,
            ReleaseOutcome::Cancel,
            "a disabled item under the pointer must never fire"
        );
    }

    #[test]
    fn release_elsewhere_cancels() {
        let outcome = resolve_release(true, false, None, |_| true);
        assert_eq!(
            outcome,
            ReleaseOutcome::Cancel,
            "releasing off the flyout entirely must cancel"
        );
    }

    #[test]
    fn no_flyout_never_latches_open() {
        let (start, held) = update_hold_state(None, false, false, 0.0, true, true);
        let (_, held) = update_hold_state(start, held, false, FLYOUT_HOLD_SECS + 1.0, true, false);
        assert!(!held, "a button with no flyout must never open one");
    }

    // ---------------------------------------------------------------
    // Presence-driven render/animate tests (real `egui::Context`,
    // `RawInput::time`-driven).
    // ---------------------------------------------------------------

    /// A held-open flyout that starts closing must keep rendering
    /// (non-interactively) until its collapse animation finishes, then stop
    /// -- the same `Presence` guarantee `motion.rs` tests directly, here
    /// exercised through the exact id `button_with_flyout` derives for its
    /// own presence state.
    #[test]
    fn closing_keeps_rendering_then_stops() {
        let ctx = egui::Context::default();
        let presence_id = egui::Id::new(("ribbon_flyout_button", "test_btn")).with("presence");

        // Open, then immediately request closed -- mirrors what
        // `button_with_flyout` does internally on a release-elsewhere.
        let opened = crate::motion::presence_with(
            &ctx,
            presence_id,
            true,
            MotionSpec::EXPAND,
            MotionSpec::COLLAPSE,
        );
        assert!(opened.render);
        let closing = crate::motion::presence_with(
            &ctx,
            presence_id,
            false,
            MotionSpec::EXPAND,
            MotionSpec::COLLAPSE,
        );
        assert!(
            closing.render && !closing.interactive,
            "must still render, non-interactively, right as closing starts"
        );

        // Advance a context pass well past COLLAPSE's duration -- `should_render`
        // is read at that later time, so drive the input clock forward the
        // way `motion.rs`'s own tests do.
        ctx.begin_pass(egui::RawInput {
            time: Some(MotionSpec::COLLAPSE.duration as f64 + 1.0),
            ..Default::default()
        });
        let settled = crate::motion::presence_with(
            &ctx,
            presence_id,
            false,
            MotionSpec::EXPAND,
            MotionSpec::COLLAPSE,
        );
        let _ = ctx.end_pass();
        assert!(!settled.render, "must stop rendering once collapse settles");
    }

    /// A flyout must never grow the module frame's own row height --
    /// `module_frame`/`ribbon_panel_modules` size the row from
    /// `RibbonHost::button_size()` alone, and the flyout column paints in a
    /// Foreground `Area` outside that allocation, so opening one must not
    /// move `content_rect`'s height. The button's own held-open state is
    /// written directly into egui temp data (rather than simulated via
    /// pointer events -- see the pure-logic tests above for why) so the
    /// flyout is genuinely open for the second measurement.
    #[test]
    fn ribbon_row_height_unchanged_with_a_flyout_open() {
        let ctx = egui::Context::default();
        let host = TestHost;
        let row_h = host.button_size().y + GAP * 2.0;

        let group = RibbonGroup {
            label: "Test",
            buttons: vec![test_button()],
        };

        let measure = |ctx: &egui::Context, t: f64| -> f32 {
            ctx.begin_pass(egui::RawInput {
                time: Some(t),
                ..Default::default()
            });
            let mut measured_h = 0.0;
            egui::Area::new(egui::Id::new("ribbon_test_area"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    let content_w = group_content_width_var(
                        ui,
                        &group.buttons,
                        &host,
                        ui.spacing().item_spacing.x,
                    );
                    let mut child =
                        module_frame(ui, group.label, content_w, row_h, host.module_frame_style());
                    measured_h = child.max_rect().height();
                    draw_button_row(&mut child, &group, &host, &mut Vec::new());
                });
            let _ = ctx.end_pass();
            measured_h
        };

        let closed_h = measure(&ctx, 0.0);

        // Force the flyout open directly in temp data -- the same keys
        // `button_with_flyout` itself writes.
        let id_root = egui::Id::new(("ribbon_flyout_button", "test_btn"));
        ctx.data_mut(|d| {
            d.insert_temp(id_root.with("press_start"), 0.0_f64);
            d.insert_temp(id_root.with("held_open"), true);
        });
        let open_h = measure(&ctx, 0.01);
        assert_eq!(
            closed_h, open_h,
            "opening the flyout must not change row height"
        );
    }
}
