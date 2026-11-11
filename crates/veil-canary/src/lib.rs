//! A deterministic, high-entropy visual canary.
//!
//! Verification never trusts an API return code. Instead we render a token as
//! an 8x8 grid of saturated colour cells, put it on the screen (usually inside
//! the window being protected), capture the way an audience would, and search
//! the captured pixels for the token. 8^64 possible patterns make accidental
//! matches effectively impossible while staying trivially decodable.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use veil_core::{Color, Point, Rect, Size};
use veil_surface::{Frame, PixelFormat};

pub const GRID: u32 = 8;
pub const CELLS: usize = (GRID * GRID) as usize;
pub const MIN_CELL: u32 = 2;
pub const DEFAULT_CELL: u32 = 8;
/// Per-channel tolerance; capture pipelines may round colours slightly.
pub const COLOR_TOLERANCE: u8 = 12;
/// Cells that must match for a positive detection (out of 64).
pub const MIN_MATCHES: usize = 60;

const PALETTE: [Color; 8] = [
    Color::rgb(255, 0, 0),
    Color::rgb(0, 255, 0),
    Color::rgb(0, 0, 255),
    Color::rgb(255, 255, 0),
    Color::rgb(0, 255, 255),
    Color::rgb(255, 0, 255),
    Color::rgb(255, 128, 0),
    Color::rgb(128, 0, 255),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token(pub u64);

impl Token {
    /// Stable token derived from a label; identical on every run, which makes
    /// the self-test reproducible.
    pub fn from_seed(seed: &str) -> Self {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in seed.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Token(hash)
    }

    /// Random enough for a live session (time + pid, no external crates).
    pub fn fresh() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x5eed);
        Token(nanos ^ ((std::process::id() as u64) << 32) ^ 0xa5a5_5a5a)
    }

    pub fn hex(self) -> String {
        format!("{:016x}", self.0)
    }
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The 64 colours this token renders as, row-major.
pub fn cells(token: Token) -> [Color; CELLS] {
    let mut state = token.0 | 1;
    let mut out = [Color::BLACK; CELLS];
    let mut previous: Option<usize> = None;
    for cell in out.iter_mut() {
        let mut index = (splitmix64(&mut state) % PALETTE.len() as u64) as usize;
        // neighbouring cells must differ, otherwise a shifted detection could
        // still match; cheap to enforce and improves visual signature
        if Some(index) == previous {
            index = (index + 1) % PALETTE.len();
        }
        *cell = PALETTE[index];
        previous = Some(index);
    }
    out
}

pub fn pattern_size(cell: u32) -> Size {
    let cell = cell.max(MIN_CELL);
    Size::new(GRID * cell, GRID * cell)
}

/// Render the token as a standalone frame.
pub fn render(token: Token, cell: u32) -> Frame {
    let cell = cell.max(MIN_CELL);
    let size = pattern_size(cell);
    let colors = cells(token);
    let mut frame = Frame::new(size.w, size.h, PixelFormat::Bgra8);
    for (i, color) in colors.iter().enumerate() {
        let col = (i as u32) % GRID;
        let row = (i as u32) / GRID;
        frame.fill_rect(
            Rect::new((col * cell) as i32, (row * cell) as i32, cell, cell),
            veil_core::FillStyle::Solid { color: *color },
        );
    }
    frame
}

