//! Headless motion-clip recorder for the ribbon pods/flyout and the modal
//! sheet -- built so Chris can judge animation "fluidity" from a GIF instead
//! of relaunching the app (his complaint 2026-09-24: the current motion
//! reads "dry, not fluid or dynamic enough", referencing the Apple iOS/
//! macOS spring feel and `docs/design/2026-09-24_quickshell-morph-reference.md`'s
//! layout-morphing chrome).
//!
//! **Never opens a window** -- `egui::Context` is driven directly (`ctx.run`
//! with synthetic `RawInput`, no `eframe`/`winit`), and frames are
//! rasterized by a small software triangle rasterizer below (texture-
//! mapped, vertex-color-modulated, matching what a GPU backend does closely
//! enough for a design review) rather than opening any GPU surface or OS
//! window. `cargo run --example motion_clips -- <out_dir>`.
//!
//! **Per-family tuning (2026-09-24 evening pass)**: this recorder no longer
//! takes a "subtle"/"expressive" variant argument. The kit now tunes
//! Flyout/Pods/Modal independently (`spatial_ui_kit::motion::MotionFamily`,
//! `FamilyTuning`) with per-family shipped defaults (`MotionFamily::
//! shipped_default`) instead of one global bounce amount -- this clip set
//! records exactly those shipped defaults (no `set_family_tuning`/`set_
//! bounce_amount` override at all), which is what everyone actually ships
//! with. Reduce Motion is still exercised the same way, via `set_reduce_
//! motion`, in variant recordings a caller adds by hand if needed.

use spatial_ui_kit::ribbon::{
    ribbon_panel_modules, FlyoutItem, FlyoutKind, RibbonButton, RibbonFlyout, RibbonGroup,
    RibbonHost, RibbonModule,
};
use spatial_ui_kit::window_chrome::ThemedWindow;
use std::collections::HashMap;

// ---------------------------------------------------------------------
// A minimal `RibbonHost`: flat rounded-rect buttons with a small filled
// glyph dot standing in for a real icon (the host app's own icon atlas isn't
// available to this crate -- it lives in the app). No caption text on the
// button itself, so the only glyphs this recorder has to rasterize are the
// module label pills' uppercase text, which `ribbon.rs` draws regardless of
// host.
// ---------------------------------------------------------------------

struct ClipHost;

impl RibbonHost for ClipHost {
    fn icon_button(
        &self,
        ui: &mut egui::Ui,
        key: &str,
        _label: &str,
        selected: bool,
        enabled: bool,
        _disabled_hint: &str,
    ) -> egui::Response {
        let size = self.button_size();
        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
        if ui.is_rect_visible(rect) {
            let bg = if selected {
                egui::Color32::from_rgb(0x3D, 0x8B, 0xFD)
            } else if resp.hovered() {
                egui::Color32::from_gray(70)
            } else {
                egui::Color32::TRANSPARENT
            };
            if bg != egui::Color32::TRANSPARENT {
                ui.painter().rect_filled(rect, 5.0, bg);
            }
            let dot_color = if enabled {
                egui::Color32::from_gray(225)
            } else {
                egui::Color32::from_gray(110)
            };
            // A stand-in "icon": a filled circle sized/positioned off the
            // button's own key hash, so different buttons look distinct in
            // the clip without needing a real icon atlas.
            let seed = key
                .bytes()
                .fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
            let r = size.x * (0.14 + 0.05 * ((seed % 5) as f32 / 4.0));
            let cx = rect.center().x + (((seed / 5) % 7) as f32 - 3.0) * 1.5;
            let cy = rect.center().y;
            ui.painter().circle_filled(egui::pos2(cx, cy), r, dot_color);
        }
        resp
    }

    fn button_size(&self) -> egui::Vec2 {
        egui::vec2(40.0, 40.0)
    }
}

