//! Automatic start and end points: an object starts where the needle already is, so the move to
//! it is as short as it can be (fewer trims, less thread to cut). Lines sew their parts nearest
//! first, an open line from whichever end is nearer, and a closed one from its nearest point;
//! a tatami fill starts from whichever side is nearer.
//!
//! [`generate_routed`] also ends each object as near the next one's start as it can: of the ways
//! to sew it (a fill from either side, every object forwards or backwards over the same
//! penetrations) it takes the one with the shortest joins on both sides, and adds stitches where
//! they hide: travel inside a fill under its own top stitches to a start placed by hand, a closed
//! line run on along itself to where the next object is nearest.
//!
//! Only changes that keep the object looking the same are made: an E-stitch or rope keeps its
//! direction (its prongs and slant depend on it), Satin Blocks keep the start drawn as rung 0,
//! manual stitches stay as they are, and a column's width profile keeps its direction.

use kurbo::Point;

use crate::{Polyline, Shape, Stitch, StitchKind, StitchSettings, StitchType, generate};

/// [`generate`], starting as near `from` (where the needle is, mm) as the object allows.
pub fn generate_from(shape: &Shape, settings: &StitchSettings, from: Option<Point>) -> Vec<Stitch> {
    let Some(from) = from.filter(|p| p.x.is_finite() && p.y.is_finite()) else { return generate(shape, settings) };
    match (settings.kind, shape) {
        (StitchType::Run | StitchType::Triple | StitchType::Satin, Shape::Paths(ps)) => generate(&Shape::Paths(order(ps, from, true)), settings),
        (StitchType::EStitch | StitchType::Rope, Shape::Paths(ps)) => generate(&Shape::Paths(order(ps, from, false)), settings),
        (StitchType::Tatami, Shape::Area(_) | Shape::Paths(_)) => {
            // The same rows sewn from the other side: whichever starts nearer.
            let a = generate(shape, settings);
            let mut turned = settings.clone();
            turned.fill.angle = (settings.fill.angle + 180.0).rem_euclid(360.0);
            let b = generate(shape, &turned);
            let dist = |v: &[Stitch]| v.first().map_or(f64::INFINITY, |s| s.point().distance(from));
            if dist(&b) + 1e-6 < dist(&a) { b } else { a }
        }
        _ => generate(shape, settings),
    }
}

