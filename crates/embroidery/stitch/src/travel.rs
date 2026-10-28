//! Moves inside an area: the needle goes from one place of a fill to another by running stitches
//! inside the shape (straight when the way stays inside, else along the shape's own edge, the
//! shorter way round) instead of a jump that leaves a loose thread across the design.

use kurbo::Point;

use crate::{Polyline, Stitch, StitchKind};

/// A travel stitch is about this long, mm.
pub const TRAVEL_MM: f64 = 2.0;
/// A move is checked every this many mm.
const SAMPLE_MM: f64 = 0.5;
/// A move may run this far outside the edge, mm (where pull compensation puts the fill's own
/// stitches).
pub const EDGE_SLACK_MM: f64 = 0.6;
/// A way round the edge is taken while it is at most this many times the straight distance,
/// plus [`EDGE_EXTRA_MM`]; longer, a jump (with its trim) is the better move.
const EDGE_FACTOR: f64 = 6.0;
const EDGE_EXTRA_MM: f64 = 20.0;

/// Whether `p` is inside the area `rings` (even-odd).
pub fn contains(rings: &[Polyline], p: Point) -> bool {
    let mut inside = false;
    for r in rings {
        for (a, b) in r.points.iter().zip(r.points.iter().cycle().skip(1)).take(r.points.len()) {
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                inside = !inside;
            }
        }
    }
    inside
}

/// Whether `p` is within `slack` of an edge of `rings`.
pub fn near_edge(rings: &[Polyline], p: Point, slack: f64) -> bool {
    rings.iter().any(|r| nearest_on_ring(r, p).is_some_and(|(_, q)| q.distance(p) <= slack))
}

/// Whether the straight way from `a` to `b` stays inside the area, or on its edge within
/// [`EDGE_SLACK_MM`].
pub fn runs_inside(rings: &[Polyline], a: Point, b: Point) -> bool {
    let n = ((a.distance(b) / SAMPLE_MM).ceil() as usize).clamp(1, 10_000);
    (0..=n).all(|k| {
        let p = a.lerp(b, k as f64 / n as f64);
        contains(rings, p) || near_edge(rings, p, EDGE_SLACK_MM)
    })
}

