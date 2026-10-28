//! The embroidery model: which objects sew, each object's two stitch parts,
//! the stitch plan and the machine file.
//!
//! An object sews only once it has been converted to embroidery (Convert to
//! Satin, a stitch type chosen in the Embroidery panel, lettering); the
//! others stay vectors and sew nothing. Converted objects sew in paint order
//! (bottom to top). Each has two parts: its fill sews with a fill stitch in
//! the fill colour, then its outline with a line stitch in the outline
//! colour. A part has stitch settings of its own, or none and does not sew;
//! a part without settings shows the fabric's defaults (tatami for a fill,
//! a running stitch for an outline) in the panel until a type is chosen.
//!
//! The settings are kept in the object's data under [`STITCH_KEY`] as JSON:
//! `{fill?, stroke?, strokeFirst?}`, each a `StitchSettings`. A value that is
//! itself one `StitchSettings` (it has a `type`) is the part its type belongs
//! to, and the other part does not sew.
//!
//! The page is the hoop: its centre is where the needle starts, so a design
//! sews where it sits on the page. The model is in millimetres with y up; the
//! stitch crates work with y down, so `y` becomes `page height - y` on the
//! way in and back again on the way out.

use serde_json::Value;
use tracedraw_core::geometry::{Affine, BezPath, Point};
use tracedraw_core::{Color, Document, Fill, PageId, Shape, ShapeId, ShapeKind};
use tracedraw_emb_formats::{Command as Machine, Design, Format, Thread};
use tracedraw_emb_plan::{plan_kinds, review, PlanObject, PlanSettings};
use tracedraw_emb_stitch::{
    generate_routed, medial, polylines_from_bezpath, Ends, Fabric, Point as SPoint, Polyline,
    Shape as StitchShape, StitchKind, StitchSettings, StitchType,
};

/// The object data key holding an object's stitch settings.
pub const STITCH_KEY: &str = "modela.embroidery.stitch";
/// The object data key holding where an object starts and ends sewing, when
/// placed by hand: `{start?: [x, y], end?: [x, y]}` in the object's own
/// space (mm, y up, before its transform).
pub const ENDS_KEY: &str = "modela.embroidery.ends";

/// Where an object starts and ends sewing, placed by hand (the object's own
/// space); `None` where the plan chooses.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pins {
    pub start: Option<Point>,
    pub end: Option<Point>,
}

impl Pins {
    pub fn is_empty(&self) -> bool {
        self.start.is_none() && self.end.is_none()
    }
}

/// The start and end `shape` sews from and to, placed by hand.
pub fn pins(shape: &Shape) -> Pins {
    let Some(v) = shape
        .data
        .iter()
        .find(|(k, _)| k == ENDS_KEY)
        .and_then(|(_, v)| serde_json::from_str::<Value>(v).ok())
    else {
        return Pins::default();
    };
    let point = |k: &str| {
        let a = v.get(k)?.as_array()?;
        let (x, y) = (a.first()?.as_f64()?, a.get(1)?.as_f64()?);
        (x.is_finite() && y.is_finite()).then(|| Point::new(x, y))
    };
    Pins {
        start: point("start"),
        end: point("end"),
    }
}

/// `shape`'s object data with `pins` stored (or removed when empty).
pub fn data_with_pins(shape: &Shape, pins: &Pins) -> Vec<(String, String)> {
    let mut data: Vec<(String, String)> = shape
        .data
        .iter()
        .filter(|(k, _)| k != ENDS_KEY)
        .cloned()
        .collect();
    if !pins.is_empty() {
        let mut o = serde_json::Map::new();
        for (k, p) in [("start", pins.start), ("end", pins.end)] {
            if let Some(p) = p {
                o.insert(k.into(), serde_json::json!([p.x, p.y]));
            }
        }
        data.push((ENDS_KEY.to_string(), Value::Object(o).to_string()));
    }
    data
}
/// Thread thickness, mm: the outline width a running stitch shows at.
pub const THREAD_MM: f64 = 0.4;
/// How closely curves are followed when flattened for stitching, mm.
const FLATTEN_MM: f64 = 0.05;

/// One of an object's two stitch parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    Fill,
    Stroke,
}

impl Part {
    pub const BOTH: [Part; 2] = [Part::Fill, Part::Stroke];

    /// The part a stitch type belongs to.
    pub fn of(kind: StitchType) -> Part {
        if kind.is_fill() {
            Part::Fill
        } else {
            Part::Stroke
        }
    }

    /// The stitch types this part can sew, "none" first.
    pub fn kinds(self) -> Vec<StitchType> {
        StitchType::ALL
            .into_iter()
            .filter(|k| *k == StitchType::None || Part::of(*k) == self)
            .collect()
    }
}

