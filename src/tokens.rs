//! Minimal W3C DTCG-shaped (`$type`/`$value`) leaf token types shared by
//! [`crate::theme`]'s `ThemePalette` and, per
//! `docs/design/2026-08-30_theme-system-spec.md` (the host app) "Future direction",
//! any later token domain (text styles, spacing) that wants the same JSON
//! shape. Deliberately just the two leaf kinds the host app's window-chrome tokens
//! need today -- not a general DTCG type system (no `$description`,
//! `$extensions`, composite/alias tokens, dimension units besides `px`).

use serde::{Deserialize, Serialize};

/// Error parsing a token's `$value` into a usable Rust type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenValueError {
    /// `$value` wasn't `#RRGGBB` or `#RRGGBBAA` hex.
    InvalidColor(String),
    /// `$value` wasn't a bare number followed by `px`.
    InvalidDimension(String),
}

impl std::fmt::Display for TokenValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenValueError::InvalidColor(v) => {
                write!(
                    f,
                    "invalid color token value {v:?}, expected #RRGGBB or #RRGGBBAA"
                )
            }
            TokenValueError::InvalidDimension(v) => {
                write!(
                    f,
                    "invalid dimension token value {v:?}, expected a number followed by \"px\""
                )
            }
        }
    }
}

impl std::error::Error for TokenValueError {}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
enum ColorTokenType {
    Color,
}

/// A DTCG `{"$type": "color", "$value": "#RRGGBBAA"}` leaf token.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct ColorToken {
    #[serde(rename = "$type")]
    kind: ColorTokenType,
    #[serde(rename = "$value")]
    value: String,
}

impl ColorToken {
    /// Builds a token from an already-resolved `egui::Color32`, formatting
    /// `$value` as `#RRGGBBAA` (alpha always included, even at `FF`, so the
    /// shape is uniform regardless of whether a given color is opaque).
    pub fn new(color: egui::Color32) -> Self {
        // `egui::Color32` stores premultiplied alpha internally; `to_array()`
        // would round-trip a lossy premultiplied hex (e.g. white at 15%
        // alpha comes back as `#26262626`, not `#FFFFFF26`). Recover the
        // straight components the caller actually specified.
        let [r, g, b, a] = color.to_srgba_unmultiplied();
        Self {
            kind: ColorTokenType::Color,
            value: format!("#{r:02X}{g:02X}{b:02X}{a:02X}"),
        }
    }

    /// Parses `$value` as `#RRGGBB` (opaque) or `#RRGGBBAA`.
    pub fn color32(&self) -> Result<egui::Color32, TokenValueError> {
        let hex = self
            .value
            .strip_prefix('#')
            .ok_or_else(|| TokenValueError::InvalidColor(self.value.clone()))?;
        let bytes = match hex.len() {
            6 => {
                let rgb = u32::from_str_radix(hex, 16)
                    .map_err(|_| TokenValueError::InvalidColor(self.value.clone()))?;
                [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 0xFF]
            }
            8 => {
                let rgba = u32::from_str_radix(hex, 16)
                    .map_err(|_| TokenValueError::InvalidColor(self.value.clone()))?;
                [
                    (rgba >> 24) as u8,
                    (rgba >> 16) as u8,
                    (rgba >> 8) as u8,
                    rgba as u8,
                ]
            }
            _ => return Err(TokenValueError::InvalidColor(self.value.clone())),
        };
        Ok(egui::Color32::from_rgba_unmultiplied(
            bytes[0], bytes[1], bytes[2], bytes[3],
        ))
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
enum DimensionTokenType {
    Dimension,
}

/// A DTCG `{"$type": "dimension", "$value": "12px"}` leaf token. the host app only
/// ever needs pixel dimensions (corner radii, blur radii, heights) so `px`
/// is the sole supported unit -- not a general CSS-dimension parser.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct DimensionToken {
    #[serde(rename = "$type")]
    kind: DimensionTokenType,
    #[serde(rename = "$value")]
    value: String,
}

impl DimensionToken {
    pub fn new(px: f32) -> Self {
        Self {
            kind: DimensionTokenType::Dimension,
            value: format!("{px}px"),
        }
    }

    pub fn px(&self) -> Result<f32, TokenValueError> {
        self.value
            .strip_suffix("px")
            .and_then(|n| n.trim().parse::<f32>().ok())
            .ok_or_else(|| TokenValueError::InvalidDimension(self.value.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_round_trips_through_hex() {
        let c = egui::Color32::from_rgba_unmultiplied(0x22, 0x22, 0x26, 0xFF);
        let token = ColorToken::new(c);
        assert_eq!(token.color32().unwrap(), c);
    }

    #[test]
    fn color_parses_six_digit_as_opaque() {
        let token = ColorToken {
            kind: ColorTokenType::Color,
            value: "#2E2E32".to_string(),
        };
        assert_eq!(
            token.color32().unwrap(),
            egui::Color32::from_rgb(0x2E, 0x2E, 0x32)
        );
    }

    #[test]
    fn color_rejects_bad_value() {
        let token = ColorToken {
            kind: ColorTokenType::Color,
            value: "orange".to_string(),
        };
        assert!(token.color32().is_err());
    }

    #[test]
    fn dimension_round_trips() {
        let token = DimensionToken::new(12.0);
        assert_eq!(token.px().unwrap(), 12.0);
    }

    #[test]
    fn dimension_rejects_bad_value() {
        let token = DimensionToken {
            kind: DimensionTokenType::Dimension,
            value: "12".to_string(),
        };
        assert!(token.px().is_err());
    }

    #[test]
    fn tokens_serialize_dtcg_shape() {
        let color = ColorToken::new(egui::Color32::from_rgba_unmultiplied(
            0xFF, 0xFF, 0xFF, 0x26,
        ));
        let json = serde_json::to_string(&color).unwrap();
        assert_eq!(json, r##"{"$type":"color","$value":"#FFFFFF26"}"##);

        let dim = DimensionToken::new(48.0);
        let json = serde_json::to_string(&dim).unwrap();
        assert_eq!(json, r#"{"$type":"dimension","$value":"48px"}"#);
    }
}
