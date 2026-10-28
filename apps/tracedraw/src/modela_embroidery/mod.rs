//! Modela embroidery: stitch settings, the sewing sequence, the hoop and
//! thread chart, the stitch view and machine files, for the Embroidery
//! workspace.
//!
//! The stitch generators, the stitch plan and the machine formats are the
//! `tracedraw-emb-*` crates; this module owns the UI and the object data
//! (see `sew.rs`). TraceDraw only needs a few registration points, listed
//! in `README.md`.

mod check;
pub mod icons;
mod letters;
pub mod sew;
mod view;

use std::cell::RefCell;
use std::sync::Arc;

use crate::app::App;
use crate::i18n::{tr, trf};
use crate::theme::Tokens;
use egui::{Color32, Painter, Pos2, Stroke as EStroke, Ui};
use icons::Icon;
use sew::{DesignSettings, Part, Preview, Slot, Slots};
use tracedraw_core::geometry::{Affine, Size};
use tracedraw_core::{Color, Command, Fill, Shape, ShapeId, ShapeKind, Stroke};
use tracedraw_emb_formats::{hoops, Format};
use tracedraw_emb_stitch::{medial, StitchSettings, StitchType, FABRICS};

const PREFS_ID: &str = "modela_embroidery_prefs";
const PLAYER_ID: &str = "modela_embroidery_player";

/// The user's embroidery choices, kept between runs.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Prefs {
    /// What the canvas shows: the artwork, the stitches or realistic
    /// thread ([`view::VECTOR`], [`view::STITCHES`], [`view::REALISTIC`]).
    view: u8,
    show_jumps: bool,
    show_underlay: bool,
    /// Heat map of thread layers over the stitch view.
    density: bool,
    /// Ring each problem Check Design finds on the canvas.
    mark_problems: bool,
    fabric: String,
    chart: String,
    format: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            view: view::VECTOR,
            show_jumps: true,
            show_underlay: true,
            density: false,
            mark_problems: true,
            fabric: "woven".into(),
            chart: "none".into(),
            format: "dst".into(),
        }
    }
}

fn prefs(ctx: &egui::Context) -> Prefs {
    ctx.data_mut(|d| d.get_persisted::<Prefs>(egui::Id::new(PREFS_ID)))
        .unwrap_or_default()
}

fn set_prefs(ctx: &egui::Context, p: Prefs) {
    ctx.data_mut(|d| d.insert_persisted(egui::Id::new(PREFS_ID), p));
}

/// The sew-out player: the stitch view sews the design stitch by stitch.
#[derive(Clone, Debug, PartialEq)]
struct Player {
    on: bool,
    playing: bool,
    /// Stitches sewn so far.
    at: f64,
    /// Stitches a second.
    speed: f64,
}

impl Default for Player {
    fn default() -> Self {
        Player {
            on: false,
            playing: false,
            at: 0.0,
            speed: 200.0,
        }
    }
}

fn player(ctx: &egui::Context) -> Player {
    ctx.data(|d| d.get_temp::<Player>(egui::Id::new(PLAYER_ID)))
        .unwrap_or_default()
}

fn set_player(ctx: &egui::Context, p: Player) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(PLAYER_ID), p));
}

fn design(p: &Prefs) -> DesignSettings {
    DesignSettings::new(&p.fabric, &p.chart)
}

type CacheKey = (u64, u64, tracedraw_core::PageId, String, String);

/// The stitch preview and the problems found in it.
struct Cached {
    preview: Arc<Preview>,
    problems: Arc<Vec<check::Problem>>,
}

thread_local! {
    /// The stitch preview of the last drawing state asked for: worked out
    /// again only when the drawing, the page, the fabric or the chart change.
    static CACHE: RefCell<Option<(CacheKey, Cached)>> = const { RefCell::new(None) };
}

fn cache_key(app: &App, p: &Prefs) -> CacheKey {
    (
        app.engine.instance(),
        app.engine.revision(),
        app.page,
        p.fabric.clone(),
        p.chart.clone(),
    )
}

fn cached<T>(app: &App, p: &Prefs, f: impl FnOnce(&Cached) -> T) -> T {
    let key = cache_key(app, p);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.as_ref().is_none_or(|(k, _)| *k != key) {
            let pv = sew::preview(app.doc(), app.page, &design(p));
            let problems = check::check(app.doc(), app.page, &pv);
            *c = Some((
                key,
                Cached {
                    preview: Arc::new(pv),
                    problems: Arc::new(problems),
                },
            ));
        }
        match c.as_ref() {
            Some((_, v)) => f(v),
            None => f(&Cached {
                preview: Arc::default(),
                problems: Arc::default(),
            }),
        }
    })
}

/// The stitch preview of the current page.
fn preview(app: &App, p: &Prefs) -> Arc<Preview> {
    cached(app, p, |c| c.preview.clone())
}

