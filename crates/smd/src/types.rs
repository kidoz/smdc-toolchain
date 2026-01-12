//! Common type definitions for Sega Mega Drive development
//!
//! This module provides type aliases and fixed-point math utilities.

/// Fixed-point 16.16 number
///
/// Upper 16 bits are the integer part, lower 16 bits are fractional.
///
/// # Example
/// ```
/// use smd::types::Fix16;
///
/// let a = Fix16::from_int(5);      // 5.0
/// let b = Fix16::from_raw(0x28000); // 2.5
/// let c = a + b;                    // 7.5
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct Fix16(pub i32);

impl Fix16 {
    /// Zero value
    pub const ZERO: Fix16 = Fix16(0);
    /// One (1.0)
    pub const ONE: Fix16 = Fix16(0x10000);
    /// One half (0.5)
    pub const HALF: Fix16 = Fix16(0x8000);

    /// Create from integer value
    #[inline]
    pub const fn from_int(n: i16) -> Self {
        Fix16((n as i32) << 16)
    }

    /// Create from raw fixed-point value
    #[inline]
    pub const fn from_raw(raw: i32) -> Self {
        Fix16(raw)
    }

    /// Get integer part (truncated toward zero)
    #[inline]
    pub const fn to_int(self) -> i16 {
        (self.0 >> 16) as i16
    }

    /// Get raw fixed-point value
    #[inline]
    pub const fn to_raw(self) -> i32 {
        self.0
    }

    /// Multiply two Fix16 values
    #[inline]
    pub fn mul(self, other: Fix16) -> Fix16 {
        Fix16(((self.0 as i64 * other.0 as i64) >> 16) as i32)
    }

    /// Divide two Fix16 values
    #[inline]
    pub fn div(self, other: Fix16) -> Fix16 {
        Fix16((((self.0 as i64) << 16) / other.0 as i64) as i32)
    }

    /// Absolute value
    #[inline]
    pub const fn abs(self) -> Fix16 {
        if self.0 < 0 {
            Fix16(-self.0)
        } else {
            self
        }
    }

    /// Negate
    #[inline]
    pub const fn neg(self) -> Fix16 {
        Fix16(-self.0)
    }
}

impl core::ops::Add for Fix16 {
    type Output = Fix16;
    #[inline]
    fn add(self, other: Fix16) -> Fix16 {
        Fix16(self.0 + other.0)
    }
}

impl core::ops::Sub for Fix16 {
    type Output = Fix16;
    #[inline]
    fn sub(self, other: Fix16) -> Fix16 {
        Fix16(self.0 - other.0)
    }
}

impl core::ops::AddAssign for Fix16 {
    #[inline]
    fn add_assign(&mut self, other: Fix16) {
        self.0 += other.0;
    }
}

impl core::ops::SubAssign for Fix16 {
    #[inline]
    fn sub_assign(&mut self, other: Fix16) {
        self.0 -= other.0;
    }
}

/// 2D vector with Fix16 components
#[derive(Clone, Copy, Default)]
pub struct Vec2 {
    pub x: Fix16,
    pub y: Fix16,
}

impl Vec2 {
    /// Zero vector
    pub const ZERO: Vec2 = Vec2 { x: Fix16::ZERO, y: Fix16::ZERO };

    /// Create new vector
    #[inline]
    pub const fn new(x: Fix16, y: Fix16) -> Self {
        Vec2 { x, y }
    }

    /// Create from integer coordinates
    #[inline]
    pub const fn from_ints(x: i16, y: i16) -> Self {
        Vec2 {
            x: Fix16::from_int(x),
            y: Fix16::from_int(y),
        }
    }

    /// Add two vectors
    #[inline]
    pub fn add(self, other: Vec2) -> Vec2 {
        Vec2 {
            x: Fix16(self.x.0 + other.x.0),
            y: Fix16(self.y.0 + other.y.0),
        }
    }

    /// Subtract two vectors
    #[inline]
    pub fn sub(self, other: Vec2) -> Vec2 {
        Vec2 {
            x: Fix16(self.x.0 - other.x.0),
            y: Fix16(self.y.0 - other.y.0),
        }
    }
}

/// Rectangle structure
#[derive(Clone, Copy, Default)]
pub struct Rect {
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

impl Rect {
    /// Create new rectangle
    #[inline]
    pub const fn new(x: i16, y: i16, width: u16, height: u16) -> Self {
        Rect { x, y, width, height }
    }

    /// Check if point is inside rectangle
    #[inline]
    pub fn contains(&self, px: i16, py: i16) -> bool {
        px >= self.x
            && px < self.x + self.width as i16
            && py >= self.y
            && py < self.y + self.height as i16
    }

    /// Check if two rectangles overlap
    #[inline]
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width as i16
            && self.x + self.width as i16 > other.x
            && self.y < other.y + other.height as i16
            && self.y + self.height as i16 > other.y
    }
}
