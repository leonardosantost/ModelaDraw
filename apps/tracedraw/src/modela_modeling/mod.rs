//! Modela pattern-making integration.
//!
//! This module deliberately owns the Modela-specific UI and object metadata.
//! TraceDraw only needs a few small integration points, which keeps upstream
//! updates and rebases manageable.

use crate::app::{App, DockerTab};
use crate::i18n::tr;
use crate::theme::Tokens;
use crate::tools::Tool;
use egui::{Color32, Painter, Pos2, Response, Stroke as EStroke, Ui};
use kurbo::{Affine, BezPath, Point, Rect, Shape as _, Vec2};
use tracedraw_core::{Color, Command, Fill, Shape, ShapeKind, Stroke};

const TYPE: &str = "modela.type";
const MOLD: &str = "mold";
const GRAIN: &str = "modela.grain_angle_deg";
const ROTATION: &str = "modela.rotation_constraint";
const SHOW_NAME: &str = "modela.show_name";
const PAIR: &str = "modela.pair";
const MIRROR: &str = "modela.allow_mirror";
const QUANTITY: &str = "modela.quantity";
const FABRIC: &str = "modela.fabric";
const SEAM_MM: &str = "modela.seam_allowance_mm";
const GRADE_BASE: &str = "modela.grade.base_size";
const GRADE_SIZES: &str = "modela.grade.sizes";
const GRADE_X_MM: &str = "modela.grade.increment_x_mm";
const GRADE_Y_MM: &str = "modela.grade.increment_y_mm";
const GRADE_SIZE: &str = "modela.grade.size";
const FOLD_KIND: &str = "modela.fold.original_kind";
const FOLD_AXIS: &str = "modela.fold.axis";
const NOTCH: &str = "notch";
const NOTCH_PARENT: &str = "modela.notch.parent";
const ROLE: &str = "modela.role";
const OUTLINE: &str = "outline";
const INTERNAL: &str = "internal";
const TOOL_ID: &str = "modela.active_canvas_tool";
const HOVER_ID: &str = "modela.canvas_hover";
const INTERNAL_START_ID: &str = "modela.internal_start";
static SHOW_GRADING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelTool {
    Notch,
    Internal,
    Fold,
    Unfold,
}

fn active_tool(ui: &Ui) -> Option<ModelTool> {
    ui.ctx().data(|data| data.get_temp(egui::Id::new(TOOL_ID)))
}

fn activate_tool(ui: &Ui, tool: ModelTool) {
    ui.ctx()
        .data_mut(|data| data.insert_temp(egui::Id::new(TOOL_ID), tool));
}

fn value<'a>(data: &'a [(String, String)], key: &str) -> Option<&'a str> {
    data.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

fn put(data: &mut Vec<(String, String)>, key: &str, value: impl ToString) {
    let value = value.to_string();
    if let Some((_, old)) = data.iter_mut().find(|(k, _)| k == key) {
        *old = value;
    } else {
        data.push((key.to_string(), value));
    }
}

fn bool_value(data: &[(String, String)], key: &str, default: bool) -> bool {
    value(data, key).map_or(default, |v| v == "true")
}

