//! Modela additions to TraceDraw's built-in workspaces.
//!
//! The selector is kept here so updating TraceDraw normally requires only
//! one call from the menu bar. The workspace definitions themselves remain
//! in `app.rs`, beside TraceDraw's other built-in workspaces.

use crate::app::{App, Workspace};
use crate::i18n::tr;
use egui::Ui;

/// Workspace selector shown at the right edge of the main menu bar.
pub fn selector(app: &mut App, ui: &mut Ui) {
    let current = workspace_name(app.workspace);
    let mut selected = app.workspace;

    ui.add_space(8.0);
    egui::ComboBox::from_id_salt("modela_workspace_selector")
        .selected_text(current)
        .width(150.0)
        .show_ui(ui, |ui| {
            for workspace in Workspace::ALL {
                ui.selectable_value(&mut selected, workspace, workspace_name(workspace));
            }
        })
        .response
        .on_hover_text(tr("menu.window.workspace"));

    if selected != app.workspace {
        app.set_workspace(selected);
    }
}

pub fn workspace_name(workspace: Workspace) -> String {
    tr(&format!("menu.window.workspace_{}", workspace.id()))
}
