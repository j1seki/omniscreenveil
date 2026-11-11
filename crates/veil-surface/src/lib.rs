//! The Veil Surface: a shareable, filtered view of the desktop.
//!
//! Capture backends hand us raw frames; this crate performs the privacy edit.
//! Everything is plain CPU memory so the exact same code can be exercised by
//! unit tests, the headless self-test and the GPU upload path.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use veil_core::{Color, FillStyle, Rect, Size};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PixelFormat {
    /// Windows, X11 and PipeWire default.
    Bgra8,
    /// macOS / CoreGraphics default.
    Rgba8,
}

impl PixelFormat {
    pub const fn bytes_per_pixel(self) -> usize {
        4
    }

    /// Serialize a logical color into the on-screen byte order.
    pub fn encode(self, color: Color) -> [u8; 4] {
        match self {
            PixelFormat::Bgra8 => [color.b, color.g, color.r, color.a],
            PixelFormat::Rgba8 => [color.r, color.g, color.b, color.a],
        }
    }

    pub fn decode(self, bytes: [u8; 4]) -> Color {
        match self {
            PixelFormat::Bgra8 => Color {
                b: bytes[0],
                g: bytes[1],
                r: bytes[2],
                a: bytes[3],
            },
            PixelFormat::Rgba8 => Color {
                r: bytes[0],
                g: bytes[1],
                b: bytes[2],
                a: bytes[3],
            },
        }
    }
}

/// Tightly packed 8-bit-per-channel frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub pixels: Vec<u8>,
}

impl Frame {
    pub fn new(width: u32, height: u32, format: PixelFormat) -> Self {
        let len = width as usize * height as usize * format.bytes_per_pixel();
        Self {
            width,
            height,
            format,
            pixels: vec![0; len],
        }
    }

    /// Build from a tightly packed buffer (e.g. an X11 `GetImage` reply).
    pub fn from_bytes(
        width: u32,
        height: u32,
        format: PixelFormat,
        bytes: Vec<u8>,
    ) -> Result<Self, FrameError> {
        let expected = width as usize * height as usize * format.bytes_per_pixel();
        if bytes.len() != expected {
            return Err(FrameError::Size {
                expected,
                actual: bytes.len(),
            });
        }
        Ok(Self {
            width,
            height,
            format,
            pixels: bytes,
        })
    }

    pub fn filled(width: u32, height: u32, format: PixelFormat, color: Color) -> Self {
        let mut frame = Self::new(width, height, format);
        for chunk in frame.pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&format.encode(color));
        }
        frame
    }

    pub fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    pub fn bounds(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }

    pub fn stride(&self) -> usize {
        self.width as usize * self.format.bytes_per_pixel()
    }

    pub fn offset(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(y as usize * self.stride() + x as usize * self.format.bytes_per_pixel())
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, color: Color) -> bool {
        let Some(offset) = self.offset(x, y) else {
            return false;
        };
        let encoded = self.format.encode(color);
        self.pixels[offset..offset + 4].copy_from_slice(&encoded);
        true
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Color> {
        let offset = self.offset(x, y)?;
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&self.pixels[offset..offset + 4]);
        Some(self.format.decode(bytes))
    }

    /// Fill a rectangle, clipped to the frame. Returns the number of pixels
    /// actually written (useful for assertions and telemetry).
    pub fn fill_rect(&mut self, rect: Rect, style: FillStyle) -> u64 {
        let Some(clipped) = rect.clip_to(self.bounds()) else {
            return 0;
        };
        for y in clipped.y as u32..(clipped.y as u32 + clipped.h) {
            for x in clipped.x as u32..(clipped.x as u32 + clipped.w) {
                let color = style.color_at(x, y);
                self.set_pixel(x, y, color);
            }
        }
        clipped.area()
    }

    /// Copy another frame into this one at the given destination, converting
    /// pixel order if needed. Used to letterbox captures whose size differs
    /// from the advertised surface size.
    pub fn blit_from(&mut self, src: &Frame, dst: Rect) -> u64 {
        if dst.is_empty() {
            return 0;
        }
        let sx = src.width as f64 / dst.w as f64;
        let sy = src.height as f64 / dst.h as f64;
        let mut written = 0u64;
        for y in 0..dst.h {
            let src_y = (y as f64 * sy) as u32;
            if src_y >= src.height {
                break;
            }
            let dst_y = dst.y as i64 + y as i64;
            if dst_y < 0 {
                continue;
            }
            if dst_y as u32 >= self.height {
                break;
            }
            for x in 0..dst.w {
                let src_x = (x as f64 * sx) as u32;
                if src_x >= src.width {
                    break;
                }
                let dst_x = dst.x as i64 + x as i64;
                if dst_x < 0 {
                    continue;
                }
                if dst_x as u32 >= self.width {
                    break;
                }
                if let Some(color) = src.get_pixel(src_x, src_y) {
                    self.set_pixel(dst_x as u32, dst_y as u32, color);
                    written += 1;
                }
            }
        }
        written
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("buffer size mismatch: expected {expected} bytes, got {actual}")]
    Size { expected: usize, actual: usize },
}