/// The point of closed `ring` nearest `p`, and the index of the point starting its edge.
pub fn nearest_on_ring(ring: &Polyline, p: Point) -> Option<(usize, Point)> {
    let n = ring.points.len();
    let mut best: Option<(usize, Point, f64)> = None;
    for (i, (a, b)) in ring.points.iter().zip(ring.points.iter().cycle().skip(1)).take(n).enumerate() {
        let ab = *b - *a;
        let len2 = ab.hypot2();
        let t = if len2 > 1e-18 { ((p - *a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let q = *a + ab * t;
        let d = q.distance(p);
        if best.is_none_or(|(_, _, bd)| d < bd) {
            best = Some((i, q, d));
        }
    }
    best.map(|(i, q, _)| (i, q))
}

/// The points along `ring` from `qa` (on edge `ea`) to `qb` (on edge `eb`), `qb` included, the
/// shorter way round.
fn along_ring(ring: &Polyline, ea: usize, qa: Point, eb: usize, qb: Point) -> Vec<Point> {
    let n = ring.points.len();
    let pt = |i: usize| ring.points.get(i).copied();
    let (mut forward, mut backward) = (vec![], vec![]);
    if ea != eb {
        let mut i = (ea + 1) % n;
        for _ in 0..n {
            forward.extend(pt(i));
            if i == eb {
                break;
            }
            i = (i + 1) % n;
        }
        let mut i = ea;
        for _ in 0..n {
            if i == eb {
                break;
            }
            backward.extend(pt(i));
            i = (i + n - 1) % n;
        }
    }
    let length = |pts: &[Point]| {
        let mut prev = qa;
        let mut l = 0.0;
        for p in pts.iter().chain(std::iter::once(&qb)) {
            l += prev.distance(*p);
            prev = *p;
        }
        l
    };
    let mut way = if length(&forward) <= length(&backward) { forward } else { backward };
    way.push(qb);
    way
}

/// A way from `a` to `b` that stays inside the area: the points after `a`, up to and including
/// `b`. Straight when that stays inside; else onto the edge of the ring nearest both, along it
/// and off it again. `None` when no such way is short enough (separate parts of a shape, or a
/// long way round).
pub fn inside_way(rings: &[Polyline], a: Point, b: Point) -> Option<Vec<Point>> {
    if runs_inside(rings, a, b) {
        return Some(vec![b]);
    }
    let straight = a.distance(b);
    let mut best: Option<(f64, Vec<Point>)> = None;
    for ring in rings.iter().filter(|r| r.points.len() >= 3) {
        let (Some((ea, qa)), Some((eb, qb))) = (nearest_on_ring(ring, a), nearest_on_ring(ring, b)) else { continue };
        if !runs_inside(rings, a, qa) || !runs_inside(rings, qb, b) {
            continue;
        }
        let mut way = vec![qa];
        way.extend(along_ring(ring, ea, qa, eb, qb));
        way.push(b);
        way.dedup_by(|x, y| x.distance(*y) < 1e-9);
        let mut prev = a;
        let mut length = 0.0;
        for p in &way {
            length += prev.distance(*p);
            prev = *p;
        }
        if best.as_ref().is_none_or(|(bl, _)| length < *bl) {
            best = Some((length, way));
        }
    }
    let (length, way) = best?;
    (length <= straight * EDGE_FACTOR + EDGE_EXTRA_MM).then_some(way)
}

/// Travel stitches along `way` from `a`, about [`TRAVEL_MM`] long, every point of `way` sewn.
pub fn stitches_along(a: Point, way: &[Point], kind: StitchKind) -> Vec<Stitch> {
    let mut out = vec![];
    let mut prev = a;
    for p in way {
        let n = ((prev.distance(*p) / TRAVEL_MM).ceil() as usize).clamp(1, 10_000);
        out.extend((1..=n).map(|k| Stitch::new(prev.lerp(*p, k as f64 / n as f64), kind)));
        prev = *p;
    }
    out
}

/// Each jump inside the area `rings` turned into travel stitches that stay inside it (see
/// [`inside_way`]), so no loose thread crosses the design; a jump with no way inside stays.
pub fn link_jumps(st: &mut Vec<Stitch>, rings: &[Polyline]) {
    if !st.iter().skip(1).any(|s| s.kind == StitchKind::Jump) {
        return;
    }
    let mut out: Vec<Stitch> = Vec::with_capacity(st.len());
    for (i, s) in st.iter().enumerate() {
        let prev = out.last().map(Stitch::point);
        match (s.kind, prev) {
            (StitchKind::Jump, Some(a)) if i > 0 => match inside_way(rings, a, s.point()) {
                Some(way) => {
                    // Travel up to the jump's end, which is then sewn as the stitch it leads to.
                    let mut travel = stitches_along(a, &way, StitchKind::Travel);
                    travel.pop();
                    out.extend(travel);
                    out.push(Stitch::new(s.point(), StitchKind::Travel));
                }
                None => out.push(*s),
            },
            _ => out.push(*s),
        }
    }
    *st = out;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An L: a 10 mm stem up the left of a 40 mm wide, 10 mm tall bar.
    fn ell() -> Vec<Polyline> {
        let p = |x: f64, y: f64| Point::new(x, y);
        vec![Polyline::new(vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 30.0), p(40.0, 30.0), p(40.0, 40.0), p(0.0, 40.0)], true)]
    }

    #[test]
    fn a_straight_way_inside_is_kept() {
        let rings = ell();
        assert_eq!(inside_way(&rings, Point::new(5.0, 5.0), Point::new(5.0, 35.0)), Some(vec![Point::new(5.0, 35.0)]));
    }

    #[test]
    fn a_way_across_a_notch_goes_round_inside() {
        let rings = ell();
        // From the stem's foot to the bar's far end: the straight line crosses the empty corner.
        let (a, b) = (Point::new(9.0, 1.0), Point::new(39.0, 31.0));
        assert!(!runs_inside(&rings, a, b));
        let way = inside_way(&rings, a, b).unwrap_or_default();
        assert_eq!(way.last(), Some(&b));
        let mut prev = a;
        for p in &way {
            assert!(runs_inside(&rings, prev, *p), "{prev:?} -> {p:?}");
            prev = *p;
        }
    }

    #[test]
    fn separate_parts_keep_their_jump() {
        let p = |x: f64, y: f64| Point::new(x, y);
        let sq = |x: f64| Polyline::new(vec![p(x, 0.0), p(x + 10.0, 0.0), p(x + 10.0, 10.0), p(x, 10.0)], true);
        let rings = vec![sq(0.0), sq(30.0)];
        assert_eq!(inside_way(&rings, p(5.0, 5.0), p(35.0, 5.0)), None);
        let mut st = vec![Stitch::new(p(5.0, 5.0), StitchKind::Top), Stitch::new(p(35.0, 5.0), StitchKind::Jump)];
        link_jumps(&mut st, &rings);
        assert_eq!(st.len(), 2);
        assert_eq!(st.get(1).map(|s| s.kind), Some(StitchKind::Jump));
    }

    #[test]
    fn a_jump_inside_becomes_travel() {
        let rings = ell();
        let (a, b) = (Point::new(9.0, 1.0), Point::new(39.0, 31.0));
        let mut st = vec![Stitch::new(a, StitchKind::Top), Stitch::new(b, StitchKind::Jump), Stitch::new(Point::new(30.0, 35.0), StitchKind::Top)];
        link_jumps(&mut st, &rings);
        assert!(st.iter().all(|s| s.kind != StitchKind::Jump));
        assert!(st.windows(2).all(|w| w.first().zip(w.get(1)).is_some_and(|(x, y)| runs_inside(&rings, x.point(), y.point()))));
        assert!(st.iter().any(|s| s.point() == b));
    }
}
