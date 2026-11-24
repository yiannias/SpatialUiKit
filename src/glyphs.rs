//! One close X, one chevron (four directions) and one magnifier for every
//! Spatial app (SDB TASK-350, wave 3 of the glyph library). Each used to be
//! drawn several times with different strokes: four close X's (1.0 / 1.3 px
//! and a text character), seven chevrons (stroked V's at 1.3 / 1.5 px, a filled
//! triangle and font characters) and two magnifiers (1.4 px against the
//! icon library's own). They all follow the glyph standard now:
//!
//! - the box is a whole number of physical pixels, on the pixel grid;
//! - the stroke is 7.5% of the box, snapped to 0.5 px, never under 1.0 px;
//! - straight vertices are snapped so the stroke lands on whole pixels.
//!
//! Geometry is authored in a 0..100 unit square and fills the rectangle it is
//! given, so a caller sizes the glyph by the rectangle. Additive: nothing here
//! changes an existing kit item.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke};

/// The three stroke weights, in physical pixels (SDB TASK-350, wave 5): a
/// hairline for borders and window controls, the regular glyph weight, and a
/// bold weight for the canvas. A glyph computes its own weight from its box
/// ([`glyph_width`]); these are for non-glyph chrome strokes.
pub mod stroke {
    /// Borders, dividers, window controls.
    pub const THIN: f32 = 1.0;
    /// The regular glyph weight at the 18 px reference box (7.5%, snapped).
    pub const REGULAR: f32 = 1.5;
    /// Canvas overlays and markers.
    pub const BOLD: f32 = 2.0;

    /// `px` physical pixels as a logical stroke width on `painter`, so the
    /// line is exactly that many pixels wide at any display scale.
    pub fn px(painter: &egui::Painter, px: f32) -> f32 {
        px / painter.pixels_per_point()
    }
}

/// The glyph standard's stroke for a box of `side` logical points on `painter`,
/// as a logical width: 7.5% of the box in physical pixels, snapped to 0.5 px,
/// never under 1.0 px.
pub fn glyph_width(painter: &Painter, side: f32) -> f32 {
    let ppp = painter.pixels_per_point();
    let side_px = (side * ppp).round().max(1.0);
    ((side_px * 0.075 / 0.5).round() * 0.5).max(1.0) / ppp
}

/// Which way a chevron points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

/// A glyph's box mapped onto the pixel grid.
struct Frame<'a> {
    painter: &'a Painter,
    origin: Pos2,
    scale: f32,
    width: f32,
}

impl<'a> Frame<'a> {
    fn new(painter: &'a Painter, rect: Rect) -> Self {
        let ppp = painter.pixels_per_point();
        let side_px = (rect.width().min(rect.height()) * ppp).round().max(1.0);
        let snap = |v: f32| v.round() / ppp;
        let origin = Pos2::new(
            snap(rect.center().x * ppp - side_px / 2.0),
            snap(rect.center().y * ppp - side_px / 2.0),
        );
        let side = side_px / ppp;
        Self {
            painter,
            origin,
            scale: side / 100.0,
            width: glyph_width(painter, side),
        }
    }

    /// A unit-space point on the pixel grid, so a stroke of this width lands on
    /// whole pixels (centred on a pixel for an odd pixel width, on a boundary
    /// for an even one).
    fn at(&self, x: f32, y: f32) -> Pos2 {
        let ppp = self.painter.pixels_per_point();
        let off = if (self.width * ppp).round() as i32 % 2 == 1 {
            0.5
        } else {
            0.0
        };
        let p = Pos2::new(
            self.origin.x + x * self.scale,
            self.origin.y + y * self.scale,
        );
        let snap = |v: f32| ((v * ppp - off).round() + off) / ppp;
        Pos2::new(snap(p.x), snap(p.y))
    }

    fn line(&self, a: (f32, f32), b: (f32, f32), color: Color32) {
        self.painter.line_segment(
            [self.at(a.0, a.1), self.at(b.0, b.1)],
            Stroke::new(self.width, color),
        );
    }

