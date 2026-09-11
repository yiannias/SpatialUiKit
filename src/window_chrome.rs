//! Custom-drawn chrome for in-app `egui::Window` panels/dialogs, styled
//! from a [`crate::theme::ThemePalette`]'s window-chrome tokens. See
//! `docs/design/2026-08-30_theme-system-spec.md` (SDB repo) -- this covers
//! only canvas-drawn panels, never real OS-level secondary viewports,
//! which keep native OS titlebar/min/max/close untouched.
//!
//! Button style deliberately matches native Windows chrome (flat glyphs,
//! no fill at rest, a highlight rect on hover) rather than the original
//! Lunacy mockup's filled circle -- Chris's call, 2026-08-30, after seeing
//! the mockup-accurate version rendered next to his real title bar.

use crate::theme::ThemePalette;
use crate::tokens::{ColorToken, DimensionToken};

fn resolved_color(token: &ColorToken, fallback: egui::Color32) -> egui::Color32 {
    token.color32().unwrap_or(fallback)
}

fn resolved_px(token: &DimensionToken, fallback: f32) -> f32 {
    token.px().unwrap_or(fallback)
}

/// Which button was clicked in the header this frame.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum ButtonKind {
    Minimize,
    Maximize,
    Close,
}

/// Minimize/maximize/collapsed-to-header UI state. Pure presentation state
/// (not a persisted setting), held in egui's own temporary memory keyed by
/// the window's `Id` -- resets on app restart, same as any other
/// `egui::Window`'s remembered position.
#[derive(Clone, Copy, Default)]
struct ChromeState {
    minimized: bool,
    maximized: bool,
    /// Position to snap back to on the frame after un-maximizing. Taken
    /// (consumed) the first frame it's applied, so the window is free to
    /// be dragged again afterward -- see `ThemedWindow::show`'s "restore"
    /// handling.
    restore_pos: Option<egui::Pos2>,
}

/// Builder for a themed, title-bar-less `egui::Window`. Mirrors
/// `egui::Window`'s own builder style (`new` + chained setters + `show`)
/// rather than a single big function, since more positioning/sizing
/// options are likely to accrete here the way they have on `egui::Window`
/// itself.
pub struct ThemedWindow<'a> {
    id: egui::Id,
    title: String,
    palette: &'a ThemePalette,
    default_pos: Option<egui::Pos2>,
    fixed_size: Option<egui::Vec2>,
    resizable: bool,
    minimizable: bool,
    maximizable: bool,
    headerless: bool,
    modal: bool,
    /// Y (in screen/viewport space) the sheet hangs flush against when
    /// `modal` -- the bottom edge of whatever's above the content area
    /// (menu bar, ribbon), passed in by the caller since this crate has no
    /// idea what SDB/SSP stack on top of their own central content.
    /// Defaults to `0.0` (flush against the very top of the viewport) when
    /// never set.
    sheet_anchor_top: f32,
}

impl<'a> ThemedWindow<'a> {
    pub fn new(
        id_source: impl std::hash::Hash + std::fmt::Debug,
        title: impl Into<String>,
        palette: &'a ThemePalette,
    ) -> Self {
        Self {
            id: egui::Id::new(id_source),
            title: title.into(),
            palette,
            default_pos: None,
            fixed_size: None,
            resizable: true,
            minimizable: false,
            maximizable: false,
            headerless: false,
            modal: false,
            sheet_anchor_top: 0.0,
        }
    }

    pub fn default_pos(mut self, pos: impl Into<egui::Pos2>) -> Self {
        self.default_pos = Some(pos.into());
        self
    }

    pub fn fixed_size(mut self, size: impl Into<egui::Vec2>) -> Self {
        self.fixed_size = Some(size.into());
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.resizable = resizable;
        self
    }

    /// Shows a minimize button that collapses the panel to just its header
    /// row. Off by default -- per Chris, 2026-08-30, minimize/maximize
    /// should "only appear where they are relevant" (e.g. not on a small
    /// alert dialog).
    pub fn minimizable(mut self, minimizable: bool) -> Self {
        self.minimizable = minimizable;
        self
    }