// ---------------------------------------------------------------------
// Software rasterizer: turns `epaint::ClippedPrimitive`s into an RGBA8
// frame buffer. Deliberately simple (nearest-neighbour texture sampling,
// no MSAA) -- this is for judging motion timing/easing, not pixel-perfect
// output.
// ---------------------------------------------------------------------

struct Canvas {
    w: usize,
    h: usize,
    pixels: Vec<[u8; 4]>,
}

impl Canvas {
    fn new(w: usize, h: usize, bg: egui::Color32) -> Self {
        Self {
            w,
            h,
            pixels: vec![[bg.r(), bg.g(), bg.b(), 255]; w * h],
        }
    }

    fn blend(&mut self, x: usize, y: usize, color: egui::Color32) {
        if x >= self.w || y >= self.h {
            return;
        }
        let a = color.a() as f32 / 255.0;
        if a <= 0.0 {
            return;
        }
        let dst = &mut self.pixels[y * self.w + x];
        for c in 0..3 {
            let s = [color.r(), color.g(), color.b()][c] as f32;
            let d = dst[c] as f32;
            dst[c] = (s * a + d * (1.0 - a)).round().clamp(0.0, 255.0) as u8;
        }
    }

    fn rgba_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.w * self.h * 4);
        for p in &self.pixels {
            out.extend_from_slice(&[p[0], p[1], p[2], 255]);
        }
        out
    }
}

fn sample_texture(img: &egui::ColorImage, u: f32, v: f32) -> egui::Color32 {
    let [w, h] = img.size;
    if w == 0 || h == 0 {
        return egui::Color32::WHITE;
    }
    let x = ((u * w as f32) as i64).clamp(0, w as i64 - 1) as usize;
    let y = ((v * h as f32) as i64).clamp(0, h as i64 - 1) as usize;
    img.pixels[y * w + x]
}

fn modulate(tex: egui::Color32, vertex: egui::Color32) -> egui::Color32 {
    let m = |a: u8, b: u8| ((a as u32 * b as u32) / 255) as u8;
    egui::Color32::from_rgba_premultiplied(
        m(tex.r(), vertex.r()),
        m(tex.g(), vertex.g()),
        m(tex.b(), vertex.b()),
        m(tex.a(), vertex.a()),
    )
}

/// Rasterizes one tessellated frame onto `canvas`, at `pixels_per_point`
/// scale (positions in `shapes` are in points).
fn rasterize(
    canvas: &mut Canvas,
    primitives: &[egui::ClippedPrimitive],
    textures: &HashMap<egui::TextureId, egui::ColorImage>,
    ppp: f32,
) {
    for cp in primitives {
        let egui::epaint::Primitive::Mesh(mesh) = &cp.primitive else {
            continue;
        };
        let Some(tex) = textures.get(&mesh.texture_id) else {
            continue;
        };
        let clip = egui::Rect::from_min_max(
            (cp.clip_rect.min.to_vec2() * ppp).to_pos2(),
            (cp.clip_rect.max.to_vec2() * ppp).to_pos2(),
        );
        for tri in mesh.indices.chunks_exact(3) {
            let v0 = mesh.vertices[tri[0] as usize];
            let v1 = mesh.vertices[tri[1] as usize];
            let v2 = mesh.vertices[tri[2] as usize];
            let p0 = v0.pos.to_vec2() * ppp;
            let p1 = v1.pos.to_vec2() * ppp;
            let p2 = v2.pos.to_vec2() * ppp;

            let min_x = p0.x.min(p1.x).min(p2.x).floor().max(clip.left()).max(0.0);
            let max_x =
                p0.x.max(p1.x)
                    .max(p2.x)
                    .ceil()
                    .min(clip.right())
                    .min(canvas.w as f32);
            let min_y = p0.y.min(p1.y).min(p2.y).floor().max(clip.top()).max(0.0);
            let max_y =
                p0.y.max(p1.y)
                    .max(p2.y)
                    .ceil()
                    .min(clip.bottom())
                    .min(canvas.h as f32);
            if min_x >= max_x || min_y >= max_y {
                continue;
            }

            let area = edge(p0, p1, p2);
            if area.abs() < 1e-6 {
                continue;
            }

            let mut y = min_y as usize;
            while (y as f32) < max_y {
                let mut x = min_x as usize;
                while (x as f32) < max_x {
                    let p = egui::vec2(x as f32 + 0.5, y as f32 + 0.5);
                    let w0 = edge(p1, p2, p) / area;
                    let w1 = edge(p2, p0, p) / area;
                    let w2 = edge(p0, p1, p) / area;
                    if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                        let u = w0 * v0.uv.x + w1 * v1.uv.x + w2 * v2.uv.x;
                        let v = w0 * v0.uv.y + w1 * v1.uv.y + w2 * v2.uv.y;
                        let vc = blend_vertex_colors(v0.color, v1.color, v2.color, w0, w1, w2);
                        let tex_c = sample_texture(tex, u, v);
                        let c = modulate(tex_c, vc);
                        canvas.blend(x, y, c);
                    }
                    x += 1;
                }
                y += 1;
            }
        }
    }
}