fn number(data: &[(String, String)], key: &str, default: f64) -> f64 {
    value(data, key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn is_mold(shape: &Shape) -> bool {
    value(&shape.data, TYPE) == Some(MOLD)
}

fn mold_outline_path(shape: &Shape) -> BezPath {
    if let ShapeKind::Group { children } = &shape.kind {
        if let Some(outline) = children
            .iter()
            .find(|child| value(&child.data, ROLE) == Some(OUTLINE))
        {
            return shape.transform * outline.transform * outline.local_path();
        }
    }
    shape.page_path()
}

fn mold_bounds(shape: &Shape) -> Rect {
    mold_outline_path(shape).bounding_box()
}

/// Draw the pattern-only presentation without changing the source vector:
/// orange contour, piece name and grain-direction arrow.
pub fn draw_molds(app: &App, painter: &Painter) {
    for layer in app.page_layers() {
        if !layer.visible {
            continue;
        }
        for shape in &layer.shapes {
            draw_mold_tree(app, painter, shape, Affine::IDENTITY);
        }
    }
}

fn draw_mold_tree(app: &App, painter: &Painter, shape: &Shape, parent: Affine) {
    if !shape.visible {
        return;
    }
    let mut page_shape = shape.clone();
    page_shape.transform = parent * shape.transform;
    if is_mold(&page_shape) {
        draw_one_mold(app, painter, &page_shape);
        return;
    }
    if let ShapeKind::Group { children } = &shape.kind {
        let next = parent * shape.transform;
        for child in children {
            draw_mold_tree(app, painter, child, next);
        }
    }
}

fn draw_one_mold(app: &App, painter: &Painter, shape: &Shape) {
    if value(&shape.data, GRADE_SIZE).is_some()
        && !SHOW_GRADING.load(std::sync::atomic::Ordering::Relaxed)
    {
        return;
    }
    let path = mold_outline_path(shape);
    let color = value(&shape.data, "modela.grade.color")
        .and_then(parse_hex_color)
        .unwrap_or(Color32::from_rgb(232, 104, 42));
    let mut points = Vec::new();
    kurbo::flatten(path.elements().iter().copied(), 0.2, &mut |el| match el {
        kurbo::PathEl::MoveTo(p) => {
            if points.len() > 1 {
                painter.add(egui::Shape::line(
                    std::mem::take(&mut points),
                    EStroke::new(2.0, color),
                ));
            }
            points.push(app.view.to_screen(p));
        }
        kurbo::PathEl::LineTo(p) => points.push(app.view.to_screen(p)),
        kurbo::PathEl::ClosePath => {
            if let Some(first) = points.first().copied() {
                points.push(first);
            }
        }
        _ => {}
    });
    if points.len() > 1 {
        painter.add(egui::Shape::line(points, EStroke::new(2.0, color)));
    }
    let bounds = path.bounding_box();
    let center = bounds.center();
    if bool_value(&shape.data, SHOW_NAME, true) {
        painter.text(
            app.view.to_screen(center) + egui::vec2(0.0, 18.0),
            egui::Align2::CENTER_TOP,
            shape.name.as_deref().unwrap_or("Molde"),
            egui::FontId::proportional(13.0),
            color,
        );
    }
    let direction = match value(&shape.data, GRAIN).unwrap_or("down") {
        "up" => Vec2::new(0.0, -1.0),
        "right" => Vec2::new(1.0, 0.0),
        "left" => Vec2::new(-1.0, 0.0),
        _ => Vec2::new(0.0, 1.0),
    };
    let length = (bounds.width().min(bounds.height()) * 0.28).max(8.0);
    let a = app.view.to_screen(center);
    let b = app.view.to_screen(center + direction * length);
    painter.line_segment([a, b], EStroke::new(1.5, color));
    grain_arrow_head(painter, b, a, color);
}

fn parse_hex_color(value: &str) -> Option<Color32> {
    let hex = value.strip_prefix('#').unwrap_or(value);
    (hex.len() == 6).then_some(())?;
    let number = u32::from_str_radix(hex, 16).ok()?;
    Some(Color32::from_rgb(
        (number >> 16) as u8,
        (number >> 8) as u8,
        number as u8,
    ))
}

fn hex_color(color: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}

pub fn draw_tool_feedback(app: &App, painter: &Painter) {
    let Some((tool, point, a, b)) = painter
        .ctx()
        .data(|data| data.get_temp::<(ModelTool, Point, Point, Point)>(egui::Id::new(HOVER_ID)))
    else {
        return;
    };
    let color = match tool {
        ModelTool::Notch => Color32::RED,
        ModelTool::Internal => Color32::BLUE,
        ModelTool::Fold | ModelTool::Unfold => Color32::from_rgb(232, 104, 42),
    };
    let p = app.view.to_screen(point);
    painter.circle_filled(p, 4.0, color);
    if matches!(tool, ModelTool::Fold | ModelTool::Unfold) {
        painter.line_segment(
            [app.view.to_screen(a), app.view.to_screen(b)],
            EStroke::new(3.0, color),
        );
    }
}

fn grain_arrow_head(painter: &Painter, tip: Pos2, from: Pos2, color: Color32) {
    let d = (tip - from).normalized();
    let n = egui::vec2(-d.y, d.x);
    painter.line_segment([tip, tip - d * 8.0 + n * 4.0], EStroke::new(1.5, color));
    painter.line_segment([tip, tip - d * 8.0 - n * 4.0], EStroke::new(1.5, color));
}

fn selected_mold(app: &App) -> Option<Shape> {
    let shapes = app.selected_shapes();
    (shapes.len() == 1 && is_mold(&shapes[0])).then(|| shapes[0].clone())
}

fn is_closed(shape: &Shape) -> bool {
    matches!(
        shape.page_path().elements().last(),
        Some(kurbo::PathEl::ClosePath)
    )
}

/// Convert the selected closed vector object into a persistent Modela mold.
pub fn create_mold(app: &mut App) {
    let shapes = app.selected_shapes();
    if shapes.len() != 1 || app.selection_nested() {
        app.status = tr("modela.status.select_one_closed");
        return;
    }
    let shape = &shapes[0];
    if !is_closed(shape) {
        app.status = tr("modela.status.shape_must_be_closed");
        return;
    }

    let mut data = shape.data.clone();
    put(&mut data, TYPE, MOLD);
    if value(&data, GRAIN).is_none() {
        put(&mut data, GRAIN, "down");
        put(&mut data, ROTATION, "free");
        put(&mut data, SHOW_NAME, true);
        put(&mut data, PAIR, false);
        put(&mut data, MIRROR, false);
        put(&mut data, QUANTITY, 1);
        put(&mut data, FABRIC, "");
        put(&mut data, SEAM_MM, 10);
    }
    let name = shape
        .name
        .clone()
        .or_else(|| Some(tr("modela.default_mold_name")));
    let mut outline = shape.clone();
    outline.id = app.engine.new_shape_id();
    outline.name = None;
    outline.transform = Affine::IDENTITY;
    put(&mut outline.data, ROLE, OUTLINE);
    let commands = [
        Command::SetShapeKind {
            shape: shape.id,
            kind: ShapeKind::Group {
                children: vec![outline],
            },
        },
        Command::SetObjectData {
            shape: shape.id,
            data,
        },
        Command::SetShapeName {
            shape: shape.id,
            name: name.clone(),
        },
    ];
    if app
        .run_batch_keeping_lines("Create Modela Mold", &commands)
        .is_ok()
    {
        app.status = tr("modela.status.mold_created");
        app.dialog = crate::ui::dialogs::Dialog::RenameObject {
            shape: shape.id,
            name: name.unwrap_or_default(),
        };
    }
}

fn nearest_segment(path: &BezPath, target: Point) -> Option<(Point, Point, Point)> {
    let mut nearest = None;
    let mut best = f64::MAX;
    let mut previous = None;
    let mut start = None;
    let mut consider = |a: Point, b: Point| {
        let ab = b - a;
        let t = if ab.hypot2() > 1e-12 {
            ((target - a).dot(ab) / ab.hypot2()).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let point = a + ab * t;
        let distance = (target - point).hypot2();
        if distance < best {
            best = distance;
            nearest = Some((point, a, b));
        }
    };
    kurbo::flatten(path.elements().iter().copied(), 0.1, &mut |el| match el {
        kurbo::PathEl::MoveTo(p) => {
            previous = Some(p);
            start = Some(p);
        }
        kurbo::PathEl::LineTo(p) => {
            if let Some(a) = previous {
                consider(a, p);
            }
            previous = Some(p);
        }
        kurbo::PathEl::ClosePath => {
            if let (Some(a), Some(b)) = (previous, start) {
                consider(a, b);
            }
        }
        _ => {}
    });
    nearest
}

fn add_child_path(app: &mut App, mold: Shape, page_path: BezPath, role: &str, color: Color) {
    let inverse = mold.transform.inverse();
    let id = app.engine.new_shape_id();
    let mut child = Shape::new(
        id,
        ShapeKind::Path {
            path: inverse * page_path,
            closed: false,
        },
    );
    child.name = Some(tr(if role == NOTCH {
        "modela.notch"
    } else {
        "modela.internal_line"
    }));
    child.fill = Fill::None;
    child.stroke = Some(Stroke::new(color, if role == NOTCH { 0.5 } else { 0.35 }));
    child.data = vec![
        (ROLE.into(), role.into()),
        (NOTCH_PARENT.into(), mold.id.0.to_string()),
    ];
    let mut children = match mold.kind.clone() {
        ShapeKind::Group { children } => children,
        kind => {
            let mut outline = mold.clone();
            outline.id = app.engine.new_shape_id();
            outline.kind = kind;
            outline.transform = Affine::IDENTITY;
            put(&mut outline.data, ROLE, OUTLINE);
            vec![outline]
        }
    };
    children.push(child);
    app.run(Command::SetShapeKind {
        shape: mold.id,
        kind: ShapeKind::Group { children },
    });
    app.select(vec![mold.id]);
}

/// Modela canvas tools run before TraceDraw's normal selection tool.
pub fn canvas_input(app: &mut App, response: &Response, point: Point) -> bool {
    let Some(tool) = response
        .ctx
        .data(|data| data.get_temp::<ModelTool>(egui::Id::new(TOOL_ID)))
    else {
        return false;
    };
    response.ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
    let Some(mold) = selected_mold(app) else {
        return true;
    };
    let path = mold_outline_path(&mold);
    let Some((near, a, b)) = nearest_segment(&path, point) else {
        return true;
    };
    let threshold = 10.0 / app.view.zoom as f64;
    let tool_point = if tool == ModelTool::Internal && path.contains(point) {
        Some(point)
    } else if (near - point).hypot() <= threshold {
        Some(near)
    } else {
        None
    };
    if let Some(tool_point) = tool_point {
        response.ctx.data_mut(|data| {
            data.insert_temp(egui::Id::new(HOVER_ID), (tool, tool_point, a, b));
        });
        if response.clicked_by(egui::PointerButton::Primary) {
            match tool {
                ModelTool::Notch => add_notch_at(app, mold, tool_point),
                ModelTool::Internal => {
                    let id = egui::Id::new(INTERNAL_START_ID);
                    let start = response.ctx.data(|data| data.get_temp::<Point>(id));
                    if let Some(start) = start {
                        let mut line = BezPath::new();
                        line.move_to(start);
                        line.line_to(tool_point);
                        add_child_path(app, mold, line, INTERNAL, Color::rgb8(35, 105, 220));
                        response.ctx.data_mut(|data| data.remove::<Point>(id));
                    } else {
                        response
                            .ctx
                            .data_mut(|data| data.insert_temp(id, tool_point));
                    }
                }
                ModelTool::Fold => fold_from_segment(app, a, b),
                ModelTool::Unfold => unfold_from_segment(app, a, b),
            }
        }
    } else {
        response.ctx.data_mut(|data| {
            data.remove::<(ModelTool, Point, Point, Point)>(egui::Id::new(HOVER_ID))
        });
    }
    true
}

fn add_notch_at(app: &mut App, mold: Shape, edge: Point) {
    let toward_center = mold_bounds(&mold).center() - edge;
    if toward_center.hypot2() < 1e-12 {
        return;
    }
    let length = number(&mold.data, "modela.notch_length_mm", 10.0).max(1.0);
    let end = edge + toward_center.normalize() * length;
    let mut path = BezPath::new();
    path.move_to(edge);
    path.line_to(end);
    add_child_path(app, mold, path, NOTCH, Color::rgb8(220, 35, 45));
    app.status = tr("modela.status.notch_created");
}

fn fold_from_segment(app: &mut App, a: Point, b: Point) {
    let Some(mold) = selected_mold(app) else {
        return;
    };
    let d = b - a;
    let center = mold_bounds(&mold).center();
    if d.y.abs() >= d.x.abs() {
        fold(app, true, a.x <= center.x);
    } else {
        fold(app, false, a.y <= center.y);
    }
}

fn unfold_from_segment(app: &mut App, a: Point, b: Point) {
    let Some(mold) = selected_mold(app) else {
        return;
    };
    let d = b - a;
    if d.hypot2() < 1e-12 {
        return;
    }
    let angle = d.atan2();
    let reflect = Affine::translate(a.to_vec2())
        * Affine::rotate(angle)
        * Affine::scale_non_uniform(1.0, -1.0)
        * Affine::rotate(-angle)
        * Affine::translate(-a.to_vec2());
    let original = mold_outline_path(&mold);
    let mirrored = reflect * original.clone();
    let result =
        tracedraw_core::shaping::overlay(&original, &mirrored, tracedraw_core::shaping::Op::Weld);
    let local = mold.transform.inverse() * result;
    let mut children = match mold.kind.clone() {
        ShapeKind::Group { children } => children,
        _ => Vec::new(),
    };
    if let Some(outline) = children
        .iter_mut()
        .find(|child| value(&child.data, ROLE) == Some(OUTLINE))
    {
        outline.kind = ShapeKind::Path {
            path: local,
            closed: true,
        };
        outline.transform = Affine::IDENTITY;
    } else {
        app.status = tr("modela.status.fold_failed");
        return;
    }
    app.run(Command::SetShapeKind {
        shape: mold.id,
        kind: ShapeKind::Group { children },
    });
    app.status = tr("modela.status.unfolded");
}

/// Keep one half of the mold and remember its exact original geometry so
/// Unfold can restore it losslessly.
pub fn fold(app: &mut App, vertical: bool, keep_min: bool) {
    let Some(mold) = selected_mold(app) else {
        app.status = tr("modela.select_mold");
        return;
    };
    if value(&mold.data, FOLD_KIND).is_some() {
        app.status = tr("modela.status.already_folded");
        return;
    }
    let bounds = mold_bounds(&mold);
    let center = bounds.center();
    let half = if vertical {
        if keep_min {
            Rect::new(bounds.x0, bounds.y0, center.x, bounds.y1)
        } else {
            Rect::new(center.x, bounds.y0, bounds.x1, bounds.y1)
        }
    } else if keep_min {
        Rect::new(bounds.x0, bounds.y0, bounds.x1, center.y)
    } else {
        Rect::new(bounds.x0, center.y, bounds.x1, bounds.y1)
    };
    let clip = half.to_path(0.01);
    let result = tracedraw_core::shaping::overlay(
        &mold_outline_path(&mold),
        &clip,
        tracedraw_core::shaping::Op::Intersect,
    );
    if result.elements().is_empty() {
        app.status = tr("modela.status.fold_failed");
        return;
    }
    let Ok(original) = serde_json::to_string(&mold.kind) else {
        app.status = tr("modela.status.fold_failed");
        return;
    };
    let mut data = mold.data.clone();
    put(&mut data, FOLD_KIND, original);
    put(
        &mut data,
        FOLD_AXIS,
        match (vertical, keep_min) {
            (true, true) => "vertical_min",
            (true, false) => "vertical_max",
            (false, true) => "horizontal_min",
            (false, false) => "horizontal_max",
        },
    );
    let local = mold.transform.inverse() * result;
    let commands = [
        Command::SetShapeKind {
            shape: mold.id,
            kind: ShapeKind::Path {
                path: local,
                closed: true,
            },
        },
        Command::SetObjectData {
            shape: mold.id,
            data,
        },
    ];
    if app
        .run_batch_keeping_lines("Fold Modela Mold", &commands)
        .is_ok()
    {
        app.status = tr("modela.status.folded");
    }
}

pub fn unfold(app: &mut App) {
    let Some(mold) = selected_mold(app) else {
        app.status = tr("modela.select_mold");
        return;
    };
    let Some(encoded) = value(&mold.data, FOLD_KIND) else {
        app.status = tr("modela.status.not_folded");
        return;
    };
    let Ok(kind) = serde_json::from_str::<ShapeKind>(encoded) else {
        app.status = tr("modela.status.fold_failed");
        return;
    };
    let mut data = mold.data.clone();
    data.retain(|(key, _)| key != FOLD_KIND && key != FOLD_AXIS);
    let commands = [
        Command::SetShapeKind {
            shape: mold.id,
            kind,
        },
        Command::SetObjectData {
            shape: mold.id,
            data,
        },
    ];
    if app
        .run_batch_keeping_lines("Unfold Modela Mold", &commands)
        .is_ok()
    {
        app.status = tr("modela.status.unfolded");
    }
}

/// Create editable contours for every configured size. Each size grows or
/// shrinks around the base contour by the configured horizontal/vertical
/// increment, preserving the original object and its Modela metadata.
pub fn generate_grading(app: &mut App) {
    let Some(mold) = selected_mold(app) else {
        app.status = tr("modela.select_mold");
        return;
    };
    let sizes: Vec<String> = value(&mold.data, GRADE_SIZES)
        .unwrap_or("PP, P, M, G, GG")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    let base = value(&mold.data, GRADE_BASE).unwrap_or("M");
    let Some(base_index) = sizes.iter().position(|size| size == base) else {
        app.status = tr("modela.status.base_size_missing");
        return;
    };
    let bounds = mold_bounds(&mold);
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return;
    }
    let dx = number(&mold.data, GRADE_X_MM, 10.0);
    let dy = number(&mold.data, GRADE_Y_MM, 10.0);
    let Some(layer) = app.active_layer() else {
        return;
    };
    let center = bounds.center();
    let mut commands = Vec::new();
    let mut ids = Vec::new();
    let palette = [
        "#e53935", "#fb8c00", "#fdd835", "#43a047", "#1e88e5", "#5e35b1", "#d81b60",
    ];
    for (index, size) in sizes.iter().enumerate() {
        if index == base_index {
            continue;
        }
        let step = index as f64 - base_index as f64;
        let width = bounds.width() + dx * step;
        let height = bounds.height() + dy * step;
        if width <= 0.0 || height <= 0.0 {
            continue;
        }
        let id = app.engine.new_shape_id();
        let mut copy = crate::object_clone::linked_copy(&mold, id, &mut app.engine);
        let scale = Affine::translate(center.to_vec2())
            * Affine::scale_non_uniform(width / bounds.width(), height / bounds.height())
            * Affine::translate(-center.to_vec2());
        copy.transform = scale * copy.transform;
        copy.name = Some(format!(
            "{} - {}",
            mold.name.as_deref().unwrap_or("Molde"),
            size
        ));
        put(&mut copy.data, GRADE_SIZE, size);
        put(
            &mut copy.data,
            "modela.grade.color",
            palette[index % palette.len()],
        );
        commands.push(Command::AddShape { layer, shape: copy });
        ids.push(id);
    }
    if commands.is_empty() {
        return;
    }
    if app
        .run_batch_keeping_lines("Generate Modela Grading", &commands)
        .is_ok()
    {
        let count = commands.len();
        if ids.len() > 1 {
            app.run(Command::Group {
                shapes: ids.clone(),
            });
            if let Some(group) = app
                .page_layers()
                .into_iter()
                .flat_map(|layer| &layer.shapes)
                .filter(|candidate| matches!(candidate.kind, ShapeKind::Group { .. }))
                .max_by_key(|candidate| candidate.id)
            {
                app.run(Command::SetShapeName {
                    shape: group.id,
                    name: Some(format!(
                        "{} - {}",
                        mold.name.as_deref().unwrap_or("Molde"),
                        tr("docker.modela_grading")
                    )),
                });
            }
        }
        app.select(vec![mold.id]);
        app.status = crate::i18n::trf(
            "modela.status.grading_created",
            &[("count", &count.to_string())],
        );
    }
}

fn open(app: &mut App, tab: DockerTab) {
    app.open_docker(tab);
}

/// Compact shortcut bar shown only by the Modeling workspace.
pub fn top_bar(app: &mut App, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.strong(tr("modela.modeling"));
        ui.separator();
        if ui.button(tr("modela.create_mold")).clicked() {
            create_mold(app);
        }
        if ui.button(tr("docker.modela_properties")).clicked() {
            open(app, DockerTab::ModelaProperties);
        }
        if ui.button(tr("modela.notch")).clicked() {
            activate_tool(ui, ModelTool::Notch);
        }
        if ui.button(tr("modela.fold")).clicked() {
            activate_tool(ui, ModelTool::Fold);
        }
        if ui.button(tr("modela.unfold")).clicked() {
            activate_tool(ui, ModelTool::Unfold);
        }
        if ui.button(tr("docker.modela_grading")).clicked() {
            open(app, DockerTab::ModelaGrading);
        }
        if ui.button(tr("docker.modela_nesting")).clicked() {
            open(app, DockerTab::ModelaNesting);
        }
        if ui.button(tr("docker.objects")).clicked() {
            open(app, DockerTab::Objects);
        }
    });
}

