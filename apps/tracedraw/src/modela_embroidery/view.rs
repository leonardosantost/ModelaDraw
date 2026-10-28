//! The embroidery views over the canvas: the hoop round the page (the page
//! is the hoop's sewing field), the stitches as lines or as realistic lit
//! thread, stitches past the field's edge in red, the density map and the
//! sew-out player's needle.
//!
//! Everything comes from the cached preview; a frame only maps the
//! stitches to the screen, culled to the canvas.

use super::sew::{Preview, THREAD_MM};
use egui::{Color32, CornerRadius, Painter, Pos2, Rect as ERect, Shape as EShape, Stroke};
use tracedraw_core::geometry::Point;

/// The views: the artwork, the stitches as lines, realistic thread.
pub const VECTOR: u8 = 0;
pub const STITCHES: u8 = 1;
pub const REALISTIC: u8 = 2;

/// A jump without a cut (the thread lies across the fabric) is drawn in
/// this.
pub const LOOSE: Color32 = Color32::from_rgba_unmultiplied_const(214, 140, 0, 220);
/// Stitches past the field's edge are drawn in this.
pub const OUTSIDE: Color32 = Color32::from_rgb(220, 38, 38);
/// A stitch may end this far past the edge (a machine file's rounding), mm.
const OUTSIDE_SLACK: f64 = 0.15;
/// Most stitches drawn red past the edge.
const MAX_OUTSIDE: usize = 200_000;
/// The hoop's frame reaches this far round the sewing field, mm.
const FRAME_MM: f64 = 12.0;

/// The fabric under the stitches.
pub fn fabric(realistic: bool) -> Color32 {
    if realistic {
        Color32::from_rgb(236, 233, 226)
    } else {
        Color32::from_rgb(250, 250, 248)
    }
}

/// The thread's colour (0..1) as a surface: 0.04 to 0.94, mid tones kept.
fn base(rgb: [f64; 3]) -> [f64; 3] {
    rgb.map(|v| 0.04 + v.clamp(0.0, 1.0).powf(1.25) * 0.90)
}

/// Linear up to 0.8, then a soft roll-off towards 1 instead of clipping.
fn tone(x: f64) -> f64 {
    const K: f64 = 0.8;
    if x < K {
        x.max(0.0)
    } else {
        K + (1.0 - K) * (1.0 - (-(x - K) / (1.0 - K)).exp())
    }
}

