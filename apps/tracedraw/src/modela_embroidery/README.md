# Modela embroidery integration

The embroidery UI and object data live in this directory; the stitch
generators, the plan and the machine formats are the `tracedraw-emb-*`
crates in `crates/embroidery/`. Integration points outside this directory:

1. `main.rs`: `mod modela_embroidery;`
2. `app.rs`: the `DockerTab::ModelaEmbroidery`, `ModelaSequence` and
   `ModelaHoop` variants, list, menu group and keys, and the Embroidery
   workspace's open panels
3. `ui/dockers2.rs`: dispatch the tabs to `panel`, `sequence_panel` and
   `hoop_panel`
4. `ui/bars.rs`: `Bar::Embroidery` (and `Bar::Modeling`), a movable toolbar
   shown in its workspace; `ui/menus.rs` lists it in Window > Toolbars
5. `canvas.rs`: `modela_embroidery::draw_stitches` after the page is drawn
6. `help.rs`: the help topic fallback
7. `lang/*.toml`: `docker.modela_embroidery` and `modela_embroidery.*`
8. `Cargo.toml` (workspace and app): the three crates
9. `ui/menus.rs`: File > Open Machine File and Export Machine File (and
   their rows in the menu test's `SKIP` list, as they open file choosers)
   and, in the row pictures, `icons::for_menu` for those rows' icons
10. `ui/dockers.rs`: `tab_icon` for the three tabs, from `icons::draw_tab`

`icons.rs` draws the icons from SVG path data on a 24 x 24 grid with the
painter; `LICENSE-lucide.txt` is the notice the Lucide icons need.

Stitch settings are stored in `Shape::data` under
`modela.embroidery.stitch`, so `.tdraw` files need no schema change.
