//! Custom-drawn chrome for in-app `egui::Window` panels/dialogs, styled
//! from a [`crate::theme::ThemePalette`]'s window-chrome tokens. See
//! `docs/design/2026-08-30_theme-system-spec.md` (the host app repo) -- this covers
//! only canvas-drawn panels, never real OS-level secondary viewports,
//! which keep native OS titlebar/min/max/close untouched.
//!
//! Button style deliberately matches native Windows chrome (flat glyphs,
//! no fill at rest, a highlight rect on hover) rather than the original
//! Lunacy mockup's filled circle -- Chris's call, 2026-08-30, after seeing
//! the mockup-accurate version rendered next to his real title bar.

use crate::motion::{self, MotionSpec};
use crate::theme::ThemePalette;
use crate::tokens::{ColorToken, DimensionToken};

/// How far (points) a modal sheet's content starts below its settled
/// position at `content_opacity == 0.0`, sliding up to `0` as it reveals --
/// see `paint_sheet_content`'s use of this. Small and deliberately
/// unobtrusive; the fade + the sheet's own height growth already carry most
/// of the "unveiling" read, this is a secondary cue, not the main motion.
const CONTENT_SLIDE_PX: f32 = 10.0;

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
    /// idea what the host app/the host app stack on top of their own central content.
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
    /// the host app's delete-sheet confirm) that the close button is redundant with
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
    /// `docs/design/2026-08-30_chrome-ideas-sketch.md` (the host app repo), idea 3.
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
        if self.modal {
            return self.show_modal(ctx, open, add_contents);
        }

        if !*open {
            return None;
        }

        let state_key = self.id.with("chrome_state");
        let mut state: ChromeState = ctx.data(|d| d.get_temp(state_key)).unwrap_or_default();

        let ResolvedChrome {
            frame,
            colors: chrome_colors,
            header_h,
            ..
        } = self.resolve_chrome();

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

    /// Resolves this window's theme tokens once into the frame/colors both
    /// `show` (free-floating) and `show_modal` (sheet) paint with -- they
    /// used to duplicate this resolution inline.
    fn resolve_chrome(&self) -> ResolvedChrome {
        let w = &self.palette.window;
        let background = resolved_color(&w.background, egui::Color32::from_rgb(0x22, 0x22, 0x26));
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
            .fill(background)
            .corner_radius(corner_radius)
            .stroke(egui::Stroke::new(border_width, border_color))
            .shadow(egui::Shadow {
                offset: [0, 0],
                blur: shadow_blur,
                spread: 0,
                color: shadow_color,
            })
            .inner_margin(0);

        let colors = ChromeColors {
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

        ResolvedChrome {
            frame,
            colors,
            header_h,
            shadow_blur,
            shadow_color,
        }
    }

    /// The `modal` path of `show`: a macOS-"sheet"-style panel hanging
    /// flush from `sheet_anchor_top`, driven by [`motion::presence_with`]
    /// instead of the one-shot `animate_bool_with_time_and_easing` this
    /// used to call (which only ever ran while `open`, and initialized
    /// unseen ids at their end value -- so the slide-in never actually
    /// played; see the design doc's 2026-09-24 review). Per Chris's sketch
    /// (`docs/design/2026-08-30_chrome-ideas-sketch.md` idea 3, the host app repo):
    /// true macOS sheets attach with no visible seam to what's above them,
    /// so the top edge stays flush at all times -- the reveal animates the
    /// *height* downward from that fixed top edge, like a window shade,
    /// rather than sliding the whole sheet in from above (which would open
    /// a gap under the title bar during a back-out overshoot).
    fn show_modal(
        self,
        ctx: &egui::Context,
        open: &mut bool,
        add_contents: impl FnOnce(&mut egui::Ui),
    ) -> Option<egui::Response> {
        // Spring physics (2026-09-24, Chris: "gently bouncy, Apple spring"
        // feel): `SNAPPY` opening carries a small bounce -- interruptible
        // and velocity-preserving, so a rapid re-open/close doesn't kink --
        // `SMOOTH` closing stays critically damped, no bounce on the way
        // out. Tuned per the Modal family (`motion::MotionFamily::Modal`,
        // `docs/design/2026-09-19_animated-reveals-transforms.md`'s
        // "Per-family tuning") rather than the legacy global bounce amount,
        // so the host app's Settings > Motion > Modal Windows sliders reach this and
        // only this family (`SMOOTH`'s close stays a no-op under bounce,
        // see `Spring::scaled_by_bounce`).
        let presence_frame = motion::spring_presence_family(
            ctx,
            self.id.with("sheet_presence"),
            *open,
            motion::MotionFamily::Modal,
            motion::Spring::SNAPPY,
            motion::Spring::SMOOTH,
        );
        if !presence_frame.render {
            return None;
        }

        let state_key = self.id.with("chrome_state");
        let mut state: ChromeState = ctx.data(|d| d.get_temp(state_key)).unwrap_or_default();

        let ResolvedChrome {
            frame,
            colors: chrome_colors,
            shadow_blur,
            shadow_color,
            ..
        } = self.resolve_chrome();

        let sheet_frame = frame
            .corner_radius(egui::CornerRadius {
                nw: 0,
                ne: 0,
                sw: chrome_colors.corner_radius,
                se: chrome_colors.corner_radius,
            })
            // Shifted down rather than centered on the rect, so the blur
            // doesn't bleed out above the flush top edge -- it reads as the
            // sheet casting a shadow downward onto what's below it, not
            // floating free on all sides.
            .shadow(egui::Shadow {
                offset: [0, (shadow_blur / 2).max(1) as i8],
                blur: shadow_blur,
                spread: 0,
                color: shadow_color,
            });

        let content_opacity = presence_frame.reveal.clamp(0.0, 1.0);
        let backdrop = shadow_color.gamma_multiply(content_opacity);

        // The revealed height, in px, content is clipped to -- a fraction
        // of the sheet's *natural* (fully-open) height, which we only know
        // after laying content out. `None` on the very first frame this
        // id is ever shown (nothing cached yet): that one frame renders
        // unclipped rather than guessing, and every frame after uses the
        // previous frame's measured height, which is stable for the
        // content-sized dialogs this renders (see `paint_sheet_content`).
        let height_key = self.id.with("sheet_natural_height");
        let prior_height: Option<f32> = ctx.data(|d| d.get_temp(height_key));
        let revealed_height_px = prior_height.map(|natural| {
            motion::cap_overshoot(
                presence_frame.reveal.max(0.0) * natural,
                natural,
                MotionSpec::EXPAND_OVERSHOOT_CAP_PX,
            )
        });

        let response = if presence_frame.interactive {
            // Opening/Open: a real blocking modal, same as before -- the
            // backdrop blocks input to the rest of the app, Esc/backdrop-
            // click closes it. We paint the sheet's own frame ourselves
            // (see `paint_sheet_content`), so no frame here.
            let area = egui::Modal::default_area(self.id).anchor(
                egui::Align2::CENTER_TOP,
                egui::vec2(0.0, self.sheet_anchor_top),
            );
            let modal_response = egui::Modal::new(self.id)
                .area(area)
                .frame(egui::Frame::NONE)
                .backdrop_color(backdrop)
                .show(ctx, |ui| {
                    self.paint_sheet_content(
                        ui,
                        ctx,
                        open,
                        &mut state,
                        &chrome_colors,
                        &sheet_frame,
                        add_contents,
                        revealed_height_px,
                        content_opacity,
                        height_key,
                    );
                });
            if modal_response.should_close() {
                *open = false;
            }
            modal_response.response
        } else {
            // Closing: keep rendering the collapse, but stop blocking the
            // rest of the app -- a plain, non-interactable, disabled Area
            // rather than `egui::Modal`, with the backdrop fading out
            // alongside the content.
            let area = egui::Area::new(self.id)
                .order(egui::Order::Foreground)
                .anchor(
                    egui::Align2::CENTER_TOP,
                    egui::vec2(0.0, self.sheet_anchor_top),
                )
                .interactable(false)
                .enabled(false);
            area.show(ctx, |ui| {
                let bg_rect = ui.ctx().content_rect();
                ui.painter().rect_filled(bg_rect, 0.0, backdrop);
                self.paint_sheet_content(
                    ui,
                    ctx,
                    open,
                    &mut state,
                    &chrome_colors,
                    &sheet_frame,
                    add_contents,
                    revealed_height_px,
                    content_opacity,
                    height_key,
                );
            })
            .response
        };

        ctx.data_mut(|d| d.insert_temp(state_key, state));
        Some(response)
    }

    /// Paints the sheet's header/content (via `paint_chrome`) at natural
    /// size, then paints the sheet's own frame/shadow (`sheet_frame`) at
    /// the *revealed* rect on top -- clipping content to it -- so the
    /// frame's bottom edge is what animates, window-shade style, while the
    /// flush top edge never moves. `revealed_height_px` of `None` skips
    /// clipping for this one frame (see `show_modal`). Caches this frame's
    /// measured natural height under `natural_height_key` for the next.
    #[allow(clippy::too_many_arguments)]
    fn paint_sheet_content(
        &self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        open: &mut bool,
        state: &mut ChromeState,
        colors: &ChromeColors,
        sheet_frame: &egui::Frame,
        add_contents: impl FnOnce(&mut egui::Ui),
        revealed_height_px: Option<f32>,
        content_opacity: f32,
        natural_height_key: egui::Id,
    ) {
        // Reserved now, filled in once the natural rect is known below --
        // mirrors `egui::Frame`'s own deferred-background-paint pattern
        // (`Frame::begin`/`Frame::paint`), except sized to the revealed
        // rect rather than the natural one.
        let where_to_put_frame = ui.painter().add(egui::Shape::Noop);

        let top_left = ui.cursor().min;
        if let Some(revealed_height) = revealed_height_px {
            ui.shrink_clip_rect(egui::Rect::from_min_size(
                egui::pos2(f32::NEG_INFINITY, top_left.y),
                egui::vec2(f32::INFINITY, revealed_height.max(0.0)),
            ));
        }
        ui.multiply_opacity(content_opacity);
        // Content slides down into place alongside the fade, instead of
        // sitting laid out at full size behind a fading curtain -- Chris,
        // 2026-09-24 evening verdict: "the sheet's content must animate
        // with the sheet -- a blank sheet that morphs in and is filled
        // afterward is wrong; content is part of the unveiling." The frame
        // itself (`sheet_frame`, painted below from `natural_rect`, whose
        // `min` is `top_left`, unaffected by this) still hangs flush from
        // the fixed top edge -- only the header/content inside slides,
        // shrinking to no offset once `content_opacity` reaches `1.0`, so
        // it costs nothing at rest and doesn't disturb the sheet's own
        // "no gap under the flush top edge" invariant (`sheet_anchor_top`'s
        // doc comment).
        ui.add_space((1.0 - content_opacity).clamp(0.0, 1.0) * CONTENT_SLIDE_PX);

        self.paint_chrome(ui, ctx, open, state, colors, add_contents);

        let natural_rect = ui.min_rect();
        ctx.data_mut(|d| d.insert_temp(natural_height_key, natural_rect.height()));

        let frame_height = match revealed_height_px {
            Some(revealed_height) => {
                revealed_height.min(natural_rect.height() + MotionSpec::EXPAND_OVERSHOOT_CAP_PX)
            }
            None => natural_rect.height(),
        }
        .max(0.0);
        let sheet_rect = egui::Rect::from_min_size(
            natural_rect.min,
            egui::vec2(natural_rect.width(), frame_height),
        );
        ui.painter()
            .set(where_to_put_frame, sheet_frame.paint(sheet_rect));
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

/// `ThemedWindow`'s theme tokens, resolved once by `resolve_chrome` and
/// shared by the free-floating (`show`) and sheet (`show_modal`) paths.
struct ResolvedChrome {
    frame: egui::Frame,
    colors: ChromeColors,
    header_h: f32,
    shadow_blur: u8,
    shadow_color: egui::Color32,
}

#[cfg(test)]
mod modal_tests {
    use super::*;
    use crate::motion::SpringPresence;

    /// Drives one egui pass at time `t` (see `motion.rs`'s own `step` test
    /// helper for why `begin_pass`/`end_pass` rather than `Context::run`).
    fn step<R>(ctx: &egui::Context, t: f64, f: impl FnOnce(&egui::Context) -> R) -> R {
        ctx.begin_pass(egui::RawInput {
            time: Some(t),
            ..Default::default()
        });
        let result = f(ctx);
        let _ = ctx.end_pass();
        result
    }

    /// Mirrors `motion::spring_presence_key`'s (private) key composition --
    /// `show_modal` stores its `SpringPresence` under `self.id.with("sheet_
    /// presence")`, which `spring_presence_with` further salts internally.
    fn reveal_of(ctx: &egui::Context, id: egui::Id, now: f64) -> f32 {
        let key = id
            .with("sheet_presence")
            .with("spatial_ui_kit::motion::spring_presence");
        let presence: SpringPresence = ctx
            .data(|d| d.get_temp(key))
            .expect("presence should exist after showing a modal ThemedWindow at least once");
        presence.reveal(now)
    }

    /// Generous settling time for a spring -- unlike the old fixed-duration
    /// `MotionSpec`, a spring settles asymptotically, so tests wait a fixed
    /// "plenty of time" rather than reading an exact duration constant.
    const SPRING_SETTLE_SECS: f64 = 2.0;

    fn show_sheet(
        ctx: &egui::Context,
        palette: &ThemePalette,
        id_source: &str,
        open: &mut bool,
        t: f64,
    ) -> Option<egui::Response> {
        step(ctx, t, |ctx| {
            ThemedWindow::new(id_source, "Test", palette)
                .modal(true)
                .headerless(true)
                .show(ctx, open, |ui| {
                    ui.label("content");
                })
        })
    }

    #[test]
    fn sheet_reveal_is_less_than_one_on_first_open_frame() {
        let ctx = egui::Context::default();
        let palette = ThemePalette::dark();
        let id = egui::Id::new("modal_regression_a");
        let mut open = true;
        show_sheet(&ctx, &palette, "modal_regression_a", &mut open, 0.0);
        assert!(reveal_of(&ctx, id, 0.0) < 1.0);
    }

    #[test]
    fn sheet_reveal_is_less_than_one_on_second_open_after_closing() {
        let ctx = egui::Context::default();
        let palette = ThemePalette::dark();
        let id = egui::Id::new("modal_regression_b");
        let mut open = true;

        // First open, then settle.
        show_sheet(&ctx, &palette, "modal_regression_b", &mut open, 0.0);
        let settled = SPRING_SETTLE_SECS;
        show_sheet(&ctx, &palette, "modal_regression_b", &mut open, settled);
        assert!((reveal_of(&ctx, id, settled) - 1.0).abs() < 1e-2);

        // Close, then settle -- must stop rendering once fully closed.
        open = false;
        show_sheet(&ctx, &palette, "modal_regression_b", &mut open, settled);
        let closed_at = settled + SPRING_SETTLE_SECS;
        let response = show_sheet(&ctx, &palette, "modal_regression_b", &mut open, closed_at);
        assert!(response.is_none());

        // Reopen: must animate again from 0, not jump straight to 1.0.
        open = true;
        let reopened_at = closed_at + 1.0;
        show_sheet(&ctx, &palette, "modal_regression_b", &mut open, reopened_at);
        assert!(reveal_of(&ctx, id, reopened_at) < 1.0);
    }

    #[test]
    fn sheet_keeps_rendering_while_closing_then_stops() {
        let ctx = egui::Context::default();
        let palette = ThemePalette::dark();
        let mut open = true;

        show_sheet(&ctx, &palette, "modal_regression_c", &mut open, 0.0);
        let settled = SPRING_SETTLE_SECS;
        show_sheet(&ctx, &palette, "modal_regression_c", &mut open, settled);

        open = false;
        let mid_close = settled + 0.05;
        let rendered_mid_close =
            show_sheet(&ctx, &palette, "modal_regression_c", &mut open, mid_close);
        assert!(rendered_mid_close.is_some());

        let after_close = settled + SPRING_SETTLE_SECS;
        let rendered_after_close =
            show_sheet(&ctx, &palette, "modal_regression_c", &mut open, after_close);
        assert!(rendered_after_close.is_none());
    }

    #[test]
    fn non_modal_window_behaves_as_before_no_early_animation_state() {
        // Non-modal windows must not go through `show_modal` at all -- no
        // presence stored for them.
        let ctx = egui::Context::default();
        let palette = ThemePalette::dark();
        let id = egui::Id::new("non_modal_regression");
        let mut open = true;
        step(&ctx, 0.0, |ctx| {
            ThemedWindow::new("non_modal_regression", "Test", &palette).show(
                ctx,
                &mut open,
                |ui| {
                    ui.label("content");
                },
            );
        });
        let stored: Option<SpringPresence> = ctx.data(|d| d.get_temp(id.with("sheet_presence")));
        assert!(stored.is_none());
    }
}
