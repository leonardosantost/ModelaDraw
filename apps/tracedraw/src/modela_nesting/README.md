# Modela integration boundary

The Modela feature lives in this directory. Updating TraceDraw should normally
require resolving only these small registration points:

1. `main.rs`: `mod modela_nesting;`
2. `app.rs`: the `DockerTab::ModelaNesting` variant, list, menu group and key
3. `ui/dockers2.rs`: dispatch the tab to `modela_nesting::panel`
4. `help.rs`: the help topic fallback
5. `lang/*.toml`: the `docker.modela_nesting` and `modela_nesting.*` strings

The UI, validation, placement algorithm, page resizing, transforms and tests
are all in `mod.rs`. A contour-based engine can replace `pack` while preserving
the panel and the four integration points above.
