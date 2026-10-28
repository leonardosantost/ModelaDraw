//! Check Design: what will sew badly, before it is sewn, with where it is
//! and, when one action fixes it, that action; and Remove Overlaps, which
//! cuts fills where fills sewn later cover them.
//!
//! - the design past the hoop's (the page's) edge, or too large for it;
//! - spots where objects stack and thread piles up past 2.6 layers (an
//!   error past 4), which breaks needles and puckers fabric;
//! - fills under 1.5 mm wide on average (a sliver sews as ragged rows);
//! - satins over 10 mm wide (loose stitches snag) or under 1 mm (they bunch);
//! - objects under 1.5 mm.

use std::collections::{BTreeSet, HashMap};

use super::sew::{self, DesignSettings, Part, Preview, Sewn};
use tracedraw_core::geometry::Point;
use tracedraw_core::shaping::{offset, overlay, Op};
use tracedraw_core::{Document, PageId, Shape, ShapeId, ShapeKind};
use tracedraw_emb_formats::{hoops, Command as Machine};
use tracedraw_emb_plan::review;
use tracedraw_emb_stitch::{Polyline, StitchKind, StitchType};

pub const THIN_FILL_MM: f64 = 1.5;
pub const WIDE_SATIN_MM: f64 = 10.0;
pub const NARROW_SATIN_MM: f64 = 1.0;
pub const TINY_MM: f64 = 1.5;
pub const HEAVY_LAYERS: f64 = 4.0;
const MAX_SPOTS: usize = 20;
/// How far a fill keeps running under the shapes that cover it, mm.
pub const DEFAULT_OVERLAP: f64 = 1.0;
/// Cuts that take away less than this share of a fill are skipped.
const MIN_CUT: f64 = 1e-4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    OutsideHoop,
    TooLarge,
    TooDense,
    ThinFill,
    WideSatin,
    NarrowSatin,
    Tiny,
}

impl Kind {
    pub fn id(self) -> &'static str {
        match self {
            Kind::OutsideHoop => "outside_hoop",
            Kind::TooLarge => "too_large",
            Kind::TooDense => "too_dense",
            Kind::ThinFill => "thin_fill",
            Kind::WideSatin => "wide_satin",
            Kind::NarrowSatin => "narrow_satin",
            Kind::Tiny => "tiny",
        }
    }
}