fn edge(a: egui::Vec2, b: egui::Vec2, c: egui::Vec2) -> f32 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

fn blend_vertex_colors(
    c0: egui::Color32,
    c1: egui::Color32,
    c2: egui::Color32,
    w0: f32,
    w1: f32,
    w2: f32,
) -> egui::Color32 {
    let mix = |f: fn(&egui::Color32) -> u8| -> u8 {
        (f(&c0) as f32 * w0 + f(&c1) as f32 * w1 + f(&c2) as f32 * w2)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    egui::Color32::from_rgba_premultiplied(
        mix(egui::Color32::r),
        mix(egui::Color32::g),
        mix(egui::Color32::b),
        mix(egui::Color32::a),
    )
}

fn apply_textures_delta(
    textures: &mut HashMap<egui::TextureId, egui::ColorImage>,
    delta: &egui::TexturesDelta,
) {
    for (id, image_delta) in &delta.set {
        let egui::ImageData::Color(new_img) = &image_delta.image;
        if let Some(pos) = image_delta.pos {
            if let Some(existing) = textures.get_mut(id) {
                let [ew, _eh] = existing.size;
                let [nw, nh] = new_img.size;
                for row in 0..nh {
                    for col in 0..nw {
                        let dst_x = pos[0] + col;
                        let dst_y = pos[1] + row;
                        if dst_x < ew {
                            let dst_idx = dst_y * ew + dst_x;
                            if dst_idx < existing.pixels.len() {
                                existing.pixels[dst_idx] = new_img.pixels[row * nw + col];
                            }
                        }
                    }
                }
                continue;
            }
        }
        textures.insert(*id, (**new_img).clone());
    }
    for id in &delta.free {
        textures.remove(id);
    }
}

// ---------------------------------------------------------------------
// Frame driver: runs one egui pass at time `t`, rasterizes it, returns RGBA
// bytes.
// ---------------------------------------------------------------------

struct Recorder {
    ctx: egui::Context,
    textures: HashMap<egui::TextureId, egui::ColorImage>,
    w: usize,
    h: usize,
    bg: egui::Color32,
}

impl Recorder {
    fn new(w: usize, h: usize) -> Self {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(1.5);
        Self {
            ctx,
            textures: HashMap::new(),
            w,
            h,
            bg: egui::Color32::from_rgb(0x20, 0x22, 0x26),
        }
    }

    fn frame(&mut self, t: f64, add_contents: impl FnOnce(&egui::Context)) -> Vec<u8> {
        let raw_input = egui::RawInput {
            time: Some(t),
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(self.w as f32, self.h as f32) / self.ctx.pixels_per_point(),
            )),
            ..Default::default()
        };
        let mut add_contents = Some(add_contents);
        let out = self.ctx.run_ui(raw_input, |ui| {
            if let Some(f) = add_contents.take() {
                f(ui.ctx());
            }
        });
        apply_textures_delta(&mut self.textures, &out.textures_delta);
        let ppp = out.pixels_per_point;
        let primitives = self.ctx.tessellate(out.shapes, ppp);
        let mut canvas = Canvas::new(self.w, self.h, self.bg);
        rasterize(&mut canvas, &primitives, &self.textures, ppp);
        canvas.rgba_bytes()
    }
}