/// What a part is set to.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Slot {
    /// The automatic rule.
    Auto,
    Set(StitchSettings),
}

impl Slot {
    fn off() -> Slot {
        Slot::Set(StitchSettings::of(StitchType::None))
    }

    fn is_off(&self) -> bool {
        matches!(self, Slot::Set(s) if s.kind == StitchType::None)
    }
}

/// Both parts of an object, as stored.
#[derive(Clone, Debug, PartialEq)]
pub struct Slots {
    pub fill: Slot,
    pub stroke: Slot,
    /// The outline sews before the fill.
    pub stroke_first: bool,
}

impl Slots {
    pub const AUTO: Slots = Slots {
        fill: Slot::Auto,
        stroke: Slot::Auto,
        stroke_first: false,
    };

    /// Read the stored JSON (missing or unreadable: automatic).
    pub fn read(text: Option<&str>) -> Slots {
        let Some(v) = text.and_then(|t| serde_json::from_str::<Value>(t).ok()) else {
            return Slots::AUTO;
        };
        let parse = |v: &Value| serde_json::from_value::<StitchSettings>(v.clone()).ok();
        let only = |fill, stroke| Slots {
            fill,
            stroke,
            stroke_first: false,
        };
        if v.get("type").is_some() {
            return match parse(&v) {
                Some(s) if s.kind == StitchType::None => only(Slot::off(), Slot::off()),
                Some(s) if s.kind.is_fill() => only(Slot::Set(s), Slot::off()),
                Some(s) => only(Slot::off(), Slot::Set(s)),
                None => Slots::AUTO,
            };
        }
        let slot = |k: &str| v.get(k).and_then(parse).map_or(Slot::Auto, Slot::Set);
        Slots {
            fill: slot("fill"),
            stroke: slot("stroke"),
            stroke_first: v
                .get("strokeFirst")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        }
    }

    /// The JSON to store (`None`: both automatic, nothing stored).
    pub fn to_json(&self) -> Option<String> {
        let v = |s: &StitchSettings| serde_json::to_value(s).unwrap_or(Value::Null);
        let value = match (&self.fill, &self.stroke) {
            (Slot::Auto, Slot::Auto) if !self.stroke_first => return None,
            (f, s) if f.is_off() && s.is_off() => v(&StitchSettings::of(StitchType::None)),
            (Slot::Set(f), s) if s.is_off() && f.kind.is_fill() => v(f),
            (f, Slot::Set(s)) if f.is_off() && !s.kind.is_fill() => v(s),
            (f, s) => {
                let mut o = serde_json::Map::new();
                for (k, slot) in [("fill", f), ("stroke", s)] {
                    if let Slot::Set(st) = slot {
                        o.insert(k.into(), v(st));
                    }
                }
                if self.stroke_first {
                    o.insert("strokeFirst".into(), Value::Bool(true));
                }
                Value::Object(o)
            }
        };
        Some(value.to_string())
    }

    pub fn get(&self, part: Part) -> &Slot {
        match part {
            Part::Fill => &self.fill,
            Part::Stroke => &self.stroke,
        }
    }

    pub fn get_mut(&mut self, part: Part) -> &mut Slot {
        match part {
            Part::Fill => &mut self.fill,
            Part::Stroke => &mut self.stroke,
        }
    }

    /// Whether the outline part is the whole object (Satin Blocks, manual
    /// stitches, lettering).
    pub fn whole(&self) -> bool {
        matches!(&self.stroke, Slot::Set(s)
            if matches!(s.kind, StitchType::SatinBlocks | StitchType::Manual)
                || s.lettering.is_some())
    }
}

/// The stored parts of `shape`.
pub fn slots(shape: &Shape) -> Slots {
    Slots::read(
        shape
            .data
            .iter()
            .find(|(k, _)| k == STITCH_KEY)
            .map(|(_, v)| v.as_str()),
    )
}

/// `shape`'s object data with `slots` stored (or removed when automatic).
pub fn data_with(shape: &Shape, slots: &Slots) -> Vec<(String, String)> {
    let mut data: Vec<(String, String)> = shape
        .data
        .iter()
        .filter(|(k, _)| k != STITCH_KEY)
        .cloned()
        .collect();
    if let Some(json) = slots.to_json() {
        data.push((STITCH_KEY.to_string(), json));
    }
    data
}

fn solid_rgb(fill: &Fill) -> Option<[u8; 3]> {
    match fill {
        Fill::Solid(c) => Some(c.to_rgb8()),
        Fill::Fountain(_) | Fill::Pattern(_) | Fill::Texture(_) | Fill::Mesh(_) => Some([0, 0, 0]),
        Fill::None => None,
    }
}

