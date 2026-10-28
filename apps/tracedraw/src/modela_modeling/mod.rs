//! Modela pattern-making integration.
//!
//! This module deliberately owns the Modela-specific UI and object metadata.
//! TraceDraw only needs a few small integration points, which keeps upstream
//! updates and rebases manageable.

use crate::app::{App, DockerTab};
use crate::i18n::tr;
use crate::theme::Tokens;
use crate::tools::Tool;
use egui::Ui;
use tracedraw_core::{Command, Shape};

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
        put(&mut data, GRAIN, 90);
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
    let commands = [
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

fn tool_button(ui: &mut Ui, text: &str, tip: String, active: bool) -> bool {
    let button = egui::Button::new(text).selected(active);
    ui.add_sized([34.0, 34.0], button)
        .on_hover_text(tip)
        .clicked()
}

/// Second, Modela-owned toolbox column beside TraceDraw's native toolbox.
pub fn toolbox(app: &mut App, ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        if tool_button(ui, "M+", tr("modela.create_mold"), false) {
            create_mold(app);
        }
        if tool_button(ui, "MP", tr("docker.modela_properties"), false) {
            open(app, DockerTab::ModelaProperties);
        }
        ui.separator();
        if tool_button(
            ui,
            "LI",
            tr("modela.internal_line"),
            app.tool == Tool::TwoPointLine,
        ) {
            app.set_tool(Tool::TwoPointLine);
        }
        if tool_button(
            ui,
            "MA",
            tr("modela.seam_allowance"),
            app.tool == Tool::Contour,
        ) {
            app.set_tool(Tool::Contour);
            open(app, DockerTab::Contour);
        }
        if tool_button(
            ui,
            "ME",
            tr("modela.measure"),
            app.tool == Tool::SegmentDimension,
        ) {
            app.set_tool(Tool::SegmentDimension);
        }
        if tool_button(ui, "GR", tr("docker.modela_grading"), false) {
            open(app, DockerTab::ModelaGrading);
        }
        if tool_button(ui, "EN", tr("docker.modela_nesting"), false) {
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

    let mut grain = number(&data, GRAIN, 90.0);
    ui.horizontal(|ui| {
        ui.label(tr("modela.grain_angle"));
        changed |= ui
            .add(egui::DragValue::new(&mut grain).suffix("°"))
            .changed();
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
