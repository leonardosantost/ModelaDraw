//! Modela nesting panel and placement engine.
//!
//! This module is deliberately self-contained. The TraceDraw integration is
//! limited to registering one docker tab and dispatching its UI. Replacing the
//! rectangular placement engine with Modela's polygon engine should not require
//! changes elsewhere in the application.

use crate::app::App;
use crate::i18n::{tr, trf};
use egui::Ui;
use tracedraw_core::{
    geometry::{Affine, Point, Rect, Size},
    Command, PageId, ShapeId,
};

const STATE_ID: &str = "modela_nesting_state";
const MIN_SIZE_MM: f64 = 0.001;

#[derive(Debug, Clone)]
struct State {
    document: u64,
    page: PageId,
    page_width_mm: f64,
    width_mm: f64,
    spacing_mm: f64,
    allow_rotation: bool,
    feedback: Option<Feedback>,
}

impl State {
    fn for_page(document: u64, page: PageId, width_mm: f64) -> Self {
        Self {
            document,
            page,
            page_width_mm: width_mm,
            width_mm,
            spacing_mm: 5.0,
            allow_rotation: true,
            feedback: None,
        }
    }
}

#[derive(Debug, Clone)]
enum Feedback {
    Success { count: usize, length_mm: f64 },
    Error(NestError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NestError {
    NoSelection,
    OnlyPageObjects,
    InvalidWidth,
    InvalidObject,
    TooWide,
}

#[derive(Debug, Clone, Copy)]
struct Item {
    id: ShapeId,
    bounds: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PlacedRect {
    item: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    rotated: bool,
}

/// Window > Panels > Modela Nesting.
pub fn panel(app: &mut App, ui: &mut Ui) {
    let id = egui::Id::new(STATE_ID);
    let document = app.engine.instance();
    let page = app.page;
    let page_width = app.page_size().width;
    let mut state = ui
        .ctx()
        .data_mut(|data| data.get_temp::<State>(id))
        .unwrap_or_else(|| State::for_page(document, page, page_width));
    if state.document != document || state.page != page {
        state = State::for_page(document, page, page_width);
    } else if (state.page_width_mm - page_width).abs() > 1e-9 {
        state.page_width_mm = page_width;
        state.width_mm = page_width;
    }

    ui.label(tr("modela_nesting.intro"));
    ui.add_space(6.0);
    egui::Grid::new("modela_nesting_options")
        .num_columns(2)
        .spacing(egui::vec2(8.0, 6.0))
        .show(ui, |ui| {
            let units = app.units;
            let suffix = format!(" {}", units.short());
            let mut width = units.from_mm(state.width_mm);
            ui.label(tr("modela_nesting.width"));
            if ui
                .add(
                    egui::DragValue::new(&mut width)
                        .range(units.from_mm(0.1)..=units.from_mm(100_000.0))
                        .speed(units.from_mm(5.0).max(0.01))
                        .suffix(&suffix),
                )
                .changed()
            {
                state.width_mm = units.to_mm(width);
            }
            ui.end_row();

            let mut spacing = units.from_mm(state.spacing_mm);
            ui.label(tr("modela_nesting.spacing"));
            if ui
                .add(
                    egui::DragValue::new(&mut spacing)
                        .range(0.0..=units.from_mm(10_000.0))
                        .speed(units.from_mm(1.0).max(0.01))
                        .suffix(&suffix),
                )
                .changed()
            {
                state.spacing_mm = units.to_mm(spacing);
            }
            ui.end_row();
        });

    ui.checkbox(
        &mut state.allow_rotation,
        tr("modela_nesting.allow_rotation"),
    )
    .on_hover_text(tr("modela_nesting.rotation_tip"));
    ui.add_space(8.0);

    let selected_count = app.selection.len();
    let nested = app.selection_nested();
    ui.label(trf(
        "modela_nesting.selected_groups",
        &[("count", &selected_count.to_string())],
    ));

    let enabled = selected_count > 0 && !nested;
    if ui
        .add_enabled(enabled, egui::Button::new(tr("modela_nesting.run")))
        .clicked()
    {
        state.feedback = Some(
            match apply(app, state.width_mm, state.spacing_mm, state.allow_rotation) {
                Ok((count, length_mm)) => {
                    state.page_width_mm = state.width_mm;
                    Feedback::Success { count, length_mm }
                }
                Err(error) => Feedback::Error(error),
            },
        );
    }

    if selected_count == 0 {
        ui.weak(tr("modela_nesting.select_groups"));
    } else if nested {
        ui.colored_label(ui.visuals().warn_fg_color, tr("modela_nesting.only_groups"));
    }
    if let Some(feedback) = &state.feedback {
        ui.add_space(8.0);
        match feedback {
            Feedback::Success { count, length_mm } => {
                let count = count.to_string();
                let length = format!("{:.2}", app.units.from_mm(*length_mm));
                ui.label(trf(
                    "modela_nesting.done",
                    &[
                        ("count", &count),
                        ("length", &length),
                        ("unit", app.units.short()),
                    ],
                ));
            }
            Feedback::Error(error) => {
                ui.colored_label(ui.visuals().warn_fg_color, tr(error.key()));
            }
        }
    }
    ui.ctx().data_mut(|data| data.insert_temp(id, state));
}

impl NestError {
    fn key(self) -> &'static str {
        match self {
            NestError::NoSelection => "modela_nesting.select_groups",
            NestError::OnlyPageObjects => "modela_nesting.only_groups",
            NestError::InvalidWidth => "modela_nesting.invalid_width",
            NestError::InvalidObject => "modela_nesting.invalid_group",
            NestError::TooWide => "modela_nesting.too_wide",
        }
    }
}

/// Arrange the selected page objects and resize the page. The whole operation is a
/// single undo step.
fn apply(
    app: &mut App,
    width_mm: f64,
    spacing_mm: f64,
    allow_rotation: bool,
) -> Result<(usize, f64), NestError> {
    if !width_mm.is_finite() || width_mm < MIN_SIZE_MM {
        return Err(NestError::InvalidWidth);
    }
    let spacing_mm = if spacing_mm.is_finite() {
        spacing_mm.max(0.0)
    } else {
        0.0
    };
    let shapes = app.selected_shapes();
    if shapes.is_empty() {
        return Err(NestError::NoSelection);
    }
    if app.selection_nested() {
        return Err(NestError::OnlyPageObjects);
    }

    let mut items = Vec::with_capacity(shapes.len());
    for shape in &shapes {
        let bounds = app.visual_bounds(shape);
        if !valid_bounds(bounds) {
            return Err(NestError::InvalidObject);
        }
        items.push(Item {
            id: shape.id,
            bounds,
        });
    }
    let placements = pack(&items, width_mm, spacing_mm, allow_rotation)?;
    let used_height = placements
        .iter()
        .map(|placed| placed.y + placed.height)
        .fold(0.0, f64::max)
        .max(MIN_SIZE_MM);

    let mut commands = Vec::with_capacity(placements.len() + 1);
    commands.push(Command::ResizePage {
        page: app.page,
        size: Size::new(width_mm, used_height),
    });
    for placed in &placements {
        let item = items.get(placed.item).ok_or(NestError::InvalidObject)?;
        let rotation = if placed.rotated {
            Affine::rotate_about(std::f64::consts::FRAC_PI_2, item.bounds.center())
        } else {
            Affine::IDENTITY
        };
        let rotated_bounds = transformed_bounds(item.bounds, rotation);
        let shift = Point::new(placed.x, placed.y) - rotated_bounds.origin();
        commands.push(Command::TransformShapes {
            shapes: vec![item.id],
            transform: Affine::translate(shift) * rotation,
        });
    }

    app.run_batch_keeping_lines("Modela Nesting", &commands)
        .map_err(|_| NestError::InvalidObject)?;
    app.fit_pending = true;
    app.status = trf(
        "modela_nesting.done",
        &[
            ("count", &placements.len().to_string()),
            ("length", &format!("{:.2}", app.units.from_mm(used_height))),
            ("unit", app.units.short()),
        ],
    );
    Ok((placements.len(), used_height))
}

fn valid_bounds(bounds: Rect) -> bool {
    bounds.x0.is_finite()
        && bounds.y0.is_finite()
        && bounds.x1.is_finite()
        && bounds.y1.is_finite()
        && bounds.width() >= MIN_SIZE_MM
        && bounds.height() >= MIN_SIZE_MM
}

fn transformed_bounds(bounds: Rect, transform: Affine) -> Rect {
    let corners = [
        Point::new(bounds.x0, bounds.y0),
        Point::new(bounds.x1, bounds.y0),
        Point::new(bounds.x1, bounds.y1),
        Point::new(bounds.x0, bounds.y1),
    ];
    let first = transform * corners[0];
    corners[1..]
        .iter()
        .map(|point| transform * *point)
        .fold(Rect::from_points(first, first), |rect, point| {
            rect.union_pt(point)
        })
}

/// Bottom-left rectangle packing with unbounded roll length. Candidates are
/// the right and top edges of every placed item; larger items are placed first.
fn pack(
    items: &[Item],
    width: f64,
    spacing: f64,
    allow_rotation: bool,
) -> Result<Vec<PlacedRect>, NestError> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| {
        let a_bounds = items[a].bounds;
        let b_bounds = items[b].bounds;
        let a_key = (a_bounds.width().max(a_bounds.height()), a_bounds.area());
        let b_key = (b_bounds.width().max(b_bounds.height()), b_bounds.area());
        b_key
            .0
            .total_cmp(&a_key.0)
            .then(b_key.1.total_cmp(&a_key.1))
    });