pub fn fill_rgb(shape: &Shape) -> Option<[u8; 3]> {
    solid_rgb(&shape.fill)
}

pub fn stroke_rgb(shape: &Shape) -> Option<[u8; 3]> {
    shape
        .stroke
        .as_ref()
        .filter(|s| s.width > 0.0)
        .map(|s| s.color.to_rgb8())
}

/// Whether a stitch type's width is the outline width.
fn has_width(kind: StitchType) -> bool {
    matches!(
        kind,
        StitchType::Satin | StitchType::EStitch | StitchType::Rope
    )
}

fn width_of(s: &StitchSettings) -> Option<f64> {
    match s.kind {
        StitchType::Satin => Some(s.satin.width),
        StitchType::EStitch => Some(s.e_stitch.width),
        StitchType::Rope => Some(s.rope.width),
        _ => None,
    }
}

fn set_width(s: &mut StitchSettings, w: f64) {
    match s.kind {
        StitchType::Satin => s.satin.width = w,
        StitchType::EStitch => s.e_stitch.width = w,
        StitchType::Rope => s.rope.width = w,
        _ => {}
    }
}

/// The settings `part` of `shape` has, sewn or not, and whether they come
/// from the automatic rule. A satin, E-stitch or rope outline is as wide as
/// the outline; lettering keeps the column width that suits its letters.
pub fn part_settings(shape: &Shape, fabric: &Fabric, part: Part) -> (StitchSettings, bool) {
    let (mut s, auto) = match slots(shape).get(part) {
        Slot::Set(s) => (s.clone(), false),
        Slot::Auto => (
            fabric.settings(if part == Part::Fill {
                StitchType::Tatami
            } else {
                StitchType::Run
            }),
            true,
        ),
    };
    if part == Part::Stroke && has_width(s.kind) && s.lettering.is_none() {
        if let Some(st) = shape
            .stroke
            .as_ref()
            .filter(|st| st.width.is_finite() && st.width > 0.0)
        {
            set_width(&mut s, st.width);
        }
    }
    (s, auto)
}

/// The parts of `shape` that sew, in sewing order, with their settings and
/// whether they are automatic (never, now: a part without settings of its
/// own does not sew). A part set without its paint sews in the other
/// colour.
pub fn sewn(shape: &Shape, fabric: &Fabric) -> Vec<(Part, StitchSettings, bool)> {
    let slots = slots(shape);
    let has = |part: Part| match part {
        Part::Fill => fill_rgb(shape).is_some(),
        Part::Stroke => stroke_rgb(shape).is_some(),
    };
    let wanted = |part: Part| match slots.get(part) {
        Slot::Auto => false,
        Slot::Set(s) => s.kind != StitchType::None,
    };
    let stroke = wanted(Part::Stroke);
    let fill = wanted(Part::Fill) && !slots.whole() && (has(Part::Fill) || !stroke);
    let stroke = stroke && (has(Part::Stroke) || !fill);
    let order = if slots.stroke_first {
        [(Part::Stroke, stroke), (Part::Fill, fill)]
    } else {
        [(Part::Fill, fill), (Part::Stroke, stroke)]
    };
    order
        .into_iter()
        .filter(|(_, on)| *on)
        .map(|(part, _)| {
            let (s, auto) = part_settings(shape, fabric, part);
            (part, s, auto)
        })
        .collect()
}

/// The new outline width when `slots` is stored: a satin, E-stitch or rope
/// outline shows as wide as it sews, a running stitch set now at thread
/// thickness.
pub fn stroke_width_for(slots: &Slots, thread: bool) -> Option<f64> {
    let Slot::Set(s) = &slots.stroke else {
        return None;
    };
    let s = s.sanitized();
    if let Some(w) = width_of(&s).filter(|w| w.is_finite() && *w > 0.0) {
        Some(w)
    } else if thread && matches!(s.kind, StitchType::Run | StitchType::Triple) {
        Some(THREAD_MM)
    } else {
        None
    }
}

/// One object to sew: in page space (groups' transforms taken in).
#[derive(Clone, Debug)]
pub struct Object {
    pub shape: Shape,
}

/// Every object on `page` that sews, in paint order: the leaves of visible
/// layers (groups opened, their transforms applied to the children).
pub fn objects(doc: &Document, page: PageId) -> Vec<Object> {
    let mut out = vec![];
    for layer in doc.layers_for_page(page).unwrap_or_default() {
        if !layer.visible {
            continue;
        }
        for s in &layer.shapes {
            collect(s, Affine::IDENTITY, &mut out);
        }
    }
    out
}