#[derive(Clone, Copy)]
enum ModelIcon {
    Create,
    Properties,
    Notch,
    Fold,
    Unfold,
    Internal,
    Seam,
    Measure,
    Grade,
    Nest,
}

fn tool_button(ui: &mut Ui, icon: ModelIcon, tip: String, active: bool) -> bool {
    let response = ui
        .add_sized([34.0, 34.0], egui::Button::new("").selected(active))
        .on_hover_text(tip);
    let r = response.rect.shrink(8.0);
    let p = ui.painter();
    let c = Tokens::text();
    let s = EStroke::new(1.7, c);
    match icon {
        ModelIcon::Create => {
            p.rect_stroke(r, 2.0, s, egui::StrokeKind::Middle);
            p.line_segment([r.center_top(), r.center_bottom()], s);
            p.line_segment([r.left_center(), r.right_center()], s);
        }
        ModelIcon::Properties => {
            for y in [r.top() + 3.0, r.center().y, r.bottom() - 3.0] {
                p.line_segment([egui::pos2(r.left(), y), egui::pos2(r.right(), y)], s);
                p.circle_filled(egui::pos2(r.left() + 5.0, y), 2.2, c);
            }
        }
        ModelIcon::Notch => {
            p.line_segment([r.left_bottom(), r.center_top()], s);
            p.line_segment([r.center_top(), r.right_bottom()], s);
            p.line_segment(
                [r.center_top(), r.center_bottom()],
                EStroke::new(2.2, Color32::RED),
            );
        }
        ModelIcon::Fold | ModelIcon::Unfold => {
            p.rect_stroke(r, 1.0, s, egui::StrokeKind::Middle);
            let x = r.center().x;
            p.line_segment([egui::pos2(x, r.top()), egui::pos2(x, r.bottom())], s);
            let dir = if matches!(icon, ModelIcon::Fold) {
                -1.0
            } else {
                1.0
            };
            let a = egui::pos2(x + dir * 7.0, r.center().y);
            let b = egui::pos2(x + dir * 1.0, r.center().y);
            p.arrow(a, b - a, s);
        }
        ModelIcon::Internal => {
            p.rect_stroke(r, 3.0, EStroke::new(1.0, c), egui::StrokeKind::Middle);
            p.line_segment(
                [r.left_center(), r.right_center()],
                EStroke::new(2.0, Color32::BLUE),
            );
        }
        ModelIcon::Seam => {
            p.rect_stroke(r, 4.0, s, egui::StrokeKind::Middle);
            p.rect_stroke(
                r.shrink(4.0),
                3.0,
                EStroke::new(1.0, c),
                egui::StrokeKind::Middle,
            );
        }
        ModelIcon::Measure => {
            p.line_segment([r.left_bottom(), r.right_top()], s);
            p.line_segment([r.left_bottom(), r.left_bottom() + egui::vec2(5.0, 0.0)], s);
            p.line_segment([r.right_top(), r.right_top() - egui::vec2(5.0, 0.0)], s);
        }
        ModelIcon::Grade => {
            for inset in [0.0, 3.0, 6.0] {
                p.rect_stroke(
                    r.shrink(inset),
                    2.0,
                    EStroke::new(1.0, c),
                    egui::StrokeKind::Middle,
                );
            }
        }
        ModelIcon::Nest => {
            p.rect_stroke(r, 1.0, s, egui::StrokeKind::Middle);
            p.circle_stroke(r.left_top() + egui::vec2(5.0, 5.0), 3.0, s);
            p.circle_stroke(r.right_bottom() - egui::vec2(5.0, 5.0), 3.0, s);
        }
    }
    response.clicked()
}