/// What Check Design finds on the current page.
fn problems(app: &App, p: &Prefs) -> Arc<Vec<check::Problem>> {
    cached(app, p, |c| c.problems.clone())
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn kind_name(k: StitchType) -> String {
    tr(&format!("modela_embroidery.type_{}", k.id()))
}

fn minutes_text(m: f64) -> String {
    let m = m.max(0.0).round() as u64;
    if m >= 60 {
        trf(
            "modela_embroidery.hours",
            &[("h", &(m / 60).to_string()), ("m", &(m % 60).to_string())],
        )
    } else {
        trf("modela_embroidery.minutes", &[("m", &m.to_string())])
    }
}

// ----- the stitch view ------------------------------------------------------

const WARN: Color32 = Color32::from_rgb(200, 120, 0);
const ERROR: Color32 = Color32::from_rgb(200, 40, 40);

/// The colour of `layers` of thread in the density map: clear up to one
/// layer, then green, amber and red.
fn density_color(layers: f64) -> Option<Color32> {
    let dense = tracedraw_emb_plan::review::DENSE_LAYERS;
    let too = tracedraw_emb_plan::review::TOO_DENSE_LAYERS;
    if layers <= 1.0 {
        None
    } else if layers <= dense {
        Some(Color32::from_rgba_unmultiplied(40, 170, 80, 70))
    } else if layers <= too {
        Some(Color32::from_rgba_unmultiplied(230, 160, 0, 110))
    } else {
        Some(Color32::from_rgba_unmultiplied(220, 30, 30, 140))
    }
}

/// The Embroidery workspace's canvas: the page drawn as the hoop, with the
/// stitches past its edge in red, in every view; in the stitch views the
/// fabric, the stitches (as lines or realistic thread), the density map and
/// the sew-out player's needle; Check Design's rings on top.
pub fn draw_stitches(app: &App, painter: &Painter) {
    if app.workspace != crate::app::Workspace::Embroidery {
        return;
    }
    let p = prefs(painter.ctx());
    let pv = preview(app, &p);
    let v = &app.view;
    let to_screen = |q: tracedraw_core::geometry::Point| v.to_screen(q);
    let field = v.rect_to_screen(app.page_rect());
    let hoop = page_hoop(app);
    let round = hoop.is_some_and(|h| h.round);
    if p.view != view::VECTOR {
        let pl = player(painter.ctx());
        let realistic = p.view == view::REALISTIC;
        painter.rect_filled(painter.clip_rect(), 0.0, view::fabric(realistic));
        let look = view::Look {
            realistic,
            show_jumps: p.show_jumps,
            show_underlay: p.show_underlay,
            sewn: if pl.on {
                pl.at.max(0.0) as usize
            } else {
                usize::MAX
            },
        };
        let needle = view::stitches(painter, &pv, v.zoom, &look, to_screen);
        if p.density {
            draw_density(painter, &pv, &to_screen);
        }
        if pl.on {
            if let Some(q) = needle {
                painter.circle_stroke(q, 5.0, EStroke::new(1.5, ERROR));
            }
        }
    }
    let name = hoop.map_or_else(String::new, |h| format!("{} {}", h.brand, h.name));
    let outside = view::outside_segments(&pv, round);
    view::hoop(painter, field, v.zoom, round, &name, &outside, to_screen);
    if p.mark_problems {
        draw_problems(app, painter, &p);
    }
}

fn draw_density(
    painter: &Painter,
    pv: &Preview,
    to_screen: &impl Fn(tracedraw_core::geometry::Point) -> Pos2,
) {
    let m = &pv.density;
    let cols = m.cols.max(1);
    for (i, l) in m.layers.iter().enumerate() {
        let Some(c) = density_color(f64::from(*l)) else {
            continue;
        };
        let (x, y) = (
            m.x0 + (i % cols) as f64 * m.cell,
            m.y0 + (i / cols) as f64 * m.cell,
        );
        let a = to_screen(pv.to_page(x, y));
        let b = to_screen(pv.to_page(x + m.cell, y + m.cell));
        painter.rect_filled(egui::Rect::from_two_pos(a, b), 0.0, c);
    }
}

/// A ring round each problem Check Design finds.
fn draw_problems(app: &App, painter: &Painter, p: &Prefs) {
    let view = &app.view;
    for pr in problems(app, p).iter() {
        let Some(at) = pr.at else {
            continue;
        };
        let c = if pr.error { ERROR } else { WARN };
        let r = (pr.radius.max(1.5) as f32 * view.zoom).max(8.0);
        painter.circle_stroke(view.to_screen(at), r, EStroke::new(2.0, c));
    }
}

// ----- commands ---------------------------------------------------------------

/// The selected objects that sew, opened down to the leaves of groups.
fn selected_objects(app: &App) -> Vec<Shape> {
    let mut out = vec![];
    for s in app.selected_shapes() {
        let mut v = vec![];
        sew_collect(&s, &mut v);
        out.extend(v);
    }
    out
}

fn sew_collect(s: &Shape, out: &mut Vec<Shape>) {
    match &s.kind {
        ShapeKind::Group { children } => {
            for c in children {
                let mut c = c.clone();
                c.transform = s.transform * c.transform;
                sew_collect(&c, out);
            }
        }
        ShapeKind::Bitmap { .. }
        | ShapeKind::Table(_)
        | ShapeKind::SymbolInstance { .. }
        | ShapeKind::ClipFrame { .. } => {}
        _ => out.push(s.clone()),
    }
}

/// Store `new` on each object (and its outline width), as one undo step.
fn store(app: &mut App, label: &'static str, edits: Vec<(Shape, Slots, bool)>) {
    let mut cmds = vec![];
    for (shape, slots, thread) in edits {
        if let Some(w) = sew::stroke_width_for(&slots, thread) {
            let mut st = shape
                .stroke
                .clone()
                .unwrap_or_else(|| Stroke::hairline(Color::BLACK));
            if (st.width - w).abs() > 1e-9 || shape.stroke.is_none() {
                st.width = w;
                cmds.push(Command::SetStroke {
                    shapes: vec![shape.id],
                    stroke: Some(st),
                });
            }
        }
        cmds.push(Command::SetObjectData {
            shape: shape.id,
            data: sew::data_with(&shape, &slots),
        });
    }
    if !cmds.is_empty() {
        let _ = app.run_batch_keeping_lines(label, &cmds);
    }
}

/// Set the stitch type of one part of the selection.
fn set_kind(app: &mut App, fabric: &tracedraw_emb_stitch::Fabric, part: Part, kind: StitchType) {
    let edits = selected_objects(app)
        .into_iter()
        .map(|s| {
            let mut slots = sew::slots(&s);
            let (mut cur, _) = sew::part_settings(&s, fabric, part);
            if cur.kind != kind {
                let keep = cur.clone();
                cur = fabric.settings(kind);
                // A width the outline gives stays.
                cur.satin.width = keep.satin.width;
            }
            cur.kind = kind;
            // Satin Blocks and manual stitches are the whole object.
            if matches!(kind, StitchType::SatinBlocks | StitchType::Manual) {
                slots.fill = Slot::Set(StitchSettings::of(StitchType::None));
            }
            *slots.get_mut(part) = Slot::Set(cur);
            let thread = matches!(kind, StitchType::Run | StitchType::Triple);
            (s, slots, thread)
        })
        .collect();
    store(app, "Stitch Type", edits);
}

/// Change one part's settings on the selection with `f`.
fn edit_settings(
    app: &mut App,
    fabric: &tracedraw_emb_stitch::Fabric,
    part: Part,
    f: impl Fn(&mut StitchSettings),
) {
    let edits = selected_objects(app)
        .into_iter()
        .map(|s| {
            let mut slots = sew::slots(&s);
            let (mut cur, _) = sew::part_settings(&s, fabric, part);
            f(&mut cur);
            *slots.get_mut(part) = Slot::Set(cur);
            (s, slots, false)
        })
        .collect();
    store(app, "Stitch Settings", edits);
}

fn reset_selection(app: &mut App) {
    let edits = selected_objects(app)
        .into_iter()
        .map(|s| (s, Slots::AUTO, false))
        .collect();
    store(app, "Stitch Settings", edits);
}

fn set_stroke_first(app: &mut App, first: bool) {
    let edits = selected_objects(app)
        .into_iter()
        .map(|s| {
            let mut slots = sew::slots(&s);
            slots.stroke_first = first;
            (s, slots, false)
        })
        .collect();
    store(app, "Sewing Order of Fill and Outline", edits);
}

/// Convert each selected closed shape into Satin Blocks columns, sewn just
/// after it; the shape stays, hidden.
pub fn convert_to_satin(app: &mut App, method: medial::Method) {
    let height = app.page_size().height;
    let Some(layer) = app.active_layer() else {
        return;
    };
    let mut cmds = vec![];
    let mut made = vec![];
    let mut hidden = vec![];
    let mut widest: f64 = 0.0;
    let satin = sew::Slots {
        fill: Slot::Set(StitchSettings::of(StitchType::None)),
        stroke: Slot::Set(StitchSettings::of(StitchType::SatinBlocks)),
        stroke_first: false,
    };
    for s in selected_objects(app) {
        let (paths, w) = sew::satin_columns(&s, height, method);
        if paths.is_empty() {
            continue;
        }
        widest = widest.max(w);
        let rgb = sew::fill_rgb(&s)
            .or_else(|| sew::stroke_rgb(&s))
            .unwrap_or([0, 0, 0]);
        let mut reference = s.id;
        for (k, path) in paths.into_iter().enumerate() {
            let id = app.engine.new_shape_id();
            let mut n = Shape::new(
                id,
                ShapeKind::Path {
                    path,
                    closed: false,
                },
            );
            n.fill = Fill::None;
            n.stroke = Some(Stroke::hairline(Color::rgb8(rgb[0], rgb[1], rgb[2])));
            n.name = Some(if k == 0 {
                tr("modela_embroidery.satin")
            } else {
                format!("{} {}", tr("modela_embroidery.satin"), k + 1)
            });
            n.data = sew::data_with(&n, &satin);
            cmds.push(Command::AddShape { layer, shape: n });
            cmds.push(Command::OrderRelative {
                shapes: vec![id],
                reference,
                in_front: true,
            });
            reference = id;
            made.push(id);
        }
        hidden.push(s.id);
    }
    if made.is_empty() {
        app.status = tr("modela_embroidery.convert_none");
        return;
    }
    cmds.push(Command::SetVisible {
        shapes: hidden,
        visible: false,
    });
    if app
        .run_batch_keeping_lines("Convert to Satin", &cmds)
        .is_ok()
    {
        app.select(made);
        app.status = if widest > medial::WIDE_MM {
            trf(
                "modela_embroidery.convert_wide",
                &[("w", &format!("{widest:.1}"))],
            )
        } else {
            tr("modela_embroidery.converted")
        };
    }
}

/// Each layer's objects reordered by thread colour, colours in the order
/// they first sew and each colour's objects in their old order: fewer
/// colour changes, where the stacking allows.
pub fn sort_by_color(app: &mut App) {
    let mut cmds = vec![];
    let page = app.page;
    let Ok(p) = app.doc().page(page) else {
        return;
    };
    for layer in &p.layers {
        if layer.locked {
            continue;
        }
        let color = |s: &Shape| {
            let mut v = vec![];
            sew_collect(s, &mut v);
            v.first()
                .map(|s| sew::thread_of(s, Part::Fill, None).rgb)
                .unwrap_or([0, 0, 0])
        };
        let mut order: Vec<[u8; 3]> = vec![];
        for s in &layer.shapes {
            let c = color(s);
            if !order.contains(&c) {
                order.push(c);
            }
        }
        let mut sorted: Vec<&Shape> = layer.shapes.iter().collect();
        sorted.sort_by_key(|s| order.iter().position(|c| *c == color(s)).unwrap_or(0));
        if sorted
            .iter()
            .map(|s| s.id)
            .ne(layer.shapes.iter().map(|s| s.id))
        {
            for (index, s) in sorted.iter().enumerate() {
                cmds.push(Command::Reorder {
                    shape: s.id,
                    layer: layer.id,
                    index,
                });
            }
        }
    }
    if !cmds.is_empty() {
        let _ = app.run_batch_keeping_lines("Sort by Color", &cmds);
    }
}

/// Change each object's sewing colours to the nearest thread in the chart.
fn match_threads(app: &mut App, ds: &DesignSettings) {
    let Some(chart) = &ds.chart else {
        app.status = tr("modela_embroidery.no_chart");
        return;
    };
    let objects: Vec<Shape> = if app.selection.is_empty() {
        sew::objects(app.doc(), app.page)
            .into_iter()
            .map(|o| o.shape)
            .collect()
    } else {
        selected_objects(app)
    };
    let mut cmds = vec![];
    for s in objects {
        if let (Fill::Solid(_), Some(c)) = (&s.fill, sew::fill_rgb(&s)) {
            if let Some(n) = sew::nearest_thread(chart, c) {
                cmds.push(Command::SetFill {
                    shapes: vec![s.id],
                    fill: Fill::Solid(n),
                });
            }
        }
        if let (Some(st), Some(c)) = (&s.stroke, sew::stroke_rgb(&s)) {
            if let Some(n) = sew::nearest_thread(chart, c) {
                let mut st = st.clone();
                st.color = n;
                cmds.push(Command::SetStroke {
                    shapes: vec![s.id],
                    stroke: Some(st),
                });
            }
        }
    }
    if !cmds.is_empty() {
        let _ = app.run_batch_keeping_lines("Match to Thread Chart", &cmds);
    }
}

/// Move the selection (or every unlocked object) so its middle is the
/// page's middle: the centre of the hoop.
fn center_in_hoop(app: &mut App) {
    let ids: Vec<ShapeId> = if app.selection.is_empty() {
        app.page_layers()
            .iter()
            .filter(|l| l.visible && !l.locked)
            .flat_map(|l| l.shapes.iter().filter(|s| !s.locked).map(|s| s.id))
            .collect()
    } else {
        app.selection.clone()
    };
    let shapes: Vec<Shape> = ids
        .iter()
        .filter_map(|id| app.doc().find_shape(*id).cloned())
        .collect();
    let Some(b) = shapes
        .iter()
        .map(|s| kurbo::Shape::bounding_box(&s.page_path()))
        .reduce(|a, b| a.union(b))
    else {
        return;
    };
    let size = app.page_size();
    let (dx, dy) = (
        size.width / 2.0 - (b.x0 + b.x1) / 2.0,
        size.height / 2.0 - (b.y0 + b.y1) / 2.0,
    );
    if dx.hypot(dy) > 1e-6 {
        app.run(Command::TransformShapes {
            shapes: ids,
            transform: Affine::translate((dx, dy)),
        });
    }
}

/// Size the page to a hoop's sewing field, about its centre.
fn set_hoop(app: &mut App, hoop: &hoops::Hoop) {
    app.run(Command::ResizePage {
        page: app.page,
        size: Size::new(hoop.width, hoop.height),
    });
}

/// The hoop the page is, if its size is one of the library's.
fn page_hoop(app: &App) -> Option<&'static hoops::Hoop> {
    let s = app.page_size();
    hoops::HOOPS
        .iter()
        .find(|h| (h.width - s.width).abs() < 0.05 && (h.height - s.height).abs() < 0.05)
}