fn collect(shape: &Shape, parent: Affine, out: &mut Vec<Object>) {
    if !shape.visible {
        return;
    }
    match &shape.kind {
        ShapeKind::Group { children } => {
            let t = parent * shape.transform;
            for c in children {
                collect(c, t, out);
            }
        }
        ShapeKind::Bitmap { .. }
        | ShapeKind::Table(_)
        | ShapeKind::SymbolInstance { .. }
        | ShapeKind::ClipFrame { .. } => {}
        _ => {
            let mut s = shape.clone();
            s.transform = parent * s.transform;
            out.push(Object { shape: s });
        }
    }
}

/// Page space (mm, y up) to stitch space (mm, y down).
fn to_stitch(height: f64) -> Affine {
    Affine::new([1.0, 0.0, 0.0, -1.0, 0.0, height])
}

/// The object's outline as polylines in stitch space.
pub fn polylines(shape: &Shape, height: f64) -> Vec<Polyline> {
    let path: BezPath = to_stitch(height) * shape.page_path();
    polylines_from_bezpath(&path, FLATTEN_MM)
}

/// The anchors of each subpath in stitch space (Satin Blocks rails and
/// manual stitches use the points as they are, not the flattened curve).
fn anchors(shape: &Shape, height: f64) -> Vec<Polyline> {
    use kurbo::PathEl;
    let path: BezPath = to_stitch(height) * shape.page_path();
    let mut subs: Vec<Polyline> = vec![];
    let mut cur: Vec<SPoint> = vec![];
    let push = |cur: &mut Vec<SPoint>, closed: bool, subs: &mut Vec<Polyline>| {
        if !cur.is_empty() {
            subs.push(Polyline::new(std::mem::take(cur), closed));
        }
    };
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                push(&mut cur, false, &mut subs);
                cur.push(SPoint::new(p.x, p.y));
            }
            PathEl::LineTo(p) | PathEl::QuadTo(_, p) | PathEl::CurveTo(_, _, p) => {
                cur.push(SPoint::new(p.x, p.y))
            }
            PathEl::ClosePath => push(&mut cur, true, &mut subs),
        }
    }
    push(&mut cur, false, &mut subs);
    subs
}

/// The thread `part` of `shape` sews with: fills in the fill colour, lines
/// in the outline colour, each in the other when the object has only that.
pub fn thread_of(
    shape: &Shape,
    part: Part,
    chart: Option<&tracedraw_emb_formats::charts::Chart>,
) -> Thread {
    let fill = fill_rgb(shape);
    let stroke = stroke_rgb(shape);
    let rgb = if part == Part::Fill {
        fill.or(stroke)
    } else {
        stroke.or(fill)
    }
    .unwrap_or([0, 0, 0]);
    chart.and_then(|c| c.exact(rgb)).cloned().unwrap_or(Thread {
        rgb,
        ..Default::default()
    })
}

/// What the design is sewn with: the fabric and the thread chart.
#[derive(Clone, Debug)]
pub struct DesignSettings {
    pub fabric: &'static Fabric,
    pub chart: Option<tracedraw_emb_formats::charts::Chart>,
}

impl DesignSettings {
    pub fn new(fabric: &str, chart: &str) -> Self {
        DesignSettings {
            fabric: Fabric::find(fabric).unwrap_or_else(Fabric::default_fabric),
            chart: tracedraw_emb_formats::charts::built_in(chart),
        }
    }
}

/// One part of an object, sewn.
#[derive(Clone, Debug)]
pub struct Sewn {
    pub id: ShapeId,
    pub part: Part,
    pub kind: StitchType,
    pub auto: bool,
    pub obj: PlanObject,
}