    /// Shows a maximize/restore button that snaps the panel to fill the
    /// current viewport, remembering its prior position to restore on
    /// un-maximize. Off by default, same rationale as `minimizable`.
    pub fn maximizable(mut self, maximizable: bool) -> Self {
        self.maximizable = maximizable;
        self
    }

    /// Renders with no header strip at all -- no title, no close/minimize/
    /// maximize buttons, just the themed frame (background, corner radius,
    /// shadow, border) around `add_contents`. Off by default, purely
    /// additive: every existing caller keeps its header. `minimizable`/
    /// `maximizable` are ignored when this is on, since there's no header
    /// to put their buttons in.
    ///
    /// A headerless window has nothing to drag by, so it's also made
    /// immovable (see `show`) rather than falling back to egui's
    /// drag-anywhere behavior for title-bar-less windows, which would make
    /// clicking the content itself move the window. Intended for dialogs
    /// with a fixed/anchored/centered position where that's correct
    /// anyway -- e.g. a confirmation modal whose only actions are its own
    /// buttons, where the caller decided (product call, 2026-09-11, on
    /// SDB's delete-sheet confirm) that the close button is redundant with
    /// Cancel and the title bar earns nothing.
    pub fn headerless(mut self, headerless: bool) -> Self {
        self.headerless = headerless;
        self
    }

    /// Dims and blocks input to the rest of the window behind it (backed by
    /// `egui::Modal`), instead of floating freely like a normal
    /// `ThemedWindow`. Centered, sized to content -- `default_pos`/
    /// `fixed_size`/`resizable`/minimize/maximize don't apply in this mode.
    /// Off by default; Chris, 2026-08-30: "modal blocking for certain
    /// things" (the bitmap-export sizing dialog specifically), not a
    /// blanket change.
    pub fn modal(mut self, modal: bool) -> Self {
        self.modal = modal;
        self
    }

    /// Where a `modal` sheet hangs flush against, in screen/viewport space
    /// -- pass the bottom edge of whatever sits above the caller's central
    /// content (e.g. `ctx.available_rect().top()` read right after the
    /// menu bar/ribbon panels are laid out, before this is shown). True
    /// macOS sheets attach with no visible seam to what's above them,
    /// rather than floating with a gap underneath -- Chris's sketch,
    /// `docs/design/2026-08-30_chrome-ideas-sketch.md` (SDB repo), idea 3.
    /// No-op on a non-`modal` window.
    pub fn sheet_anchor_top(mut self, y: f32) -> Self {
        self.sheet_anchor_top = y;
        self
    }

    /// Shows the window. `open` follows `egui::Window`'s own convention: the
    /// close button sets it to `false`; the caller checks it afterward and
    /// drops/hides its panel state accordingly. Returns `None` when the
    /// window isn't shown this frame (e.g. `*open` was already `false`, or
    /// egui itself skipped rendering it -- see `egui::Window::show`).
    pub fn show(
        self,
        ctx: &egui::Context,
        open: &mut bool,
        add_contents: impl FnOnce(&mut egui::Ui),
    ) -> Option<egui::Response> {
        if !*open {
            return None;
        }

        let state_key = self.id.with("chrome_state");
        let mut state: ChromeState = ctx.data(|d| d.get_temp(state_key)).unwrap_or_default();

        let w = &self.palette.window;
        let bg = resolved_color(&w.background, egui::Color32::from_rgb(0x22, 0x22, 0x26));
        let corner_radius = resolved_px(&w.corner_radius, 12.0).round() as u8;
        let shadow_blur = resolved_px(&w.shadow_blur, 20.0).round() as u8;
        let shadow_color = resolved_color(&w.shadow_color, egui::Color32::from_black_alpha(0x40));
        let border_width = resolved_px(&w.border_width, 0.0);
        let border_color = resolved_color(&w.border_color, egui::Color32::TRANSPARENT);
        let header_bg = resolved_color(
            &w.header.background,
            egui::Color32::from_rgb(0x2E, 0x2E, 0x32),
        );
        let header_h = resolved_px(&w.header.height, 32.0);
        let header_border = resolved_color(
            &w.header.border_bottom,
            egui::Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, 0x26),
        );
        let title_color = resolved_color(&w.header.title_color, egui::Color32::WHITE);
        let button_hover_bg = resolved_color(
            &w.button.hover_background,
            egui::Color32::from_rgb(0x38, 0x38, 0x3C),
        );
        let button_icon_color = resolved_color(&w.button.icon_color, egui::Color32::WHITE);

