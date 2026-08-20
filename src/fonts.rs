//! Optional UI typeface (Tier-1 "Application-Wide" setting, alongside
//! `crate::theme::Theme`/`TextScale`) -- Chris, 2026-07-22: wants a nicer
//! proportional typeface than egui's bundled default. Exactly matching a
//! specific reference font (e.g. this chat's own UI) isn't possible here --
//! different rendering stack, and that font is very likely not freely
//! redistributable -- so instead this offers **Inter** (SIL Open Font
//! License, `assets/Inter.ttf` + `assets/Inter-OFL.txt`), a widely-used,
//! freely bundleable font visually close to common system UI fonts.
//!
//! `Inter.ttf` is Google Fonts' variable instance (`opsz,wght` axes) rather
//! than a static weight -- egui 0.35's `skrifa`-based text backend renders
//! variable fonts at their default instance correctly, so no static-weight
//! export was needed.
//!
//! Only replaces the `Proportional` family; `Monospace` (command lines,
//! dynamic input, etc.) is untouched.

use serde::{Deserialize, Serialize};

const INTER_BYTES: &[u8] = include_bytes!("../assets/Inter.ttf");
const INTER_FONT_KEY: &str = "Inter";

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug, Default)]
pub enum Typeface {
    #[default]
    Default,
    Inter,
}

impl Typeface {
    pub fn label(&self) -> &'static str {
        match self {
            Typeface::Default => "Default",
            Typeface::Inter => "Inter",
        }
    }

    pub const ALL: [Typeface; 2] = [Typeface::Default, Typeface::Inter];
}

/// Applies `typeface` to `ctx`'s `Proportional` font family. Rebuilds
/// `FontDefinitions` from egui's own default each call (cheap, only called
/// once at boot and again on an explicit setting change) -- registers the
/// Inter font data unconditionally and, when selected, prepends it to the
/// `Proportional` fallback chain ahead of egui's bundled font (keeping the
/// existing entries after it so glyphs Inter lacks, e.g. emoji, still
/// resolve).
pub fn apply_typeface(ctx: &egui::Context, typeface: Typeface) {
    let mut defs = egui::FontDefinitions::default();
    defs.font_data.insert(
        INTER_FONT_KEY.to_owned(),
        egui::FontData::from_static(INTER_BYTES).into(),
    );

    if typeface == Typeface::Inter {
        if let Some(family) = defs.families.get_mut(&egui::FontFamily::Proportional) {
            family.insert(0, INTER_FONT_KEY.to_owned());
        }
    }

    ctx.set_fonts(defs);
}