/// One part of `shape`, sewn, started and ended where `ends` asks as
/// nearly as the part allows.
fn stitch_part(
    shape: &Shape,
    ds: &DesignSettings,
    part: Part,
    s: &StitchSettings,
    ends: &Ends,
    height: f64,
) -> PlanObject {
    let thread = thread_of(shape, part, ds.chart.as_ref());
    let stitch_shape = match s.kind {
        StitchType::None => None,
        StitchType::SatinBlocks => match anchors(shape, height).as_slice() {
            [l, r, ..] => Some(StitchShape::Rails {
                left: l.points.clone(),
                right: r.points.clone(),
            }),
            _ => Some(StitchShape::Paths(polylines(shape, height))),
        },
        StitchType::Manual => Some(StitchShape::Paths(anchors(shape, height))),
        k if k.is_fill() => Some(StitchShape::Area(polylines(shape, height))),
        _ => Some(StitchShape::Paths(polylines(shape, height))),
    };
    // A fill, or a satin column between its rails, hides travel sewn before
    // it.
    let cover = match (&stitch_shape, s.kind.is_fill()) {
        (Some(StitchShape::Area(rings)), true) => rings.clone(),
        (Some(StitchShape::Rails { left, right }), _) if left.len() >= 2 && right.len() >= 2 => {
            let mut ring = left.clone();
            ring.extend(right.iter().rev().copied());
            vec![Polyline::new(ring, true)]
        }
        _ => vec![],
    };
    // Lettering keeps the stroke order its font gives.
    let ends = if s.lettering.is_some() {
        Ends::default()
    } else {
        *ends
    };
    let stitches = stitch_shape
        .map(|sh| generate_routed(&sh, s, &ends))
        .unwrap_or_default();
    PlanObject {
        stitches,
        thread,
        cover,
    }
}

/// Each part of `objects` stitched, in sewing order. Each part starts near
/// where the last one ended and ends near where the next object comes
/// nearest it (the shortest join), or at the start and end placed by hand:
/// an object's start applies to its first part, its end to its last.
pub fn sew_parts(objects: &[Object], ds: &DesignSettings, height: f64) -> Vec<Sewn> {
    // Every part to sew, in order, with its object.
    let mut queue: Vec<(usize, Part, StitchSettings, bool)> = vec![];
    for (i, o) in objects.iter().enumerate() {
        for (part, settings, auto) in sewn(&o.shape, ds.fabric) {
            queue.push((i, part, settings, auto));
        }
    }
    let to_stitch = to_stitch(height);
    let pinned: Vec<(Option<SPoint>, Option<SPoint>)> = objects
        .iter()
        .map(|o| {
            let p = pins(&o.shape);
            let at = |q: Option<Point>| {
                q.map(|q| {
                    let q = to_stitch * o.shape.transform * q;
                    SPoint::new(q.x, q.y)
                })
            };
            (at(p.start), at(p.end))
        })
        .collect();
    // The outline of each object to sew, sampled, for the shortest joins.
    let mut outlines: Vec<Option<Vec<SPoint>>> = vec![None; objects.len()];
    let mut outline = |i: usize| -> Vec<SPoint> {
        if let Some(Some(v)) = outlines.get(i) {
            return v.clone();
        }
        let v = objects
            .get(i)
            .map(|o| sampled(&polylines(&o.shape, height)))
            .unwrap_or_default();
        if let Some(slot) = outlines.get_mut(i) {
            *slot = Some(v.clone());
        }
        v
    };
    let mut out = Vec::with_capacity(queue.len());
    let mut at: Option<SPoint> = None;
    for (k, (i, part, settings, auto)) in queue.iter().enumerate() {
        let Some(o) = objects.get(*i) else { continue };
        let first_part = k == 0 || queue.get(k - 1).is_none_or(|p| p.0 != *i);
        let next = queue.get(k + 1);
        let last_part = next.is_none_or(|p| p.0 != *i);
        let (start, end) = pinned.get(*i).copied().unwrap_or_default();
        // Where the next object starts: its own start when placed by hand,
        // else its point nearest this object.
        let toward = match next {
            Some((j, ..)) if *j != *i => pinned
                .get(*j)
                .and_then(|p| p.0)
                .or_else(|| nearest_to(&outline(*j), &outline(*i))),
            _ => None,
        };
        let ends = Ends {
            from: at,
            toward,
            start: start.filter(|_| first_part),
            end: end.filter(|_| last_part),
        };
        let obj = stitch_part(&o.shape, ds, *part, settings, &ends, height);
        if let Some(last) = obj.stitches.last() {
            at = Some(last.point());
        }
        out.push(Sewn {
            id: o.shape.id,
            part: *part,
            kind: settings.kind,
            auto: *auto,
            obj,
        });
    }
    out
}

/// At most this many points of an outline are compared for the shortest
/// join.
const JOIN_SAMPLES: usize = 160;

/// Up to [`JOIN_SAMPLES`] points spread along `lines`.
fn sampled(lines: &[Polyline]) -> Vec<SPoint> {
    let all: Vec<SPoint> = lines
        .iter()
        .flat_map(|l| l.points.iter().copied())
        .collect();
    let step = all.len().div_ceil(JOIN_SAMPLES).max(1);
    all.into_iter().step_by(step).collect()
}

