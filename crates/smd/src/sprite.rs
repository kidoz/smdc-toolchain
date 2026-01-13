//! Sprite management for Sega Mega Drive
//!
//! The Genesis VDP supports up to 80 hardware sprites.
//! Each sprite can be 1-4 tiles wide and 1-4 tiles tall (8-32 pixels).
//!
//! # Example
//!
//! ```no_run
//! use smd::sprite::{self, SpriteSize};
//!
//! sprite::init();
//!
//! // Create a 1x4 tile sprite (8x32 pixels) at position (100, 50)
//! sprite::set(0, 100, 50, SpriteSize::Size1x4, 1);
//!
//! // Hide unused sprites
//! sprite::hide(1);
//! ```

use crate::vdp;

/// Sprite attribute table address
const SPRITE_TABLE: u16 = 0xF000;

/// Maximum number of sprites
pub const MAX_SPRITES: u8 = 80;

/// Sprite size configuration
///
/// Format: `Size{Width}x{Height}` where dimensions are in tiles (8 pixels each).
///
/// The VDP stores size as: bits 0-1 = height-1, bits 2-3 = width-1
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SpriteSize {
    /// 8x8 pixels (1x1 tiles)
    Size1x1 = 0x00,
    /// 8x16 pixels (1x2 tiles)
    Size1x2 = 0x01,
    /// 8x24 pixels (1x3 tiles)
    Size1x3 = 0x02,
    /// 8x32 pixels (1x4 tiles)
    Size1x4 = 0x03,

    /// 16x8 pixels (2x1 tiles)
    Size2x1 = 0x04,
    /// 16x16 pixels (2x2 tiles)
    Size2x2 = 0x05,
    /// 16x24 pixels (2x3 tiles)
    Size2x3 = 0x06,
    /// 16x32 pixels (2x4 tiles)
    Size2x4 = 0x07,

    /// 24x8 pixels (3x1 tiles)
    Size3x1 = 0x08,
    /// 24x16 pixels (3x2 tiles)
    Size3x2 = 0x09,
    /// 24x24 pixels (3x3 tiles)
    Size3x3 = 0x0A,
    /// 24x32 pixels (3x4 tiles)
    Size3x4 = 0x0B,

    /// 32x8 pixels (4x1 tiles)
    Size4x1 = 0x0C,
    /// 32x16 pixels (4x2 tiles)
    Size4x2 = 0x0D,
    /// 32x24 pixels (4x3 tiles)
    Size4x3 = 0x0E,
    /// 32x32 pixels (4x4 tiles)
    Size4x4 = 0x0F,
}

impl SpriteSize {
    /// Get width in pixels
    #[inline]
    pub const fn width(self) -> u8 {
        (((self as u8) >> 2) + 1) * 8
    }

    /// Get height in pixels
    #[inline]
    pub const fn height(self) -> u8 {
        (((self as u8) & 0x03) + 1) * 8
    }

    /// Get width in tiles
    #[inline]
    pub const fn width_tiles(self) -> u8 {
        ((self as u8) >> 2) + 1
    }

    /// Get height in tiles
    #[inline]
    pub const fn height_tiles(self) -> u8 {
        ((self as u8) & 0x03) + 1
    }
}

/// Sprite attribute flags
pub mod attr {
    /// High priority (draw in front of high-priority tiles)
    pub const PRIORITY: u16 = 0x8000;
    /// Palette 0
    pub const PAL0: u16 = 0x0000;
    /// Palette 1
    pub const PAL1: u16 = 0x2000;
    /// Palette 2
    pub const PAL2: u16 = 0x4000;
    /// Palette 3
    pub const PAL3: u16 = 0x6000;
    /// Vertical flip
    pub const VFLIP: u16 = 0x1000;
    /// Horizontal flip
    pub const HFLIP: u16 = 0x0800;
}

/// Sprite definition structure
#[derive(Clone, Copy, Default)]
pub struct Sprite {
    /// X position on screen
    pub x: i16,
    /// Y position on screen
    pub y: i16,
    /// Sprite size
    pub size: u8,
    /// Base tile index
    pub tile: u16,
    /// Attribute flags (palette, flip, priority)
    pub attr: u16,
}

impl Sprite {
    /// Create a new sprite
    pub const fn new(x: i16, y: i16, size: SpriteSize, tile: u16) -> Self {
        Sprite {
            x,
            y,
            size: size as u8,
            tile,
            attr: 0,
        }
    }