/// Thread `c` lit by `light` (1: its average) plus a white `sheen`: light
/// and sheen show on white and black thread too.
pub fn shade(c: [u8; 3], light: f64, sheen: f64) -> Color32 {
    let b = base(c.map(|v| f64::from(v) / 255.0));
    let m = |v: f64| {
        (tone(v * 0.92 * light + sheen) * 255.0)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(m(b[0]), m(b[1]), m(b[2]))
}

/// Whether page point `p` is past the sewing field (the page, `w` x `h`
/// mm; a circle of diameter `w` when `round`).
pub fn outside(p: Point, w: f64, h: f64, round: bool) -> bool {
    let (dx, dy) = (p.x - w / 2.0, p.y - h / 2.0);
    if round {
        dx.hypot(dy) > w / 2.0 + OUTSIDE_SLACK
    } else {
        dx.abs() > w / 2.0 + OUTSIDE_SLACK || dy.abs() > h / 2.0 + OUTSIDE_SLACK
    }
}

/// The sewn stitches with an end past the field, as (from, to) on the page.
pub fn outside_segments(pv: &Preview, round: bool) -> Vec<(Point, Point)> {
    let (w, h) = pv.page;
    let mut segs = vec![];
    let mut prev: Option<Point> = None;
    for s in &pv.stitches {
        if s.sew && outside(s.p, w, h, round) {
            segs.push((prev.filter(|_| !s.trim).unwrap_or(s.p), s.p));
            if segs.len() >= MAX_OUTSIDE {
                break;
            }
        }
        prev = Some(s.p);
    }
    segs
}

/// The page drawn as the hoop: a frame round the sewing field, its edge
/// dashed (red when stitches fall past it), a round hoop's corners shaded,
/// the stitches past the edge in red, and the hoop's name.
#[allow(clippy::too_many_arguments)]
pub fn hoop(
    painter: &Painter,
    field: ERect,
    zoom: f32,
    round: bool,
    name: &str,
    outside: &[(Point, Point)],
    to_screen: impl Fn(Point) -> Pos2,
) {
    let pad = (FRAME_MM as f32 * zoom).max(4.0);
    let frame = field.expand(pad);
    let ring = Color32::from_rgba_unmultiplied(120, 120, 128, 150);
    let edge = if outside.is_empty() {
        Color32::from_rgb(40, 140, 220)
    } else {
        OUTSIDE
    };
    let band = Stroke::new((pad * 0.5).max(3.0), ring);
    let dash = |pts: &[Pos2]| EShape::dashed_line(pts, Stroke::new(1.5, edge), 6.0, 4.0);
    if round {
        painter.circle_stroke(frame.center(), frame.width() / 2.0, band);
        let r = field.width() / 2.0;
        let arc = |i: usize| {
            field.center() + egui::Vec2::angled(i as f32 / 96.0 * std::f32::consts::TAU) * r
        };
        // The page's corners lie outside a round field: each shaded as a
        // fan from the corner to its quarter of the circle.
        let shade = Color32::from_black_alpha(46);
        let mut mesh = egui::Mesh::default();
        for (corner, quarter) in [
            (field.right_bottom(), 0),
            (field.left_bottom(), 1),
            (field.left_top(), 2),
            (field.right_top(), 3),
        ] {
            let base = mesh.vertices.len() as u32;
            mesh.colored_vertex(corner, shade);
            for k in 0..=24 {
                mesh.colored_vertex(arc(quarter * 24 + k), shade);
            }
            for k in 0..24 {
                mesh.add_triangle(base, base + 1 + k, base + 2 + k);
            }
        }
        painter.add(EShape::mesh(mesh));
        let pts: Vec<Pos2> = (0..=96).map(arc).collect();
        painter.extend(dash(&pts));
    } else {
        let corner = CornerRadius::same((pad * 1.4).clamp(4.0, 120.0) as u8);
        painter.rect_stroke(frame, corner, band, egui::StrokeKind::Middle);
        let pts = [
            field.left_top(),
            field.right_top(),
            field.right_bottom(),
            field.left_bottom(),
            field.left_top(),
        ];
        painter.extend(dash(&pts));
    }
    if !outside.is_empty() {
        let width = (THREAD_MM as f32 * zoom).max(2.0);
        let cull = painter.clip_rect().expand(width);
        let shapes: Vec<EShape> = outside
            .iter()
            .map(|(a, b)| (to_screen(*a), to_screen(*b)))
            .filter(|(a, b)| ERect::from_two_pos(*a, *b).intersects(cull))
            .map(|(a, b)| {
                if a.distance(b) < 0.5 {
                    EShape::circle_filled(a, width / 2.0, OUTSIDE)
                } else {
                    EShape::line_segment([a, b], Stroke::new(width, OUTSIDE))
                }
            })
            .collect();
        painter.extend(shapes);
    }
    if !name.is_empty() {
        painter.text(
            field.left_top() + egui::vec2(4.0, 4.0),
            egui::Align2::LEFT_TOP,
            name,
            egui::FontId::proportional(11.0),
            edge,
        );
    }
}

/// What a stitch view draws.
pub struct Look {
    pub realistic: bool,
    pub show_jumps: bool,
    pub show_underlay: bool,
    /// Stitches sewn so far (the sew-out player), or all of them.
    pub sewn: usize,
}

/// The stitches, each a line in its thread colour; realistic: a shadow, the
/// lit thread body and a sheen down its middle. Returns where the needle is.
pub fn stitches(
    painter: &Painter,
    pv: &Preview,
    zoom: f32,
    look: &Look,
    to_screen: impl Fn(Point) -> Pos2,
) -> Option<Pos2> {
    let cull = painter.clip_rect().expand(8.0);
    let width = (THREAD_MM as f32 * zoom).max(if look.realistic { 1.0 } else { 0.6 });
    // A move with the thread cut first leaves nothing: dashed. One without
    // a cut leaves the thread lying across: solid, in amber.
    let cut = Stroke::new(1.0, Color32::from_rgba_unmultiplied(120, 120, 120, 150));
    let loose = Stroke::new(1.4, LOOSE);
    let n = look.sewn.min(pv.stitches.len());
    let mut shadows: Vec<EShape> = vec![];
    let mut bodies: Vec<EShape> = Vec::with_capacity(n);
    let mut sheens: Vec<EShape> = vec![];
    let mut prev: Option<Pos2> = None;
    for s in &pv.stitches[..n] {
        let p = to_screen(s.p);
        if let Some(a) = prev {
            let near = cull.contains(a) || cull.contains(p);
            if !s.sew {
                if look.show_jumps && !s.underlay && near && a.distance(p) > 0.5 {
                    if s.trim {
                        bodies.extend(EShape::dashed_line(&[a, p], cut, 4.0, 3.0));
                    } else {
                        bodies.push(EShape::line_segment([a, p], loose));
                    }
                }
            } else if near && !(s.underlay && !look.show_underlay) {
                let rgb = pv.threads.get(s.thread).copied().unwrap_or([0, 0, 0]);
                if look.realistic {
                    let off = egui::vec2(width * 0.12, width * 0.18);
                    shadows.push(EShape::line_segment(
                        [a + off, p + off],
                        Stroke::new(width * 1.1, Color32::from_black_alpha(60)),
                    ));
                    bodies.push(EShape::line_segment(
                        [a, p],
                        Stroke::new(width, shade(rgb, 0.88, 0.0)),
                    ));
                    sheens.push(EShape::line_segment(
                        [a, p],
                        Stroke::new(width * 0.4, shade(rgb, 1.12, 0.12)),
                    ));
                } else {
                    let mut c = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                    if s.underlay {
                        c = c.gamma_multiply(0.45);
                    }
                    bodies.push(EShape::line_segment([a, p], Stroke::new(width, c)));
                }
            }
        }
        prev = Some(p);
    }
    painter.extend(shadows);
    painter.extend(bodies);
    painter.extend(sheens);
    prev
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shading_keeps_white_and_black_shaped() {
        // White still has a highlight above its body, black a lift.
        let body = shade([255, 255, 255], 0.88, 0.0);
        let sheen = shade([255, 255, 255], 1.12, 0.12);
        assert!(sheen.r() > body.r());
        assert!(shade([0, 0, 0], 1.12, 0.12).r() > shade([0, 0, 0], 0.88, 0.0).r());
    }

    #[test]
    fn outside_the_field() {
        assert!(!outside(Point::new(50.0, 50.0), 100.0, 100.0, false));
        assert!(outside(Point::new(100.5, 50.0), 100.0, 100.0, false));
        // A corner of the page is past a round field.
        assert!(outside(Point::new(2.0, 2.0), 100.0, 100.0, true));
        assert!(!outside(Point::new(2.0, 2.0), 100.0, 100.0, false));
    }
}