/// File > Export Machine File: the page's design as an embroidery machine
/// file; the format is the chosen file's extension (the one used last is
/// offered first, and remembered).
pub fn export_machine(app: &mut App, ctx: &egui::Context) {
    let p = prefs(ctx);
    let last = Format::from_extension(&p.format).unwrap_or(Format::Dst);
    let name = {
        let t = app.doc().title.trim().to_string();
        if t.is_empty() {
            "design".to_string()
        } else {
            t
        }
    };
    let ds = design(&p);
    let mut dialog = crate::files::Dialog::new();
    for f in std::iter::once(last).chain(Format::writable().filter(|f| *f != last)) {
        dialog = dialog.add_filter(f.label(), &[f.extension()]);
    }
    let ctx = ctx.clone();
    dialog
        .set_file_name(format!("{name}.{}", last.extension()))
        .save_file(app, move |app, path| {
            let format = path
                .extension()
                .and_then(|e| e.to_str())
                .and_then(Format::from_extension)
                .filter(|f| f.can_write())
                .unwrap_or(last);
            let page = app.page;
            match sew::machine_file(app.doc(), page, &ds, format, &name) {
                Ok(bytes) => match crate::files::write(&path, &bytes) {
                    Ok(()) => {
                        let mut p = prefs(&ctx);
                        p.format = format.extension().to_string();
                        set_prefs(&ctx, p);
                        app.status = trf(
                            "modela_embroidery.exported",
                            &[("path", &path.display().to_string())],
                        )
                    }
                    Err(e) => app.status = e.to_string(),
                },
                Err(e) => app.status = e,
            }
        });
}

/// Open a machine file onto the current page: each run of stitches becomes
/// a manual-stitch path in its thread's colour.
pub fn import_machine(app: &mut App) {
    let exts: Vec<&str> = [
        "dst", "pes", "jef", "exp", "vp3", "xxx", "u01", "tap", "dsb", "dsz", "hus",
    ]
    .to_vec();
    crate::files::Dialog::new()
        .add_filter(tr("modela_embroidery.machine_files"), &exts)
        .pick_file(app, |app, path| {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let Some(format) = Format::from_extension(&ext) else {
                return;
            };
            let bytes = match crate::files::read(&path) {
                Ok(b) => b,
                Err(e) => {
                    app.status = e.to_string();
                    return;
                }
            };
            let size = app.page_size();
            let paths = match sew::machine_paths(&bytes, format, size.width, size.height) {
                Ok(p) => p,
                Err(e) => {
                    app.status = e;
                    return;
                }
            };
            let Some(layer) = app.active_layer() else {
                return;
            };
            let manual = Slots {
                fill: Slot::Set(StitchSettings::of(StitchType::None)),
                stroke: Slot::Set(StitchSettings::of(StitchType::Manual)),
                stroke_first: false,
            };
            let mut cmds = vec![];
            let mut ids = vec![];
            for (path, c) in paths {
                let id = app.engine.new_shape_id();
                let mut s = Shape::new(
                    id,
                    ShapeKind::Path {
                        path,
                        closed: false,
                    },
                );
                s.fill = Fill::None;
                let mut st = Stroke::hairline(Color::rgb8(c[0], c[1], c[2]));
                st.width = sew::THREAD_MM;
                s.stroke = Some(st);
                s.data = sew::data_with(&s, &manual);
                cmds.push(Command::AddShape { layer, shape: s });
                ids.push(id);
            }
            if ids.len() > 1 {
                cmds.push(Command::Group {
                    shapes: ids.clone(),
                });
            }
            if app
                .run_batch_keeping_lines("Open Machine File", &cmds)
                .is_ok()
            {
                app.status = trf(
                    "modela_embroidery.imported",
                    &[("count", &ids.len().to_string())],
                );
            }
        });
}

/// A new lettering object in the built-in satin font, its baseline
/// across the middle of the page, in the current outline colour.
fn add_lettering(app: &mut App) {
    let Some(layer) = app.active_layer() else {
        return;
    };
    let l = tracedraw_emb_stitch::lettering::Lettering {
        text: tr("modela_embroidery.lettering_sample"),
        ..Default::default()
    };
    let (w, _) = letters::strokes(&l);
    let bb = kurbo::Shape::bounding_box(&w);
    let size = app.page_size();
    let x = size.width / 2.0 - bb.width() / 2.0;
    let y = size.height / 2.0 - l.height / 2.0;
    let color = app
        .default_stroke()
        .map(|s| s.color)
        .unwrap_or(Color::BLACK);
    let id = app.engine.new_shape_id();
    let (shape, _) = letters::new_shape(id, &l, x, y, color);
    if app
        .run_batch_keeping_lines("Add Lettering", &[Command::AddShape { layer, shape }])
        .is_ok()
    {
        app.select(vec![id]);
    }
}

/// Lay a lettering object out again with `l`.
fn set_lettering(app: &mut App, id: ShapeId, l: &tracedraw_emb_stitch::lettering::Lettering) {
    let Some(shape) = app.doc().find_shape(id).cloned() else {
        return;
    };
    let (_, missing) = letters::strokes(l);
    let cmds = letters::edit(&shape, l);
    let _ = app.run_batch_keeping_lines("Lettering", &cmds);
    if !missing.is_empty() {
        let chars: String = missing.into_iter().collect();
        app.status = trf("modela_embroidery.missing_chars", &[("chars", &chars)]);
    }
}

/// Cut fills where later fills cover them (the selection's, or every fill).
fn remove_overlaps(app: &mut App, ds: &DesignSettings, ids: &[ShapeId]) {
    let cuts = check::overlap_cuts(app.doc(), app.page, ds, ids, check::DEFAULT_OVERLAP);
    let n = cuts.len();
    let cmds: Vec<Command> = cuts
        .into_iter()
        .map(|(shape, kind)| Command::SetShapeKind { shape, kind })
        .collect();
    if !cmds.is_empty() {
        let _ = app.run_batch_keeping_lines("Remove Overlaps", &cmds);
    }
    app.status = trf(
        "modela_embroidery.overlaps_removed",
        &[("count", &n.to_string())],
    );
}

/// Run the action that fixes a problem.
fn apply_fix(app: &mut App, ds: &DesignSettings, fix: &check::Fix) {
    match fix {
        check::Fix::CenterInHoop => {
            app.selection.clear();
            center_in_hoop(app);
        }
        check::Fix::RemoveOverlaps(ids) => remove_overlaps(app, ds, ids),
        check::Fix::ConvertToSatin(id) => {
            app.select(vec![*id]);
            convert_to_satin(app, medial::Method::Centerline);
        }
        check::Fix::SplitSatin(id, part) => {
            app.select(vec![*id]);
            let split = tracedraw_emb_stitch::SatinSettings::default().split_length;
            edit_settings(app, ds.fabric, *part, |s| s.satin.split_length = split);
        }
        check::Fix::SewAsRun(id) => {
            app.select(vec![*id]);
            set_kind(app, ds.fabric, Part::Stroke, StitchType::Run);
        }
    }
}

