//! The dimmed, input-blocking cover drawn over a window while a child window
//! is open on top of it (walkthrough item 25).
//!
//! Each manager or editor is its own OS window, so a child that is "modal on its
//! parent" cannot rely on egui to stop the parent's clicks: the parent is a
//! different viewport. The simplest thing that is robust on every platform is to
//! have the parent window cover itself. [`modal_scrim`] fills the window with a
//! translucent layer that owns all pointer input, and registers that layer as
//! egui's modal layer for this viewport, which also stops keyboard focus and
//! hover on every widget beneath it. The window is still a normal OS window (it
//! can be moved, and on Windows it is the child's owner so the child stays above
//! it); only its contents are locked.

/// Covers the whole viewport with a dim, input-blocking layer saying which
/// window to finish first. Call it last in the window's frame, every frame the
/// child is open.
pub fn modal_scrim(ctx: &egui::Context, child_title: &str) {
    let rect = ctx.content_rect();
    let area = egui::Area::new(egui::Id::new("spatial_ui_kit_modal_scrim"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .interactable(true);
    let layer = area.layer();
    area.show(ctx, |ui| {
        let (rect, response) = ui.allocate_exact_size(rect.size(), egui::Sense::click_and_drag());
        ui.painter()
            .rect_filled(rect, 0.0, egui::Color32::from_black_alpha(150));
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            format!("Finish in the {child_title} window first"),
            egui::TextStyle::Heading.resolve(ui.style()),
            egui::Color32::from_white_alpha(230),
        );
        response.on_hover_cursor(egui::CursorIcon::NotAllowed);
    });
    ctx.memory_mut(|memory| memory.set_modal_layer(layer));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(ctx: &egui::Context, events: Vec<egui::Event>, clicked: &mut u32) {
        let _ = ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 300.0),
                )),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    if ui.button("Behind").clicked() {
                        *clicked += 1;
                    }
                });
                if ui.memory(|m| m.data.get_temp::<bool>(egui::Id::new("locked"))) == Some(true) {
                    modal_scrim(ui.ctx(), "Pattern Editor");
                }
            },
        );
    }

    fn click_at(ctx: &egui::Context, at: egui::Pos2, clicked: &mut u32) {
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        run(ctx, vec![egui::Event::PointerMoved(at)], clicked);
        run(ctx, vec![button(true)], clicked);
        run(ctx, vec![button(false)], clicked);
        run(ctx, Vec::new(), clicked);
    }

    /// A button behind the scrim does not react to a click; once the scrim is
    /// gone it does.
    #[test]
    fn the_scrim_blocks_clicks_on_the_window_beneath_it() {
        let ctx = egui::Context::default();
        let mut clicked = 0;
        run(&ctx, Vec::new(), &mut clicked);
        run(&ctx, Vec::new(), &mut clicked);
        // The default button sits near the top-left of the central panel.
        let at = egui::pos2(30.0, 20.0);
        click_at(&ctx, at, &mut clicked);
        assert_eq!(clicked, 1, "the button works with no scrim");

        ctx.memory_mut(|m| m.data.insert_temp(egui::Id::new("locked"), true));
        run(&ctx, Vec::new(), &mut clicked);
        run(&ctx, Vec::new(), &mut clicked);
        click_at(&ctx, at, &mut clicked);
        assert_eq!(clicked, 1, "the scrim must swallow the click");

        ctx.memory_mut(|m| m.data.insert_temp(egui::Id::new("locked"), false));
        run(&ctx, Vec::new(), &mut clicked);
        run(&ctx, Vec::new(), &mut clicked);
        click_at(&ctx, at, &mut clicked);
        assert_eq!(clicked, 2, "and the window works again once it is gone");
    }
}