/// The point of `of` nearest any point of `to`.
fn nearest_to(of: &[SPoint], to: &[SPoint]) -> Option<SPoint> {
    let mut best: Option<(f64, SPoint)> = None;
    for p in of {
        for q in to {
            let d = p.distance(*q);
            if best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, *p));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// The stitch plan's settings for a page: the needle starts at its centre,
/// so the design keeps its place in the hoop.
pub fn plan_settings(width: f64, height: f64) -> PlanSettings {
    PlanSettings {
        origin: Some((width / 2.0, height / 2.0)),
        ..PlanSettings::default()
    }
}

/// One needle position of the stitch preview, in page space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewStitch {
    pub p: Point,
    /// The needle goes down here (false: a jump moves it here).
    pub sew: bool,
    /// Index into [`Preview::threads`].
    pub thread: usize,
    pub underlay: bool,
    /// The thread is cut before the needle moves here.
    pub trim: bool,
}

/// What the machine will sew on a page, and its totals.
#[derive(Clone, Debug, Default)]
pub struct Preview {
    pub stitches: Vec<PreviewStitch>,
    pub threads: Vec<[u8; 3]>,
    pub parts: Vec<Sewn>,
    pub design: Design,
    pub sew_minutes: f64,
    /// Top thread for the whole design, m.
    pub thread_m: f64,
    pub bobbin_m: f64,
    /// Layers of thread in the densest spot.
    pub max_density: f64,
    /// Layers of thread across the design, in the plan's coordinates
    /// (see [`Preview::to_page`]).
    pub density: review::DensityMap,
    /// The page's size, mm: the plan's origin is its centre.
    pub page: (f64, f64),
}

impl Preview {
    /// A point of the plan (mm, y down, the page centre at the origin) on
    /// the page.
    pub fn to_page(&self, x: f64, y: f64) -> Point {
        let (w, h) = self.page;
        Point::new(x + w / 2.0, h - (y + h / 2.0))
    }
}

/// The stitch preview of `page`.
pub fn preview(doc: &Document, page: PageId, ds: &DesignSettings) -> Preview {
    let Ok(p) = doc.page(page) else {
        return Preview::default();
    };
    let (w, h) = (p.size.width, p.size.height);
    let parts = sew_parts(&objects(doc, page), ds, h);
    let objs: Vec<PlanObject> = parts.iter().map(|o| o.obj.clone()).collect();
    let (design, kinds) = plan_kinds(&objs, &plan_settings(w, h), "");
    let (ox, oy) = (w / 2.0, h / 2.0);
    let mut thread = 0;
    let mut trim = false;
    let mut stitches = Vec::with_capacity(design.stitches.len());
    for (i, s) in design.stitches.iter().enumerate() {
        match s.command {
            Machine::ColorChange => {
                thread += 1;
                trim = true;
            }
            Machine::Trim => trim = true,
            Machine::End => {}
            c => stitches.push(PreviewStitch {
                // The plan moved the page centre to the origin.
                p: Point::new(s.x + ox, h - (s.y + oy)),
                sew: c == Machine::Stitch,
                thread,
                underlay: kinds.get(i) == Some(&StitchKind::Underlay),
                trim: std::mem::take(&mut trim),
            }),
        }
    }
    let use_ = review::thread_use(&design);
    let density = review::density(&design, review::DENSITY_CELL_MM);
    Preview {
        stitches,
        threads: design.threads.iter().map(|t| t.rgb).collect(),
        sew_minutes: review::sew_minutes(&design, &review::MachineSpeed::default()),
        thread_m: use_.top(),
        bobbin_m: use_.bobbin,
        max_density: density.max(),
        density,
        page: (w, h),
        parts,
        design,
    }
}

/// The machine file of `page` in `format`, named `name`.
pub fn machine_file(
    doc: &Document,
    page: PageId,
    ds: &DesignSettings,
    format: Format,
    name: &str,
) -> Result<Vec<u8>, String> {
    let p = doc.page(page).map_err(|e| e.to_string())?;
    let (w, h) = (p.size.width, p.size.height);
    let parts = sew_parts(&objects(doc, page), ds, h);
    let objs: Vec<PlanObject> = parts.into_iter().map(|o| o.obj).collect();
    let mut design = tracedraw_emb_plan::plan(&objs, &plan_settings(w, h), name);
    design.name = name.to_string();
    format.write(&design).map_err(|e| e.to_string())
}

