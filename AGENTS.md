# SpatialUiKit project agreements

## Backlog.md

Open work is tracked on the Backlog.md board in `backlog/`. Start with `backlog task list --plain`, and read a task in full with `backlog task <id> --plain` before starting it. Settled decisions: `backlog decision list --plain`. Create and update tasks with the `backlog` CLI, not by hand-editing task files. Use `backlog browser` for the web view. Auto-commit is on. This kit's board is its own; SpatialDrawingBoard's board tracks the app-side work that uses it.

## Kit rules

- MIT licensed (LICENSE). Never copy GPL or unlicensed code in; record provenance of any borrowed code (SDB ADR 0016).
- Never depends on an app's crates (no `sdb_*`, no product crates). Consumers today: SpatialDrawingBoard and Layer Herder.
- Changes are additive and backward-compatible, and pushed the same sitting they are made: a local working copy hides a break from the consuming apps. A kit change and its matching SDB change land together.
- Only one agent edits the kit at a time when several are active (SDB AGENTS.md).
- Public items carry `///` docs that say why. `cargo fmt -- --check` and `cargo test` are clean before pushing.