// ---------------------------------------------------------------------
// GIF encoding.
// ---------------------------------------------------------------------

#[allow(dead_code)]
fn debug_png(path: &std::path::Path, rgba: &[u8], w: usize, h: usize) {
    image::save_buffer(path, rgba, w as u32, h as u32, image::ColorType::Rgba8).ok();
}

fn write_gif(path: &std::path::Path, frames: &[Vec<u8>], w: usize, h: usize, fps: u32) {
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame, RgbaImage};

    let file = std::fs::File::create(path).expect("create gif file");
    let mut encoder = GifEncoder::new_with_speed(file, 10);
    encoder.set_repeat(Repeat::Infinite).ok();
    let delay = Delay::from_numer_denom_ms(1000 / fps, 1);
    for f in frames {
        let img = RgbaImage::from_raw(w as u32, h as u32, f.clone()).expect("frame buffer size");
        encoder
            .encode_frame(Frame::from_parts(img, 0, 0, delay))
            .expect("encode gif frame");
    }
}

// ---------------------------------------------------------------------
// Scenario content.
// ---------------------------------------------------------------------

fn output_flyout<A: Clone>(actions: [A; 4]) -> RibbonFlyout<A> {
    let [print, pdf, image, publish] = actions;
    RibbonFlyout {
        kind: FlyoutKind::Variant,
        items: vec![
            FlyoutItem {
                key: "print",
                label: "Print",
                actions: vec![print],
                enabled: true,
                disabled_hint: "",
            },
            FlyoutItem {
                key: "pdf",
                label: "PDF",
                actions: vec![pdf],
                enabled: true,
                disabled_hint: "",
            },
            FlyoutItem {
                key: "image",
                label: "Image",
                actions: vec![image],
                enabled: true,
                disabled_hint: "",
            },
            FlyoutItem {
                key: "publish",
                label: "Publish",
                actions: vec![publish],
                enabled: true,
                disabled_hint: "",
            },
        ],
    }
}

fn file_group(with_flyout: bool) -> RibbonGroup<i32> {
    RibbonGroup {
        label: "FILE",
        buttons: vec![
            RibbonButton {
                key: "link",
                label: "Link",
                action: 1,
                selected: false,
                enabled: true,
                disabled_hint: "",
                flyout: None,
            },
            RibbonButton {
                key: "image",
                label: "Output",
                action: 2,
                selected: false,
                enabled: true,
                disabled_hint: "",
                flyout: if with_flyout {
                    Some(output_flyout([20, 21, 22, 23]))
                } else {
                    None
                },
            },
        ],
    }
}

fn create_group() -> RibbonGroup<i32> {
    RibbonGroup {
        label: "CREATE",
        buttons: vec![
            RibbonButton {
                key: "line",
                label: "Line",
                action: 3,
                selected: false,
                enabled: true,
                disabled_hint: "",
                flyout: None,
            },
            RibbonButton {
                key: "rect",
                label: "Rectangle",
                action: 4,
                selected: false,
                enabled: true,
                disabled_hint: "",
                flyout: None,
            },
            RibbonButton {
                key: "circle",
                label: "Circle",
                action: 5,
                selected: false,
                enabled: true,
                disabled_hint: "",
                flyout: None,
            },
        ],
    }
}

fn line_tool_group() -> RibbonGroup<i32> {
    RibbonGroup {
        label: "LINE",
        buttons: vec![
            RibbonButton {
                key: "endpoint",
                label: "Endpoint",
                action: 6,
                selected: true,
                enabled: true,
                disabled_hint: "",
                flyout: None,
            },
            RibbonButton {
                key: "midpoint",
                label: "Midpoint",
                action: 7,
                selected: false,
                enabled: true,
                disabled_hint: "",
                flyout: None,
            },
        ],
    }
}