/// A machine file read back as paths: each run of stitches one open path
/// in its thread's colour, in page space on a page of `width` x `height`
/// with the design centred.
pub fn machine_paths(
    bytes: &[u8],
    format: Format,
    width: f64,
    height: f64,
) -> Result<Vec<(BezPath, [u8; 3])>, String> {
    let design = format.read(bytes).map_err(|e| e.to_string())?;
    let (cx, cy) = (width / 2.0, height / 2.0);
    let mut out = vec![];
    let mut thread = 0usize;
    let mut cur: Vec<Point> = vec![];
    let rgb = |t: usize| design.threads.get(t).map_or([0, 0, 0], |t| t.rgb);
    let flush = |cur: &mut Vec<Point>, t: usize, out: &mut Vec<(BezPath, [u8; 3])>| {
        if cur.len() >= 2 {
            let mut bp = BezPath::new();
            bp.move_to(cur[0]);
            cur[1..].iter().for_each(|p| bp.line_to(*p));
            out.push((bp, rgb(t)));
        }
        cur.clear();
    };
    for s in &design.stitches {
        let p = Point::new(s.x + cx, height - (s.y + cy));
        match s.command {
            Machine::Stitch => cur.push(p),
            Machine::Jump | Machine::Trim => {
                flush(&mut cur, thread, &mut out);
                cur.push(p);
            }
            Machine::ColorChange => {
                flush(&mut cur, thread, &mut out);
                thread += 1;
            }
            Machine::End => flush(&mut cur, thread, &mut out),
        }
    }
    flush(&mut cur, thread, &mut out);
    Ok(out)
}

/// Satin Blocks columns for a closed shape, as two-subpath paths in page
/// space (left rail then right rail), and the widest rung, mm.
pub fn satin_columns(shape: &Shape, height: f64, method: medial::Method) -> (Vec<BezPath>, f64) {
    let rings: Vec<Polyline> = polylines(shape, height)
        .into_iter()
        .filter(|r| r.points.len() >= 3)
        .collect();
    let columns = medial::to_satin_by(&rings, method);
    let widest = columns
        .iter()
        .map(medial::Column::widest)
        .fold(0.0, f64::max);
    let back = to_stitch(height); // its own inverse
    let paths = columns
        .into_iter()
        .map(|c| {
            let mut bp = BezPath::new();
            for rail in [&c.left, &c.right] {
                if let Some((first, rest)) = rail.split_first() {
                    bp.move_to(Point::new(first.x, first.y));
                    rest.iter().for_each(|p| bp.line_to(Point::new(p.x, p.y)));
                }
            }
            back * bp
        })
        .collect();
    (paths, widest)
}

/// The nearest chart colour for `rgb`.
pub fn nearest_thread(chart: &tracedraw_emb_formats::charts::Chart, rgb: [u8; 3]) -> Option<Color> {
    chart
        .nearest(rgb)
        .map(|(t, _)| Color::rgb8(t.rgb[0], t.rgb[1], t.rgb[2]))
}

/// Tests: the settings of an object converted with a tatami fill and a
/// running-stitch outline.
#[cfg(test)]
pub(crate) fn converted_slots() -> Slots {
    Slots {
        fill: Slot::Set(StitchSettings::of(StitchType::Tatami)),
        stroke: Slot::Set(StitchSettings::of(StitchType::Run)),
        stroke_first: false,
    }
}

