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

use crate::motion::MotionSpec;

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
/// Proportion of the pod's own height `row_h` that its corner radius (and,
/// per [`pod_corner_radius`]'s doc comment, the flow-out fillet radius) is
/// drawn at -- `docs/design/2026-09-24_ribbon-pods-spec.md`'s proportion
/// table: "Pod corner radius: 18 sketch units / 0.138 of H / 8px at H=56".
const FRAME_RADIUS_PROPORTION: f32 = 0.138;
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
/// Corner radius for a pod (module capsule) *and* its flow-out fillets,
/// derived from the pod's own height `row_h` -- `0.138 * row_h`, per
/// `docs/design/2026-09-24_ribbon-pods-spec.md`'s proportion table (8px at
/// the default `row_h` = 56).
///
/// **Root cause of "flares lose their place when Interface Scale is
/// raised"** (2026-09-24 designer markup, `docs/design/
/// 2026-09-24_flyout-end-condition.md`): the flyout fillet and the pod's own
/// corner used to both read the flat literal `FRAME_RADIUS` independently --
/// two call sites that happened to agree only because nobody had yet made
/// `row_h` vary without also changing that literal by hand. `row_h` itself
/// already scales correctly with Interface Scale/Text Size (it flows from
/// `RibbonHost::button_size()`, all in egui points, so `ctx.set_zoom_factor`
/// scales it uniformly like everything else painted in points) -- the bug
/// was never really a points-vs-pixels mismatch, it was that the corner and
/// the fillet had no enforced relationship, so any future divergence between
/// them (a host overriding button size, a proportion tweak on one call site
/// and not the other) reads as the fillet drifting off the corner it is
/// supposed to continue. Routing both through this one function makes that
/// class of bug impossible instead of merely coincidentally absent.
pub fn pod_corner_radius(row_h: f32) -> f32 {
    (row_h * FRAME_RADIUS_PROPORTION).max(1.0)
}

/// egui temp-data key for the pod [`module_frame`] most recently drew.
fn current_pod_rect_id() -> egui::Id {
    egui::Id::new("spatial_ui_kit::ribbon::current_pod_rect")
}

/// egui temp-data key for that pod's fill color.
fn current_pod_fill_id() -> egui::Id {
    egui::Id::new("spatial_ui_kit::ribbon::current_pod_fill")
}

/// egui temp-data key for that pod's own corner radius -- read back by
/// [`draw_flyout_column`] so its combined pod+column paint uses the exact
/// radius the pod itself was drawn with, rather than a value recomputed
/// from a possibly-different `row_h`. See [`flow_out_outline`]'s doc comment
/// for why this consistency is the whole point of the 2026-09-24 rethink.
fn current_pod_radius_id() -> egui::Id {
    egui::Id::new("spatial_ui_kit::ribbon::current_pod_radius")
}

fn module_frame(
    ui: &mut egui::Ui,
    label: &str,
    content_w: f32,
    row_h: f32,
    style: ModuleFrameStyle,
) -> egui::Ui {
    let outer_size = egui::vec2(LABEL_STRIP_W + GAP + content_w + GAP, row_h);
    let (outer_rect, _) = ui.allocate_exact_size(outer_size, egui::Sense::hover());
    module_frame_at(ui, outer_rect, label, content_w, row_h, style, 1.0)
}