/// Second, Modela-owned toolbox column beside TraceDraw's native toolbox.
pub fn toolbox(app: &mut App, ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        if tool_button(ui, ModelIcon::Create, tr("modela.create_mold"), false) {
            create_mold(app);
        }
        if tool_button(
            ui,
            ModelIcon::Properties,
            tr("docker.modela_properties"),
            false,
        ) {
            open(app, DockerTab::ModelaProperties);
        }
        if tool_button(
            ui,
            ModelIcon::Notch,
            tr("modela.notch"),
            active_tool(ui) == Some(ModelTool::Notch),
        ) {
            activate_tool(ui, ModelTool::Notch);
        }
        if tool_button(
            ui,
            ModelIcon::Fold,
            tr("modela.fold"),
            active_tool(ui) == Some(ModelTool::Fold),
        ) {
            activate_tool(ui, ModelTool::Fold);
        }
        if tool_button(
            ui,
            ModelIcon::Unfold,
            tr("modela.unfold"),
            active_tool(ui) == Some(ModelTool::Unfold),
        ) {
            activate_tool(ui, ModelTool::Unfold);
        }
        ui.separator();
        if tool_button(
            ui,
            ModelIcon::Internal,
            tr("modela.internal_line"),
            active_tool(ui) == Some(ModelTool::Internal),
        ) {
            activate_tool(ui, ModelTool::Internal);
        }
        if tool_button(
            ui,
            ModelIcon::Seam,
            tr("modela.seam_allowance"),
            app.tool == Tool::Contour,
        ) {
            app.set_tool(Tool::Contour);
            open(app, DockerTab::Contour);
        }
        if tool_button(
            ui,
            ModelIcon::Measure,
            tr("modela.measure"),
            app.tool == Tool::SegmentDimension,
        ) {
            app.set_tool(Tool::SegmentDimension);
        }
        if tool_button(ui, ModelIcon::Grade, tr("docker.modela_grading"), false) {
            open(app, DockerTab::ModelaGrading);
        }
        if tool_button(ui, ModelIcon::Nest, tr("docker.modela_nesting"), false) {
            open(app, DockerTab::ModelaNesting);
        }
    });
}

