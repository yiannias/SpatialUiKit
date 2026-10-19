//! Shared `egui` UI chrome mechanics for the host application and
//! the host application. See `docs/decisions/0001-generic-over-action-type.md`
//! for the founding design principle: every module here is generic over an
//! app-supplied action type and context type, and this crate never depends
//! on either app's domain crates.

pub mod command_status_panel;
pub mod dock;
pub mod dropdown;
pub mod font_picker;
pub mod fonts;
pub mod idle_input;
pub mod menu;
pub mod mini_ribbon;
pub mod modal;
pub mod motion;
pub mod os_style;
pub mod ribbon;
pub mod runtime_fonts;
pub mod settings;
pub mod sketch;
pub mod style_list;
pub mod theme;
pub mod tokens;
pub mod window_chrome;