/// One masked region in the surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exclusion {
    pub rect: Rect,
    /// Overrides the surface fill for this region (e.g. a red "blocked" tile
    /// for a window the user asked to highlight).
    #[serde(default)]
    pub style: Option<FillStyle>,
}

impl Exclusion {
    pub fn new(rect: Rect) -> Self {
        Self { rect, style: None }
    }

    pub fn styled(rect: Rect, style: FillStyle) -> Self {
        Self {
            rect,
            style: Some(style),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfacePlan {
    pub size: Size,
    pub fill: FillStyle,
    pub exclusions: Vec<Exclusion>,
}

impl SurfacePlan {
    pub fn new(size: Size, fill: FillStyle) -> Self {
        Self {
            size,
            fill,
            exclusions: Vec::new(),
        }
    }

    pub fn exclude(mut self, rect: Rect) -> Self {
        if !rect.is_empty() {
            self.exclusions.push(Exclusion::new(rect));
        }
        self
    }

    pub fn exclude_styled(mut self, rect: Rect, style: FillStyle) -> Self {
        if !rect.is_empty() {
            self.exclusions.push(Exclusion::styled(rect, style));
        }
        self
    }
}

/// Compose the output the user shares.
///
/// The source is copied 1:1 when sizes match; otherwise it is scaled to fit.
/// Any part of the surface not covered by the source keeps the fill pattern,
/// so a shrinking capture never leaks stale pixels.
pub fn render(src: &Frame, plan: &SurfacePlan) -> Frame {
    let mut out = Frame::new(plan.size.w, plan.size.h, src.format);
    out.fill_rect(out.bounds(), plan.fill);
    out.blit_from(src, Rect::from_size(plan.size));
    for exclusion in &plan.exclusions {
        let style = exclusion.style.unwrap_or(plan.fill);
        out.fill_rect(exclusion.rect, style);
    }
    out
}

/// In-place mask, for backends that already own a full-size output frame.
pub fn apply_exclusions(frame: &mut Frame, plan: &SurfacePlan) -> u64 {
    let mut written = 0;
    for exclusion in &plan.exclusions {
        let style = exclusion.style.unwrap_or(plan.fill);
        written += frame.fill_rect(exclusion.rect, style);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color::rgb(255, 0, 0);
    const GREEN: Color = Color::rgb(0, 255, 0);
    const BLUE: Color = Color::rgb(0, 0, 255);

    fn solid(width: u32, height: u32, color: Color) -> Frame {
        Frame::filled(width, height, PixelFormat::Bgra8, color)
    }

    #[test]
    fn pixel_round_trip_bgra() {
        let mut f = Frame::new(2, 2, PixelFormat::Bgra8);
        assert!(f.set_pixel(1, 1, RED));
        assert_eq!(f.get_pixel(1, 1), Some(RED));
        // BGRA order on the wire
        assert_eq!(&f.pixels[12..16], &[0, 0, 255, 255]);
        assert_eq!(f.get_pixel(2, 0), None);
        assert!(!f.set_pixel(9, 9, RED));
    }

    #[test]
    fn pixel_round_trip_rgba() {
        let mut f = Frame::new(1, 1, PixelFormat::Rgba8);
        f.set_pixel(0, 0, GREEN);
        assert_eq!(&f.pixels[..4], &[0, 255, 0, 255]);
        assert_eq!(f.get_pixel(0, 0), Some(GREEN));
    }

    #[test]
    fn exclusion_masks_only_its_rect() {
        let src = solid(100, 100, GREEN);
        let plan = SurfacePlan::new(Size::new(100, 100), FillStyle::veiled())
            .exclude(Rect::new(10, 20, 30, 40));
        let out = render(&src, &plan);
        assert_eq!(out.get_pixel(11, 21), Some(Color::VEIL_ACCENT));
        assert_eq!(out.get_pixel(5, 5), Some(GREEN));
        assert_eq!(out.get_pixel(40, 60), Some(GREEN));
        // boundary is exclusive on the right/bottom
        assert_eq!(out.get_pixel(40, 21), Some(GREEN));
    }

    #[test]
    fn exclusions_are_clipped_to_the_surface() {
        let src = solid(20, 20, GREEN);
        let plan = SurfacePlan::new(Size::new(20, 20), FillStyle::veiled())
            .exclude(Rect::new(-5, -5, 10, 10))
            .exclude(Rect::new(8, 8, 100, 100));
        let out = render(&src, &plan);
        assert_eq!(out.get_pixel(0, 0), Some(Color::VEIL));
        assert_eq!(out.get_pixel(9, 9), Some(Color::VEIL));
        assert_eq!(out.get_pixel(6, 6), Some(GREEN));
        assert_eq!(out.get_pixel(12, 12), Some(Color::VEIL));
    }

    #[test]
    fn empty_exclusions_are_dropped() {
        let plan =
            SurfacePlan::new(Size::new(4, 4), FillStyle::veiled()).exclude(Rect::new(0, 0, 0, 10));
        assert!(plan.exclusions.is_empty());
    }

    #[test]
    fn overlapping_exclusions_apply_in_order() {
        let src = solid(20, 20, GREEN);
        let plan = SurfacePlan::new(Size::new(20, 20), FillStyle::veiled())
            .exclude(Rect::new(0, 0, 10, 10))
            .exclude_styled(Rect::new(5, 5, 10, 10), FillStyle::Solid { color: RED });
        let out = render(&src, &plan);
        assert_eq!(out.get_pixel(2, 2), Some(Color::VEIL));
        assert_eq!(out.get_pixel(7, 7), Some(RED));
        assert_eq!(out.get_pixel(12, 12), Some(RED));
        assert_eq!(out.get_pixel(16, 16), Some(GREEN));
    }

    #[test]
    fn mismatched_source_size_is_scaled() {
        let src = solid(50, 50, RED);
        let plan = SurfacePlan::new(Size::new(100, 100), FillStyle::Solid { color: BLUE });
        let out = render(&src, &plan);
        assert_eq!(out.width, 100);
        // top-left is scaled source, bottom-right is still source because 1:1
        // scaling fills the whole surface
        assert_eq!(out.get_pixel(0, 0), Some(RED));
        assert_eq!(out.get_pixel(99, 99), Some(RED));

        let plan = SurfacePlan::new(Size::new(200, 200), FillStyle::Solid { color: BLUE });
        let out = render(&src, &plan);
        assert_eq!(out.get_pixel(199, 199), Some(RED));
    }

    #[test]
    fn non_uniform_scale_fills_surface() {
        let src = solid(10, 10, GREEN);
        // blit_from with dst larger than src only writes the source area when
        // scaling up would repeat; verify the far corner too
        let plan = SurfacePlan::new(Size::new(10, 20), FillStyle::Solid { color: BLUE });
        let out = render(&src, &plan);
        assert_eq!(out.get_pixel(0, 0), Some(GREEN));
        assert_eq!(out.get_pixel(0, 19), Some(GREEN));
        assert_eq!(out.get_pixel(5, 15), Some(GREEN));
    }

    #[test]
    fn in_place_masking_matches_render() {
        let src = solid(30, 30, GREEN);
        let plan = SurfacePlan::new(Size::new(30, 30), FillStyle::veiled())
            .exclude(Rect::new(5, 5, 10, 10));
        let rendered = render(&src, &plan);
        let mut masked = src.clone();
        apply_exclusions(&mut masked, &plan);
        assert_eq!(rendered, masked);
    }

    #[test]
    fn fill_rect_reports_written_pixels() {
        let mut f = solid(10, 10, GREEN);
        assert_eq!(
            f.fill_rect(Rect::new(0, 0, 10, 10), FillStyle::Solid { color: RED }),
            100
        );
        assert_eq!(
            f.fill_rect(Rect::new(5, 5, 100, 100), FillStyle::Solid { color: RED }),
            25
        );
        assert_eq!(
            f.fill_rect(Rect::new(100, 100, 1, 1), FillStyle::Solid { color: RED }),
            0
        );
    }

    #[test]
    fn from_bytes_validates_length() {
        assert!(Frame::from_bytes(2, 2, PixelFormat::Bgra8, vec![0; 16]).is_ok());
        assert!(Frame::from_bytes(2, 2, PixelFormat::Bgra8, vec![0; 15]).is_err());
    }
}
