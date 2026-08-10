# SpatialUiKit

Shared `egui` UI chrome mechanics for [SpatialSketchPad](https://github.com/yiannias/SpatialSketchPad)
(SSP) and its sibling app SpatialDrawingBoard (SDB). Consumed as a path/git dependency by
`ssp-ui` and `sdb_ui`.

## Scope

This crate holds only generic UI *mechanics* — declarative menu trees, panel docking/tabbing/
floating shells, the Passive Panel's ambient-chrome primitives, the Ribbon's rendering shell,
and (eventually) a schema-driven Preferences renderer. It never holds panel *content* (Layers,
Properties, Planes, entity data, etc.) and never depends on either app's domain crates
(`ssp-core`, `ssp-2d`, `sdb_core`, ...).

Every module is generic over an app-supplied action type and context type — see
[`docs/decisions/0001-generic-over-action-type.md`](docs/decisions/0001-generic-over-action-type.md)
for the founding design principle.

## Status

Under active extraction from SSP, one subsystem at a time. See each consuming app's own docs
(SSP: `docs/decisions/0010-shared-ui-kit-crate.md`; SDB: `docs/decisions/00XX-shared-ui-kit-crate.md`)
for the roadmap and what's landed so far.

### Coordination briefs

The two apps drift when one has UI attention and the other doesn't. When that happens, the app
that moved writes the other a dated brief here — what needs a decision, what must be pulled, and
which bugs it found that the sibling probably shares.

- [`docs/2026-08-10-sdb-ui-changes-for-ssp.md`](docs/2026-08-10-sdb-ui-changes-for-ssp.md) —
  SDB → SSP, covering ~2026-07-20 to 2026-08-10. Open items for SSP: the "Passive Panel" rename,
  window draw-order scoping, and whether `DockHost` should own a `ScrollArea`.

## Modules

- `menu` — declarative `MenuNode<A, Ctx>` tree + generic egui menu-bar renderer.