/// `paths` in sewing order from `from`: each next part the one nearest where the last ended, an
/// open part reversed when its end is nearer (if `reverse`), a closed one started at its point
/// nearest the needle.
pub fn order(paths: &[Polyline], from: Point, reverse: bool) -> Vec<Polyline> {
    let mut left: Vec<&Polyline> = paths.iter().filter(|p| !p.points.is_empty()).collect();
    let mut out = Vec::with_capacity(left.len());
    let mut at = from;
    while !left.is_empty() {
        // The part that can start nearest, and how.
        let mut best: Option<(usize, f64)> = None;
        for (i, p) in left.iter().enumerate() {
            let d = entry_distance(p, at, reverse);
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        let Some((i, _)) = best else { break };
        let p = left.remove(i);
        let routed = enter(p, at, reverse);
        if let Some(end) = exit(&routed) {
            at = end;
        }
        out.push(routed);
    }
    out
}

/// How near `at` the part can start.
fn entry_distance(p: &Polyline, at: Point, reverse: bool) -> f64 {
    if p.closed {
        return nearest_on_ring(p, at).map_or(f64::INFINITY, |(_, q)| q.distance(at));
    }
    let first = p.points.first().map_or(f64::INFINITY, |q| q.distance(at));
    let last = p.points.last().map_or(f64::INFINITY, |q| q.distance(at));
    if reverse { first.min(last) } else { first }
}

/// The part started as near `at` as it can be.
fn enter(p: &Polyline, at: Point, reverse: bool) -> Polyline {
    if p.closed {
        let Some((i, q)) = nearest_on_ring(p, at) else { return p.clone() };
        // Start at q on the edge from point i, round to point i, and back to q.
        let n = p.points.len();
        let mut pts = vec![q];
        pts.extend((1..=n).filter_map(|k| p.points.get((i + k) % n).copied()));
        // Drop q's twin when it is the point itself.
        pts.dedup_by(|a, b| a.distance(*b) < 1e-9);
        if pts.len() > 1 && pts.first().zip(pts.last()).is_some_and(|(a, b)| a.distance(*b) < 1e-9) {
            pts.pop();
        }
        return Polyline::new(pts, true);
    }
    let (first, last) = (p.points.first().copied(), p.points.last().copied());
    match (first, last) {
        (Some(f), Some(l)) if reverse && l.distance(at) < f.distance(at) => Polyline::new(p.points.iter().rev().copied().collect(), false),
        _ => p.clone(),
    }
}

/// Where sewing the part ends.
fn exit(p: &Polyline) -> Option<Point> {
    if p.closed { p.points.first().copied() } else { p.points.last().copied() }
}

/// The point of closed `p` nearest `at`, and the index of the point starting its edge.
fn nearest_on_ring(p: &Polyline, at: Point) -> Option<(usize, Point)> {
    let n = p.points.len();
    let mut best: Option<(usize, Point, f64)> = None;
    for (i, (a, b)) in p.points.iter().zip(p.points.iter().cycle().skip(1)).take(n).enumerate() {
        let ab = *b - *a;
        let len2 = ab.hypot2();
        let t = if len2 > 1e-18 { ((at - *a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let q = *a + ab * t;
        let d = q.distance(at);
        if best.is_none_or(|(_, _, bd)| d < bd) {
            best = Some((i, q, d));
        }
    }
    best.map(|(i, q, _)| (i, q))
}

/// Where an object is asked to start and end, mm (stitch space).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ends {
    /// Where the needle is when the object starts: the last object's end.
    pub from: Option<Point>,
    /// Where the next object starts: the object ends as near it as it can.
    pub toward: Option<Point>,
    /// A start placed by hand: the object starts there.
    pub start: Option<Point>,
    /// An end placed by hand: the object ends there.
    pub end: Option<Point>,
}

/// Travel added inside an object is a stitch about this long, mm.
const TRAVEL_MM: f64 = 2.0;
/// A start or end this near where the stitches already begin or end needs nothing added, mm.
const NEAR_MM: f64 = 0.5;

/// [`generate`], started and ended where `ends` asks: the way of sewing it with the shortest
/// joins (from the needle into its start, from its end toward the next object), then a start or
/// end placed by hand reached by stitches that hide. Manual stitches and lettering keep their
/// own order.
pub fn generate_routed(shape: &Shape, settings: &StitchSettings, ends: &Ends) -> Vec<Stitch> {
    let entry = ends.start.or(ends.from).filter(|p| finite(*p));
    let exit = ends.end.or(ends.toward).filter(|p| finite(*p));
    if settings.kind == StitchType::Manual || settings.lettering.is_some() || (entry.is_none() && exit.is_none()) {
        let mut st = generate_from(shape, settings, entry);
        if let Shape::Area(rings) = shape {
            crate::travel::link_jumps(&mut st, rings);
        }
        return st;
    }
    let mut bases = vec![generate_from(shape, settings, entry)];
    match (settings.kind, shape) {
        (StitchType::Tatami, Shape::Area(_) | Shape::Paths(_)) => {
            // Both sides, whichever generate_from took.
            bases.push(generate(shape, settings));
            let mut turned = settings.clone();
            turned.fill.angle = (settings.fill.angle + 180.0).rem_euclid(360.0);
            bases.push(generate(shape, &turned));
        }
        (_, Shape::Paths(_)) => {
            if let Some(x) = exit {
                bases.push(generate_from(shape, settings, Some(x)));
            }
        }
        _ => {}
    }
    let mut candidates = Vec::with_capacity(bases.len() * 2);
    for b in bases {
        let r = reversed_top(&b, shape);
        candidates.push(b);
        candidates.push(r);
    }
    let cost = |v: &[Stitch]| {
        let first = v.first().map(Stitch::point);
        let last = v.last().map(Stitch::point);
        gap(entry, first) + gap(last, exit)
    };
    // The cheapest; on a tie the earlier one (the object as generate_from sews it comes first),
    // so nothing changes where routing gains nothing.
    let mut best: Option<(f64, Vec<Stitch>)> = None;
    for c in candidates.into_iter().filter(|c| !c.is_empty()) {
        let d = cost(&c);
        if best.as_ref().is_none_or(|(bd, _)| d < *bd - 1e-9) {
            best = Some((d, c));
        }
    }
    let Some((_, mut stitches)) = best else { return vec![] };
    if let Shape::Area(rings) = shape {
        // Moves between the fill's parts stay inside it, under the stitches sewn after them.
        crate::travel::link_jumps(&mut stitches, rings);
        if let Some(start) = ends.start {
            travel_in(&mut stitches, rings, start);
        }
        if let Some(end) = ends.end {
            travel_out(&mut stitches, rings, end);
        }
    }
    if let (Some(x), Shape::Paths(ps), StitchType::Run | StitchType::Triple) = (exit, shape, settings.kind) {
        run_on(&mut stitches, ps, x);
    }
    stitches
}

fn finite(p: Point) -> bool {
    p.x.is_finite() && p.y.is_finite()
}

/// The distance between `a` and `b`, nothing when either is unknown.
fn gap(a: Option<Point>, b: Option<Point>) -> f64 {
    match (a, b) {
        (Some(a), Some(b)) => a.distance(b),
        _ => 0.0,
    }
}

/// `st` with its top stitches sewn the other way round: the same penetrations, so the same
/// look, starting where they ended. The underlay stays first. A jump keeps marking the same gap.
/// The way from the underlay into the top, now longer, stays inside an area (sewn under the
/// top) or becomes a jump.
fn reversed_top(st: &[Stitch], shape: &Shape) -> Vec<Stitch> {
    let k = underlay_end(st);
    let (under, top) = st.split_at(k);
    let mut out = under.to_vec();
    let n = top.len();
    for j in 0..n {
        let i = n - 1 - j;
        let Some(&s) = top.get(i) else { continue };
        let jumped = top.get(i + 1).is_some_and(|next| next.kind == StitchKind::Jump);
        let kind = match s.kind {
            _ if jumped => StitchKind::Jump,
            StitchKind::Jump => StitchKind::Top,
            other => other,
        };
        out.push(Stitch { kind, ..s });
    }
    let first_top_jumps = top.first().is_some_and(|s| s.kind == StitchKind::Jump);
    if let (Some(&last_under), Some(first)) = (under.last(), out.get_mut(k)) {
        let inside = match shape {
            Shape::Area(rings) => inside_all(rings, last_under.point(), first.point()),
            _ => false,
        };
        if first_top_jumps || !inside {
            first.kind = StitchKind::Jump;
        }
    }
    out
}

/// Where the underlay ends: the index after its last stitch (0 without underlay). The moves
/// inside it (its first jump, travel between its parts) belong to it.
fn underlay_end(st: &[Stitch]) -> usize {
    st.iter().rposition(|s| s.kind == StitchKind::Underlay).map_or(0, |i| i + 1)
}

fn inside_all(rings: &[Polyline], a: Point, b: Point) -> bool {
    crate::travel::runs_inside(rings, a, b)
}

/// Travel stitches from `a` to `b`, `b` included.
fn travel(a: Point, b: Point) -> impl Iterator<Item = Stitch> {
    let n = ((a.distance(b) / TRAVEL_MM).ceil() as usize).clamp(1, 10_000);
    (1..=n).map(move |k| Stitch::new(a.lerp(b, k as f64 / n as f64), StitchKind::Travel))
}

/// A fill started at `start` (placed by hand): travel from there inside the area to where its
/// stitches begin, sewn first so its own stitches cover it. Left as it is when the way leaves
/// the area.
fn travel_in(st: &mut Vec<Stitch>, rings: &[Polyline], start: Point) {
    let Some(first) = st.first().map(Stitch::point) else { return };
    if start.distance(first) < NEAR_MM || !inside_all(rings, start, first) {
        return;
    }
    let mut pre: Vec<Stitch> = vec![Stitch::new(start, StitchKind::Travel)];
    let n = ((start.distance(first) / TRAVEL_MM).ceil() as usize).clamp(1, 10_000);
    pre.extend((1..n).map(|k| Stitch::new(start.lerp(first, k as f64 / n as f64), StitchKind::Travel)));
    if let Some(s) = st.first_mut() {
        // The first stitch was where the needle came in; now the way goes on through it.
        if s.kind == StitchKind::Jump {
            s.kind = StitchKind::Travel;
        }
    }
    pre.append(st);
    *st = pre;
}

/// A fill ended at `end` (placed by hand): travel from its last stitch inside the area to there,
/// in the fill's colour over the fill. Left as it is when the way leaves the area.
fn travel_out(st: &mut Vec<Stitch>, rings: &[Polyline], end: Point) {
    let Some(last) = st.last().map(Stitch::point) else { return };
    if last.distance(end) < NEAR_MM || !inside_all(rings, last, end) {
        return;
    }
    st.extend(travel(last, end));
}

/// A closed line that ends far from `toward`: sewn on along itself (over its own stitches, so it
/// hides) to its point nearest `toward`, the shorter way round.
fn run_on(st: &mut Vec<Stitch>, paths: &[Polyline], toward: Point) {
    let Some(last) = st.last().map(Stitch::point) else { return };
    // The ring the line ended on, and where on it.
    let Some((ring, le, lq)) = paths
        .iter()
        .filter(|p| p.closed && p.points.len() >= 3)
        .filter_map(|p| nearest_on_ring(p, last).map(|(e, q)| (p, e, q)))
        .find(|(_, _, q)| q.distance(last) < 0.05)
    else {
        return;
    };
    let Some((te, target)) = nearest_on_ring(ring, toward) else { return };
    if target.distance(toward) + NEAR_MM >= last.distance(toward) {
        return;
    }
    let n = ring.points.len();
    let pt = |i: usize| ring.points.get(i).copied();
    // Forward: the ends of the edges from here to the target's edge; backward: their starts.
    let (mut forward, mut backward) = (vec![], vec![]);
    if le != te {
        let mut i = (le + 1) % n;
        for _ in 0..n {
            forward.extend(pt(i));
            if i == te {
                break;
            }
            i = (i + 1) % n;
        }
        let mut i = le;
        for _ in 0..n {
            if i == te {
                break;
            }
            backward.extend(pt(i));
            i = (i + n - 1) % n;
        }
    }
    let length = |pts: &[Point]| {
        let mut prev = lq;
        let mut l = 0.0;
        for p in pts.iter().chain(std::iter::once(&target)) {
            l += prev.distance(*p);
            prev = *p;
        }
        l
    };
    let way = if length(&forward) <= length(&backward) { forward } else { backward };
    let mut prev = last;
    for p in way.into_iter().chain(std::iter::once(target)) {
        if p.distance(prev) > 1e-9 {
            st.extend(travel(prev, p));
            prev = p;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StitchKind;

    fn line(a: (f64, f64), b: (f64, f64)) -> Polyline {
        Polyline::new(vec![Point::new(a.0, a.1), Point::new(b.0, b.1)], false)
    }

    fn square(x: f64, y: f64, size: f64) -> Polyline {
        Polyline::new(vec![Point::new(x, y), Point::new(x + size, y), Point::new(x + size, y + size), Point::new(x, y + size)], true)
    }

    #[test]
    fn lines_start_at_the_nearer_end_and_parts_go_nearest_first() {
        let ps = [line((0.0, 0.0), (10.0, 0.0)), line((50.0, 0.0), (40.0, 0.0)), line((20.0, 0.0), (30.0, 0.0))];
        let o = order(&ps, Point::new(60.0, 0.0), true);
        let starts: Vec<f64> = o.iter().map(|p| p.points[0].x).collect();
        assert_eq!(starts, [50.0, 30.0, 10.0]);
        // Not reversible: each still starts at its own start.
        let o = order(&ps, Point::new(60.0, 0.0), false);
        assert!(o.iter().all(|p| ps.iter().any(|q| q.points == p.points)));
    }

    #[test]
    fn closed_paths_start_at_the_point_nearest_the_needle() {
        let o = order(&[square(0.0, 0.0, 10.0)], Point::new(5.0, -3.0), true);
        assert_eq!(o[0].points[0], Point::new(5.0, 0.0));
        assert_eq!(o[0].points.len(), 5);
        assert!(o[0].closed);
        // On a corner: no doubled point.
        let o = order(&[square(0.0, 0.0, 10.0)], Point::new(12.0, 12.0), true);
        assert_eq!(o[0].points[0], Point::new(10.0, 10.0));
        assert_eq!(o[0].points.len(), 4);
    }

    #[test]
    fn a_run_and_a_fill_start_near_the_needle() {
        let run = StitchSettings::of(StitchType::Run);
        let st = generate_from(&Shape::Paths(vec![line((0.0, 0.0), (30.0, 0.0))]), &run, Some(Point::new(31.0, 0.0)));
        assert_eq!(st[0].point(), Point::new(30.0, 0.0));
        // Without a needle position: as generate.
        let shape = Shape::Area(vec![square(0.0, 0.0, 20.0)]);
        let fill = StitchSettings::of(StitchType::Tatami);
        assert_eq!(generate_from(&shape, &fill, None), generate(&shape, &fill));
        let plain = generate(&shape, &fill);
        let p0 = plain[0].point();
        // From the opposite corner, the fill starts on that side instead.
        let far = Point::new(20.0 - p0.x, 20.0 - p0.y);
        let st = generate_from(&shape, &fill, Some(far));
        assert!(st[0].point().distance(far) < p0.distance(far) - 5.0, "{:?} {:?}", st[0].point(), p0);
        assert_eq!(st.iter().filter(|s| s.kind == StitchKind::Top).count(), plain.iter().filter(|s| s.kind == StitchKind::Top).count());
    }
    #[test]
    fn a_line_ends_toward_the_next_object() {
        let run = StitchSettings::of(StitchType::Run);
        let shape = Shape::Paths(vec![line((0.0, 0.0), (30.0, 0.0))]);
        let ends = Ends { toward: Some(Point::new(-2.0, 0.0)), ..Ends::default() };
        let st = generate_routed(&shape, &run, &ends);
        assert_eq!(st.last().map(Stitch::point), Some(Point::new(0.0, 0.0)));
        // With nothing asked, as generate_from.
        assert_eq!(generate_routed(&shape, &run, &Ends::default()), generate_from(&shape, &run, None));
    }

    #[test]
    fn reversing_keeps_the_penetrations_and_the_underlay_first() {
        let shape = Shape::Area(vec![square(0.0, 0.0, 20.0)]);
        let fill = StitchSettings::of(StitchType::Tatami);
        let plain = generate(&shape, &fill);
        let rev = reversed_top(&plain, &shape);
        assert_eq!(rev.len(), plain.len());
        let key = |s: &Stitch| ((s.x * 1000.0).round() as i64, (s.y * 1000.0).round() as i64);
        let mut a: Vec<_> = plain.iter().map(key).collect();
        let mut b: Vec<_> = rev.iter().map(key).collect();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b);
        // The fill has underlay, opened by a jump to its start.
        let under = underlay_end(&plain);
        assert!(under > 1 && plain.first().map(|s| s.kind) == Some(StitchKind::Jump));
        // Reversed, the underlay is sewn first all the same, as it was.
        assert_eq!(rev.get(..under), plain.get(..under));
        assert!(rev.iter().skip(under).all(|s| s.kind != StitchKind::Underlay));
        assert_eq!(rev.last().map(Stitch::point), plain.get(under).map(Stitch::point));
    }

    #[test]
    fn a_fill_ends_near_the_next_object() {
        let shape = Shape::Area(vec![square(0.0, 0.0, 20.0)]);
        let fill = StitchSettings::of(StitchType::Tatami);
        let plain = generate(&shape, &fill);
        let Some(end) = plain.last().map(Stitch::point) else { return };
        // The next object waits at the far corner from where the fill would end.
        let toward = Point::new(20.0 - end.x, 20.0 - end.y);
        let st = generate_routed(&shape, &fill, &Ends { toward: Some(toward), ..Ends::default() });
        let got = st.last().map_or(f64::INFINITY, |s| s.point().distance(toward));
        assert!(got < end.distance(toward) - 5.0, "{got} {}", end.distance(toward));
    }

    #[test]
    fn a_closed_line_runs_on_to_where_the_next_object_is_nearest() {
        let run = StitchSettings::of(StitchType::Run);
        let shape = Shape::Paths(vec![square(0.0, 0.0, 10.0)]);
        let ends = Ends { from: Some(Point::new(-1.0, -1.0)), toward: Some(Point::new(11.0, 11.0)), ..Ends::default() };
        let st = generate_routed(&shape, &run, &ends);
        assert_eq!(st.first().map(Stitch::point), Some(Point::new(0.0, 0.0)));
        assert!(st.last().is_some_and(|s| s.point().distance(Point::new(10.0, 10.0)) < 1e-6), "{:?}", st.last());
        // The way on is along the line, over its own stitches.
        assert!(st.iter().all(|s| s.x.abs() < 1e-6 || s.y.abs() < 1e-6 || (s.x - 10.0).abs() < 1e-6 || (s.y - 10.0).abs() < 1e-6));
    }

    #[test]
    fn a_fill_starts_and_ends_where_placed_by_hand_through_hidden_travel() {
        let shape = Shape::Area(vec![square(0.0, 0.0, 20.0)]);
        let fill = StitchSettings::of(StitchType::Tatami);
        let (start, end) = (Point::new(10.0, 10.0), Point::new(15.0, 5.0));
        let st = generate_routed(&shape, &fill, &Ends { start: Some(start), end: Some(end), ..Ends::default() });
        assert_eq!(st.first().map(Stitch::point), Some(start));
        assert_eq!(st.last().map(Stitch::point), Some(end));
        // The way in is travel, sewn before the fill's own stitches.
        assert_eq!(st.first().map(|s| s.kind), Some(StitchKind::Travel));
        // A start outside the shape is not reached through it.
        let st = generate_routed(&shape, &fill, &Ends { start: Some(Point::new(40.0, 40.0)), ..Ends::default() });
        assert_ne!(st.first().map(Stitch::point), Some(Point::new(40.0, 40.0)));
    }
    #[test]
    fn a_fill_in_parts_links_them_inside_itself_with_no_jump() {
        // A U, open at the top: its rows split in two over the arms.
        let p = |x: f64, y: f64| Point::new(x, y);
        let u = Polyline::new(
            vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 20.0), p(20.0, 20.0), p(20.0, 0.0), p(30.0, 0.0), p(30.0, 30.0), p(0.0, 30.0)],
            true,
        );
        let rings = vec![u];
        let shape = Shape::Area(rings.clone());
        let mut fill = StitchSettings::of(StitchType::Tatami);
        fill.fill.angle = 0.0;
        // Sewn as generated, its parts are joined by jumps.
        assert!(generate(&shape, &fill).iter().skip(1).any(|s| s.kind == StitchKind::Jump));
        for ends in [Ends::default(), Ends { from: Some(p(35.0, 35.0)), toward: Some(p(-5.0, -5.0)), ..Ends::default() }] {
            let st = generate_routed(&shape, &fill, &ends);
            assert!(st.len() > 100);
            assert!(st.iter().skip(1).all(|s| s.kind != StitchKind::Jump), "{ends:?}");
            // No stitch crosses the open notch of the U.
            for w in st.windows(2) {
                if let [a, b] = w {
                    assert!(crate::travel::runs_inside(&rings, a.point(), b.point()), "{:?} -> {:?}", a.point(), b.point());
                }
            }
        }
    }
    #[test]
    fn the_underlay_is_always_sewn_before_the_top() {
        let shape = Shape::Area(vec![square(0.0, 0.0, 20.0)]);
        let p = Point::new;
        for kind in [StitchType::Tatami, StitchType::Contour, StitchType::Circular] {
            let fill = StitchSettings::of(kind);
            for ends in [
                Ends::default(),
                Ends { from: Some(p(-5.0, -5.0)), toward: Some(p(25.0, 25.0)), ..Ends::default() },
                Ends { from: Some(p(25.0, 25.0)), toward: Some(p(-5.0, -5.0)), ..Ends::default() },
                Ends { toward: Some(p(25.0, -5.0)), ..Ends::default() },
                Ends { start: Some(p(10.0, 10.0)), end: Some(p(15.0, 5.0)), ..Ends::default() },
            ] {
                let st = generate_routed(&shape, &fill, &ends);
                let first_top = st.iter().position(|s| s.kind == StitchKind::Top).unwrap_or(st.len());
                assert!(st.iter().skip(first_top).all(|s| s.kind != StitchKind::Underlay), "{kind:?} {ends:?}");
                if fill.fill.underlay {
                    assert!(st.iter().take(first_top).any(|s| s.kind == StitchKind::Underlay), "{kind:?} {ends:?}");
                }
            }
        }
    }
    /// A five-pointed star and a C, concave both: test shapes whose fills split into parts.
    fn concave_shapes() -> Vec<Vec<Polyline>> {
        let star: Vec<Point> = (0..10)
            .map(|i| {
                let a = std::f64::consts::PI * i as f64 / 5.0;
                let r = if i % 2 == 0 { 30.0 } else { 11.0 };
                Point::new(40.0 + r * a.cos(), 40.0 + r * a.sin())
            })
            .collect();
        let c: Vec<Point> = (0..=40)
            .map(|i| {
                let a = 0.6 + (std::f64::consts::TAU - 1.2) * i as f64 / 40.0;
                Point::new(40.0 + 25.0 * a.cos(), 40.0 + 25.0 * a.sin())
            })
            .chain((0..=40).rev().map(|i| {
                let a = 0.6 + (std::f64::consts::TAU - 1.2) * i as f64 / 40.0;
                Point::new(40.0 + 14.0 * a.cos(), 40.0 + 14.0 * a.sin())
            }))
            .collect();
        vec![vec![Polyline::new(star, true)], vec![Polyline::new(c, true)]]
    }

    #[test]
    fn no_stitch_of_a_fill_leaves_its_shape() {
        let p = Point::new;
        for rings in concave_shapes() {
            let shape = Shape::Area(rings.clone());
            for kind in [StitchType::Tatami, StitchType::Contour] {
                for angle in [0.0, 30.0, 45.0, 90.0, 135.0] {
                    let mut fill = StitchSettings::of(kind);
                    fill.fill.angle = angle;
                    for ends in [Ends::default(), Ends { from: Some(p(0.0, 0.0)), toward: Some(p(80.0, 80.0)), ..Ends::default() }] {
                        let st = generate_routed(&shape, &fill, &ends);
                        for w in st.windows(2) {
                            if let [a, b] = w {
                                if b.kind == StitchKind::Jump {
                                    continue;
                                }
                                assert!(
                                    crate::travel::runs_inside(&rings, a.point(), b.point()),
                                    "{kind:?} {angle}: {:?} -> {:?} ({:?})",
                                    a.point(),
                                    b.point(),
                                    b.kind
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
