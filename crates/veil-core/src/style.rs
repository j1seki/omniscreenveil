use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }

    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    /// Neutral placeholder that reads as "deliberately hidden" without being
    /// mistaken for a broken capture (pure black often is).
    pub const VEIL: Color = Color::rgb(24, 26, 33);
    pub const VEIL_ACCENT: Color = Color::rgb(38, 42, 54);
}

/// How a masked area is painted in the shared surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FillStyle {
    Solid { color: Color },
    Checker { tile: u32, a: Color, b: Color },
}

impl FillStyle {
    pub const fn veiled() -> Self {
        FillStyle::Checker {
            tile: 16,
            a: Color::VEIL,
            b: Color::VEIL_ACCENT,
        }
    }

    pub fn color_at(&self, x: u32, y: u32) -> Color {
        match *self {
            FillStyle::Solid { color } => color,
            FillStyle::Checker { tile, a, b } => {
                let tile = tile.max(1);
                if (x / tile + y / tile) % 2 == 0 {
                    a
                } else {
                    b
                }
            }
        }
    }
}
