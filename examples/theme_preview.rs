//! Throwaway visual check for `window_chrome::ThemedWindow` against the
//! Dark and Light `ThemePalette` built-ins -- not part of the crate's
//! public surface, just a one-off `cargo run --example theme_preview` used
//! while implementing docs/design/2026-08-30_theme-system-spec.md (the host app
//! repo). Safe to delete once the real Themes Panel exists in the host app.

use spatial_ui_kit::theme::ThemePalette;
use spatial_ui_kit::window_chrome::ThemedWindow;

struct PreviewApp {
    dark: ThemePalette,
    light: ThemePalette,
    dark_open: bool,
    light_open: bool,
}

fn with_accent_border(mut palette: ThemePalette) -> ThemePalette {
    use spatial_ui_kit::tokens::{ColorToken, DimensionToken};
    palette.window.border_width = DimensionToken::new(1.0);
    palette.window.border_color = ColorToken::new(egui::Color32::from_rgb(0x3D, 0x8B, 0xFD));
    palette
}

impl eframe::App for PreviewApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ui.painter()
            .rect_filled(ui.max_rect(), 0, egui::Color32::from_gray(200));

        ThemedWindow::new("preview_dark", "Window - Dark", &self.dark)
            .default_pos([60.0, 80.0])
            .fixed_size([420.0, 260.0])
            .minimizable(true)
            .maximizable(true)
            .show(&ctx, &mut self.dark_open, |ui| {
                ui.label("Dark theme body content. This one has min/max buttons.");
            });

        ThemedWindow::new("preview_light", "Window - Light", &self.light)
            .default_pos([540.0, 80.0])
            .fixed_size([300.0, 180.0])
            .show(&ctx, &mut self.light_open, |ui| {
                ui.label("Light theme body content. Close-only, as scoped.");
            });
    }
}

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "ThemedWindow preview",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([900.0, 400.0]),
            persist_window: false,
            ..Default::default()
        },
        Box::new(|_cc| {
            Ok(Box::new(PreviewApp {
                dark: with_accent_border(ThemePalette::dark()),
                light: ThemePalette::light(),
                dark_open: true,
                light_open: true,
            }))
        }),
    )
}
