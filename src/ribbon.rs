//! Ribbon rendering shell, shared between SpatialSketchPad and
//! SpatialDrawingBoard (SDB's first ribbon -- it had none before this).
//!
//! Split the same way SSP's original `panels/ribbon/` was: this module is
//! purely *how to draw* a mode tag + button groups (the "quiet workshop"
//! visual treatment Chris picked 2026-07-12 -- flat borderless buttons,
//! hairline separators, group names in small muted type, color reserved for
//! state, and the activation-flash pulse on click). *What* buttons/groups
//! are visible for a given app state is app-specific business logic and
//! stays in each app's own `panels/ribbon/context.rs`-equivalent, generic
//! over that app's own `Action` type -- `RibbonButton<A>`/`RibbonGroup<A>`
//! here are thin generic containers, not full context computation.
//!
//! Icon drawing needs each app's own icon atlas, so it goes through a
//! [`RibbonHost`] trait implementation, the same pattern `menu::MenuHost`
//! uses.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RibbonMode {
    Draft,
    Edit,
    /// A drawing tool/command is armed; the payload is the short tag label
    /// ("LINE", "ARC", ...).
    Tool(&'static str),
}

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
}

/// How long the clicked-button flash lasts, seconds.
const FLASH_SECS: f64 = 0.28;

fn mode_tag(ui: &mut egui::Ui, mode: &RibbonMode, height: f32) {
    let (text, color, bg) = match mode {
        RibbonMode::Draft => (
            "DRAFT",
            egui::Color32::from_rgb(141, 142, 150),
            egui::Color32::from_rgb(43, 44, 49),
        ),
        RibbonMode::Edit => (
            "EDIT",
            egui::Color32::from_rgb(255, 178, 82),
            egui::Color32::from_rgb(51, 41, 26),
        ),
        RibbonMode::Tool(label) => (
            *label,
            egui::Color32::from_rgb(104, 222, 158),
            egui::Color32::from_rgb(23, 40, 30),
        ),
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(18.0, height), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    painter.rect_filled(rect, 6.0, bg);
    let galley = painter.layout_no_wrap(text.to_string(), egui::FontId::monospace(9.0), color);
    // Rotated 90 CCW: reads bottom-to-top. The layout origin lands at the
    // bottom-left of the rotated text run.
    let pos = egui::pos2(rect.center().x - galley.size().y / 2.0, rect.center().y + galley.size().x / 2.0);
    let shape = egui::epaint::TextShape::new(pos, galley, color).with_angle(-std::f32::consts::FRAC_PI_2);
    painter.add(shape);
}

/// Render the mode tag + button groups. Returns the actions clicked this
/// frame. One row, horizontally scrollable if the groups exceed the window
/// width -- groups must never wrap onto extra rows (each group is measured
/// and allocated exactly, since a bare child inside a horizontal row claims
/// all remaining width and stacks every group onto its own line).
pub fn ribbon_panel<A: Clone>(ui: &mut egui::Ui, mode: &RibbonMode, groups: &[RibbonGroup<A>], host: &impl RibbonHost) -> Vec<A> {
    let mut actions: Vec<A> = Vec::new();

    egui::ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal(|ui| {
            mode_tag(ui, mode, 56.0);
            for (i, group) in groups.iter().enumerate() {
                if i > 0 {
                    ui.separator();
                }
                let spacing_x = ui.spacing().item_spacing.x;
                let group_w = group.buttons.len() as f32 * 40.0 + (group.buttons.len().saturating_sub(1)) as f32 * spacing_x;
                ui.allocate_ui_with_layout(egui::vec2(group_w, 56.0), egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.horizontal(|ui| {
                        for button in &group.buttons {
                            let resp = host.icon_button(ui, button.key, button.label, button.selected, button.enabled, button.disabled_hint);
                            let flash_id = egui::Id::new(("ribbon_flash", button.key));
                            if resp.clicked() {
                                actions.push(button.action.clone());
                                let now = ui.ctx().input(|i| i.time);
                                ui.ctx().data_mut(|d| d.insert_temp(flash_id, now));
                            }
                            // Activation flash: a brief amber pulse over the
                            // clicked button. Painted from egui temp data
                            // (not caller state) since it's pure
                            // presentation.
                            if let Some(t0) = ui.ctx().data(|d| d.get_temp::<f64>(flash_id)) {
                                let dt = ui.ctx().input(|i| i.time) - t0;
                                if dt < FLASH_SECS {
                                    let a = (1.0 - dt / FLASH_SECS) as f32;
                                    let amber = egui::Color32::from_rgb(255, 178, 82);
                                    ui.painter().rect_filled(resp.rect, 6.0, amber.gamma_multiply(0.22 * a));
                                    ui.painter().rect_stroke(resp.rect, 6.0, egui::Stroke::new(1.5, amber.gamma_multiply(a)), egui::StrokeKind::Outside);
                                    ui.ctx().request_repaint();
                                } else {
                                    ui.ctx().data_mut(|d| d.remove::<f64>(flash_id));
                                }
                            }
                        }
                    });
                    ui.label(egui::RichText::new(group.label).size(10.0).color(egui::Color32::from_rgb(132, 133, 141)));
                });
            }
        });
    });

    actions
}
