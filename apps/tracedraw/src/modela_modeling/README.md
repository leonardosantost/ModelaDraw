# Modela modeling integration

Modela-specific pattern tools, metadata and panels live in this directory so
upstream TraceDraw updates touch as little custom code as possible.

Integration points outside this directory:

1. `main.rs` registers this module.
2. `ui/mod.rs` adds the modeling shortcut bar and second toolbox column.
3. `app.rs` registers the two Modela docker tabs and workspace layout.
4. `ui/dockers2.rs` dispatches those tabs here.
5. `help.rs` gives the custom tabs a fallback help topic.
6. Language files provide their visible labels.

Pattern data is stored in TraceDraw's existing `Shape::data` field with
`modela.*` keys, so `.tdraw` compatibility does not depend on a forked core
document schema.