// ----- the bar and the panel ---------------------------------------------------

/// The embroidery bar across the top of the Embroidery workspace.
pub fn top_bar(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let mut p = prefs(&ctx);
    let before = p.clone();
    let mut pl = player(&ctx);
    let pl_before = pl.clone();
    let pv = preview(app, &p);
    ui.horizontal(|ui| {
        ui.strong(tr("modela_embroidery.title"));
        ui.separator();
        ui.spacing_mut().item_spacing.x = 2.0;
        for (v, icon, key) in [
            (view::VECTOR, Icon::Vector, "view_vector"),
            (view::STITCHES, Icon::Stitches, "view_stitches"),
            (view::REALISTIC, Icon::Realistic, "view_realistic"),
        ] {
            let tip = format!(
                "{}\n{}",
                tr(&format!("modela_embroidery.{key}")),
                tr(&format!("modela_embroidery.{key}_tip"))
            );
            if icons::button(ui, icon, &tip, p.view == v).clicked() {
                p.view = v;
            }
        }
        if p.view != view::VECTOR {
            ui.separator();
            for (on, icon, key) in [
                (&mut p.show_jumps, Icon::Jumps, "jumps"),
                (&mut p.show_underlay, Icon::Underlay, "underlay"),
                (&mut p.density, Icon::Density, "density_map"),
            ] {
                let tip = format!(
                    "{}\n{}",
                    tr(&format!("modela_embroidery.{key}")),
                    tr(&format!("modela_embroidery.{key}_tip"))
                );
                if icons::button(ui, icon, &tip, *on).clicked() {
                    *on = !*on;
                }
            }
            let tip = format!(
                "{}\n{}",
                tr("modela_embroidery.player"),
                tr("modela_embroidery.player_tip")
            );
            if icons::button(ui, Icon::Play, &tip, pl.on).clicked() {
                pl.on = !pl.on;
                pl.playing = pl.on;
                pl.at = 0.0;
            }
        } else {
            pl.on = false;
        }
        ui.separator();
        if pl.on {
            player_controls(ui, &mut pl, pv.stitches.len());
        } else {
            tool_buttons(app, ui, &p);
        }
        ui.separator();
        ui.label(trf(
            "modela_embroidery.totals",
            &[
                ("stitches", &pv.design.stitch_count().to_string()),
                ("colors", &pv.threads.len().to_string()),
                ("time", &minutes_text(pv.sew_minutes)),
            ],
        ));
    });
    if pl.on && pl.playing {
        let dt = f64::from(ctx.input(|i| i.stable_dt).min(0.1));
        pl.at += pl.speed * dt;
        if pl.at >= pv.stitches.len() as f64 {
            pl.at = pv.stitches.len() as f64;
            pl.playing = false;
        }
        ctx.request_repaint();
    }
    if pl != pl_before {
        set_player(&ctx, pl);
    }
    if p != before {
        set_prefs(&ctx, p);
    }
}

/// The sew-out player: play and pause, step, a scrubber and the speed.
fn player_controls(ui: &mut Ui, pl: &mut Player, total: usize) {
    let play = if pl.playing { Icon::Pause } else { Icon::Play };
    if icons::button(ui, play, &tr("modela_embroidery.play_tip"), false).clicked() {
        if !pl.playing && pl.at >= total as f64 {
            pl.at = 0.0;
        }
        pl.playing = !pl.playing;
    }
    if icons::button(ui, Icon::Rewind, &tr("modela_embroidery.rewind_tip"), false).clicked() {
        pl.at = 0.0;
        pl.playing = false;
    }
    if icons::button(ui, Icon::Step, &tr("modela_embroidery.step_tip"), false).clicked() {
        pl.playing = false;
        pl.at = (pl.at.floor() + 1.0).min(total as f64);
    }
    let mut at = pl.at as usize;
    if ui
        .add(egui::Slider::new(&mut at, 0..=total).show_value(true))
        .on_hover_text(tr("modela_embroidery.scrub_tip"))
        .changed()
    {
        pl.at = at as f64;
        pl.playing = false;
    }
    egui::ComboBox::from_id_salt("modela_embroidery_speed")
        .selected_text(format!("{:.0}/s", pl.speed))
        .width(70.0)
        .show_ui(ui, |ui| {
            for v in [50.0, 200.0, 1000.0, 5000.0] {
                ui.selectable_value(&mut pl.speed, v, format!("{v:.0}/s"));
            }
        })
        .response
        .on_hover_text(tr("modela_embroidery.speed_tip"));
}

/// The bar's tools.
fn tool_buttons(app: &mut App, ui: &mut Ui, p: &Prefs) {
    if icons::labeled(
        ui,
        Icon::Lettering,
        &tr("modela_embroidery.add_lettering"),
        &tr("modela_embroidery.add_lettering_tip"),
    )
    .clicked()
    {
        add_lettering(app);
    }
    if icons::labeled(
        ui,
        Icon::Satin,
        &tr("modela_embroidery.convert_to_satin"),
        &tr("modela_embroidery.convert_to_satin_tip"),
    )
    .clicked()
    {
        convert_to_satin(app, medial::Method::Centerline);
    }
    if icons::labeled(
        ui,
        Icon::RemoveOverlaps,
        &tr("modela_embroidery.remove_overlaps"),
        &tr("modela_embroidery.remove_overlaps_tip"),
    )
    .clicked()
    {
        let ids: Vec<ShapeId> = selected_objects(app).iter().map(|s| s.id).collect();
        remove_overlaps(app, &design(p), &ids);
    }
    if icons::labeled(
        ui,
        Icon::SortByColor,
        &tr("modela_embroidery.sort_by_color"),
        &tr("modela_embroidery.sort_by_color_tip"),
    )
    .clicked()
    {
        sort_by_color(app);
    }
    if icons::labeled(
        ui,
        Icon::CenterInHoop,
        &tr("modela_embroidery.center_in_hoop"),
        &tr("modela_embroidery.center_in_hoop_tip"),
    )
    .clicked()
    {
        center_in_hoop(app);
    }
}

fn number(
    ui: &mut Ui,
    label: &str,
    v: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    suffix: &str,
) -> bool {
    ui.horizontal(|ui| {
        ui.label(tr(label));
        ui.add(
            egui::DragValue::new(v)
                .speed(0.05)
                .range(range)
                .max_decimals(2)
                .suffix(suffix),
        )
        .on_hover_text(tr(&format!("{label}_tip")))
        .changed()
    })
    .inner
}