    let mut placed: Vec<PlacedRect> = Vec::with_capacity(items.len());
    for item in order {
        let bounds = items[item].bounds;
        let mut orientations = vec![(bounds.width(), bounds.height(), false)];
        if allow_rotation && (bounds.width() - bounds.height()).abs() > 1e-9 {
            orientations.push((bounds.height(), bounds.width(), true));
        }
        if !orientations.iter().any(|(w, _, _)| *w <= width + 1e-9) {
            return Err(NestError::TooWide);
        }

        let mut xs = vec![0.0];
        let mut ys = vec![0.0];
        for other in &placed {
            xs.push(other.x + other.width + spacing);
            ys.push(other.y + other.height + spacing);
        }
        xs.sort_by(f64::total_cmp);
        ys.sort_by(f64::total_cmp);
        xs.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        ys.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

        let current_height = placed
            .iter()
            .map(|other| other.y + other.height)
            .fold(0.0, f64::max);
        let mut best: Option<((f64, f64, f64, bool), PlacedRect)> = None;
        for (item_width, item_height, rotated) in orientations {
            for &y in &ys {
                for &x in &xs {
                    let candidate = PlacedRect {
                        item,
                        x,
                        y,
                        width: item_width,
                        height: item_height,
                        rotated,
                    };
                    if x + item_width > width + 1e-9
                        || placed
                            .iter()
                            .any(|other| overlaps(candidate, *other, spacing))
                    {
                        continue;
                    }
                    let score = (current_height.max(y + item_height), y, x, rotated);
                    if best.as_ref().is_none_or(|(old, _)| score < *old) {
                        best = Some((score, candidate));
                    }
                }
            }
        }
        let Some((_, candidate)) = best else {
            return Err(NestError::TooWide);
        };
        placed.push(candidate);
    }
    placed.sort_by_key(|placement| placement.item);
    Ok(placed)
}