        let frame = egui::Frame::new()
            .fill(bg)
            .corner_radius(corner_radius)
            .stroke(egui::Stroke::new(border_width, border_color))
            .shadow(egui::Shadow {
                offset: [0, 0],
                blur: shadow_blur,
                spread: 0,
                color: shadow_color,
            })
            .inner_margin(0);

        let chrome_colors = ChromeColors {
            corner_radius,
            header_bg,
            header_h,
            header_border,
            title_color,
            button_hover_bg,
            button_icon_color,
            // Sheets always attach flush to whatever's above them -- see
            // `sheet_anchor_top`'s doc comment -- so the top corners read as
            // square, matching what they're hanging from, not rounded.
            flush_top: self.modal,
        };

        if self.modal {
            // macOS "sheet" style, per Chris, 2026-08-30: hangs from the top
            // of the parent window and slides/fades in, rather than
            // egui::Modal's plain dead-center default. `animate_bool_with_
            // time_and_easing` eases 0->1 exactly once, the first frame
            // `open` becomes true, then holds at 1 -- a one-shot open
            // animation, not a continuous oscillation.
            //
            // Flush-attach to `sheet_anchor_top`, per Chris's sketch
            // (`docs/design/2026-08-30_chrome-ideas-sketch.md` idea 3, SDB
            // repo): "true macOS sheets attach with no visible seam to the
            // parent window's title bar" -- no rest gap (unlike the first
            // version of this animation, which left a 28px gap and rounded
            // top corners, "stylistically... needs work" per his live
            // review), and the top corners are squared off above so the
            // shadow below is the only visual separation from what it's
            // hanging from.
            let anim_t = ctx.animate_bool_with_time_and_easing(
                self.id.with("sheet_anim"),
                true,
                0.22,
                egui::emath::easing::cubic_out,
            );
            const SLIDE_FROM_PX: f32 = 40.0;
            let offset_y = self.sheet_anchor_top - SLIDE_FROM_PX * (1.0 - anim_t);
            let area = egui::Modal::default_area(self.id)
                .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, offset_y));
            let backdrop = resolved_color(&w.shadow_color, egui::Color32::from_black_alpha(0x40))
                .gamma_multiply(anim_t);

            let sheet_frame = frame
                .corner_radius(egui::CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: corner_radius,
                    se: corner_radius,
                })
                // Shifted down rather than centered on the rect, so the
                // blur doesn't bleed out above the flush top edge -- it
                // reads as the sheet casting a shadow downward onto what's
                // below it, not floating free on all sides.
                .shadow(egui::Shadow {
                    offset: [0, (shadow_blur / 2).max(1) as i8],
                    blur: shadow_blur,
                    spread: 0,
                    color: shadow_color,
                });

            let modal_response = egui::Modal::new(self.id)
                .area(area)
                .frame(sheet_frame)
                .backdrop_color(backdrop)
                .show(ctx, |ui| {
                    ui.multiply_opacity(anim_t);
                    self.paint_chrome(ui, ctx, open, &mut state, &chrome_colors, add_contents);
                });
            if modal_response.should_close() {
                *open = false;
            }
            ctx.data_mut(|d| d.insert_temp(state_key, state));
            return Some(modal_response.response);
        }

        let mut window = egui::Window::new(&self.title)
            .id(self.id)
            .title_bar(false)
            .collapsible(false)
            .resizable(self.resizable && !state.maximized && !state.minimized)
            .movable(!self.headerless)
            .frame(frame);

        if state.maximized {
            let screen = ctx.content_rect();
            window = window.current_pos(screen.min).fixed_size(screen.size());
        } else {
            if let Some(pos) = state.restore_pos.take() {
                window = window.current_pos(pos);
            } else if let Some(pos) = self.default_pos {
                window = window.default_pos(pos);
            }
            if state.minimized {
                let width = self.fixed_size.map_or(300.0, |s| s.x);
                window = window.fixed_size(egui::vec2(width, header_h));
            } else if let Some(size) = self.fixed_size {
                window = window.fixed_size(size);
            }
        }

        let response = window.show(ctx, |ui| {
            self.paint_chrome(ui, ctx, open, &mut state, &chrome_colors, add_contents);
        });

        ctx.data_mut(|d| d.insert_temp(state_key, state));

        response.map(|inner| inner.response)
    }

    /// Header row (title, close/maximize/minimize glyphs) plus `add_contents`
    /// -- shared by both the free-floating (`egui::Window`) and `modal`
    /// (`egui::Modal`) containers in `show`, which differ only in what
    /// wraps this.
    fn paint_chrome(
        &self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        open: &mut bool,
        state: &mut ChromeState,
        colors: &ChromeColors,
        add_contents: impl FnOnce(&mut egui::Ui),
    ) {
        if self.headerless {
            add_contents(ui);
            return;
        }

        let width = ui.available_width();
        let (header_rect, _) =
            ui.allocate_exact_size(egui::vec2(width, colors.header_h), egui::Sense::hover());

        let painter = ui.painter();
        let top_rounding = egui::CornerRadius {
            nw: if colors.flush_top {
                0
            } else {
                colors.corner_radius
            },
            ne: if colors.flush_top {
                0
            } else {
                colors.corner_radius
            },
            sw: 0,
            se: 0,
        };
        painter.rect_filled(header_rect, top_rounding, colors.header_bg);
        painter.line_segment(
            [header_rect.left_bottom(), header_rect.right_bottom()],
            egui::Stroke::new(1.0, colors.header_border),
        );
        painter.text(
            header_rect.center(),
            egui::Align2::CENTER_CENTER,
            &self.title,
            egui::FontId::proportional(13.0),
            colors.title_color,
        );

        let mut clicked: Option<ButtonKind> = None;
        let button_w = 32.0;
        let mut next_right = header_rect.right();
        let mut button = |ui: &mut egui::Ui, kind: ButtonKind, next_right: &mut f32| {
            let rect = egui::Rect::from_min_size(
                egui::pos2(*next_right - button_w, header_rect.top()),
                egui::vec2(button_w, colors.header_h),
            );
            *next_right -= button_w;
            let response = ui.interact(
                rect,
                self.id.with(("chrome_button", kind)),
                egui::Sense::click(),
            );
            if response.hovered() {
                ui.painter().rect_filled(rect, 0, colors.button_hover_bg);
            }
            let icon_stroke = egui::Stroke::new(1.0, colors.button_icon_color);
            let icon = rect.shrink(11.0);
            match kind {
                ButtonKind::Minimize => {
                    let y = icon.bottom();
                    ui.painter().line_segment(
                        [egui::pos2(icon.left(), y), egui::pos2(icon.right(), y)],
                        icon_stroke,
                    );
                }
                ButtonKind::Maximize => {
                    ui.painter()
                        .rect_stroke(icon, 0, icon_stroke, egui::StrokeKind::Inside);
                }
                ButtonKind::Close => {
                    ui.painter()
                        .line_segment([icon.left_top(), icon.right_bottom()], icon_stroke);
                    ui.painter()
                        .line_segment([icon.left_bottom(), icon.right_top()], icon_stroke);
                }
            }
            if response.clicked() {
                clicked = Some(kind);
            }
        };
        button(ui, ButtonKind::Close, &mut next_right);
        if self.maximizable {
            button(ui, ButtonKind::Maximize, &mut next_right);
        }
        if self.minimizable {
            button(ui, ButtonKind::Minimize, &mut next_right);
        }

        match clicked {
            Some(ButtonKind::Close) => *open = false,
            Some(ButtonKind::Minimize) => state.minimized = !state.minimized,
            Some(ButtonKind::Maximize) => {
                if state.maximized {
                    state.restore_pos = ctx.memory(|m| m.area_rect(self.id)).map(|r| r.min);
                    state.maximized = false;
                } else {
                    state.maximized = true;
                }
            }
            None => {}
        }

        if !state.minimized {
            add_contents(ui);
        }
    }
}

struct ChromeColors {
    corner_radius: u8,
    header_bg: egui::Color32,
    header_h: f32,
    header_border: egui::Color32,
    title_color: egui::Color32,
    button_hover_bg: egui::Color32,
    button_icon_color: egui::Color32,
    flush_top: bool,
}