/// The settings of one part of the first selected object.
fn part_ui(app: &mut App, ui: &mut Ui, ds: &DesignSettings, shape: &Shape, part: Part) {
    let sewn = sew::sewn(shape, ds.fabric);
    let sews = sewn.iter().any(|(p, ..)| *p == part);
    let (st, auto) = sew::part_settings(shape, ds.fabric, part);
    let shown = if sews { st.kind } else { StitchType::None };
    let title = if part == Part::Fill {
        "modela_embroidery.fill"
    } else {
        "modela_embroidery.outline"
    };
    ui.horizontal(|ui| {
        ui.strong(tr(title));
        if auto && sews {
            ui.label(
                egui::RichText::new(tr("modela_embroidery.auto"))
                    .small()
                    .color(Tokens::text_dim()),
            );
        }
    });
    let mut kind = shown;
    egui::ComboBox::from_id_salt(("modela_embroidery_kind", title))
        .selected_text(kind_name(kind))
        .width(170.0)
        .show_ui(ui, |ui| {
            for k in part.kinds() {
                ui.selectable_value(&mut kind, k, kind_name(k));
            }
        })
        .response
        .on_hover_text(tr("modela_embroidery.kind_tip"));
    if kind != shown {
        set_kind(app, ds.fabric, part, kind);
        return;
    }
    if !sews {
        return;
    }
    let mm = " mm";
    let deg = "°";
    let mut s = st.clone();
    let mut changed = false;
    match s.kind {
        StitchType::Run | StitchType::Triple => {
            changed |= number(
                ui,
                "modela_embroidery.stitch_length",
                &mut s.run.length,
                0.3..=12.0,
                mm,
            );
        }
        StitchType::Satin | StitchType::SatinBlocks => {
            if s.kind == StitchType::Satin {
                changed |= number(
                    ui,
                    "modela_embroidery.width",
                    &mut s.satin.width,
                    0.3..=12.0,
                    mm,
                );
            }
            changed |= number(
                ui,
                "modela_embroidery.density",
                &mut s.satin.density,
                0.15..=2.0,
                mm,
            );
            changed |= number(
                ui,
                "modela_embroidery.pull_compensation",
                &mut s.satin.pull_compensation,
                0.0..=2.0,
                mm,
            );
            if s.satin.width > medial::WIDE_MM {
                ui.colored_label(
                    Color32::from_rgb(200, 120, 0),
                    tr("modela_embroidery.wide_satin"),
                );
            }
        }
        StitchType::EStitch => {
            changed |= number(
                ui,
                "modela_embroidery.width",
                &mut s.e_stitch.width,
                0.5..=10.0,
                mm,
            );
            changed |= number(
                ui,
                "modela_embroidery.spacing",
                &mut s.e_stitch.spacing,
                0.5..=10.0,
                mm,
            );
        }
        StitchType::Rope => {
            changed |= number(
                ui,
                "modela_embroidery.width",
                &mut s.rope.width,
                0.5..=6.0,
                mm,
            );
            changed |= number(
                ui,
                "modela_embroidery.spacing",
                &mut s.rope.spacing,
                0.2..=4.0,
                mm,
            );
        }
        StitchType::Motif => {
            changed |= number(
                ui,
                "modela_embroidery.angle",
                &mut s.motif.angle,
                -360.0..=360.0,
                deg,
            );
            changed |= number(
                ui,
                "modela_embroidery.spacing",
                &mut s.motif.spacing,
                0.5..=20.0,
                mm,
            );
        }
        k if k.is_fill() => {
            changed |= number(
                ui,
                "modela_embroidery.angle",
                &mut s.fill.angle,
                -360.0..=360.0,
                deg,
            );
            changed |= number(
                ui,
                "modela_embroidery.row_spacing",
                &mut s.fill.row_spacing,
                0.2..=5.0,
                mm,
            );
            changed |= number(
                ui,
                "modela_embroidery.stitch_length",
                &mut s.fill.stitch_length,
                1.0..=12.0,
                mm,
            );
            changed |= number(
                ui,
                "modela_embroidery.pull_compensation",
                &mut s.fill.pull_compensation,
                0.0..=2.0,
                mm,
            );
            changed |= ui
                .checkbox(&mut s.fill.underlay, tr("modela_embroidery.underlay"))
                .on_hover_text(tr("modela_embroidery.underlay_tip"))
                .changed();
        }
        _ => {}
    }
    if changed {
        let s = s.clone();
        edit_settings(app, ds.fabric, part, move |cur| {
            let kind = cur.kind;
            *cur = s.clone();
            cur.kind = kind;
        });
    }
}

/// The text being typed into a lettering object, kept until Enter or the
/// field is left (one undo step for the edit, not one a key).
#[derive(Clone, Debug, Default)]
struct TextDraft {
    shape: Option<ShapeId>,
    text: String,
}

/// Lettering: add it, and edit the selected lettering object.
fn lettering_section(app: &mut App, ui: &mut Ui) {
    egui::CollapsingHeader::new(tr("modela_embroidery.lettering"))
        .default_open(true)
        .show(ui, |ui| {
            let sel = app.selected_shapes();
            let lettering = match sel.as_slice() {
                [s] => letters::lettering_of(s).map(|l| (s.id, l)),
                _ => None,
            };
            let Some((id, l)) = lettering else {
                if icons::labeled(
                    ui,
                    Icon::Lettering,
                    &tr("modela_embroidery.add_lettering"),
                    &tr("modela_embroidery.add_lettering_tip"),
                )
                .clicked()
                {
                    add_lettering(app);
                }
                return;
            };
            let draft_id = egui::Id::new("modela_embroidery_text_draft");
            let mut draft: TextDraft = ui.ctx().data(|d| d.get_temp(draft_id)).unwrap_or_default();
            if draft.shape != Some(id) {
                draft = TextDraft {
                    shape: Some(id),
                    text: l.text.clone(),
                };
            }
            ui.label(tr("modela_embroidery.text"));
            let r = ui
                .add(egui::TextEdit::multiline(&mut draft.text).desired_rows(2))
                .on_hover_text(tr("modela_embroidery.text_tip"));
            let commit = r.lost_focus() && draft.text != l.text;
            let mut new = l.clone();
            if commit {
                new.text = draft.text.clone();
            }
            ui.ctx().data_mut(|d| d.insert_temp(draft_id, draft));
            let mut changed = commit;
            changed |= number(
                ui,
                "modela_embroidery.letter_height",
                &mut new.height,
                2.0..=300.0,
                " mm",
            );
            changed |= number(
                ui,
                "modela_embroidery.letter_spacing",
                &mut new.spacing,
                -50.0..=100.0,
                " mm",
            );
            changed |= number(
                ui,
                "modela_embroidery.line_spacing",
                &mut new.line_spacing,
                0.8..=5.0,
                "×",
            );
            if changed && new != l {
                set_lettering(app, id, &new);
            }
        })
        .header_response
        .on_hover_text(tr("modela_embroidery.lettering_tip"));
}

fn problem_text(pr: &check::Problem) -> String {
    let v = format!("{:.1}", pr.value);
    trf(
        &format!("modela_embroidery.problem_{}", pr.kind.id()),
        &[("value", &v)],
    )
}

/// Check Design: what will sew badly, errors first; a click selects the
/// objects involved, Fix runs the action that fixes it.
fn check_section(app: &mut App, ui: &mut Ui, p: &mut Prefs, ds: &DesignSettings) {
    egui::CollapsingHeader::new(tr("modela_embroidery.check"))
        .default_open(true)
        .show(ui, |ui| {
            ui.checkbox(&mut p.mark_problems, tr("modela_embroidery.mark_problems"))
                .on_hover_text(tr("modela_embroidery.mark_problems_tip"));
            let found = problems(app, p);
            if found.is_empty() {
                ui.label(
                    egui::RichText::new(tr("modela_embroidery.no_problems"))
                        .color(Tokens::text_dim()),
                );
                return;
            }
            let mut pick: Option<Vec<ShapeId>> = None;
            let mut fix: Option<check::Fix> = None;
            for pr in found.iter() {
                ui.horizontal(|ui| {
                    let c = if pr.error { ERROR } else { WARN };
                    ui.colored_label(c, if pr.error { "●" } else { "▲" });
                    if ui
                        .selectable_label(false, problem_text(pr))
                        .on_hover_text(tr("modela_embroidery.problem_tip"))
                        .clicked()
                        && !pr.ids.is_empty()
                    {
                        pick = Some(pr.ids.clone());
                    }
                    if let Some(f) = &pr.fix {
                        if ui
                            .small_button(tr("modela_embroidery.fix"))
                            .on_hover_text(tr("modela_embroidery.fix_tip"))
                            .clicked()
                        {
                            fix = Some(f.clone());
                        }
                    }
                });
            }
            if let Some(ids) = pick {
                app.select(ids);
            }
            if let Some(f) = fix {
                apply_fix(app, ds, &f);
            }
        })
        .header_response
        .on_hover_text(tr("modela_embroidery.check_tip"));
}

/// The top-level object (in its layer) that `id` is or is inside.
fn top_of(app: &App, id: ShapeId) -> Option<(tracedraw_core::LayerId, ShapeId)> {
    fn holds(s: &Shape, id: ShapeId) -> bool {
        s.id == id
            || matches!(&s.kind, ShapeKind::Group { children } if children.iter().any(|c| holds(c, id)))
    }
    app.page_layers()
        .iter()
        .find_map(|l| l.shapes.iter().find(|s| holds(s, id)).map(|s| (l.id, s.id)))
}

/// Sew `moved` just before (or after) `target`, by moving its top-level
/// object next to the target's in the stacking order.
fn move_in_sequence(app: &mut App, moved: ShapeId, target: ShapeId, before: bool) {
    let (Some((la, a)), Some((lb, b))) = (top_of(app, moved), top_of(app, target)) else {
        return;
    };
    if a == b {
        app.status = tr("modela_embroidery.same_group");
        return;
    }
    if la != lb {
        app.status = tr("modela_embroidery.other_layer");
        return;
    }
    let _ = app.run_batch_keeping_lines(
        "Reorder Sequence",
        &[Command::OrderRelative {
            shapes: vec![a],
            reference: b,
            in_front: !before,
        }],
    );
}

/// Change the sewing colour of `parts`: a fill part's fill, an outline
/// part's outline (each the other paint when the object has only that).
fn set_thread_color(app: &mut App, parts: &[(ShapeId, Part)], c: [u8; 3]) {
    let color = Color::rgb8(c[0], c[1], c[2]);
    let mut cmds = vec![];
    for (id, part) in parts {
        let Some(s) = app.doc().find_shape(*id) else {
            continue;
        };
        let fill = match part {
            Part::Fill => sew::fill_rgb(s).is_some() || sew::stroke_rgb(s).is_none(),
            Part::Stroke => sew::stroke_rgb(s).is_none() && sew::fill_rgb(s).is_some(),
        };
        if fill {
            cmds.push(Command::SetFill {
                shapes: vec![*id],
                fill: Fill::Solid(color),
            });
        } else {
            let mut st = s
                .stroke
                .clone()
                .unwrap_or_else(|| Stroke::hairline(Color::BLACK));
            st.color = color;
            cmds.push(Command::SetStroke {
                shapes: vec![*id],
                stroke: Some(st),
            });
        }
    }
    if !cmds.is_empty() {
        let _ = app.run_batch_keeping_lines("Thread Color", &cmds);
    }
}