fn no_mold(ui: &mut Ui) {
    ui.label(tr("modela.select_mold"));
    ui.label(
        egui::RichText::new(tr("modela.select_mold_hint"))
            .small()
            .color(Tokens::text_dim()),
    );
}

/// Pattern properties stored as namespaced object data in the `.tdraw`.
pub fn properties_panel(app: &mut App, ui: &mut Ui) {
    let Some(shape) = selected_mold(app) else {
        no_mold(ui);
        return;
    };
    let mut data = shape.data.clone();
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.strong(shape.name.as_deref().unwrap_or("—"));
        if ui.small_button(tr("modela.rename")).clicked() {
            app.dialog = crate::ui::dialogs::Dialog::RenameObject {
                shape: shape.id,
                name: shape.name.clone().unwrap_or_default(),
            };
        }
    });
    ui.separator();

    let current_grain = value(&data, GRAIN).unwrap_or("down");
    let mut grain = match current_grain {
        "0" | "0.0" => "right",
        "90" | "90.0" => "down",
        "180" | "180.0" => "left",
        "270" | "270.0" => "up",
        other => other,
    }
    .to_string();
    egui::ComboBox::from_label(tr("modela.grain_direction"))
        .selected_text(tr(&format!("modela.direction_{grain}")))
        .show_ui(ui, |ui| {
            for direction in ["up", "right", "down", "left"] {
                changed |= ui
                    .selectable_value(
                        &mut grain,
                        direction.to_string(),
                        tr(&format!("modela.direction_{direction}")),
                    )
                    .changed();
            }
        });
    put(&mut data, GRAIN, grain);

    let mut rotation = value(&data, ROTATION).unwrap_or("free").to_string();
    egui::ComboBox::from_label(tr("modela.rotation"))
        .selected_text(tr(&format!("modela.rotation_{rotation}")))
        .show_ui(ui, |ui| {
            for option in ["free", "90", "180", "none"] {
                changed |= ui
                    .selectable_value(
                        &mut rotation,
                        option.to_string(),
                        tr(&format!("modela.rotation_{option}")),
                    )
                    .changed();
            }
        });
    put(&mut data, ROTATION, rotation);

    for (key, label, default) in [
        (SHOW_NAME, "modela.show_name", true),
        (PAIR, "modela.pair", false),
        (MIRROR, "modela.allow_mirror", false),
    ] {
        let mut checked = bool_value(&data, key, default);
        changed |= ui.checkbox(&mut checked, tr(label)).changed();
        put(&mut data, key, checked);
    }

    let mut quantity = number(&data, QUANTITY, 1.0) as i32;
    ui.horizontal(|ui| {
        ui.label(tr("modela.quantity"));
        changed |= ui
            .add(egui::DragValue::new(&mut quantity).range(1..=999))
            .changed();
    });
    put(&mut data, QUANTITY, quantity);

    let mut fabric = value(&data, FABRIC).unwrap_or("").to_string();
    ui.horizontal(|ui| {
        ui.label(tr("modela.fabric"));
        changed |= ui.text_edit_singleline(&mut fabric).changed();
    });
    put(&mut data, FABRIC, fabric);

    let mut seam = app.units.world_from_mm(number(&data, SEAM_MM, 10.0));
    ui.horizontal(|ui| {
        ui.label(tr("modela.seam_allowance"));
        changed |= ui
            .add(
                egui::DragValue::new(&mut seam)
                    .speed(0.1)
                    .suffix(format!(" {}", app.units.short())),
            )
            .changed();
    });
    put(&mut data, SEAM_MM, app.units.world_to_mm(seam).max(0.0));

    let mut notch_length = app
        .units
        .world_from_mm(number(&data, "modela.notch_length_mm", 10.0));
    ui.horizontal(|ui| {
        ui.label(tr("modela.notch_length"));
        changed |= ui
            .add(
                egui::DragValue::new(&mut notch_length)
                    .speed(0.1)
                    .suffix(format!(" {}", app.units.short())),
            )
            .changed();
    });
    put(
        &mut data,
        "modela.notch_length_mm",
        app.units.world_to_mm(notch_length).max(1.0),
    );

    ui.separator();
    ui.strong(tr("modela.fold"));
    ui.horizontal_wrapped(|ui| {
        if ui.small_button(tr("modela.fold_left")).clicked() {
            fold(app, true, true);
        }
        if ui.small_button(tr("modela.fold_right")).clicked() {
            fold(app, true, false);
        }
        if ui.small_button(tr("modela.fold_top")).clicked() {
            fold(app, false, true);
        }
        if ui.small_button(tr("modela.fold_bottom")).clicked() {
            fold(app, false, false);
        }
        if ui.small_button(tr("modela.unfold")).clicked() {
            unfold(app);
        }
    });

    if changed {
        app.run(Command::SetObjectData {
            shape: shape.id,
            data,
        });
    }
}

