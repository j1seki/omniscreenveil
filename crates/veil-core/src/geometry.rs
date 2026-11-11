use serde::{Deserialize, Serialize};

/// Integer rectangle in logical (desktop) coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Size {
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    pub const fn from_size(size: Size) -> Self {
        Self::new(0, 0, size.w, size.h)
    }

    pub fn right(&self) -> i64 {
        self.x as i64 + self.w as i64
    }

    pub fn bottom(&self) -> i64 {
        self.y as i64 + self.h as i64
    }

    pub fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0
    }

    pub fn area(&self) -> u64 {
        self.w as u64 * self.h as u64
    }

    pub fn contains_point(&self, p: Point) -> bool {
        (p.x as i64) >= self.x as i64
            && (p.x as i64) < self.right()
            && (p.y as i64) >= self.y as i64
            && (p.y as i64) < self.bottom()
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.intersection(other).is_some()
    }

    /// Intersection of two rectangles, or `None` when they do not overlap.
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x1 = self.x.max(other.x) as i64;
        let y1 = self.y.max(other.y) as i64;
        let x2 = self.right().min(other.right());
        let y2 = self.bottom().min(other.bottom());
        if x2 <= x1 || y2 <= y1 {
            return None;
        }
        Some(Rect::new(
            x1 as i32,
            y1 as i32,
            (x2 - x1) as u32,
            (y2 - y1) as u32,
        ))
    }

    /// Clip to a surface whose origin is (0, 0).
    pub fn clip_to(&self, bounds: Rect) -> Option<Rect> {
        self.intersection(&bounds)
    }

    pub fn translated(&self, dx: i32, dy: i32) -> Rect {
        Rect::new(
            self.x.saturating_add(dx),
            self.y.saturating_add(dy),
            self.w,
            self.h,
        )
    }

    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let x1 = self.x.min(other.x) as i64;
        let y1 = self.y.min(other.y) as i64;
        let x2 = self.right().max(other.right());
        let y2 = self.bottom().max(other.bottom());
        Rect::new(x1 as i32, y1 as i32, (x2 - x1) as u32, (y2 - y1) as u32)
    }
}

impl Size {
    pub const fn new(w: u32, h: u32) -> Self {
        Self { w, h }
    }

    pub const fn rect(self) -> Rect {
        Rect::from_size(self)
    }
}

/// Display scale factor (logical points to physical pixels).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Scale(pub f64);

impl Scale {
    pub const ONE: Scale = Scale(1.0);

    pub fn new(factor: f64) -> Self {
        debug_assert!(factor > 0.0, "scale factor must be positive");
        Scale(if factor > 0.0 { factor } else { 1.0 })
    }

    /// Convert a logical rectangle to physical pixels, rounding outwards so
    /// that a privacy mask always covers at least the whole window.
    pub fn to_physical(self, rect: Rect) -> Rect {
        let x = (rect.x as f64 * self.0).floor();
        let y = (rect.y as f64 * self.0).floor();
        let right = (rect.right() as f64 * self.0).ceil();
        let bottom = (rect.bottom() as f64 * self.0).ceil();
        Rect::new(
            x as i32,
            y as i32,
            (right - x).max(0.0) as u32,
            (bottom - y).max(0.0) as u32,
        )
    }
}

impl Default for Scale {
    fn default() -> Self {
        Scale::ONE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intersection_basic() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(5, 5, 10, 10);
        assert_eq!(a.intersection(&b), Some(Rect::new(5, 5, 5, 5)));
        assert_eq!(a.intersection(&Rect::new(20, 0, 5, 5)), None);
        assert_eq!(a.intersection(&Rect::new(10, 0, 5, 5)), None);
    }

    #[test]
    fn clip_to_surface() {
        let surface = Rect::new(0, 0, 100, 50);
        assert_eq!(
            Rect::new(90, 40, 50, 50).clip_to(surface),
            Some(Rect::new(90, 40, 10, 10))
        );
        assert_eq!(Rect::new(-20, -20, 10, 10).clip_to(surface), None);
    }

    #[test]
    fn union_with_empty() {
        let a = Rect::new(1, 2, 3, 4);
        assert_eq!(a.union(&Rect::new(0, 0, 0, 0)), a);
        assert_eq!(Rect::new(0, 0, 0, 0).union(&a), a);
        assert_eq!(a.union(&Rect::new(10, 10, 2, 2)), Rect::new(1, 2, 11, 10));
    }

    #[test]
    fn scale_rounds_outwards() {
        let r = Rect::new(10, 10, 101, 51);
        let scaled = Scale::new(1.25).to_physical(r);
        // left/bottom edges must be covered, never cropped
        assert_eq!(scaled.x, 12);
        assert_eq!(scaled.y, 12);
        assert!(scaled.right() >= (111.0 * 1.25_f64).ceil() as i64);
        assert!(scaled.bottom() >= (61.0 * 1.25_f64).ceil() as i64);

        let r = Rect::new(-100, -50, 20, 20);
        let scaled = Scale::new(2.0).to_physical(r);
        assert_eq!(scaled, Rect::new(-200, -100, 40, 40));
    }

    #[test]
    fn scale_one_is_identity() {
        let r = Rect::new(-7, 3, 1024, 768);
        assert_eq!(Scale::ONE.to_physical(r), r);
    }

    #[test]
    fn contains_point_edges() {
        let r = Rect::new(10, 10, 10, 10);
        assert!(r.contains_point(Point { x: 10, y: 10 }));
        assert!(r.contains_point(Point { x: 19, y: 19 }));
        assert!(!r.contains_point(Point { x: 20, y: 19 }));
    }
}