/// A thread swatch that opens a colour picker (and the chart's threads);
/// returns the colour chosen.
fn color_swatch(
    ui: &mut Ui,
    id: egui::Id,
    c: [u8; 3],
    chart: Option<&tracedraw_emb_formats::charts::Chart>,
) -> Option<[u8; 3]> {
    let (r, resp) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
    ui.painter().rect_filled(r, 3.0, rgb(c));
    ui.painter().rect_stroke(
        r,
        3.0,
        EStroke::new(1.0, Color32::from_gray(120)),
        egui::StrokeKind::Inside,
    );
    let resp = resp.on_hover_text(tr("modela_embroidery.thread_color_tip"));
    let mut chosen = None;
    egui::Popup::menu(&resp).id(id).show(|ui| {
        let draft_id = id.with("draft");
        let mut draft: Color32 = ui.ctx().data(|d| d.get_temp(draft_id)).unwrap_or(rgb(c));
        egui::color_picker::color_picker_color32(ui, &mut draft, egui::color_picker::Alpha::Opaque);
        ui.ctx().data_mut(|d| d.insert_temp(draft_id, draft));
        if ui
            .button(tr("modela_embroidery.apply_color"))
            .on_hover_text(tr("modela_embroidery.apply_color_tip"))
            .clicked()
        {
            chosen = Some([draft.r(), draft.g(), draft.b()]);
            ui.ctx().data_mut(|d| d.remove::<Color32>(draft_id));
            ui.close();
        }
        if let Some(chart) = chart {
            ui.separator();
            ui.label(egui::RichText::new(&chart.name).small().strong());
            egui::ScrollArea::vertical()
                .max_height(180.0)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
                        for t in &chart.threads {
                            let (r, resp) = ui
                                .allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::click());
                            ui.painter().rect_filled(r, 2.0, rgb(t.rgb));
                            if resp
                                .on_hover_text(format!("{} {}", t.code, t.name))
                                .clicked()
                            {
                                chosen = Some(t.rgb);
                                ui.close();
                            }
                        }
                    });
                });
        }
    });
    chosen
}

/// Window > Panels > Sewing Sequence: every part that sews, in order, in
/// colour blocks. Drag a row onto another to sew it just before or after
/// it; click a swatch to change that thread's colour.
pub fn sequence_panel(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let p = prefs(&ctx);
    let ds = design(&p);
    let pv = preview(app, &p);
    ui.label(trf(
        "modela_embroidery.totals",
        &[
            ("stitches", &pv.design.stitch_count().to_string()),
            ("colors", &pv.threads.len().to_string()),
            ("time", &minutes_text(pv.sew_minutes)),
        ],
    ));
    ui.label(
        egui::RichText::new(trf(
            "modela_embroidery.thread_use",
            &[
                ("top", &format!("{:.1}", pv.thread_m)),
                ("bobbin", &format!("{:.1}", pv.bobbin_m)),
                ("trims", &pv.design.trims().to_string()),
            ],
        ))
        .small()
        .color(Tokens::text_dim()),
    );
    if pv.max_density > tracedraw_emb_plan::review::TOO_DENSE_LAYERS {
        ui.colored_label(
            WARN,
            trf(
                "modela_embroidery.too_dense",
                &[("layers", &format!("{:.1}", pv.max_density))],
            ),
        );
    }
    ui.horizontal(|ui| {
        if icons::labeled(
            ui,
            Icon::SortByColor,
            &tr("modela_embroidery.sort_by_color"),
            &tr("modela_embroidery.sort_by_color_tip"),
        )
        .clicked()
        {
            sort_by_color(app);
        }
    });
    ui.separator();
    // The colour blocks: runs of parts in one thread.
    let rows: Vec<&sew::Sewn> = pv
        .parts
        .iter()
        .filter(|o| !o.obj.stitches.is_empty())
        .collect();
    let mut blocks: Vec<(sew::Sewn, Vec<usize>)> = vec![];
    for (i, o) in rows.iter().enumerate() {
        match blocks.last_mut() {
            Some((first, members)) if first.obj.thread.rgb == o.obj.thread.rgb => members.push(i),
            _ => blocks.push(((*o).clone(), vec![i])),
        }
    }
    let mut pick = None;
    let mut recolor: Option<(Vec<(ShapeId, Part)>, [u8; 3])> = None;
    let mut drop: Option<(ShapeId, ShapeId, bool)> = None;
    egui::ScrollArea::vertical()
        .id_salt("modela_embroidery_sequence")
        .show(ui, |ui| {
            for (b, (first, members)) in blocks.iter().enumerate() {
                let t = &first.obj.thread;
                let c = t.rgb;
                ui.horizontal(|ui| {
                    let parts: Vec<(ShapeId, Part)> = members
                        .iter()
                        .map(|i| (rows[*i].id, rows[*i].part))
                        .collect();
                    if let Some(n) = color_swatch(
                        ui,
                        egui::Id::new(("modela_seq_block", b)),
                        c,
                        ds.chart.as_ref(),
                    ) {
                        recolor = Some((parts, n));
                    }
                    let name = if t.name.is_empty() {
                        format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
                    } else {
                        format!("{} {}", t.code, t.name)
                    };
                    let stitches: usize = members.iter().map(|i| rows[*i].obj.stitches.len()).sum();
                    ui.label(
                        egui::RichText::new(format!("{}. {name} · {stitches}", b + 1)).strong(),
                    );
                });
                for i in members {
                    let o = rows[*i];
                    let selected = app.selection.contains(&o.id);
                    let name = app
                        .doc()
                        .find_shape(o.id)
                        .and_then(|s| s.name.clone())
                        .unwrap_or_else(|| kind_name(o.kind));
                    let part = tr(if o.part == Part::Fill {
                        "modela_embroidery.fill"
                    } else {
                        "modela_embroidery.outline"
                    });
                    let auto = if o.auto {
                        format!(" {}", tr("modela_embroidery.auto"))
                    } else {
                        String::new()
                    };
                    let text = format!(
                        "{name} · {part}: {}{auto} · {}",
                        kind_name(o.kind),
                        o.obj.stitches.len()
                    );
                    let row = ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        if let Some(n) = color_swatch(
                            ui,
                            egui::Id::new(("modela_seq_row", *i)),
                            o.obj.thread.rgb,
                            ds.chart.as_ref(),
                        ) {
                            recolor = Some((vec![(o.id, o.part)], n));
                        }
                        let handle = ui
                            .dnd_drag_source(egui::Id::new(("modela_seq_drag", *i)), o.id, |ui| {
                                ui.label("⠿");
                            })
                            .response
                            .on_hover_text(tr("modela_embroidery.drag_tip"));
                        let label = ui
                            .selectable_label(selected, text)
                            .on_hover_text(tr("modela_embroidery.sequence_row_tip"));
                        if label.clicked() {
                            pick = Some(o.id);
                        }
                        handle.union(label)
                    });
                    let r = row.response.union(row.inner);
                    let rect = r.rect;
                    if let Some(from) = r.dnd_hover_payload::<ShapeId>() {
                        if *from != o.id {
                            let before = ui
                                .ctx()
                                .pointer_hover_pos()
                                .is_some_and(|q| q.y < rect.center().y);
                            let y = if before { rect.top() } else { rect.bottom() };
                            ui.painter().hline(
                                rect.x_range(),
                                y,
                                EStroke::new(2.0, Tokens::ACCENT),
                            );
                        }
                    }
                    if let Some(from) = r.dnd_release_payload::<ShapeId>() {
                        if *from != o.id {
                            let before = ui
                                .ctx()
                                .pointer_interact_pos()
                                .is_some_and(|q| q.y < rect.center().y);
                            drop = Some((*from, o.id, before));
                        }
                    }
                }
            }
            if rows.is_empty() {
                ui.label(
                    egui::RichText::new(tr("modela_embroidery.nothing_sews"))
                        .color(Tokens::text_dim()),
                );
            }
        });
    if let Some(id) = pick {
        app.select(vec![id]);
    }
    if let Some((parts, c)) = recolor {
        set_thread_color(app, &parts, c);
    }
    if let Some((from, to, before)) = drop {
        move_in_sequence(app, from, to, before);
    }
}