/// Tests: convert `id` in `app` with [`converted_slots`].
#[cfg(test)]
pub(crate) fn convert(app: &mut crate::app::App, id: ShapeId) {
    let Some(shape) = app.doc().find_shape(id).cloned() else {
        return;
    };
    app.run(tracedraw_core::Command::SetObjectData {
        shape: id,
        data: data_with(&shape, &converted_slots()),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape as _;
    use tracedraw_core::{geometry::Rect, Stroke};

    fn square(id: u64, x: f64, y: f64, size: f64) -> Shape {
        let mut s = Shape::new(
            ShapeId(id),
            ShapeKind::Rect {
                rect: Rect::new(x, y, x + size, y + size),
                radius: 0.0,
                corners: None,
            },
        );
        s.fill = Fill::Solid(Color::rgb8(200, 0, 0));
        s
    }

    fn ds() -> DesignSettings {
        DesignSettings::new("woven", "none")
    }

    #[test]
    fn slots_round_trip_through_object_data() {
        let mut s = square(1, 0.0, 0.0, 10.0);
        assert_eq!(slots(&s), Slots::AUTO);
        let mut new = Slots::AUTO;
        new.fill = Slot::Set(StitchSettings::of(StitchType::Contour));
        new.stroke_first = true;
        s.data = data_with(&s, &new);
        assert_eq!(slots(&s), new);
        // Back to automatic removes the key.
        s.data = data_with(&s, &Slots::AUTO);
        assert!(s.data.is_empty());
        // Unreadable data reads as automatic.
        s.data = vec![(STITCH_KEY.into(), "{oops".into())];
        assert_eq!(slots(&s), Slots::AUTO);
    }

    #[test]
    fn an_object_sews_only_once_converted() {
        let mut s = square(1, 0.0, 0.0, 10.0);
        s.stroke = Some(Stroke::hairline(Color::rgb8(0, 0, 255)));
        // A plain vector sews nothing.
        assert!(sewn(&s, ds().fabric).is_empty());
        // The panel still offers the fabric's defaults for its parts.
        let (fill, auto) = part_settings(&s, ds().fabric, Part::Fill);
        assert_eq!((fill.kind, auto), (StitchType::Tatami, true));
        // Converted, it sews its fill then its outline.
        s.data = data_with(&s, &converted_slots());
        let kinds: Vec<_> = sewn(&s, ds().fabric)
            .iter()
            .map(|(p, st, auto)| (*p, st.kind, *auto))
            .collect();
        assert_eq!(
            kinds,
            vec![
                (Part::Fill, StitchType::Tatami, false),
                (Part::Stroke, StitchType::Run, false)
            ]
        );
        // Removing the stitches makes it a plain vector again.
        s.data = data_with(&s, &Slots::AUTO);
        assert!(sewn(&s, ds().fabric).is_empty());
    }

    #[test]
    fn a_satin_outline_is_as_wide_as_the_outline() {
        let mut s = square(1, 0.0, 0.0, 10.0);
        let mut st = Stroke::hairline(Color::BLACK);
        st.width = 3.0;
        s.stroke = Some(st);
        let mut new = Slots::AUTO;
        new.stroke = Slot::Set(StitchSettings::of(StitchType::Satin));
        s.data = data_with(&s, &new);
        let (got, auto) = part_settings(&s, ds().fabric, Part::Stroke);
        assert!(!auto);
        assert!((got.satin.width - 3.0).abs() < 1e-9);
    }

    #[test]
    fn a_page_sews_and_writes_every_machine_format() {
        let mut app = crate::app::App::headless();
        let id = app
            .new_shape(ShapeKind::Rect {
                rect: Rect::new(50.0, 50.0, 80.0, 80.0),
                radius: 0.0,
                corners: None,
            })
            .unwrap();
        app.run(tracedraw_core::Command::SetFill {
            shapes: vec![id],
            fill: Fill::Solid(Color::rgb8(200, 0, 0)),
        });
        app.run(tracedraw_core::Command::SetStroke {
            shapes: vec![id],
            stroke: Some(Stroke::hairline(Color::rgb8(0, 0, 255))),
        });
        convert(&mut app, id);
        let page = app.page;
        let doc = app.doc().clone();
        let pv = preview(&doc, page, &ds());
        assert!(pv.stitches.len() > 100, "{}", pv.stitches.len());
        assert_eq!(pv.threads.len(), 2);
        // The stitches sit where the square is on the page.
        let inside = pv
            .stitches
            .iter()
            .filter(|s| s.sew)
            .all(|s| (49.0..=81.0).contains(&s.p.x) && (49.0..=81.0).contains(&s.p.y));
        assert!(inside);
        for format in Format::writable() {
            let bytes = machine_file(&doc, page, &ds(), format, "test").unwrap();
            assert!(!bytes.is_empty(), "{format:?}");
            let size = doc.page(page).unwrap().size;
            let paths = machine_paths(&bytes, format, size.width, size.height).unwrap();
            assert!(!paths.is_empty(), "{format:?}");
        }
    }

    #[test]
    fn hidden_and_grouped_objects() {
        let mut a = square(1, 0.0, 0.0, 10.0);
        let b = square(2, 20.0, 0.0, 10.0);
        a.visible = false;
        let mut g = Shape::new(
            ShapeId(3),
            ShapeKind::Group {
                children: vec![a, b],
            },
        );
        g.transform = Affine::translate((5.0, 0.0));
        let mut out = vec![];
        collect(&g, Affine::IDENTITY, &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].shape.id, ShapeId(2));
        let x0 = out[0].shape.page_path().bounding_box().x0;
        assert!((x0 - 25.0).abs() < 1e-9);
    }

    #[test]
    fn convert_to_satin_gives_rails_in_page_space() {
        let mut s = square(1, 10.0, 10.0, 4.0);
        s.kind = ShapeKind::Rect {
            rect: Rect::new(10.0, 10.0, 40.0, 14.0),
            radius: 0.0,
            corners: None,
        };
        let (paths, widest) = satin_columns(&s, 297.0, medial::Method::Centerline);
        assert!(!paths.is_empty());
        assert!(widest > 3.0 && widest < 5.0, "{widest}");
        let b = paths[0].bounding_box();
        assert!(b.y0 > 9.0 && b.y1 < 15.0, "{b:?}");
    }
}
