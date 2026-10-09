//! Shared `egui` UI chrome mechanics for the host application and
//! the host application. See `docs/decisions/0001-generic-over-action-type.md`
//! for the founding design principle: every module here is generic over an
//! app-supplied action type and context type, and this crate never depends
//! on either app's domain crates.

pub mod command_status_panel;
pub mod dock;
pub mod fonts;
pub mod idle_input;
pub mod menu;
pub mod motion;
pub mod os_style;
pub mod ribbon;
pub mod settings;
pub mod theme;
pub mod tokens;
pub mod window_chrome;