/// Window > Panels > Hoop: the hoop the page is, how the design fits it,
/// the fabric and the thread chart.
pub fn hoop_panel(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let mut p = prefs(&ctx);
    let before = p.clone();
    let ds = design(&p);
    let pv = preview(app, &p);
    let current = page_hoop(app);
    let text = current.map_or_else(
        || {
            let s = app.page_size();
            trf(
                "modela_embroidery.custom_hoop",
                &[
                    ("w", &format!("{:.0}", s.width)),
                    ("h", &format!("{:.0}", s.height)),
                ],
            )
        },
        |h| format!("{} {}", h.brand, h.name),
    );
    let mut picked = None;
    ui.label(tr("modela_embroidery.hoop"));
    egui::ComboBox::from_id_salt("modela_embroidery_hoop")
        .selected_text(text)
        .width(ui.available_width().min(260.0))
        .show_ui(ui, |ui| {
            for brand in hoops::brands() {
                ui.label(egui::RichText::new(brand).small().strong());
                for h in hoops::HOOPS.iter().filter(|h| h.brand == brand) {
                    let on = current.is_some_and(|c| c.id == h.id);
                    let label = if h.round {
                        format!("{} (⌀ {:.0} mm)", h.name, h.width)
                    } else {
                        format!("{} ({:.0} x {:.0} mm)", h.name, h.width, h.height)
                    };
                    if ui.selectable_label(on, label).clicked() {
                        picked = Some(h);
                    }
                }
            }
        })
        .response
        .on_hover_text(tr("modela_embroidery.hoop_tip"));
    if let Some(h) = picked {
        set_hoop(app, h);
    }
    if current.is_some_and(|h| (h.width - h.height).abs() > 0.05)
        && icons::labeled(
            ui,
            Icon::TurnHoop,
            &tr("modela_embroidery.turn_hoop"),
            &tr("modela_embroidery.turn_hoop_tip"),
        )
        .clicked()
    {
        let s = app.page_size();
        app.run(Command::ResizePage {
            page: app.page,
            size: Size::new(s.height, s.width),
        });
    }
    if let Some(b) = pv.design.bounds() {
        let s = app.page_size();
        let fit = hoops::fit(
            b.width(),
            b.height(),
            s.width,
            s.height,
            current.is_some_and(|h| h.round),
        );
        let (key, color) = match fit {
            hoops::Fit::Fits => ("modela_embroidery.fits", Tokens::text_dim()),
            hoops::Fit::Rotated => ("modela_embroidery.fits_rotated", WARN),
            hoops::Fit::TooLarge { .. } => ("modela_embroidery.too_large", ERROR),
        };
        ui.colored_label(
            color,
            trf(
                key,
                &[
                    ("w", &format!("{:.1}", b.width())),
                    ("h", &format!("{:.1}", b.height())),
                ],
            ),
        );
        let round = current.is_some_and(|h| h.round);
        if !view::outside_segments(&pv, round).is_empty() {
            ui.colored_label(ERROR, tr("modela_embroidery.outside_note"));
            if icons::labeled(
                ui,
                Icon::CenterInHoop,
                &tr("modela_embroidery.center_in_hoop"),
                &tr("modela_embroidery.center_in_hoop_tip"),
            )
            .clicked()
            {
                center_in_hoop(app);
            }
        }
    }
    ui.separator();
    ui.label(tr("modela_embroidery.fabric"));
    egui::ComboBox::from_id_salt("modela_embroidery_fabric")
        .selected_text(tr(&format!("modela_embroidery.fabric_{}", ds.fabric.id)))
        .show_ui(ui, |ui| {
            for f in FABRICS.iter() {
                ui.selectable_value(
                    &mut p.fabric,
                    f.id.to_string(),
                    tr(&format!("modela_embroidery.fabric_{}", f.id)),
                );
            }
        })
        .response
        .on_hover_text(tr("modela_embroidery.fabric_tip"));
    ui.label(tr("modela_embroidery.chart"));
    let chart_name = |id: &str| {
        tracedraw_emb_formats::charts::BUILT_IN
            .iter()
            .find(|(i, _)| *i == id)
            .map_or_else(
                || tr("modela_embroidery.no_chart_name"),
                |(_, n)| n.to_string(),
            )
    };
    egui::ComboBox::from_id_salt("modela_embroidery_chart")
        .selected_text(chart_name(&p.chart))
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut p.chart,
                "none".into(),
                tr("modela_embroidery.no_chart_name"),
            );
            for (id, name) in tracedraw_emb_formats::charts::BUILT_IN {
                ui.selectable_value(&mut p.chart, id.to_string(), name);
            }
        })
        .response
        .on_hover_text(tr("modela_embroidery.chart_tip"));
    if icons::labeled(
        ui,
        Icon::MatchThreads,
        &tr("modela_embroidery.match_threads"),
        &tr("modela_embroidery.match_threads_tip"),
    )
    .clicked()
    {
        match_threads(app, &ds);
    }
    if p != before {
        set_prefs(&ctx, p);
    }
}

