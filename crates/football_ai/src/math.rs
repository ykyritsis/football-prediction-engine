//! Minimal 2D vector maths.
//!
//! The crate deliberately avoids external maths dependencies so that the
//! simulation is fully self-contained and its floating point behaviour is
//! entirely defined by this file (only `+ - * / sqrt` and `atan2` are used on
//! hot paths). Converting to/from `bevy::math::Vec2` is a field-by-field copy.

use core::ops::{Add, AddAssign, Div, Mul, MulAssign, Neg, Sub, SubAssign};

/// A 2D vector in pitch space (metres).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub const X: Vec2 = Vec2 { x: 1.0, y: 0.0 };
    pub const Y: Vec2 = Vec2 { x: 0.0, y: 1.0 };

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[inline]
    pub fn dot(self, o: Vec2) -> f32 {
        self.x * o.x + self.y * o.y
    }

    /// 2D cross product (z component of the 3D cross product).
    #[inline]
    pub fn perp_dot(self, o: Vec2) -> f32 {
        self.x * o.y - self.y * o.x
    }

    /// Vector rotated 90 degrees counter-clockwise.
    #[inline]
    pub fn perp(self) -> Vec2 {
        Vec2::new(-self.y, self.x)
    }

    #[inline]
    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    #[inline]
    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    #[inline]
    pub fn distance(self, o: Vec2) -> f32 {
        (self - o).length()
    }

    #[inline]
    pub fn distance_squared(self, o: Vec2) -> f32 {
        (self - o).length_squared()
    }

    /// Normalised vector, or zero when the length is (near) zero.
    #[inline]
    pub fn normalize_or_zero(self) -> Vec2 {
        let len = self.length();
        if len > 1e-6 {
            self / len
        } else {
            Vec2::ZERO
        }
    }

    /// Clamp the length of the vector to `max`.
    #[inline]
    pub fn clamp_length(self, max: f32) -> Vec2 {
        let len = self.length();
        if len > max && len > 1e-6 {
            self * (max / len)
        } else {
            self
        }
    }

    #[inline]
    pub fn lerp(self, o: Vec2, t: f32) -> Vec2 {
        self + (o - self) * t
    }

    #[inline]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// Angle of the vector in radians (`atan2(y, x)`).
    #[inline]
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }

    /// Bit-exact representation, used for state hashing.
    #[inline]
    pub fn to_bits(self) -> (u32, u32) {
        (self.x.to_bits(), self.y.to_bits())
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    #[inline]
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}
impl AddAssign for Vec2 {
    #[inline]
    fn add_assign(&mut self, o: Vec2) {
        self.x += o.x;
        self.y += o.y;
    }
}
impl Sub for Vec2 {
    type Output = Vec2;
    #[inline]
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}
impl SubAssign for Vec2 {
    #[inline]
    fn sub_assign(&mut self, o: Vec2) {
        self.x -= o.x;
        self.y -= o.y;
    }
}
impl Mul<f32> for Vec2 {
    type Output = Vec2;
    #[inline]
    fn mul(self, s: f32) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}
impl MulAssign<f32> for Vec2 {
    #[inline]
    fn mul_assign(&mut self, s: f32) {
        self.x *= s;
        self.y *= s;
    }
}
impl Div<f32> for Vec2 {
    type Output = Vec2;
    #[inline]
    fn div(self, s: f32) -> Vec2 {
        Vec2::new(self.x / s, self.y / s)
    }
}
impl Neg for Vec2 {
    type Output = Vec2;
    #[inline]
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}

/// Closest point to `p` on the segment `a..b`, and the segment parameter `t` in `[0, 1]`.
pub fn closest_point_on_segment(a: Vec2, b: Vec2, p: Vec2) -> (Vec2, f32) {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 < 1e-9 {
        return (a, 0.0);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    (a + ab * t, t)
}

/// Clamp helper usable in const-free contexts.
#[inline]
pub fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

/// Smooth logistic squashing used to turn time margins into probabilities.
#[inline]
pub fn logistic(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

/// Total ordering for `f32` scores (NaN sorts last). Used by every sort in the
/// crate so that tie-breaking is explicit and deterministic.
#[inline]
pub fn cmp_f32(a: f32, b: f32) -> core::cmp::Ordering {
    a.total_cmp(&b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_projection() {
        let (p, t) =
            closest_point_on_segment(Vec2::ZERO, Vec2::new(10.0, 0.0), Vec2::new(5.0, 3.0));
        assert_eq!(p, Vec2::new(5.0, 0.0));
        assert!((t - 0.5).abs() < 1e-6);
        let (p, t) =
            closest_point_on_segment(Vec2::ZERO, Vec2::new(10.0, 0.0), Vec2::new(-5.0, 3.0));
        assert_eq!(p, Vec2::ZERO);
        assert_eq!(t, 0.0);
    }

    #[test]
    fn normalize_zero_is_safe() {
        assert_eq!(Vec2::ZERO.normalize_or_zero(), Vec2::ZERO);
        let n = Vec2::new(3.0, 4.0).normalize_or_zero();
        assert!((n.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn clamp_length_caps() {
        let v = Vec2::new(30.0, 40.0).clamp_length(5.0);
        assert!((v.length() - 5.0).abs() < 1e-5);
        assert_eq!(Vec2::new(1.0, 0.0).clamp_length(5.0), Vec2::new(1.0, 0.0));
    }

    #[test]
    fn logistic_is_monotone_and_bounded() {
        assert!(logistic(-10.0) < 0.001);
        assert!(logistic(10.0) > 0.999);
        assert!((logistic(0.0) - 0.5).abs() < 1e-6);
    }
}