/// Render the token at an arbitrary origin inside an existing frame.
pub fn render_at(frame: &mut Frame, token: Token, cell: u32, origin: Point) -> Rect {
    let pattern = render(token, cell);
    let rect = Rect::new(origin.x, origin.y, pattern.width, pattern.height);
    frame.blit_from(&pattern, rect);
    rect
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Detection {
    pub found: bool,
    pub confidence: f32,
    pub origin: Option<Point>,
    pub matched: usize,
}

impl Detection {
    fn miss() -> Self {
        Self {
            found: false,
            confidence: 0.0,
            origin: None,
            matched: 0,
        }
    }
}

fn color_matches(a: Color, b: Color) -> bool {
    a.r.abs_diff(b.r) <= COLOR_TOLERANCE
        && a.g.abs_diff(b.g) <= COLOR_TOLERANCE
        && a.b.abs_diff(b.b) <= COLOR_TOLERANCE
}

/// Search the whole frame for the token.
pub fn detect(frame: &Frame, token: Token, cell: u32) -> Detection {
    detect_in(frame, token, cell, frame.bounds())
}

/// Search only candidate origins inside `region` (clipped to the frame).
pub fn detect_in(frame: &Frame, token: Token, cell: u32, region: Rect) -> Detection {
    let cell = cell.max(MIN_CELL);
    let size = pattern_size(cell);
    let colors = cells(token);
    if frame.width < size.w || frame.height < size.h {
        return Detection::miss();
    }
    let max_x = frame.width - size.w;
    let max_y = frame.height - size.h;
    let region = region
        .clip_to(frame.bounds())
        .unwrap_or(Rect::new(0, 0, 0, 0));
    let start_x = region.x.max(0) as u32;
    let start_y = region.y.max(0) as u32;
    let end_x = (region.right().max(0) as u32).min(max_x);
    let end_y = (region.bottom().max(0) as u32).min(max_y);

    for y0 in start_y..=end_y {
        for x0 in start_x..=end_x {
            if let Some(matched) = match_at(frame, &colors, cell, x0, y0) {
                if matched >= MIN_MATCHES {
                    return Detection {
                        found: true,
                        confidence: matched as f32 / CELLS as f32,
                        origin: Some(Point {
                            x: x0 as i32,
                            y: y0 as i32,
                        }),
                        matched,
                    };
                }
            }
        }
    }
    Detection::miss()
}

/// Returns the number of matching cells, or `None` for a clear early miss.
fn match_at(frame: &Frame, colors: &[Color; CELLS], cell: u32, x0: u32, y0: u32) -> Option<usize> {
    let half = cell / 2;
    let mut matched = 0usize;
    for (i, expected) in colors.iter().enumerate() {
        let col = (i as u32) % GRID;
        let row = (i as u32) / GRID;
        let x = x0 + col * cell + half;
        let y = y0 + row * cell + half;
        match frame.get_pixel(x, y) {
            Some(actual) if color_matches(actual, *expected) => matched += 1,
            _ => {
                // A single-cell mismatch is tolerated (compression, overlays),
                // but early-exit once the budget is exhausted.
                let allowed_miss = CELLS - MIN_MATCHES;
                if i + 1 - matched > allowed_miss {
                    return None;
                }
            }
        }
    }
    Some(matched)
}

#[cfg(test)]
mod tests {
    use super::*;
    use veil_core::FillStyle;

    fn veil_frame(w: u32, h: u32) -> Frame {
        Frame::filled(w, h, PixelFormat::Bgra8, Color::rgb(40, 44, 52))
    }

    #[test]
    fn token_is_deterministic() {
        assert_eq!(Token::from_seed("selftest"), Token::from_seed("selftest"));
        assert_ne!(Token::from_seed("a"), Token::from_seed("b"));
    }

    #[test]
    fn pattern_has_expected_size() {
        assert_eq!(pattern_size(8), Size::new(64, 64));
        assert_eq!(pattern_size(1), Size::new(16, 16));
    }

    #[test]
    fn detects_its_own_pattern_at_origin() {
        let token = Token::from_seed("origin");
        let frame = render(token, 8);
        let detection = detect(&frame, token, 8);
        assert!(detection.found);
        assert_eq!(detection.origin, Some(Point { x: 0, y: 0 }));
        assert_eq!(detection.matched, CELLS);
    }

    #[test]
    fn detects_translated_pattern() {
        let token = Token::from_seed("translated");
        let mut desktop = veil_frame(400, 300);
        let rect = render_at(&mut desktop, token, 6, Point { x: 133, y: 77 });
        assert_eq!(rect, Rect::new(133, 77, 48, 48));
        let detection = detect(&desktop, token, 6);
        assert!(detection.found);
        // solid cells mean a candidate origin inside the same cell matches too,
        // so allow one cell of slack
        let origin = detection.origin.unwrap();
        assert!(
            (origin.x - 133).abs() < 6 && (origin.y - 77).abs() < 6,
            "{origin:?}"
        );
    }

    #[test]
    fn region_limited_search_can_miss_and_find() {
        let token = Token::from_seed("region");
        let mut desktop = veil_frame(400, 300);
        render_at(&mut desktop, token, 8, Point { x: 250, y: 150 });
        assert!(!detect_in(&desktop, token, 8, Rect::new(0, 0, 100, 100)).found);
        assert!(detect_in(&desktop, token, 8, Rect::new(200, 100, 200, 200)).found);
    }

    #[test]
    fn wrong_token_is_not_detected() {
        let frame = render(Token::from_seed("one"), 8);
        assert!(!detect(&frame, Token::from_seed("two"), 8).found);
    }

    #[test]
    fn masked_pattern_is_not_detected() {
        let token = Token::from_seed("masked");
        let mut desktop = veil_frame(300, 300);
        let rect = render_at(&mut desktop, token, 8, Point { x: 50, y: 60 });
        assert!(detect(&desktop, token, 8).found);
        desktop.fill_rect(rect, FillStyle::veiled());
        assert!(!detect(&desktop, token, 8).found);
    }

    #[test]
    fn partial_occlusion_is_tolerated_but_large_occlusion_is_not() {
        let token = Token::from_seed("occlusion");
        let mut desktop = veil_frame(200, 200);
        render_at(&mut desktop, token, 8, Point { x: 10, y: 10 });
        // cover two cells (well within the 4-cell miss budget)
        desktop.fill_rect(
            Rect::new(10, 10, 8, 8),
            FillStyle::Solid {
                color: Color::BLACK,
            },
        );
        desktop.fill_rect(
            Rect::new(18, 10, 8, 8),
            FillStyle::Solid {
                color: Color::BLACK,
            },
        );
        assert!(detect(&desktop, token, 8).found);
        // cover a whole row (8 cells) -> too much
        let mut desktop = veil_frame(200, 200);
        render_at(&mut desktop, token, 8, Point { x: 10, y: 10 });
        desktop.fill_rect(
            Rect::new(10, 10, 64, 8),
            FillStyle::Solid {
                color: Color::BLACK,
            },
        );
        assert!(!detect(&desktop, token, 8).found);
    }

    #[test]
    fn noise_does_not_trigger_false_positive() {
        let token = Token::from_seed("noise");
        let mut frame = veil_frame(128, 128);
        let mut state = 0x1234_5678_9abc_def0u64;
        for y in 0..128 {
            for x in 0..128 {
                let v = splitmix64(&mut state);
                frame.set_pixel(
                    x,
                    y,
                    Color {
                        r: (v & 0xff) as u8,
                        g: ((v >> 8) & 0xff) as u8,
                        b: ((v >> 16) & 0xff) as u8,
                        a: 255,
                    },
                );
            }
        }
        assert!(!detect(&frame, token, 8).found);
    }

    #[test]
    fn color_tolerance_handles_pipeline_rounding() {
        let token = Token::from_seed("tolerance");
        let mut frame = render(token, 8);
        // nudge every pixel by a small amount
        for pixel in frame.pixels.chunks_exact_mut(4) {
            pixel[0] = pixel[0].saturating_add(5);
            pixel[1] = pixel[1].saturating_sub(4);
        }
        assert!(detect(&frame, token, 8).found);
    }
}