    fn path(&self, pts: &[(f32, f32)], color: Color32) {
        let pts: Vec<Pos2> = pts.iter().map(|&(x, y)| self.at(x, y)).collect();
        self.painter
            .add(Shape::line(pts, Stroke::new(self.width, color)));
    }
}

/// The close X, filling `rect`.
pub fn close_x(painter: &Painter, rect: Rect, color: Color32) {
    let f = Frame::new(painter, rect);
    f.line((0.0, 0.0), (100.0, 100.0), color);
    f.line((100.0, 0.0), (0.0, 100.0), color);
}

/// The unit-space vertices of a chevron pointing `dir`: the standard 28 x 56
/// open "V" centred in the box (the icon library's `nav.*` keys).
pub fn chevron_points(dir: Dir) -> [(f32, f32); 3] {
    match dir {
        Dir::Left => [(62.0, 22.0), (34.0, 50.0), (62.0, 78.0)],
        Dir::Right => [(38.0, 22.0), (66.0, 50.0), (38.0, 78.0)],
        Dir::Up => [(22.0, 62.0), (50.0, 34.0), (78.0, 62.0)],
        Dir::Down => [(22.0, 38.0), (50.0, 66.0), (78.0, 38.0)],
    }
}

/// A chevron pointing `dir` in the box `rect`.
pub fn chevron(painter: &Painter, rect: Rect, dir: Dir, color: Color32) {
    let f = Frame::new(painter, rect);
    f.path(&chevron_points(dir), color);
}

/// The magnifier's lens centre, radius and handle in the 0..100 unit square;
/// the icon library's zoom family shares them, so every magnifier is one shape.
pub const MAGNIFIER_LENS: ((f32, f32), f32) = ((44.0, 42.0), 26.0);
/// The magnifier's handle, from the lens to the corner.
pub const MAGNIFIER_HANDLE: ((f32, f32), (f32, f32)) = ((63.0, 61.0), (86.0, 84.0));

/// A magnifier filling `rect`: the lens upper-left, the handle to the lower-right.
pub fn magnifier(painter: &Painter, rect: Rect, color: Color32) {
    let f = Frame::new(painter, rect);
    let ((cx, cy), r) = MAGNIFIER_LENS;
    let c = f.at(cx, cy);
    painter.circle_stroke(c, r * f.scale, Stroke::new(f.width, color));
    f.line(MAGNIFIER_HANDLE.0, MAGNIFIER_HANDLE.1, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn painter(ppp: f32) -> (egui::Context, Painter) {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(ppp);
        // The scale applies from the first frame on.
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
        let painter = ctx.layer_painter(egui::LayerId::background());
        (ctx, painter)
    }

    /// 7.5% of the box, snapped to 0.5 px, floor 1.0 px (physical).
    #[test]
    fn the_stroke_follows_the_glyph_standard() {
        for (ppp, side, expect_px) in [
            (1.0, 18.0, 1.5),
            (1.0, 10.0, 1.0),
            (1.0, 24.0, 2.0),
            (1.25, 18.0, 1.5),
            (1.5, 18.0, 2.0),
            (2.0, 18.0, 2.5),
        ] {
            let (_ctx, p) = painter(ppp);
            let w = glyph_width(&p, side) * ppp;
            assert!((w - expect_px).abs() < 1e-4, "ppp {ppp} side {side}: {w}");
        }
    }

    #[test]
    fn a_chevron_stays_inside_its_box_in_every_direction() {
        for dir in [Dir::Up, Dir::Down, Dir::Left, Dir::Right] {
            for (x, y) in chevron_points(dir) {
                assert!((0.0..=100.0).contains(&x) && (0.0..=100.0).contains(&y));
            }
        }
        assert_eq!(chevron_points(Dir::Left)[1].0, 34.0);
    }

    #[test]
    fn the_helpers_draw() {
        let (ctx, p) = painter(1.0);
        let rect = Rect::from_min_size(Pos2::new(3.3, 4.7), egui::vec2(18.0, 18.0));
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {
            close_x(&p, rect, Color32::WHITE);
            chevron(&p, rect, Dir::Right, Color32::WHITE);
            magnifier(&p, rect, Color32::WHITE);
        });
    }
}