fn overlaps(a: PlacedRect, b: PlacedRect, spacing: f64) -> bool {
    a.x < b.x + b.width + spacing - 1e-9
        && a.x + a.width + spacing > b.x + 1e-9
        && a.y < b.y + b.height + spacing - 1e-9
        && a.y + a.height + spacing > b.y + 1e-9
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracedraw_core::{Shape, ShapeKind};

    fn item(id: u64, width: f64, height: f64) -> Item {
        Item {
            id: ShapeId(id),
            bounds: Rect::new(0.0, 0.0, width, height),
        }
    }

    #[test]
    fn packing_keeps_the_requested_spacing() {
        let items = [item(1, 60.0, 30.0), item(2, 40.0, 30.0)];
        let out = pack(&items, 105.0, 5.0, false).expect("packing");
        assert_eq!(out.len(), 2);
        assert!(!overlaps(out[0], out[1], 5.0));
        assert!(out.iter().all(|p| p.x + p.width <= 105.0));
    }

    #[test]
    fn rotation_can_make_a_group_fit_the_width() {
        let items = [item(1, 120.0, 50.0)];
        assert_eq!(pack(&items, 60.0, 0.0, false), Err(NestError::TooWide));
        let out = pack(&items, 60.0, 0.0, true).expect("rotated packing");
        assert!(out[0].rotated);
        assert_eq!((out[0].width, out[0].height), (50.0, 120.0));
    }

    #[test]
    fn applying_nesting_moves_groups_and_free_objects_in_one_step() {
        let mut app = App::headless();
        let child = |id, rect| {
            Shape::new(
                ShapeId(id),
                ShapeKind::Rect {
                    rect,
                    radius: 0.0,
                    corners: None,
                },
            )
        };
        let a = app
            .new_shape(ShapeKind::Group {
                children: vec![child(10_001, Rect::new(20.0, 30.0, 80.0, 60.0))],
            })
            .expect("first group");
        let b = app
            .new_shape(ShapeKind::Rect {
                rect: Rect::new(100.0, 100.0, 140.0, 130.0),
                radius: 0.0,
                corners: None,
            })
            .expect("free object");
        app.select(vec![a, b]);
        let history_before = app.engine.history_labels().0.len();
        let (count, used_height) = apply(&mut app, 105.0, 5.0, false).expect("nesting");

        assert_eq!(count, 2);
        assert!((app.page_size().width - 105.0).abs() < 1e-9);
        assert!((app.page_size().height - used_height).abs() < 1e-9);
        assert_eq!(app.engine.history_labels().0.len(), history_before + 1);
        let bounds: Vec<Rect> = app
            .selected_shapes()
            .iter()
            .map(Shape::visual_bounds)
            .collect();
        assert!(bounds
            .iter()
            .all(|rect| rect.x0 >= -1e-9 && rect.x1 <= 105.0 + 1e-9));
        assert!(bounds[0].intersect(bounds[1]).area() <= 0.0);
    }
}