/// The one action that fixes a problem.
#[derive(Clone, Debug, PartialEq)]
pub enum Fix {
    CenterInHoop,
    RemoveOverlaps(Vec<ShapeId>),
    ConvertToSatin(ShapeId),
    /// Split the satin's long stitches (the default split length).
    SplitSatin(ShapeId, Part),
    /// Sew the outline as a running stitch.
    SewAsRun(ShapeId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub kind: Kind,
    pub error: bool,
    pub ids: Vec<ShapeId>,
    /// Where on the page, mm, and how far round it.
    pub at: Option<Point>,
    pub radius: f64,
    /// The measurement the message names (mm or layers).
    pub value: f64,
    pub fix: Option<Fix>,
}

/// The problems of the page's design, errors first.
pub fn check(doc: &Document, page: PageId, pv: &Preview) -> Vec<Problem> {
    let mut out = hoop_problems(pv);
    out.extend(dense_spots(pv));
    let height = pv.page.1;
    let shapes: HashMap<ShapeId, Shape> = sew::objects(doc, page)
        .into_iter()
        .map(|o| (o.shape.id, o.shape))
        .collect();
    for part in &pv.parts {
        out.extend(part_problems(part, shapes.get(&part.id), height));
    }
    out.sort_by_key(|p| !p.error);
    out
}

fn hoop_problems(pv: &Preview) -> Vec<Problem> {
    let Some(b) = pv.design.bounds() else {
        return vec![];
    };
    let (w, h) = pv.page;
    if let hoops::Fit::TooLarge { over_w, over_h } = hoops::fit(b.width(), b.height(), w, h, false)
    {
        return vec![Problem {
            kind: Kind::TooLarge,
            error: true,
            ids: vec![],
            at: None,
            radius: 0.0,
            value: over_w.max(over_h),
            fix: None,
        }];
    }
    // The stitch furthest past the edge (the plan's origin is the centre).
    let worst = pv
        .design
        .stitches
        .iter()
        .filter(|s| s.command == Machine::Stitch)
        .map(|s| ((s.x.abs() - w / 2.0).max(s.y.abs() - h / 2.0), s.x, s.y))
        .fold(None, |m: Option<(f64, f64, f64)>, v| match m {
            Some(m) if m.0 >= v.0 => Some(m),
            _ => Some(v),
        });
    match worst {
        Some((over, x, y)) if over > 0.15 => vec![Problem {
            kind: Kind::OutsideHoop,
            error: true,
            ids: vec![],
            at: Some(pv.to_page(x, y)),
            radius: 4.0,
            value: over,
            fix: Some(Fix::CenterInHoop),
        }],
        _ => vec![],
    }
}

/// Touching cells past [`review::TOO_DENSE_LAYERS`] joined into spots, each
/// with the objects sewn there; only spots where objects stack count.
fn dense_spots(pv: &Preview) -> Vec<Problem> {
    let m = &pv.density;
    let dense = |c: usize| {
        m.layers
            .get(c)
            .is_some_and(|l| f64::from(*l) > review::TOO_DENSE_LAYERS)
    };
    let cols = m.cols.max(1);
    let mut spot_of: HashMap<usize, usize> = HashMap::new();
    let mut spots: Vec<Vec<usize>> = vec![];
    for start in 0..m.layers.len() {
        if !dense(start) || spot_of.contains_key(&start) {
            continue;
        }
        let n = spots.len();
        let mut cells = vec![];
        let mut todo = vec![start];
        spot_of.insert(start, n);
        while let Some(c) = todo.pop() {
            cells.push(c);
            let (r, col) = (c / cols, c % cols);
            let mut near = vec![];
            if col > 0 {
                near.push(c - 1);
            }
            if col + 1 < m.cols {
                near.push(c + 1);
            }
            if r > 0 {
                near.push(c - cols);
            }
            if r + 1 < m.rows {
                near.push(c + cols);
            }
            for nb in near {
                if dense(nb) && !spot_of.contains_key(&nb) {
                    spot_of.insert(nb, n);
                    todo.push(nb);
                }
            }
        }
        spots.push(cells);
    }
    if spots.is_empty() {
        return vec![];
    }
    let (w, h) = pv.page;
    let cell_of = |x: f64, y: f64| -> Option<usize> {
        // The objects' stitches are in page mm, y down; the map is the plan's.
        let (px, py) = (x - w / 2.0, y - h / 2.0);
        let (c, r) = ((px - m.x0) / m.cell, (py - m.y0) / m.cell);
        if !(c >= 0.0 && r >= 0.0) || c >= m.cols as f64 || r >= m.rows as f64 {
            return None;
        }
        Some(r as usize * cols + c as usize)
    };
    let mut ids: Vec<BTreeSet<ShapeId>> = vec![BTreeSet::new(); spots.len()];
    let mut fills: Vec<BTreeSet<ShapeId>> = vec![BTreeSet::new(); spots.len()];
    for o in &pv.parts {
        for st in &o.obj.stitches {
            if let Some(n) = cell_of(st.x, st.y).and_then(|c| spot_of.get(&c)) {
                if let Some(set) = ids.get_mut(*n) {
                    set.insert(o.id);
                }
                if o.kind.is_fill() {
                    if let Some(set) = fills.get_mut(*n) {
                        set.insert(o.id);
                    }
                }
            }
        }
    }
    let centre = |c: usize| {
        (
            m.x0 + (c % cols) as f64 * m.cell + m.cell / 2.0,
            m.y0 + (c / cols) as f64 * m.cell + m.cell / 2.0,
        )
    };
    let mut out: Vec<Problem> = spots
        .iter()
        .enumerate()
        .map(|(n, cells)| {
            let k = cells.len().max(1) as f64;
            let (sx, sy) = cells
                .iter()
                .map(|c| centre(*c))
                .fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
            let (cx, cy) = (sx / k, sy / k);
            let radius = cells
                .iter()
                .map(|c| centre(*c))
                .map(|p| (p.0 - cx).hypot(p.1 - cy))
                .fold(0.0f64, f64::max)
                + m.cell;
            let worst = cells
                .iter()
                .filter_map(|c| m.layers.get(*c))
                .copied()
                .fold(0.0f32, f32::max);
            let overlapping: Vec<ShapeId> = fills
                .get(n)
                .map(|s| s.iter().copied().collect())
                .unwrap_or_default();
            Problem {
                kind: Kind::TooDense,
                error: f64::from(worst) > HEAVY_LAYERS,
                ids: ids
                    .get(n)
                    .map(|s| s.iter().copied().collect())
                    .unwrap_or_default(),
                at: Some(pv.to_page(cx, cy)),
                radius,
                value: f64::from(worst),
                fix: (overlapping.len() > 1).then_some(Fix::RemoveOverlaps(overlapping)),
            }
        })
        .collect();
    // One object's own dense spots (a satin's corners) are how it sews.
    out.retain(|p| p.ids.len() > 1);
    out.sort_by(|a, b| b.value.total_cmp(&a.value));
    out.truncate(MAX_SPOTS);
    out
}

fn part_problems(o: &Sewn, shape: Option<&Shape>, height: f64) -> Vec<Problem> {
    let pts: Vec<(f64, f64)> = o
        .obj
        .stitches
        .iter()
        .filter(|s| s.kind != StitchKind::Jump && s.x.is_finite() && s.y.is_finite())
        .map(|s| (s.x, s.y))
        .collect();
    let Some(&first) = pts.first() else {
        return vec![];
    };
    let (lo, hi) = pts.iter().fold((first, first), |(lo, hi), p| {
        (
            (lo.0.min(p.0), lo.1.min(p.1)),
            (hi.0.max(p.0), hi.1.max(p.1)),
        )
    });
    // Stitch space is y down.
    let at = Some(Point::new(
        (lo.0 + hi.0) / 2.0,
        height - (lo.1 + hi.1) / 2.0,
    ));
    let radius = (hi.0 - lo.0).hypot(hi.1 - lo.1) / 2.0;
    let problem = |kind, value, fix| Problem {
        kind,
        error: false,
        ids: vec![o.id],
        at,
        radius,
        value,
        fix,
    };
    let size = (hi.0 - lo.0).max(hi.1 - lo.1);
    if size < TINY_MM {
        return vec![problem(Kind::Tiny, size, None)];
    }
    if o.kind.is_fill() {
        let width = shape.map_or(f64::INFINITY, |s| mean_width(&sew::polylines(s, height)));
        if width < THIN_FILL_MM {
            return vec![problem(
                Kind::ThinFill,
                width,
                Some(Fix::ConvertToSatin(o.id)),
            )];
        }
    } else if matches!(o.kind, StitchType::Satin | StitchType::SatinBlocks) {
        let mut spans: Vec<f64> = o
            .obj
            .stitches
            .windows(2)
            .filter_map(|w| match w {
                [a, b] if a.kind == StitchKind::Top && b.kind == StitchKind::Top => {
                    Some((a.x - b.x).hypot(a.y - b.y))
                }
                _ => None,
            })
            .collect();
        spans.sort_by(f64::total_cmp);
        let at_rank = |q: f64| {
            spans
                .get(((spans.len() as f64 - 1.0) * q).round().max(0.0) as usize)
                .copied()
        };
        if let Some(wide) = at_rank(0.9).filter(|w| *w > WIDE_SATIN_MM) {
            return vec![problem(
                Kind::WideSatin,
                wide,
                Some(Fix::SplitSatin(o.id, o.part)),
            )];
        }
        if let Some(narrow) = at_rank(0.5).filter(|w| *w < NARROW_SATIN_MM) {
            let fix = (o.part == Part::Stroke).then_some(Fix::SewAsRun(o.id));
            return vec![problem(Kind::NarrowSatin, narrow, fix)];
        }
    }
    vec![]
}

/// A shape's average width, mm: twice its area over its outline's length.
/// Rings inside an odd number of others are holes.
pub fn mean_width(rings: &[Polyline]) -> f64 {
    let ring_area = |r: &Polyline| {
        let n = r.points.len();
        (0..n)
            .filter_map(|i| Some((r.points.get(i)?, r.points.get((i + 1) % n)?)))
            .map(|(a, b)| a.x * b.y - b.x * a.y)
            .sum::<f64>()
            .abs()
            / 2.0
    };
    let inside = |p: tracedraw_emb_stitch::Point, r: &Polyline| {
        let mut c = false;
        for (a, b) in r.points.iter().zip(r.points.iter().cycle().skip(1)) {
            if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                c = !c;
            }
        }
        c
    };
    let mut area = 0.0;
    let mut length = 0.0;
    for (i, r) in rings.iter().enumerate() {
        let Some(&p) = r.points.first() else {
            continue;
        };
        let depth = rings
            .iter()
            .enumerate()
            .filter(|(j, o)| *j != i && inside(p, o))
            .count();
        area += if depth % 2 == 0 {
            ring_area(r)
        } else {
            -ring_area(r)
        };
        length += r
            .points
            .iter()
            .zip(r.points.iter().cycle().skip(1))
            .map(|(a, b)| (a.x - b.x).hypot(a.y - b.y))
            .sum::<f64>();
    }
    if length > 0.0 {
        2.0 * area.max(0.0) / length
    } else {
        f64::INFINITY
    }
}