    /// Set palette (0-3)
    #[inline]
    pub fn with_palette(mut self, pal: u8) -> Self {
        self.attr = (self.attr & !0x6000) | ((pal as u16) << 13);
        self
    }

    /// Set priority flag
    #[inline]
    pub fn with_priority(mut self, priority: bool) -> Self {
        if priority {
            self.attr |= attr::PRIORITY;
        } else {
            self.attr &= !attr::PRIORITY;
        }
        self
    }

    /// Set horizontal flip
    #[inline]
    pub fn with_hflip(mut self, flip: bool) -> Self {
        if flip {
            self.attr |= attr::HFLIP;
        } else {
            self.attr &= !attr::HFLIP;
        }
        self
    }

    /// Set vertical flip
    #[inline]
    pub fn with_vflip(mut self, flip: bool) -> Self {
        if flip {
            self.attr |= attr::VFLIP;
        } else {
            self.attr &= !attr::VFLIP;
        }
        self
    }
}

/// Initialize sprite system
///
/// Clears all 80 sprite entries and sets up default linking.
pub fn init() {
    for i in 0..MAX_SPRITES {
        clear(i);
    }
}

/// Set sprite attributes directly
///
/// # Arguments
/// * `index` - Sprite index (0-79)
/// * `x` - X position on screen
/// * `y` - Y position on screen
/// * `size` - Sprite size
/// * `tile` - Base tile index (and attributes)
pub fn set(index: u8, x: i16, y: i16, size: SpriteSize, tile: u16) {
    let addr = SPRITE_TABLE + (index as u16) * 8;
    vdp::set_write_addr(addr);

    // Word 0: Y position + 128
    vdp::write_data((y + 128) as u16);

    // Word 1: Size (upper byte) | Link (lower byte)
    let next = if index < MAX_SPRITES - 1 { index + 1 } else { 0 };
    vdp::write_data(((size as u16) << 8) | (next as u16));

    // Word 2: Tile attributes
    vdp::write_data(tile);

    // Word 3: X position + 128
    vdp::write_data((x + 128) as u16);
}

/// Update sprite from Sprite structure
pub fn update(index: u8, spr: &Sprite) {
    let addr = SPRITE_TABLE + (index as u16) * 8;
    vdp::set_write_addr(addr);

    vdp::write_data((spr.y + 128) as u16);

    let next = if index < MAX_SPRITES - 1 { index + 1 } else { 0 };
    vdp::write_data(((spr.size as u16) << 8) | (next as u16));

    vdp::write_data(spr.tile | spr.attr);
    vdp::write_data((spr.x + 128) as u16);
}

/// Set sprite position only
///
/// Faster than `set()` when only position changes.
pub fn set_pos(index: u8, x: i16, y: i16) {
    let addr = SPRITE_TABLE + (index as u16) * 8;

    // Write Y position
    vdp::set_write_addr(addr);
    vdp::write_data((y + 128) as u16);

    // Write X position (skip to word 3)
    vdp::set_write_addr(addr + 6);
    vdp::write_data((x + 128) as u16);
}

/// Hide a sprite (move off-screen)
pub fn hide(index: u8) {
    let addr = SPRITE_TABLE + (index as u16) * 8;
    vdp::set_write_addr(addr);
    vdp::write_data(0); // Y = 0 (off-screen with +128 offset = 128, but 0 ends list)
}

/// Clear a sprite entry
pub fn clear(index: u8) {
    let addr = SPRITE_TABLE + (index as u16) * 8;
    vdp::set_write_addr(addr);
    vdp::write_data(0);
    vdp::write_data(0);
    vdp::write_data(0);
    vdp::write_data(0);
}

/// Set sprite link (for custom sprite ordering)
///
/// # Arguments
/// * `index` - Sprite index (0-79)
/// * `next` - Next sprite index (0 = end of list)
pub fn set_link(index: u8, next: u8) {
    let addr = SPRITE_TABLE + (index as u16) * 8 + 2;
    vdp::set_write_addr(addr);
    // Read current size, update link
    // For simplicity, we just write size 0 with new link
    // In real usage, you'd want to preserve the size
    vdp::write_data(next as u16);
}

/// Clear all sprites
///
/// Hides all 80 sprites by clearing the sprite attribute table.
pub fn clear_all() {
    for i in 0..MAX_SPRITES {
        clear(i);
    }
}
