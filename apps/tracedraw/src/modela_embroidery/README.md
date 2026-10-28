# Modela embroidery integration

The embroidery UI and object data live in this directory; the stitch
generators, the plan and the machine formats are the `tracedraw-emb-*`
crates in `crates/embroidery/`. Integration points outside this directory:

1. `main.rs`: `mod modela_embroidery;`
2. `app.rs`: the `DockerTab::ModelaEmbroidery` variant, list, menu group
   and key, and the Embroidery workspace's open panels
3. `ui/dockers2.rs`: dispatch the tab to `modela_embroidery::panel`
4. `ui/mod.rs`: the embroidery bar in the Embroidery workspace
5. `canvas.rs`: `modela_embroidery::draw_stitches` after the page is drawn
6. `help.rs`: the help topic fallback
7. `lang/*.toml`: `docker.modela_embroidery` and `modela_embroidery.*`
8. `Cargo.toml` (workspace and app): the three crates

Stitch settings are stored in `Shape::data` under
`modela.embroidery.stitch`, so `.tdraw` files need no schema change.
