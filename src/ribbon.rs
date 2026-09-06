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
}

/// How long the clicked-button flash lasts, seconds.
const FLASH_SECS: f64 = 0.28;

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
    let galley = painter.layout_no_wrap(text.to_string(), egui::FontId::monospace(9.0), fg);
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

/// A group/module's content width -- `n` buttons at `button_w` each, with
/// `n - 1` gaps of the ui's own item spacing between them. Shared by both
/// entry points below and by `module_frame`'s caller, which needs this
/// number before it can allocate the frame.
fn group_content_width(button_count: usize, button_w: f32, spacing_x: f32) -> f32 {
    button_count as f32 * button_w + button_count.saturating_sub(1) as f32 * spacing_x
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
fn module_frame(ui: &mut egui::Ui, label: &str, content_w: f32, row_h: f32) -> egui::Ui {
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
            egui::Stroke::new(1.0, FRAME_STROKE),
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
            MODULE_LABEL_FG,
            MODULE_LABEL_BG,
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

/// Draws one button group's row of icon buttons (no label -- the caller's
/// [`module_frame`] already drew one) into `ui`, which is expected to already
/// be scoped to the group's content area with a left-to-right layout.
fn draw_button_row<A: Clone>(
    ui: &mut egui::Ui,
    group: &RibbonGroup<A>,
    host: &impl RibbonHost,
    actions: &mut Vec<A>,
) {
    for button in &group.buttons {
        let resp = host.icon_button(
            ui,
            button.key,
            button.label,
            button.selected,
            button.enabled,
            button.disabled_hint,
        );
        let flash_id = egui::Id::new(("ribbon_flash", button.key));
        if resp.clicked() {
            actions.push(button.action.clone());
            let now = ui.ctx().input(|i| i.time);
            ui.ctx().data_mut(|d| d.insert_temp(flash_id, now));
        }
        // Activation flash: a brief amber pulse over the clicked button.
        // Painted from egui temp data (not caller state) since it's pure
        // presentation.
        if let Some(t0) = ui.ctx().data(|d| d.get_temp::<f64>(flash_id)) {
            let dt = ui.ctx().input(|i| i.time) - t0;
            if dt < FLASH_SECS {
                let a = (1.0 - dt / FLASH_SECS) as f32;
                let amber = egui::Color32::from_rgb(255, 178, 82);
                ui.painter()
                    .rect_filled(resp.rect, 6.0, amber.gamma_multiply(0.22 * a));
                ui.painter().rect_stroke(
                    resp.rect,
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
                let content_w = group_content_width(
                    group.buttons.len(),
                    button_size.x,
                    ui.spacing().item_spacing.x,
                );
                let mut child = module_frame(ui, group.label, content_w, row_h);
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
                    let content_w = group_content_width(
                        group.buttons.len(),
                        button_size.x,
                        ui.spacing().item_spacing.x,
                    );
                    let mut child = module_frame(ui, group.label, content_w, row_h);
                    draw_button_row(&mut child, &group, host, &mut actions);
                }
                RibbonModule::Custom {
                    label,
                    width,
                    render,
                } => {
                    let mut child = module_frame(ui, label, width, row_h);
                    let acts = render(&mut child);
                    actions.extend(acts);
                }
            }
        }
    });

    actions
}