/// Grading setup attached to a mold. Point-by-point grade rules will build
/// on these persisted size and increment definitions.
pub fn grading_panel(app: &mut App, ui: &mut Ui) {
    let Some(shape) = selected_mold(app) else {
        no_mold(ui);
        return;
    };
    let mut data = shape.data.clone();
    let mut changed = false;
    ui.label(tr("modela.grading_setup_hint"));

    let mut show = SHOW_GRADING.load(std::sync::atomic::Ordering::Relaxed);
    if ui.checkbox(&mut show, tr("modela.show_grading")).changed() {
        SHOW_GRADING.store(show, std::sync::atomic::Ordering::Relaxed);
        let ids: Vec<_> = app
            .object_clones()
            .into_iter()
            .filter_map(|(clone, master)| (master == shape.id).then_some(clone))
            .collect();
        if !ids.is_empty() {
            app.run(Command::SetVisible {
                shapes: ids,
                visible: show,
            });
        }
    }

    let mut base = value(&data, GRADE_BASE).unwrap_or("M").to_string();
    ui.horizontal(|ui| {
        ui.label(tr("modela.base_size"));
        changed |= ui.text_edit_singleline(&mut base).changed();
    });
    put(&mut data, GRADE_BASE, base);

    let mut sizes = value(&data, GRADE_SIZES)
        .unwrap_or("PP, P, M, G, GG")
        .to_string();
    ui.label(tr("modela.sizes"));
    changed |= ui.text_edit_singleline(&mut sizes).changed();
    put(&mut data, GRADE_SIZES, sizes);

    let mut dx = app.units.world_from_mm(number(&data, GRADE_X_MM, 10.0));
    let mut dy = app.units.world_from_mm(number(&data, GRADE_Y_MM, 10.0));
    for (label, value_edit) in [
        ("modela.increment_x", &mut dx),
        ("modela.increment_y", &mut dy),
    ] {
        ui.horizontal(|ui| {
            ui.label(tr(label));
            changed |= ui
                .add(
                    egui::DragValue::new(value_edit)
                        .speed(0.1)
                        .suffix(format!(" {}", app.units.short())),
                )
                .changed();
        });
    }
    put(&mut data, GRADE_X_MM, app.units.world_to_mm(dx));
    put(&mut data, GRADE_Y_MM, app.units.world_to_mm(dy));

    ui.add_space(8.0);
    if ui.button(tr("modela.generate_grading")).clicked() {
        generate_grading(app);
    }

    let graded: Vec<_> = app
        .object_clones()
        .into_iter()
        .filter_map(|(clone, master)| {
            (master == shape.id)
                .then(|| app.doc().find_shape(clone).cloned())
                .flatten()
        })
        .collect();
    if !graded.is_empty() {
        ui.separator();
        ui.strong(tr("modela.graded_sizes"));
        for graded_shape in graded {
            let mut color = value(&graded_shape.data, "modela.grade.color")
                .and_then(parse_hex_color)
                .unwrap_or(Color32::LIGHT_BLUE);
            ui.horizontal(|ui| {
                if ui.color_edit_button_srgba(&mut color).changed() {
                    let mut grade_data = graded_shape.data.clone();
                    put(&mut grade_data, "modela.grade.color", hex_color(color));
                    app.run(Command::SetObjectData {
                        shape: graded_shape.id,
                        data: grade_data,
                    });
                }
                ui.label(graded_shape.name.as_deref().unwrap_or("—"));
            });
        }
    }

    if changed {
        app.run(Command::SetObjectData {
            shape: shape.id,
            data,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_put_replaces_without_duplicates() {
        let mut data = vec![(TYPE.into(), "drawing".into())];
        put(&mut data, TYPE, MOLD);
        assert_eq!(value(&data, TYPE), Some(MOLD));
        assert_eq!(data.len(), 1);
    }
}