/// As [`module_frame`], but painted at a caller-given `outer_rect` instead
/// of sequentially allocating one -- [`ribbon_panel_modules`]'s spring-
/// animated pods need this: their rect comes from a per-pod position/width
/// spring, not from egui's own left-to-right cursor, since several pods'
/// rects change together every frame while their springs settle (see that
/// function's doc comment). `content_opacity` scales the content child
/// (`Ui::set_opacity`) as well as the frame/border paint, for a pod
/// fading in its content (a morph target) or as a whole (a fully-transparent
/// caller could fade the frame too, though no current caller does).
fn module_frame_at(
    ui: &mut egui::Ui,
    outer_rect: egui::Rect,
    label: &str,
    content_w: f32,
    row_h: f32,
    style: ModuleFrameStyle,
    content_opacity: f32,
) -> egui::Ui {
    // Clamp the label strip to the pod's own current width so a pod mid-
    // shrink (or a brand-new one still growing from near-zero) never draws
    // a label wider than the capsule it's supposed to sit inside.
    let label_w = LABEL_STRIP_W.min(outer_rect.width().max(0.0));
    let label_rect = egui::Rect::from_min_size(outer_rect.min, egui::vec2(label_w, row_h));
    let frame_radius = pod_corner_radius(row_h);
    // Remembered for this frame so a flyout on the pod's first/last button
    // can sit flush with the pod's *outer* edge, not the button's (the pod
    // has end padding past its last button).
    ui.data_mut(|d| d.insert_temp(current_pod_rect_id(), outer_rect));
    // And its fill: the flyout paints in its own foreground `Area`, whose
    // style isn't the ribbon's themed one, so reading `window_fill()` there
    // gave a different color than the pod it grows out of.
    let pod_fill = ui.visuals().window_fill();
    ui.data_mut(|d| d.insert_temp(current_pod_fill_id(), pod_fill));
    ui.data_mut(|d| d.insert_temp(current_pod_radius_id(), frame_radius));
    if ui.is_rect_visible(outer_rect) && outer_rect.width() > 0.5 {
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
        ui.painter().rect_filled(outer_rect, frame_radius, pod_fill);
        ui.painter().rect_stroke(
            outer_rect,
            frame_radius,
            egui::Stroke::new(style.border_width, style.border),
            egui::StrokeKind::Inside,
        );
        if label_w > 4.0 {
            let radius = frame_radius.round().clamp(0.0, u8::MAX as f32) as u8;
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
    }
    let content_left = (outer_rect.min.x + label_w + GAP).min(outer_rect.max.x);
    let content_rect = egui::Rect::from_min_size(
        egui::pos2(content_left, outer_rect.min.y),
        egui::vec2(
            (outer_rect.max.x - content_left)
                .max(0.0)
                .min(content_w.max(0.0)),
            row_h,
        ),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(content_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.set_clip_rect(content_rect.intersect(ui.clip_rect()));
    child.set_opacity(content_opacity.clamp(0.0, 1.0));
    child
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
    let last = group.buttons.len().saturating_sub(1);
    for (i, button) in group.buttons.iter().enumerate() {
        // First/last button in the pod gets a Flush join on its *outer*
        // side (left for the first, right for the last) -- an "L" end
        // condition, per `docs/design/2026-09-24_flyout-end-condition.md`.
        // A pod with exactly one button is both first and last: both sides
        // flush, the mirror-image degenerate case.
        let left = if i == 0 { Join::Flush } else { Join::Flare };
        let right = if i == last { Join::Flush } else { Join::Flare };
        actions.extend(button_with_flyout_joined(ui, host, button, left, right));
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
    button_with_flyout_joined(ui, host, button, Join::Flare, Join::Flare)
}

/// As [`button_with_flyout`], with explicit end-condition [`Join`]s for the
/// flyout's own left/right sides -- [`draw_button_row`] uses this, deriving
/// `left`/`right` from the button's position in its group (`Flush` on the
/// pod's own first/last button's outer side, `Flare` everywhere else). A
/// caller that draws its own button row by hand (an app's `RibbonModule::
/// Custom` render) and wants correct end conditions should call this
/// directly instead of the plain `button_with_flyout`, which always flares
/// both sides (the historical behavior, and still correct for a button that
/// is never at a pod's own edge).
pub fn button_with_flyout_joined<A: Clone>(
    ui: &mut egui::Ui,
    host: &dyn RibbonHost,
    button: &RibbonButton<A>,
    left: Join,
    right: Join,
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

    // Spring physics (2026-09-24, Chris: "gently bouncy, Apple spring"
    // feel) rather than the fixed-duration `MotionSpec` tween: `BOUNCY`
    // opening is interruptible and velocity-preserving, so five rapid
    // hold/release cycles reverse smoothly instead of restarting from a
    // standstill each time. `SMOOTH` closing stays critically damped (no
    // bounce on the way out) regardless of the global bounce-amount
    // setting -- see `Spring::scaled_by_bounce`.
    let presence = crate::motion::spring_presence_with(
        &ctx,
        presence_id,
        held_open,
        crate::motion::Spring::BOUNCY,
        crate::motion::Spring::SMOOTH,
    );

    let mut hovered_item: Option<usize> = None;
    if let Some(flyout) = &button.flyout {
        if presence.render {
            hovered_item = draw_flyout_column(
                ui, host, button.key, resp.rect, flyout, left, right, presence,
            );
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

/// Number of sampled points per quarter-circle arc in [`flow_out_outline`],
/// not counting the arc's own start point (which is always the previous
/// point already pushed onto the path).
const FLOW_OUT_ARC_SEGMENTS: usize = 8;

/// How one side of a [`flow_out_outline`]/[`paint_flow_out`] shape meets the
/// pod above it -- `docs/design/2026-09-24_flyout-end-condition.md`: a
/// flyout under a *middle* button gets a flare on both sides (a "T" join); a
/// flyout under the pod's *first or last* button continues that side's
/// straight outer edge straight down instead (an "L" join, the flare only on
/// the inner side). The caller decides which from the held button's position
/// in its own pod -- this type just carries the decision into the geometry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Join {
    /// Concave fillet flaring outward from the pod's straight edge, into a
    /// convex rounded corner at the column's bottom on this side.
    Flare,
    /// This side continues the pod's straight outer edge straight down --
    /// no flare, no rounded corner, the column's edge flush with the pod's.
    Flush,
}

/// Points along a quarter-circle-or-less arc of radius `r` centred at
/// `centre`, from angle `from` to `to`, `FLOW_OUT_ARC_SEGMENTS` of them, not
/// including the arc's own mathematical start point (the caller's previous
/// point already coincides with it). Shared by [`flow_out_outline`] (the
/// stroked path) and [`paint_flow_out`] (the fanned fill wedges), so the two
/// can never sample an arc differently.
fn flow_out_arc_points(r: f32, centre: egui::Pos2, from: f32, to: f32) -> Vec<egui::Pos2> {
    (1..=FLOW_OUT_ARC_SEGMENTS)
        .map(|i| {
            let t = i as f32 / FLOW_OUT_ARC_SEGMENTS as f32;
            let theta = egui::lerp(from..=to, t);
            egui::pos2(centre.x + r * theta.cos(), centre.y + r * theta.sin())
        })
        .collect()
}

/// Clamps a fillet radius to what `column` can actually hold: at most half
/// its width, and at most half its own height above `y0` (the pod's bottom
/// edge) -- so a narrow or barely-open column still produces a valid, finite
/// outline. Shared by [`flow_out_outline`] and [`paint_flow_out`] so the two
/// can never clamp differently.
fn flow_out_radius(y0: f32, column: egui::Rect, r: f32) -> f32 {
    let vertical_room = ((column.bottom() - y0) / 2.0).max(0.0);
    r.min(column.width() / 2.0).min(vertical_room).max(0.0)
}

/// Builds the combined "flow-out" outline: a single CLOSED path around the
/// pod's own silhouette *and* the flyout column growing out of it, as one
/// shape -- the 2026-09-24 design rethink (`docs/design/
/// 2026-09-24_flyout-flush-edge-offset.md`) that replaced painting the pod
/// and the flyout separately and trying to patch the seam between them.
/// Traversed clockwise starting just past the pod's top-left corner:
///
/// - top edge, top-right corner (always rounded, radius `pod_radius`);
/// - right edge down to the pod's bottom edge `y0 = pod.bottom()`;
/// - **right side join**: [`Join::Flare`] draws the pod's own bottom-right
///   corner (rounded, `pod_radius`) and a flat run of pod-bottom-edge before
///   curving concave into the column (mirrors the old open path's right
///   arm); [`Join::Flush`] skips that corner entirely and continues the
///   pod's straight edge down past `y0` -- no separate corner to go out of
///   sync with the column, because there no longer is one;
/// - the column's bottom edge and its two bottom corners (always rounded,
///   radius `r`, clamped by [`flow_out_radius`]);
/// - **left side join**, mirroring the right;
/// - back up the pod's left edge to close.
///
/// `pod`/`pod_radius` are the pod's own outer rect and corner radius --
/// exactly the values [`module_frame_at`] paints the pod with, so a caller
/// that fills and strokes this outline is guaranteed to line up with it
/// pixel for pixel instead of drifting the way two independently-painted
/// shapes could. `column`/`r` are the flyout column's current (possibly
/// mid-morph) rect and fillet radius -- `r` may be smaller than
/// `pod_radius` while the flyout is still growing out of its neck; the
/// straight segments between them absorb the difference with no kink.
pub fn flow_out_outline(
    pod: egui::Rect,
    pod_radius: f32,
    column: egui::Rect,
    r: f32,
    left: Join,
    right: Join,
) -> Vec<egui::Pos2> {
    use std::f32::consts::{FRAC_PI_2, PI};

    let y0 = pod.bottom();
    let l = column.left();
    let right_x = column.right();
    let b = column.bottom();
    let r = flow_out_radius(y0, column, r);
    let pr = pod_radius
        .max(0.0)
        .min(pod.width() / 2.0)
        .min(pod.height() / 2.0);

    let mut pts = Vec::with_capacity(8 * FLOW_OUT_ARC_SEGMENTS + 16);

    // Top edge (top-left corner is closed implicitly at the end).
    pts.push(egui::pos2(pod.left() + pr, pod.top()));
    pts.push(egui::pos2(pod.right() - pr, pod.top()));
    // Top-right corner: -90deg -> 0deg, centred (R - pr, top + pr).
    pts.extend(flow_out_arc_points(
        pr,
        egui::pos2(pod.right() - pr, pod.top() + pr),
        -FRAC_PI_2,
        0.0,
    ));
    // Right edge down to just above the pod's bottom edge.
    pts.push(egui::pos2(pod.right(), y0 - pr));

    match right {
        Join::Flare => {
            // Pod's own bottom-right corner, still rounded (this side isn't
            // where the column attaches): 0deg -> 90deg.
            pts.extend(flow_out_arc_points(
                pr,
                egui::pos2(pod.right() - pr, y0 - pr),
                0.0,
                FRAC_PI_2,
            ));
            // Flat run along the pod bottom to the column's own flare start.
            pts.push(egui::pos2(right_x + r, y0));
            // Concave fillet down into the column: 270deg -> 180deg
            // (reverse of the old open path's forward sweep).
            pts.extend(flow_out_arc_points(
                r,
                egui::pos2(right_x + r, y0 + r),
                PI + FRAC_PI_2,
                PI,
            ));
        }
        Join::Flush => {
            // Continue the pod's own straight edge (`right_x == pod.right()`
            // here) straight down -- no corner, no flare tip. The
            // unconditional push below carries it the rest of the way.
        }
    }
    // Down the column's right side to its bottom-right corner, then the
    // corner itself: 0deg -> 90deg, centred (R - r, B - r).
    pts.push(egui::pos2(right_x, b - r));
    pts.extend(flow_out_arc_points(
        r,
        egui::pos2(right_x - r, b - r),
        0.0,
        FRAC_PI_2,
    ));

    // Column bottom edge, right to left.
    pts.push(egui::pos2(l + r, b));

    // Bottom-left column corner: 90deg -> 180deg, centred (L + r, B - r).
    pts.extend(flow_out_arc_points(
        r,
        egui::pos2(l + r, b - r),
        FRAC_PI_2,
        PI,
    ));

    match left {
        Join::Flare => {
            // Up the column's left side.
            pts.push(egui::pos2(l, y0 + r));
            // Concave fillet back out to the pod bottom: 0deg -> -90deg.
            pts.extend(flow_out_arc_points(
                r,
                egui::pos2(l - r, y0 + r),
                0.0,
                -FRAC_PI_2,
            ));
            // Flat run back to the pod's own bottom-left corner.
            pts.push(egui::pos2(pod.left() + pr, y0));
            // Pod's own bottom-left corner: 90deg -> 180deg.
            pts.extend(flow_out_arc_points(
                pr,
                egui::pos2(pod.left() + pr, y0 - pr),
                FRAC_PI_2,
                PI,
            ));
        }
        Join::Flush => {
            // Continue straight up the pod's own edge (`l == pod.left()`).
            pts.push(egui::pos2(l, y0 - pr));
        }
    }

    // Up the pod's left edge, and the top-left corner closes the loop.
    pts.push(egui::pos2(pod.left(), pod.top() + pr));
    pts.extend(flow_out_arc_points(
        pr,
        egui::pos2(pod.left() + pr, pod.top() + pr),
        PI,
        PI + FRAC_PI_2,
    ));

    pts
}

/// Paints the pod *and* its open flyout column as one silhouette -- the
/// 2026-09-24 rethink: the pod is redrawn here (in the flyout's own
/// Foreground `Area`, which always paints after the module frame's
/// Middle-order pass -- see [`draw_flyout_column`]) from the exact same
/// `pod`/`pod_radius`/`fill`/`stroke` values [`module_frame_at`] used, so
/// there is no separate "patch over the old corner" step to drift out of
/// sync with it; the whole capsule is simply repainted with the correct
/// corner shape for however it currently joins the column.
///
/// Fill (as convex pieces -- the combined outline is non-convex, so
/// `egui`'s fan-triangulated `convex_polygon` can't take the whole shape at
/// once):
/// - the pod itself, all four corners `pod_radius`-rounded except a
///   `Join::Flush` side's bottom corner, drawn square so it continues
///   straight into the column with no corner to erase;
/// - the column body, bottom corners rounded (radius `r`) on both sides --
///   flush or flare, the *column's* own bottom corner is always rounded,
///   only the *pod's* corner above it is conditionally squared off;
/// - each `Flare` side's fillet wedge, fanned from the pod-bottom corner
///   point, same as before.
///
/// Then the combined outline (pod + column, [`flow_out_outline`]) is
/// stroked once on top -- the only visible line, since the fill above
/// already covers everything it crosses.
///
/// `painter`'s clip rect must already include the flare (on whichever sides
/// have one) and the full `pod` rect -- callers should expand their clip by
/// `r + stroke.width` past `column` on a `Flare` side.
pub fn paint_flow_out(
    painter: &egui::Painter,
    pod: egui::Rect,
    pod_radius: f32,
    column: egui::Rect,
    r: f32,
    left: Join,
    right: Join,
    fill: egui::Color32,
    stroke: egui::Stroke,
) {
    let y0 = pod.bottom();
    let l = column.left();
    let right_x = column.right();
    let b = column.bottom();
    let r = flow_out_radius(y0, column, r);
    let pr = pod_radius
        .max(0.0)
        .min(pod.width() / 2.0)
        .min(pod.height() / 2.0);

    let outline = flow_out_outline(pod, pod_radius, column, r, left, right);

    // (a) the pod itself -- square only the corner a `Flush` join replaces.
    let pod_r = pr.round().clamp(0.0, u8::MAX as f32) as u8;
    painter.rect_filled(
        pod,
        egui::CornerRadius {
            nw: pod_r,
            ne: pod_r,
            sw: if left == Join::Flush { 0 } else { pod_r },
            se: if right == Join::Flush { 0 } else { pod_r },
        },
        fill,
    );

    // (b) the column body -- bottom corners always rounded (radius `r`),
    // regardless of join: the *pod's* corner above is what a `Flush` side
    // squares off, not the column's own.
    if b > y0 {
        let body = egui::Rect::from_min_max(egui::pos2(l, y0), egui::pos2(right_x, b));
        let flare_r = r.round().clamp(0.0, u8::MAX as f32) as u8;
        painter.rect_filled(
            body,
            egui::CornerRadius {
                nw: 0,
                ne: 0,
                sw: flare_r,
                se: flare_r,
            },
            fill,
        );
    }

    if r > 0.0 {
        // (c) fillet wedges, `Flare` sides only.
        if left == Join::Flare {
            let mut left_wedge = Vec::with_capacity(FLOW_OUT_ARC_SEGMENTS + 2);
            left_wedge.push(egui::pos2(l, y0));
            left_wedge.push(egui::pos2(l - r, y0));
            left_wedge.extend(flow_out_arc_points(
                r,
                egui::pos2(l - r, y0 + r),
                -std::f32::consts::FRAC_PI_2,
                0.0,
            ));
            painter.add(egui::Shape::convex_polygon(
                left_wedge,
                fill,
                egui::Stroke::NONE,
            ));
        }
        if right == Join::Flare {
            let mut right_wedge = Vec::with_capacity(FLOW_OUT_ARC_SEGMENTS + 2);
            right_wedge.push(egui::pos2(right_x, y0));
            right_wedge.push(egui::pos2(right_x, y0 + r));
            right_wedge.extend(flow_out_arc_points(
                r,
                egui::pos2(right_x + r, y0 + r),
                std::f32::consts::PI,
                std::f32::consts::PI + std::f32::consts::FRAC_PI_2,
            ));
            painter.add(egui::Shape::convex_polygon(
                right_wedge,
                fill,
                egui::Stroke::NONE,
            ));
        }
    }

    // The combined outline -- pod + column as one closed, stroked shape.
    painter.add(egui::Shape::closed_line(outline, stroke));
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
/// Fraction of the final width/height/radius the flyout starts at, at
/// `reveal == 0` -- the "small bump/neck emerging from the pod bottom" shape
/// the Dynamic-Island-style morph opens from
/// (`docs/design/2026-09-19_animated-reveals-transforms.md`), rather than a
/// full-width column that merely grows downward.
const FLOW_OUT_NECK_FRACTION: f32 = 0.4;

/// Reveal threshold past which items start fading/sliding in -- below this
/// the flyout is still mostly "shape", per the morph spec's "items fade/
/// slide in only after the shape is ~60% open". Also, run in reverse, the
/// reason closing reads as "items fade first, then the shape retracts": the
/// same single `presence.reveal` value drives both, so items reach zero
/// opacity at `reveal == ITEMS_REVEAL_THRESHOLD` while the shape itself
/// keeps shrinking all the way down to `reveal == 0`.
const ITEMS_REVEAL_THRESHOLD: f32 = 0.6;

fn draw_flyout_column<A>(
    ui: &mut egui::Ui,
    host: &dyn RibbonHost,
    key: &'static str,
    anchor_rect: egui::Rect,
    flyout: &RibbonFlyout<A>,
    left: Join,
    right: Join,
    presence: crate::motion::PresenceFrame,
) -> Option<usize> {
    let ctx = ui.ctx().clone();
    let style = host.module_frame_style();
    let button_size = host.button_size();
    let item_h = button_size.y;
    let row_h = item_h + GAP * 2.0;
    let r = pod_corner_radius(row_h);

    // Padding around each item's glyph+caption, and around the column as a
    // whole -- `docs/design/2026-09-24_flyout-end-condition.md`: "we don't
    // have to be quite so stingy with the screen real estate", sized off the
    // pod's own proportion system (`~0.17*H` is the same gap the pod uses
    // between its own buttons; `~0.12*H`/`~0.15*H` are new flyout-only
    // proportions in that spirit).
    let pad_h = 0.17 * row_h;
    let pad_v_between = 0.12 * row_h;
    let pad_v_col = 0.15 * row_h;

    let n = flyout.items.len().max(1) as f32;
    let natural_h = 2.0 * pad_v_col + item_h * n + pad_v_between * (n - 1.0).max(0.0);

    // Wide enough for the anchor button *and* every item's own content, plus
    // horizontal padding each side -- fixes items with a longer caption than
    // the button that opens them (e.g. Output's flyout: "print"/"pdf"/
    // "image"/"publish" against the "output" button) reading clipped or
    // packed tight at the column's edge.
    let content_w = flyout
        .items
        .iter()
        .fold(anchor_rect.width().max(button_size.x), |w, item| {
            w.max(host.button_width(ui, item.key, item.label))
        });
    let mut final_w = content_w + 2.0 * pad_h;

    // The column's final left edge and horizontal centre -- symmetric growth
    // past the anchor button on both sides for a T join, but pinned flush to
    // the *pod's* outer edge (growing only inward) on a Flush side, per the
    // end-condition spec. The pod edge, not the button's: the pod has end
    // padding past its first/last button, and pinning to the button left the
    // column short of the pod's straight end (Chris, 2026-09-24 screenshots).
    // Widened if needed so the column still spans the whole button.
    let pod = ui
        .data(|d| d.get_temp::<egui::Rect>(current_pod_rect_id()))
        .filter(|p| p.x_range().contains(anchor_rect.center().x))
        .unwrap_or(anchor_rect);
    let final_left = match (left, right) {
        (Join::Flush, _) => {
            final_w = final_w.max(anchor_rect.right() - pod.left());
            pod.left()
        }
        (_, Join::Flush) => {
            final_w = final_w.max(pod.right() - anchor_rect.left());
            pod.right() - final_w
        }
        _ => anchor_rect.center().x - final_w / 2.0,
    };
    let final_centre_x = final_left + final_w / 2.0;

    // -- Shape morph: width, height and fillet radius all animate together
    // from a small neck to the final shape, using the same `presence.reveal`
    // (already `MotionSpec::EXPAND`'s springy back-out while opening,
    // `MotionSpec::COLLAPSE`'s plain ease-out while closing -- see
    // `button_with_flyout`). Overshoot on each is capped at
    // `MotionSpec::EXPAND_OVERSHOOT_CAP_PX`, paint-only, and never feeds back
    // into ribbon layout (this whole function draws into a Foreground `Area`
    // that never affects row height).
    let grow = |start_frac: f32, target: f32| -> f32 {
        let start = target * start_frac;
        let span = (target - start).max(0.0);
        start
            + crate::motion::cap_overshoot(
                span * presence.reveal.max(0.0),
                span,
                MotionSpec::EXPAND_OVERSHOOT_CAP_PX,
            )
            .clamp(0.0, span + MotionSpec::EXPAND_OVERSHOOT_CAP_PX)
    };
    let revealed_w = grow(FLOW_OUT_NECK_FRACTION, final_w);
    let revealed_h = grow(FLOW_OUT_NECK_FRACTION, natural_h);
    let revealed_r = grow(FLOW_OUT_NECK_FRACTION, r);
    let revealed_left = final_centre_x - revealed_w / 2.0;

    // Shape opacity ramps in fast (this is a size morph, not a fade); item
    // opacity only starts past `ITEMS_REVEAL_THRESHOLD` -- see that const's
    // doc comment for why this also gives closing its "items fade first"
    // read for free.
    let shape_opacity = (presence.reveal.clamp(0.0, 1.0) / 0.15).min(1.0);
    let items_opacity = ((presence.reveal.clamp(0.0, 1.0) - ITEMS_REVEAL_THRESHOLD)
        / (1.0 - ITEMS_REVEAL_THRESHOLD))
        .clamp(0.0, 1.0);

    // The shape joins the *pod's* bottom edge, not the button's: pods
    // extend below their buttons, and attaching at the button's bottom put
    // the fillets inside the pod and left the pod's bottom border running
    // through the join (Chris, 2026-09-24 screenshot).
    let attach = egui::Rect::from_min_max(
        anchor_rect.min,
        egui::pos2(anchor_rect.max.x, pod.bottom().max(anchor_rect.bottom())),
    );
    let col_min = egui::pos2(revealed_left, attach.bottom());
    let full_rect = egui::Rect::from_min_size(
        egui::pos2(final_left, attach.bottom()),
        egui::vec2(final_w, natural_h),
    );
    let reveal_column = egui::Rect::from_min_size(col_min, egui::vec2(revealed_w, revealed_h));

    let mut picked = None;
    let primary_released = ctx.input(|i| i.pointer.primary_released());

    // Read back the exact radius `module_frame_at` painted this pod with --
    // see `current_pod_radius_id`'s doc comment. Falls back to the locally
    // computed `r` (which agrees with it in the ordinary case) if nothing
    // was recorded this frame, e.g. a test driving `draw_flyout_column`
    // directly without going through a module frame first.
    let pod_radius = ctx
        .data(|d| d.get_temp::<f32>(current_pod_radius_id()))
        .unwrap_or(r);

    egui::Area::new(egui::Id::new(("ribbon_flyout_area", key)))
        .fixed_pos(full_rect.left_top())
        .order(egui::Order::Foreground)
        .interactable(presence.interactive)
        .show(&ctx, |ui| {
            // Expand the clip past the widest the shape can get this frame
            // (`full_rect`, plus the flare past it on `Flare` sides) --
            // *and* past the whole pod, which this Area now repaints in
            // full (see `paint_flow_out`'s doc comment) -- otherwise the
            // fillets or the pod's own top/sides get cut off.
            let flare = r + style.border_width;
            let base = pod.union(full_rect);
            let paint_clip = egui::Rect::from_min_max(
                base.left_top() - egui::vec2(flare, style.border_width + flare),
                base.right_bottom() + egui::vec2(flare, 0.0),
            );
            ui.set_clip_rect(paint_clip);
            ui.set_opacity(shape_opacity);

            let fill_color = ui
                .data(|d| d.get_temp::<egui::Color32>(current_pod_fill_id()))
                .unwrap_or_else(|| ui.visuals().window_fill());
            let stroke = egui::Stroke::new(style.border_width, style.border);
            paint_flow_out(
                ui.painter(),
                pod,
                pod_radius,
                reveal_column,
                revealed_r,
                left,
                right,
                fill_color,
                stroke,
            );

            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(full_rect)
                    .layout(egui::Layout::top_down(egui::Align::Center)),
            );
            child.set_opacity(items_opacity);
            child.spacing_mut().item_spacing.y = pad_v_between;
            child.add_space(pad_v_col);
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

/// One pod's animated position/width -- `x` is the pod's left edge relative
/// to the row's own origin, `w` its full outer width (label strip + content
/// + padding, i.e. what `module_frame_at` calls `outer_rect.width()`), each
/// a spring so a change in the row's module set moves *every* pod's `x`
/// and `w` together as one continuous rebalancing motion rather than each
/// module jumping straight to its new slot (Chris, 2026-09-24, on the
/// Quickshell layout-morph reference: pods "shift and shuffle"). `x` and
/// `w` retarget together, from the same spring, every frame the packed
/// layout changes -- including frames where nothing *looks* different to
/// the caller, since a still-settling neighbour keeps nudging every later
/// pod's target `x` until it settles too.
#[derive(Clone, Copy, Debug)]
struct PodAnim {
    x: crate::motion::SpringTween,
    w: crate::motion::SpringTween,
    /// When this pod's *content* started fading in -- either just now (a
    /// brand-new pod) or the moment a morph replaced its predecessor. Only
    /// used for the content cross-fade opacity; position/width are the
    /// springs above.
    content_started: f64,
}

/// Per-row animation state, keyed by [`ribbon_panel_modules`]'s `row_id` in
/// `ctx` memory. `pods` is keyed by pod label, which doubles as this
/// function's stable per-pod identity -- see that function's doc comment.
#[derive(Clone, Default)]
struct RibbonRowAnim {
    pods: std::collections::HashMap<&'static str, PodAnim>,
}

/// How long a morph target's content takes to cross-fade in once its
/// outline starts reshaping from the pod it replaced -- `docs/design/
/// 2026-09-24_quickshell-morph-reference.md`: "contents cross-fade (old out
/// in the first ~40%, new in after ~50%)". A single linear ramp over the
/// whole window approximates that close enough for a pod-sized capsule
/// without a second animation channel per pod.
const POD_CONTENT_CROSSFADE_SECS: f64 = 0.22;

/// Render an ordered list of modules -- the generalized form of
/// `ribbon_panel` that also accepts `RibbonModule::Custom` groups (tool
/// option fields, category units, ...) alongside plain button groups, each
/// drawn with the same module-frame treatment so the row reads as one
/// consistent set of modules.
///
/// **Spring-animated pods** (2026-09-24, `docs/design/
/// 2026-09-24_ribbon-pods-spec.md` "Motion" +
/// `docs/design/2026-09-24_quickshell-morph-reference.md`): every pod's
/// outer rect (`x`, `w`) is a spring retargeted every frame from the
/// current packed layout, keyed by `row_id` + the pod's own `label` in
/// `ctx` memory, so:
/// - a pod entering the row grows its width from `0` while its `x` starts
///   already at its target slot (no slide-in from elsewhere);
/// - a pod leaving keeps rendering an emptying capsule outline (no content
///   -- see below) at its last `x`, width springing to `0`, while every
///   other pod's `x`/`w` retarget to the new packed layout *immediately*,
///   so neighbours visibly close the gap rather than waiting;
/// - `morphs` lets a caller declare "this pod (by label) replaces that one
///   from last frame" (e.g. CREATE -> a tool's own pod when a draw tool
///   activates): the new pod's spring state is seeded from the old pod's
///   current `(x, w)` instead of growing from `0`, so the outline reshapes
///   continuously rather than one pod vanishing and another popping in;
///   its content cross-fades in over `POD_CONTENT_CROSSFADE_SECS`.
///
/// **Limit, by design**: a leaving pod that isn't claimed by a `morphs`
/// entry has no content to keep rendering -- this function only ever
/// receives *this* frame's `modules`, and a pod the host stopped passing is
/// gone from that list for good, closure and all. It still shrinks
/// visually (an empty capsule outline, no icons/labels) so neighbours read
/// as sliding into its space rather than jumping, but "the departing pod's
/// own icons visibly shrinking away" would need the host to keep
/// supplying a leaving pod's module for its collapse duration, which no
/// SDB caller does yet -- call this out to Chris rather than fake it.
pub fn ribbon_panel_modules<A: Clone>(
    ui: &mut egui::Ui,
    row_id: egui::Id,
    modules: Vec<RibbonModule<A>>,
    morphs: &[(&'static str, &'static str)],
    host: &impl RibbonHost,
) -> Vec<A> {
    let mut actions: Vec<A> = Vec::new();
    let button_size = host.button_size();
    let row_h = button_size.y + GAP * 2.0;
    // See `ribbon_panel`'s identical lookup for why this is hoisted.
    let frame_style = host.module_frame_style();
    let ctx = ui.ctx().clone();
    let now = ctx.input(|i| i.time);
    let reduce = crate::motion::reduce_motion(&ctx);
    let bounce = crate::motion::bounce_amount(&ctx);
    // "Not too much" (Chris, 2026-09-24): `SNAPPY`'s small bounce for the
    // rebalance, not the flyout's full `BOUNCY` -- a pod-sized capsule
    // sliding across the row reads as busy with a bigger overshoot.
    let rebalance_spring = crate::motion::Spring::SNAPPY.scaled_by_bounce(bounce);
    let exit_spring = crate::motion::Spring::SMOOTH.scaled_by_bounce(bounce);
    let item_spacing = ui.spacing().item_spacing.x;

    struct Placed<'a, A> {
        label: &'static str,
        natural_content_w: f32,
        module: RibbonModule<'a, A>,
    }
    let mut placed: Vec<Placed<'_, A>> = Vec::new();
    for module in modules {
        let (label, natural_content_w) = match &module {
            RibbonModule::Buttons(group) => {
                let w = group_content_width_var(ui, &group.buttons, host, item_spacing);
                (group.label, w)
            }
            RibbonModule::Custom { label, width, .. } => (*label, *width),
        };
        placed.push(Placed {
            label,
            natural_content_w,
            module,
        });
    }

    // Pack target `(x, outer_w)` left to right at natural size -- the
    // layout every pod's spring is chasing this frame.
    let mut cursor = GAP;
    let mut targets: Vec<(f32, f32)> = Vec::with_capacity(placed.len());
    for (i, p) in placed.iter().enumerate() {
        if i > 0 {
            cursor += item_spacing;
        }
        let outer_w = LABEL_STRIP_W + GAP + p.natural_content_w + GAP;
        targets.push((cursor, outer_w));
        cursor += outer_w;
    }
    let total_w = cursor + GAP;

    let anim_key = row_id.with("spatial_ui_kit::ribbon::row_anim");
    let mut anim: RibbonRowAnim = ctx.data(|d| d.get_temp(anim_key)).unwrap_or_default();
    let present: std::collections::HashSet<&'static str> = placed.iter().map(|p| p.label).collect();

    for (p, (tx, tw)) in placed.iter().zip(targets.iter()) {
        let morph_source = morphs
            .iter()
            .find(|(new_label, _)| *new_label == p.label)
            .and_then(|(_, old_label)| anim.pods.remove(old_label));
        anim.pods.entry(p.label).or_insert_with(|| {
            if let Some(seed) = morph_source {
                PodAnim {
                    x: seed.x,
                    w: seed.w,
                    content_started: now,
                }
            } else {
                PodAnim {
                    x: crate::motion::SpringTween::new(*tx),
                    w: crate::motion::SpringTween::new(0.0),
                    content_started: now,
                }
            }
        });
        let pod = anim.pods.get_mut(p.label).expect("just inserted above");
        if reduce {
            pod.x.snap(*tx);
            pod.w.snap(*tw);
        } else {
            pod.x.retarget(now, *tx, rebalance_spring);
            pod.w.retarget(now, *tw, rebalance_spring);
        }
    }

    // Pods no longer present this frame: keep their `PodAnim` (so
    // neighbours' `x` keeps reading a real last-known slot instead of one
    // popping out of existence) and spring their width to `0`; drop them
    // once collapsed. See this function's doc comment for why they can't
    // keep rendering real content.
    anim.pods.retain(|label, pod| {
        if present.contains(label) {
            return true;
        }
        if reduce {
            pod.w.snap(0.0);
        } else {
            pod.w.retarget(now, 0.0, exit_spring);
        }
        pod.w.value(now) > 0.5 || pod.w.is_animating(now)
    });

    let leaving_labels: Vec<&'static str> = anim
        .pods
        .keys()
        .filter(|l| !present.contains(*l))
        .copied()
        .collect();

    let (row_rect, _) =
        ui.allocate_exact_size(egui::vec2(total_w.max(1.0), row_h), egui::Sense::hover());
    let row_origin = row_rect.min;
    let mut animating_any = false;

    // Leaving pods first, so present pods (drawn after) paint on top as
    // they slide across a shrinking neighbour's space.
    for label in leaving_labels {
        let pod = *anim
            .pods
            .get(label)
            .expect("label came from anim.pods.keys() above");
        animating_any |= pod.w.is_animating(now) || pod.x.is_animating(now);
        let w = pod.w.value(now).max(0.0);
        if w < 0.5 {
            continue;
        }
        let x = pod.x.value(now);
        let outer =
            egui::Rect::from_min_size(row_origin + egui::vec2(x, 0.0), egui::vec2(w, row_h));
        let content_w = (w - LABEL_STRIP_W - 2.0 * GAP).max(0.0);
        module_frame_at(ui, outer, label, content_w, row_h, frame_style, 1.0);
    }

    for (p, (tx, tw)) in placed.into_iter().zip(targets.iter()) {
        let pod = *anim
            .pods
            .get(p.label)
            .expect("retargeted into anim.pods above");
        animating_any |= pod.w.is_animating(now) || pod.x.is_animating(now);
        let x = pod.x.value(now);
        let w = pod.w.value(now).max(0.0);
        let outer =
            egui::Rect::from_min_size(row_origin + egui::vec2(x, 0.0), egui::vec2(w, row_h));
        let content_w = (w - LABEL_STRIP_W - 2.0 * GAP).max(0.0);

        let since_content = (now - pod.content_started).max(0.0);
        let content_opacity = if reduce {
            1.0
        } else {
            ((since_content / POD_CONTENT_CROSSFADE_SECS) as f32).clamp(0.0, 1.0)
        };
        if content_opacity < 1.0 {
            animating_any = true;
        }

        let mut child = module_frame_at(
            ui,
            outer,
            p.label,
            content_w,
            row_h,
            frame_style,
            content_opacity,
        );
        match p.module {
            RibbonModule::Buttons(group) => draw_button_row(&mut child, &group, host, &mut actions),
            RibbonModule::Custom { render, .. } => actions.extend(render(&mut child)),
        }
        let _ = tx;
        let _ = tw;
    }

    if animating_any {
        ctx.request_repaint();
    }
    ctx.data_mut(|d| d.insert_temp(anim_key, anim));

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

        // Open at t=0, then request closed a frame later (real usage always
        // has *some* elapsed time between frames -- a spring, unlike a
        // fixed-duration tween, settles by distance-to-target, so retargeting
        // at the exact same instant as the previous retarget, with zero
        // elapsed time, is a degenerate case a real frame loop never hits).
        ctx.begin_pass(egui::RawInput {
            time: Some(0.0),
            ..Default::default()
        });
        let opened = crate::motion::spring_presence_with(
            &ctx,
            presence_id,
            true,
            crate::motion::Spring::BOUNCY,
            crate::motion::Spring::SMOOTH,
        );
        let _ = ctx.end_pass();
        assert!(opened.render);

        ctx.begin_pass(egui::RawInput {
            time: Some(0.05),
            ..Default::default()
        });
        let closing = crate::motion::spring_presence_with(
            &ctx,
            presence_id,
            false,
            crate::motion::Spring::BOUNCY,
            crate::motion::Spring::SMOOTH,
        );
        let _ = ctx.end_pass();
        assert!(
            closing.render && !closing.interactive,
            "must still render, non-interactively, right as closing starts"
        );

        // Advance a context pass well past the collapse spring's settling
        // time -- `should_render` is read at that later time, so drive the
        // input clock forward the way `motion.rs`'s own tests do.
        ctx.begin_pass(egui::RawInput {
            time: Some(2.0),
            ..Default::default()
        });
        let settled = crate::motion::spring_presence_with(
            &ctx,
            presence_id,
            false,
            crate::motion::Spring::BOUNCY,
            crate::motion::Spring::SMOOTH,
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

    /// A middle-button (T-join, both sides `Flare`) pod, with the flyout
    /// column narrower than the pod -- the realistic case. No point of the
    /// combined outline may sit above the pod's own top edge, and the
    /// lowest point must reach the column's bottom.
    #[test]
    fn flow_out_outline_stays_within_pod_and_column_bounds() {
        let pod = egui::Rect::from_min_size(egui::pos2(50.0, 50.0), egui::vec2(200.0, 40.0));
        let column = egui::Rect::from_min_size(egui::pos2(100.0, 90.0), egui::vec2(80.0, 120.0));
        let r = 8.0;

        let outline = flow_out_outline(pod, r, column, r, Join::Flare, Join::Flare);

        for p in &outline {
            assert!(
                p.y >= pod.top() - 1e-4,
                "point {p:?} is above the pod's top"
            );
            assert!(p.x >= pod.left() - 1e-4 && p.x <= pod.right() + 1e-4);
        }
        let max_y = outline.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!(
            (max_y - column.bottom()).abs() < 1e-4,
            "outline must reach the column's bottom"
        );
    }

    /// The outline must never draw the crossed-out seam along the pod
    /// bottom directly under the column -- only points outside the column's
    /// own `[L, R]` span may sit at `y0 = pod.bottom()`.
    #[test]
    fn flow_out_outline_has_no_seam_under_the_column() {
        let pod = egui::Rect::from_min_size(egui::pos2(50.0, 50.0), egui::vec2(200.0, 40.0));
        let column = egui::Rect::from_min_size(egui::pos2(100.0, 90.0), egui::vec2(80.0, 120.0));
        let r = 8.0;
        let y0 = pod.bottom();

        let outline = flow_out_outline(pod, r, column, r, Join::Flare, Join::Flare);
        for w in outline.windows(2) {
            let seg_on_y0 = (w[0].y - y0).abs() < 1e-4 && (w[1].y - y0).abs() < 1e-4;
            if seg_on_y0 {
                let both_outside = (w[0].x <= column.left() || w[0].x >= column.right())
                    && (w[1].x <= column.left() || w[1].x >= column.right());
                assert!(
                    both_outside,
                    "segment {:?}-{:?} lies along the seam under the column",
                    w[0], w[1]
                );
            }
        }
    }

    /// On a `Join::Flush` side, the outline runs a single straight line
    /// collinear with the pod's own edge, from above the (now-absent) pod
    /// corner all the way down to the column's own bottom corner -- no
    /// separate pod corner to drift out of alignment with the column.
    #[test]
    fn flow_out_outline_flush_side_collinear_with_pod_edge() {
        // Column pinned flush to the pod's right edge, as
        // `draw_flyout_column` does for the pod's last button.
        let pod = egui::Rect::from_min_size(egui::pos2(50.0, 50.0), egui::vec2(200.0, 40.0));
        let column = egui::Rect::from_min_size(egui::pos2(170.0, 90.0), egui::vec2(80.0, 120.0));
        assert_eq!(pod.right(), column.right(), "test fixture must be flush");
        let r = 8.0;
        let y0 = pod.bottom();

        let outline = flow_out_outline(pod, r, column, r, Join::Flare, Join::Flush);

        // Every point between the top of the flush run and the column's own
        // bottom corner sits on the pod's right edge -- one straight line,
        // no kink.
        for p in &outline {
            if p.y >= y0 - r - 1e-3 && p.y <= column.bottom() - r + 1e-3 && p.x > pod.right() - r {
                assert!(
                    (p.x - pod.right()).abs() < 1e-3,
                    "point {p:?} is off the pod's flush edge (x = {})",
                    pod.right()
                );
            }
        }
    }

    /// All four of the pod's own corners are rounded (radius `pod_radius`)
    /// except a `Join::Flush` side's bottom corner, which the outline
    /// replaces with a sharp, unrounded transition straight into the
    /// column -- no interior arc points near that corner.
    #[test]
    fn flow_out_outline_rounds_pod_corners_except_flush_bottom() {
        let pod = egui::Rect::from_min_size(egui::pos2(50.0, 50.0), egui::vec2(200.0, 40.0));
        let column = egui::Rect::from_min_size(egui::pos2(170.0, 90.0), egui::vec2(80.0, 120.0));
        let pr = 8.0;

        let outline = flow_out_outline(pod, pr, column, 8.0, Join::Flare, Join::Flush);

        // A rounded corner has at least one point strictly inside both
        // margins around the corner point (curving away from the two
        // straight edges); an unrounded (flush) corner has none.
        let has_curvature_near = |corner: egui::Pos2, sx: f32, sy: f32| {
            outline.iter().any(|p| {
                let dx = (p.x - corner.x) * sx;
                let dy = (p.y - corner.y) * sy;
                dx > 1e-3 && dx < pr - 1e-3 && dy > 1e-3 && dy < pr - 1e-3
            })
        };
        assert!(
            has_curvature_near(pod.left_top(), 1.0, 1.0),
            "top-left corner should be rounded"
        );
        assert!(
            has_curvature_near(pod.right_top(), -1.0, 1.0),
            "top-right corner should be rounded"
        );
        assert!(
            has_curvature_near(pod.left_bottom(), 1.0, -1.0),
            "bottom-left corner (Flare side) should be rounded"
        );
        assert!(
            !has_curvature_near(pod.right_bottom(), -1.0, -1.0),
            "bottom-right corner (Flush side) must not be rounded"
        );
    }

    /// The combined outline is pure geometry with no fixed pixel constants,
    /// so it must scale linearly: doubling every input (pod, column, radii)
    /// about the origin doubles every output point.
    #[test]
    fn flow_out_outline_scales_linearly_with_zoom() {
        let pod = egui::Rect::from_min_size(egui::pos2(50.0, 50.0), egui::vec2(200.0, 40.0));
        let column = egui::Rect::from_min_size(egui::pos2(100.0, 90.0), egui::vec2(80.0, 120.0));
        let r = 8.0;

        let base = flow_out_outline(pod, r, column, r, Join::Flare, Join::Flare);

        let scale = 2.0_f32;
        let scaled_rect = |rect: egui::Rect| {
            egui::Rect::from_min_max(
                (rect.min.to_vec2() * scale).to_pos2(),
                (rect.max.to_vec2() * scale).to_pos2(),
            )
        };
        let scaled = flow_out_outline(
            scaled_rect(pod),
            r * scale,
            scaled_rect(column),
            r * scale,
            Join::Flare,
            Join::Flare,
        );

        assert_eq!(base.len(), scaled.len());
        for (a, b) in base.iter().zip(scaled.iter()) {
            assert!(
                (b.x - a.x * scale).abs() < 1e-2 && (b.y - a.y * scale).abs() < 1e-2,
                "point {a:?} did not scale linearly to {b:?}"
            );
        }
    }

    /// A column too short to fit two full fillet radii still produces a
    /// finite, well-ordered outline (the clamp in `flow_out_outline`).
    #[test]
    fn flow_out_outline_clamps_radius_for_tiny_column() {
        let pod = egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(80.0, 40.0));
        let column = egui::Rect::from_min_size(egui::pos2(100.0, 90.0), egui::vec2(80.0, 2.0));
        let r = 8.0;

        let outline = flow_out_outline(pod, r, column, r, Join::Flare, Join::Flare);
        assert!(outline.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
        assert!(outline.iter().all(|p| p.y >= pod.top() - 1e-4));
        let max_y = outline.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        assert!((max_y - column.bottom()).abs() < 1e-4);
    }

    // ---------------------------------------------------------------
    // `ribbon_panel_modules`'s spring-animated pods (2026-09-24): enter/
    // exit, rebalance-on-insert, and morph. Drives the public function
    // through real egui passes at explicit times, then inspects the
    // private `RibbonRowAnim` this module stores in `ctx` -- same pattern
    // `motion.rs`'s own tests use for `Presence`/`SpringPresence`.
    // ---------------------------------------------------------------

    fn run_row(ctx: &egui::Context, t: f64, row_id: egui::Id, labels: &[&'static str]) {
        ctx.begin_pass(egui::RawInput {
            time: Some(t),
            ..Default::default()
        });
        egui::Area::new(egui::Id::new("pod_test_area"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(ctx, |ui| {
                let host = TestHost;
                let modules: Vec<RibbonModule<i32>> = labels
                    .iter()
                    .map(|&label| {
                        RibbonModule::Buttons(RibbonGroup {
                            label,
                            buttons: vec![test_button()],
                        })
                    })
                    .collect();
                ribbon_panel_modules(ui, row_id, modules, &[], &host);
            });
        let _ = ctx.end_pass();
    }

    fn anim_snapshot(ctx: &egui::Context, row_id: egui::Id) -> RibbonRowAnim {
        let key = row_id.with("spatial_ui_kit::ribbon::row_anim");
        ctx.data(|d| d.get_temp(key)).unwrap_or_default()
    }

    #[test]
    fn new_pod_width_grows_from_zero_then_settles() {
        let ctx = egui::Context::default();
        let row_id = egui::Id::new("test_row_new_pod");
        run_row(&ctx, 0.0, row_id, &["FILE"]);
        let anim = anim_snapshot(&ctx, row_id);
        let pod = anim.pods.get("FILE").unwrap();
        assert_eq!(pod.w.value(0.0), 0.0, "brand-new pod starts at width 0");

        run_row(&ctx, 2.0, row_id, &["FILE"]);
        let anim = anim_snapshot(&ctx, row_id);
        let pod = anim.pods.get("FILE").unwrap();
        assert!(
            pod.w.value(2.0) > 10.0,
            "pod should have grown well past 0 by now"
        );
    }

    #[test]
    fn leaving_pod_shrinks_then_is_dropped() {
        let ctx = egui::Context::default();
        let row_id = egui::Id::new("test_row_leaving");
        run_row(&ctx, 0.0, row_id, &["FILE", "CREATE"]);
        run_row(&ctx, 2.0, row_id, &["FILE", "CREATE"]);

        run_row(&ctx, 2.01, row_id, &["FILE"]);
        let anim = anim_snapshot(&ctx, row_id);
        assert!(
            anim.pods.contains_key("CREATE"),
            "must keep animating a departed pod, not drop it instantly"
        );

        run_row(&ctx, 6.0, row_id, &["FILE"]);
        let anim = anim_snapshot(&ctx, row_id);
        assert!(
            !anim.pods.contains_key("CREATE"),
            "must drop the pod once its collapse settles"
        );
    }

    #[test]
    fn inserting_a_pod_retargets_a_later_pods_x_smoothly() {
        let ctx = egui::Context::default();
        let row_id = egui::Id::new("test_row_rebalance");
        run_row(&ctx, 0.0, row_id, &["FILE", "MODIFY"]);
        run_row(&ctx, 2.0, row_id, &["FILE", "MODIFY"]);
        let modify_x_before = anim_snapshot(&ctx, row_id)
            .pods
            .get("MODIFY")
            .unwrap()
            .x
            .value(2.0);

        run_row(&ctx, 2.01, row_id, &["FILE", "CREATE", "MODIFY"]);
        let anim = anim_snapshot(&ctx, row_id);
        let modify = anim.pods.get("MODIFY").unwrap();
        assert!(
            (modify.x.value(2.01) - modify_x_before).abs() < 1.0,
            "x must not jump instantly when a pod is inserted before it"
        );
        assert!(
            modify.x.target() > modify_x_before,
            "target should have moved right to make room for the new pod"
        );

        run_row(&ctx, 6.0, row_id, &["FILE", "CREATE", "MODIFY"]);
        let modify_x_settled = anim_snapshot(&ctx, row_id)
            .pods
            .get("MODIFY")
            .unwrap()
            .x
            .value(6.0);
        assert!(
            modify_x_settled > modify_x_before + 5.0,
            "must eventually settle at its new, further-right slot"
        );
    }

    #[test]
    fn morph_seeds_new_pod_from_old_pods_current_width() {
        let ctx = egui::Context::default();
        let row_id = egui::Id::new("test_row_morph");
        run_row(&ctx, 0.0, row_id, &["CREATE"]);
        run_row(&ctx, 2.0, row_id, &["CREATE"]);
        let create_w = anim_snapshot(&ctx, row_id)
            .pods
            .get("CREATE")
            .unwrap()
            .w
            .value(2.0);
        assert!(create_w > 10.0);

        ctx.begin_pass(egui::RawInput {
            time: Some(2.01),
            ..Default::default()
        });
        egui::Area::new(egui::Id::new("pod_test_area"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(&ctx, |ui| {
                let host = TestHost;
                let modules = vec![RibbonModule::Buttons(RibbonGroup {
                    label: "LINE_TOOL",
                    buttons: vec![test_button()],
                })];
                ribbon_panel_modules(ui, row_id, modules, &[("LINE_TOOL", "CREATE")], &host);
            });
        let _ = ctx.end_pass();

        let anim = anim_snapshot(&ctx, row_id);
        let morphed_w = anim.pods.get("LINE_TOOL").unwrap().w.value(2.01);
        assert!(
            morphed_w > create_w - 5.0,
            "morph target should start from the old pod's width ({create_w}), not 0 (got {morphed_w})"
        );
        assert!(
            !anim.pods.contains_key("CREATE"),
            "old pod's anim state should be consumed by the morph, not left behind separately"
        );
    }
}
