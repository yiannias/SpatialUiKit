//! Shared `egui` UI chrome mechanics for SpatialSketchPad and
//! SpatialDrawingBoard. See `docs/decisions/0001-generic-over-action-type.md`
//! for the founding design principle: every module here is generic over an
//! app-supplied action type and context type, and this crate never depends
//! on either app's domain crates.

pub mod dock;
pub mod fonts;
pub mod idle_input;
pub mod menu;
pub mod passive_panel;
pub mod ribbon;
pub mod settings;
pub mod theme;