fn area(p: &tracedraw_core::geometry::BezPath) -> f64 {
    kurbo::Shape::area(p).abs()
}

/// Remove Overlaps: each fill in `targets` (every fill when empty) cut
/// where fills sewn after it cover it, keeping `overlap` mm under the
/// covering shapes' edges. A fill covered completely is left as it is, and
/// text is never cut. The new outlines, in each shape's own space.
pub fn overlap_cuts(
    doc: &Document,
    page: PageId,
    ds: &DesignSettings,
    targets: &[ShapeId],
    overlap: f64,
) -> Vec<(ShapeId, ShapeKind)> {
    let fills: Vec<Shape> = sew::objects(doc, page)
        .into_iter()
        .map(|o| o.shape)
        .filter(|s| {
            sew::sewn(s, ds.fabric)
                .iter()
                .any(|(p, ..)| *p == Part::Fill)
        })
        .collect();
    let mut out = vec![];
    for (i, s) in fills.iter().enumerate() {
        if !targets.is_empty() && !targets.contains(&s.id) {
            continue;
        }
        if matches!(s.kind, ShapeKind::Text { .. }) {
            continue;
        }
        let mut cover = tracedraw_core::geometry::BezPath::new();
        for above in fills.iter().skip(i + 1) {
            cover = if cover.is_empty() {
                above.page_path()
            } else {
                overlay(&cover, &above.page_path(), Op::Weld)
            };
        }
        if cover.is_empty() {
            continue;
        }
        let path = s.page_path();
        let before = area(&path);
        let visible = overlay(&path, &cover, Op::Trim);
        if visible.is_empty() {
            continue;
        }
        let after = if overlap > 0.0 {
            overlay(&path, &offset(&visible, overlap), Op::Intersect)
        } else {
            visible
        };
        if after.is_empty() || before - area(&after) <= before * MIN_CUT {
            continue;
        }
        // Back into the shape's own space (`objects` gave it page space).
        let local = s.transform.inverse() * after;
        out.push((
            s.id,
            ShapeKind::Path {
                path: local,
                closed: true,
            },
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracedraw_emb_stitch::Point;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Polyline {
        Polyline::new(
            vec![
                Point::new(x, y),
                Point::new(x + w, y),
                Point::new(x + w, y + h),
                Point::new(x, y + h),
            ],
            true,
        )
    }

    #[test]
    fn mean_width_of_strips_and_rings() {
        assert!((mean_width(&[rect(0.0, 0.0, 100.0, 1.0)]) - 200.0 / 202.0).abs() < 1e-9);
        let w = mean_width(&[rect(0.0, 0.0, 20.0, 20.0), rect(1.0, 1.0, 18.0, 18.0)]);
        assert!((w - 2.0 * (400.0 - 324.0) / 152.0).abs() < 1e-9, "{w}");
        assert!(mean_width(&[]).is_infinite());
    }
}