/// Sets the `held_open` temp-data flag `button_with_flyout_joined` reads,
/// bypassing real pointer simulation -- see this file's module doc comment.
fn force_flyout(ctx: &egui::Context, key: &str, open: bool) {
    let id_root = egui::Id::new(("ribbon_flyout_button", key));
    ctx.data_mut(|d| d.insert_temp(id_root.with("held_open"), open));
}

fn ribbon_row(
    ctx: &egui::Context,
    host: &ClipHost,
    modules: Vec<RibbonModule<i32>>,
    morphs: &[(&'static str, &'static str)],
) {
    egui::Area::new(egui::Id::new("clip_ribbon_row"))
        .fixed_pos(egui::pos2(24.0, 24.0))
        .show(ctx, |ui| {
            ribbon_panel_modules(ui, egui::Id::new("clip_row"), modules, morphs, host);
        });
}

fn timeline(duration: f64, fps: u32) -> Vec<f64> {
    let n = (duration * fps as f64).round() as usize;
    (0..=n).map(|i| i as f64 / fps as f64).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out_dir = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "motion-clips".to_string());
    std::fs::create_dir_all(&out_dir).expect("create output dir");

    println!("recording shipped per-family defaults, out_dir = {out_dir}");

    const FPS: u32 = 60;
    const W: usize = 900;
    const H: usize = 260;

    let host = ClipHost;
    // No `set_bounce_amount`/`set_family_tuning` call -- `motion::
    // family_tuning` falls back to `MotionFamily::shipped_default` for a
    // `ctx` nothing has tuned, which is exactly the point: this records
    // what every user gets out of the box.
    let mut recorder = Recorder::new(W, H);

    // -------------------------------------------------------------
    // (a) flyout open -> hold -> close.
    // -------------------------------------------------------------
    {
        let mut frames = Vec::new();
        for t in timeline(0.4, FPS) {
            frames.push(t);
        }
        for t in timeline(0.8, FPS) {
            frames.push(0.4 + t);
        }
        for t in timeline(0.6, FPS) {
            frames.push(1.2 + t);
        }
        let open_at = 0.02;
        let close_at = 1.2;
        let mut rgba_frames = Vec::with_capacity(frames.len());
        for &t in &frames {
            let open = t >= open_at && t < close_at;
            let ctx = recorder.ctx.clone();
            force_flyout(&ctx, "image", open);
            let out = recorder.frame(t, |ctx| {
                ribbon_row(
                    ctx,
                    &host,
                    vec![file_group(true).into(), create_group().into()],
                    &[],
                );
            });
            rgba_frames.push(out);
        }
        write_gif(
            &std::path::Path::new(&out_dir).join("a_flyout_open_hold_close.gif"),
            &rgba_frames,
            W,
            H,
            FPS,
        );
        println!(
            "  wrote a_flyout_open_hold_close.gif ({} frames)",
            rgba_frames.len()
        );
        if std::env::var("MOTION_CLIPS_DEBUG_PNG").is_ok() {
            debug_png(
                &std::path::Path::new(&out_dir).join("a_debug_frame40.png"),
                &rgba_frames[40],
                W,
                H,
            );
        }
    }

    // -------------------------------------------------------------
    // (b) rapid open/close reversal.
    // -------------------------------------------------------------
    {
        let mut rgba_frames = Vec::new();
        let mut t = 0.0;
        let mut open = false;
        while t < 1.5 {
            // Flip every ~0.18s -- faster than the flyout's own settle
            // time, so the spring must reverse mid-flight.
            if (t * 100.0) as i64 % 18 == 0 {
                open = !open;
            }
            force_flyout(&recorder.ctx, "image", open);
            let out = recorder.frame(t, |ctx| {
                ribbon_row(
                    ctx,
                    &host,
                    vec![file_group(true).into(), create_group().into()],
                    &[],
                );
            });
            rgba_frames.push(out);
            t += 1.0 / FPS as f64;
        }
        write_gif(
            &std::path::Path::new(&out_dir).join("b_rapid_reversal.gif"),
            &rgba_frames,
            W,
            H,
            FPS,
        );
        println!(
            "  wrote b_rapid_reversal.gif ({} frames)",
            rgba_frames.len()
        );
    }

    // -------------------------------------------------------------
    // (c) ribbon Idle -> Tool Active -> Idle, CREATE pod morphs into LINE.
    // -------------------------------------------------------------
    {
        let mut rgba_frames = Vec::new();
        let mut t = 0.0;
        while t < 0.5 {
            let out = recorder.frame(t, |ctx| {
                ribbon_row(
                    ctx,
                    &host,
                    vec![file_group(false).into(), create_group().into()],
                    &[],
                );
            });
            rgba_frames.push(out);
            t += 1.0 / FPS as f64;
        }
        let tool_active_at = t;
        while t < tool_active_at + 1.0 {
            let out = recorder.frame(t, |ctx| {
                ribbon_row(
                    ctx,
                    &host,
                    vec![file_group(false).into(), line_tool_group().into()],
                    &[("LINE", "CREATE")],
                );
            });
            rgba_frames.push(out);
            t += 1.0 / FPS as f64;
        }
        let back_to_idle_at = t;
        while t < back_to_idle_at + 1.0 {
            let out = recorder.frame(t, |ctx| {
                ribbon_row(
                    ctx,
                    &host,
                    vec![file_group(false).into(), create_group().into()],
                    &[("CREATE", "LINE")],
                );
            });
            rgba_frames.push(out);
            t += 1.0 / FPS as f64;
        }
        write_gif(
            &std::path::Path::new(&out_dir).join("c_idle_tool_idle_morph.gif"),
            &rgba_frames,
            W,
            H,
            FPS,
        );
        println!(
            "  wrote c_idle_tool_idle_morph.gif ({} frames)",
            rgba_frames.len()
        );
    }

    // -------------------------------------------------------------
    // (d) modal sheet open -> close.
    // -------------------------------------------------------------
    {
        let mut rgba_frames = Vec::new();
        let mut t = 0.0;
        let mut open = true;
        let close_at = 1.0;
        let end_at = 1.8;
        while t < end_at {
            if t >= close_at {
                open = false;
            }
            let mut open_flag = open;
            let out = recorder.frame(t, |ctx| {
                let palette = spatial_ui_kit::theme::ThemePalette::dark();
                ThemedWindow::new("clip_modal", "Export", &palette)
                    .modal(true)
                    .fixed_size([360.0, 180.0])
                    .show(ctx, &mut open_flag, |ui| {
                        // Placeholder content with real layout (heading,
                        // rows, a button) rather than a single label -- so
                        // the recorded clip actually shows content sliding/
                        // fading in step with the sheet (`docs/design/
                        // 2026-09-19_animated-reveals-transforms.md`'s
                        // "Chris's verdict": "content is part of the
                        // unveiling", not a blank sheet that fills in after
                        // the fact) instead of one line too small to judge
                        // motion from.
                        ui.heading("Export Drawing");
                        ui.separator();
                        ui.label("Format: PDF");
                        ui.label("Scale: 1:100");
                        ui.add_space(8.0);
                        let _ = ui.button("Export");
                    });
            });
            rgba_frames.push(out);
            t += 1.0 / FPS as f64;
        }
        write_gif(
            &std::path::Path::new(&out_dir).join("d_modal_open_close.gif"),
            &rgba_frames,
            W,
            H,
            FPS,
        );
        println!(
            "  wrote d_modal_open_close.gif ({} frames)",
            rgba_frames.len()
        );
    }

    println!("done: {out_dir}");
}