/// Window > Panels > Embroidery: the selection's stitches, the sewing
/// sequence, and the hoop, fabric and threads.
pub fn panel(app: &mut App, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let mut p = prefs(&ctx);
    let before = p.clone();
    let ds = design(&p);

    lettering_section(app, ui);

    egui::CollapsingHeader::new(tr("modela_embroidery.stitches"))
        .default_open(true)
        .show(ui, |ui| {
            let objs = selected_objects(app);
            match objs.first() {
                None => {
                    ui.label(
                        egui::RichText::new(tr("modela_embroidery.select_objects"))
                            .color(Tokens::text_dim()),
                    );
                }
                Some(first) => {
                    let first = first.clone();
                    if objs.len() > 1 {
                        ui.label(trf(
                            "modela_embroidery.selected",
                            &[("count", &objs.len().to_string())],
                        ));
                    }
                    for part in Part::BOTH {
                        part_ui(app, ui, &ds, &first, part);
                        ui.add_space(4.0);
                    }
                    let mut stroke_first = sew::slots(&first).stroke_first;
                    if ui
                        .checkbox(&mut stroke_first, tr("modela_embroidery.outline_first"))
                        .on_hover_text(tr("modela_embroidery.outline_first_tip"))
                        .changed()
                    {
                        set_stroke_first(app, stroke_first);
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .button(tr("modela_embroidery.reset"))
                            .on_hover_text(tr("modela_embroidery.reset_tip"))
                            .clicked()
                        {
                            reset_selection(app);
                        }
                        if icons::labeled(
                            ui,
                            Icon::Satin,
                            &tr("modela_embroidery.convert_to_satin"),
                            &tr("modela_embroidery.convert_to_satin_tip"),
                        )
                        .clicked()
                        {
                            convert_to_satin(app, medial::Method::Centerline);
                        }
                    });
                }
            }
        })
        .header_response
        .on_hover_text(tr("modela_embroidery.stitches_tip"));

    check_section(app, ui, &mut p, &ds);

    if p != before {
        set_prefs(&ctx, p);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracedraw_core::geometry::Rect;

    fn square(app: &mut App) -> ShapeId {
        let id = app
            .new_shape(ShapeKind::Rect {
                rect: Rect::new(40.0, 40.0, 60.0, 60.0),
                radius: 0.0,
                corners: None,
            })
            .unwrap();
        app.run(Command::SetFill {
            shapes: vec![id],
            fill: Fill::Solid(Color::rgb8(10, 120, 40)),
        });
        id
    }

    #[test]
    fn stitch_type_is_one_undo_step_and_sets_the_outline_width() {
        let mut app = App::headless();
        let id = square(&mut app);
        app.select(vec![id]);
        let before = app.engine.history_labels().0.len();
        let fabric = tracedraw_emb_stitch::Fabric::default_fabric();
        set_kind(&mut app, fabric, Part::Stroke, StitchType::Satin);
        assert_eq!(app.engine.history_labels().0.len(), before + 1);
        let s = app.doc().find_shape(id).unwrap().clone();
        let parts = sew::sewn(&s, fabric);
        assert!(parts
            .iter()
            .any(|(p, st, _)| *p == Part::Stroke && st.kind == StitchType::Satin));
        let w = s.stroke.as_ref().map(|st| st.width).unwrap();
        assert!((w - StitchSettings::of(StitchType::Satin).satin.width).abs() < 1e-9);
    }

    #[test]
    fn convert_to_satin_adds_columns_and_hides_the_shape() {
        let mut app = App::headless();
        let id = app
            .new_shape(ShapeKind::Rect {
                rect: Rect::new(20.0, 20.0, 80.0, 26.0),
                radius: 0.0,
                corners: None,
            })
            .unwrap();
        app.run(Command::SetFill {
            shapes: vec![id],
            fill: Fill::Solid(Color::rgb8(10, 120, 40)),
        });
        app.select(vec![id]);
        convert_to_satin(&mut app, medial::Method::Centerline);
        assert!(!app.doc().find_shape(id).unwrap().visible);
        let made = app.selection.clone();
        assert!(!made.is_empty());
        let col = app.doc().find_shape(made[0]).unwrap().clone();
        let fabric = tracedraw_emb_stitch::Fabric::default_fabric();
        let parts = sew::sewn(&col, fabric);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].1.kind, StitchType::SatinBlocks);
        // Undo brings the shape back as it was.
        app.engine.undo().unwrap();
        assert!(app.doc().find_shape(id).unwrap().visible);
    }

    #[test]
    fn sort_by_color_groups_threads() {
        let mut app = App::headless();
        let colors = [[200, 0, 0], [0, 0, 200], [200, 0, 0], [0, 0, 200]];
        let ids: Vec<ShapeId> = colors
            .iter()
            .map(|c| {
                let id = square(&mut app);
                app.run(Command::SetFill {
                    shapes: vec![id],
                    fill: Fill::Solid(Color::rgb8(c[0], c[1], c[2])),
                });
                id
            })
            .collect();
        sort_by_color(&mut app);
        let order: Vec<ShapeId> = app
            .page_layers()
            .iter()
            .flat_map(|l| l.shapes.iter().map(|s| s.id))
            .collect();
        assert_eq!(order, vec![ids[0], ids[2], ids[1], ids[3]]);
    }

    #[test]
    fn hoop_sizes_the_page() {
        let mut app = App::headless();
        let h = &hoops::HOOPS[0];
        set_hoop(&mut app, h);
        assert_eq!(page_hoop(&app).map(|x| x.id), Some(h.id));
    }

    #[test]
    fn preview_is_cached_until_the_drawing_changes() {
        let mut app = App::headless();
        square(&mut app);
        let p = Prefs::default();
        let a = preview(&app, &p);
        let b = preview(&app, &p);
        assert!(Arc::ptr_eq(&a, &b));
        square(&mut app);
        let c = preview(&app, &p);
        assert!(!Arc::ptr_eq(&a, &c));
        assert!(c.design.stitch_count() > a.design.stitch_count());
    }

    fn rect(app: &mut App, r: Rect, c: [u8; 3]) -> ShapeId {
        let id = app
            .new_shape(ShapeKind::Rect {
                rect: r,
                radius: 0.0,
                corners: None,
            })
            .unwrap();
        app.run(Command::SetFill {
            shapes: vec![id],
            fill: Fill::Solid(Color::rgb8(c[0], c[1], c[2])),
        });
        app.run(Command::SetStroke {
            shapes: vec![id],
            stroke: None,
        });
        id
    }

    fn area(app: &App, id: ShapeId) -> f64 {
        let s = app.doc().find_shape(id).unwrap();
        kurbo::Shape::area(&s.page_path()).abs()
    }

    #[test]
    fn remove_overlaps_cuts_the_covered_fill_and_keeps_an_overlap() {
        let mut app = App::headless();
        let under = rect(&mut app, Rect::new(20.0, 20.0, 80.0, 80.0), [200, 0, 0]);
        let over = rect(&mut app, Rect::new(50.0, 20.0, 80.0, 80.0), [0, 0, 200]);
        let ds = design(&Prefs::default());
        let before = app.engine.history_labels().0.len();
        remove_overlaps(&mut app, &ds, &[]);
        assert_eq!(app.engine.history_labels().0.len(), before + 1);
        // 30 x 60 left, plus 1 mm under the covering edge.
        let a = area(&app, under);
        assert!((a - 31.0 * 60.0).abs() < 2.0, "{a}");
        // The top fill is not cut.
        assert!((area(&app, over) - 30.0 * 60.0).abs() < 1e-6);
    }

    #[test]
    fn check_finds_a_fill_too_thin_and_a_design_past_the_hoop() {
        let mut app = App::headless();
        let thin = rect(&mut app, Rect::new(20.0, 20.0, 80.0, 21.0), [0, 120, 0]);
        let size = app.page_size();
        let out = rect(
            &mut app,
            Rect::new(size.width - 5.0, 100.0, size.width + 20.0, 120.0),
            [0, 0, 200],
        );
        let p = Prefs::default();
        let found = problems(&app, &p);
        assert!(found
            .iter()
            .any(|pr| pr.kind == check::Kind::ThinFill && pr.ids == vec![thin]));
        let outside = found
            .iter()
            .find(|pr| pr.kind == check::Kind::OutsideHoop)
            .expect("outside the hoop");
        assert!(outside.error);
        // Errors come first.
        assert!(found.first().is_some_and(|pr| pr.error));
        let _ = out;
        // Center in Hoop is the fix, and it brings the design inside.
        apply_fix(&mut app, &design(&p), outside.fix.as_ref().unwrap());
        assert!(!problems(&app, &p)
            .iter()
            .any(|pr| pr.kind == check::Kind::OutsideHoop));
    }

    #[test]
    fn lettering_is_added_and_edited_in_one_step() {
        let mut app = App::headless();
        add_lettering(&mut app);
        let id = app.selection[0];
        let s = app.doc().find_shape(id).unwrap().clone();
        let l = letters::lettering_of(&s).unwrap();
        let p = Prefs::default();
        assert!(preview(&app, &p).design.stitch_count() > 50);
        let before = app.engine.history_labels().0.len();
        let mut bigger = l.clone();
        bigger.text = "AB".into();
        bigger.height = 20.0;
        set_lettering(&mut app, id, &bigger);
        assert_eq!(app.engine.history_labels().0.len(), before + 1);
        let s2 = app.doc().find_shape(id).unwrap().clone();
        assert_eq!(
            letters::lettering_of(&s2).map(|l| l.text),
            Some("AB".into())
        );
        // Where it was put stays.
        assert_eq!(s2.transform, s.transform);
        let w = s2.stroke.as_ref().unwrap().width;
        assert!((w - bigger.column_width()).abs() < 1e-9);
    }

    #[test]
    fn dropping_a_row_sews_it_before_another() {
        let mut app = App::headless();
        let a = rect(&mut app, Rect::new(10.0, 10.0, 20.0, 20.0), [200, 0, 0]);
        let b = rect(&mut app, Rect::new(30.0, 10.0, 40.0, 20.0), [0, 200, 0]);
        let c = rect(&mut app, Rect::new(50.0, 10.0, 60.0, 20.0), [0, 0, 200]);
        move_in_sequence(&mut app, c, a, true);
        let order: Vec<ShapeId> = app
            .page_layers()
            .iter()
            .flat_map(|l| l.shapes.iter().map(|s| s.id))
            .collect();
        assert_eq!(order, vec![c, a, b]);
        let pv = preview(&app, &Prefs::default());
        assert_eq!(pv.parts.first().map(|o| o.id), Some(c));
        move_in_sequence(&mut app, c, b, false);
        let order: Vec<ShapeId> = app
            .page_layers()
            .iter()
            .flat_map(|l| l.shapes.iter().map(|s| s.id))
            .collect();
        assert_eq!(order, vec![a, b, c]);
    }

    #[test]
    fn a_thread_colour_changes_the_paint_it_sews_from() {
        let mut app = App::headless();
        let a = rect(&mut app, Rect::new(10.0, 10.0, 20.0, 20.0), [200, 0, 0]);
        set_thread_color(&mut app, &[(a, Part::Fill)], [1, 2, 3]);
        let s = app.doc().find_shape(a).unwrap();
        assert_eq!(sew::fill_rgb(s), Some([1, 2, 3]));
        // An outline part changes the outline.
        app.run(Command::SetStroke {
            shapes: vec![a],
            stroke: Some(Stroke::hairline(Color::BLACK)),
        });
        set_thread_color(&mut app, &[(a, Part::Stroke)], [9, 8, 7]);
        let s = app.doc().find_shape(a).unwrap();
        assert_eq!(sew::stroke_rgb(s), Some([9, 8, 7]));
        assert_eq!(sew::fill_rgb(s), Some([1, 2, 3]));
    }

    #[test]
    fn density_colors_rise_with_layers() {
        assert!(density_color(1.0).is_none());
        assert!(density_color(1.5).is_some());
        assert_ne!(density_color(1.5), density_color(3.0));
    }

    /// How long the preview of a design of 40 filled and outlined shapes
    /// takes (it is worked out once per edit, never per frame):
    /// `cargo test --release -- --ignored frame_times --nocapture`.
    #[test]
    #[ignore]
    fn frame_times_embroidery_preview() {
        let mut app = App::headless();
        for i in 0..40 {
            let (x, y) = (10.0 + (i % 8) as f64 * 24.0, 20.0 + (i / 8) as f64 * 24.0);
            let id = rect(
                &mut app,
                Rect::new(x, y, x + 20.0, y + 20.0),
                [i as u8 * 5, 80, 40],
            );
            app.run(Command::SetStroke {
                shapes: vec![id],
                stroke: Some(Stroke::hairline(Color::BLACK)),
            });
        }
        let p = Prefs::default();
        let t = web_time::Instant::now();
        let pv = preview(&app, &p);
        let first = t.elapsed();
        let t = web_time::Instant::now();
        let _ = preview(&app, &p);
        let again = t.elapsed();
        println!(
            "embroidery preview: {} stitches in {first:?}, cached {again:?}",
            pv.design.stitch_count()
        );
    }
}
