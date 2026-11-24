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

/// Every non-ASCII character the kit writes as UI text must be in the bundled
/// fonts or it shows as an empty box (the search arrows did, TASK-244): this
/// scans the kit source for string-literal characters, in both typefaces.
/// Paint the shape with geometry instead, or use a glyph the fonts have.
#[cfg(test)]
mod glyph_coverage {
    use super::*;
    use std::path::Path;

    fn characters() -> std::collections::BTreeMap<char, String> {
        let mut found = std::collections::BTreeMap::new();
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            for (number, line) in text.lines().enumerate() {
                if line.trim_start().starts_with("#[cfg(test)]") {
                    break;
                }
                if line.trim_start().starts_with("//") {
                    continue;
                }
                let (mut in_string, mut escaped) = (false, false);
                for c in line.chars() {
                    if in_string {
                        if escaped {
                            escaped = false;
                        } else if c == '\\' {
                            escaped = true;
                        } else if c == '"' {
                            in_string = false;
                        } else if !c.is_ascii() {
                            found.entry(c).or_insert_with(|| {
                                format!(
                                    "{}:{}",
                                    path.file_name().unwrap().to_string_lossy(),
                                    number + 1
                                )
                            });
                        }
                    } else if c == '"' {
                        in_string = true;
                    }
                }
            }
        }
        found
    }

    #[test]
    fn every_character_the_kit_writes_is_in_the_fonts() {
        for typeface in Typeface::ALL {
            let ctx = egui::Context::default();
            apply_typeface(&ctx, typeface);
            let _ = ctx.run_ui(egui::RawInput::default(), |_| {});
            let font = egui::FontId::proportional(13.0);
            let missing: Vec<String> = characters()
                .into_iter()
                .filter(|(c, _)| !ctx.fonts_mut(|f| f.has_glyph(&font, *c)))
                .map(|(c, at)| format!("U+{:04X} first at {at}", c as u32))
                .collect();
            assert!(
                missing.is_empty(),
                "{} font cannot draw (empty box on screen): {}",
                typeface.label(),
                missing.join("; ")
            );
        }
    }
}
