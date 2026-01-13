//! Video Display Processor (VDP) interface
//!
//! The VDP is responsible for all graphics rendering on the Genesis.
//! It supports:
//! - Two scrolling background planes (A and B)
//! - Up to 80 hardware sprites
//! - 64-color palette (from 512 possible colors)
//! - Multiple display resolutions
//!
//! # Example
//!
//! ```no_run
//! use smd::vdp::{self, Color};
//!
//! vdp::init();
//! vdp::set_color(0, Color::BLACK);
//! vdp::set_color(1, Color::WHITE);
//!
//! loop {
//!     vdp::vsync();
//!     // Game logic here
//! }
//! ```

/// VDP data port address
const VDP_DATA: *mut u16 = 0xC00000 as *mut u16;
/// VDP control port address
const VDP_CTRL: *mut u16 = 0xC00004 as *mut u16;

/// Default VRAM addresses
pub mod vram {
    /// Plane A nametable (default)
    pub const PLANE_A: u16 = 0xC000;
    /// Plane B nametable (default)
    pub const PLANE_B: u16 = 0xE000;
    /// Window plane (default)
    pub const WINDOW: u16 = 0xD000;
    /// Sprite attribute table (default)
    pub const SPRITES: u16 = 0xF000;
    /// HScroll table (default)
    pub const HSCROLL: u16 = 0xFC00;
}

/// Predefined colors in BGR format
///
/// The Genesis uses 9-bit color: 3 bits each for Blue, Green, Red.
/// Format: `0x0BGR` where each component is 0-14 (even values only).
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Color(pub u16);

impl Color {
    /// Black (0, 0, 0)
    pub const BLACK: Color = Color(0x000);
    /// White (255, 255, 255)
    pub const WHITE: Color = Color(0xEEE);
    /// Red (255, 0, 0)
    pub const RED: Color = Color(0x00E);
    /// Green (0, 255, 0)
    pub const GREEN: Color = Color(0x0E0);
    /// Blue (0, 0, 255)
    pub const BLUE: Color = Color(0xE00);
    /// Yellow (255, 255, 0)
    pub const YELLOW: Color = Color(0x0EE);
    /// Cyan (0, 255, 255)
    pub const CYAN: Color = Color(0xEE0);
    /// Magenta (255, 0, 255)
    pub const MAGENTA: Color = Color(0xE0E);

    /// Create color from RGB components
    ///
    /// # Arguments
    /// * `r` - Red component (0-14, even values)
    /// * `g` - Green component (0-14, even values)
    /// * `b` - Blue component (0-14, even values)
    #[inline]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color(((b as u16) << 8) | ((g as u16) << 4) | (r as u16))
    }
}

/// Tile attribute flags
pub mod tile {
    /// High priority (draw in front of sprites)
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

    /// Create tile attribute word
    #[inline]
    pub const fn attr(index: u16, pal: u8, priority: bool, hflip: bool, vflip: bool) -> u16 {
        index
            | ((pal as u16) << 13)
            | if priority { 0x8000 } else { 0 }
            | if hflip { 0x0800 } else { 0 }
            | if vflip { 0x1000 } else { 0 }
    }
}

/// Initialize VDP with default settings
///
/// Configures:
/// - 320x224 (H40) display mode
/// - Display enabled, Mode 5
/// - Default plane addresses
/// - Auto-increment of 2
pub fn init() {
    set_reg(0, 0x04);  // Mode register 1
    set_reg(1, 0x44);  // Mode register 2: display on, Mode 5
    set_reg(2, 0x30);  // Plane A address: 0xC000
    set_reg(3, 0x00);  // Window address
    set_reg(4, 0x07);  // Plane B address: 0xE000
    set_reg(5, 0x78);  // Sprite table: 0xF000
    set_reg(6, 0x00);  // Unused
    set_reg(7, 0x00);  // Background color: palette 0, color 0
    set_reg(10, 0xFF); // H-interrupt counter
    set_reg(11, 0x00); // Mode register 3
    set_reg(12, 0x81); // Mode register 4: H40, no interlace
    set_reg(13, 0x3F); // HScroll table: 0xFC00
    set_reg(15, 0x02); // Auto-increment: 2
    set_reg(16, 0x01); // Plane size: 64x32
    set_reg(17, 0x00); // Window H position
    set_reg(18, 0x00); // Window V position
}

/// Set a VDP register value
///
/// # Arguments
/// * `reg` - Register number (0-23)
/// * `value` - Value to write (0-255)
#[inline]
pub fn set_reg(reg: u8, value: u8) {
    unsafe {
        VDP_CTRL.write_volatile(0x8000 | ((reg as u16) << 8) | (value as u16));
    }
}

/// Wait for vertical blank period
///
/// Blocks until the VDP enters vertical blank.
/// Use this to synchronize game updates with display refresh.
pub fn vsync() {
    unsafe {
        // Wait for vblank flag
        while (VDP_CTRL.read_volatile() & 0x08) == 0 {}
    }
}

/// Read VDP status register
#[inline]
pub fn status() -> u16 {
    unsafe { VDP_CTRL.read_volatile() }
}

/// Set VRAM write address
///
/// After calling this, write data via [`write_data()`].
pub fn set_write_addr(addr: u16) {
    unsafe {
        let cmd1 = 0x4000 | (addr & 0x3FFF);
        let cmd2 = (addr >> 14) & 0x03;
        VDP_CTRL.write_volatile(cmd1);
        VDP_CTRL.write_volatile(cmd2);
    }
}

/// Set CRAM (palette) write address
pub fn set_cram_addr(index: u8) {
    unsafe {
        VDP_CTRL.write_volatile(0xC000 | ((index as u16) << 1));
        VDP_CTRL.write_volatile(0x0000);
    }
}

/// Write a word to VDP data port
#[inline]
pub fn write_data(value: u16) {
    unsafe {
        VDP_DATA.write_volatile(value);
    }
}

/// Set a single palette color
///
/// # Arguments
/// * `index` - Color index (0-63)
/// * `color` - Color value
pub fn set_color(index: u8, color: Color) {
    set_cram_addr(index);
    write_data(color.0);
}

/// Load multiple palette colors
///
/// # Arguments
/// * `index` - Starting color index (0-63)
/// * `colors` - Slice of colors to load
pub fn load_palette(index: u8, colors: &[Color]) {
    set_cram_addr(index);
    for color in colors {
        write_data(color.0);
    }
}

/// Load tile data to VRAM
///
/// # Arguments
/// * `index` - Starting tile index (0-2047)
/// * `data` - Tile data (32 bytes per tile)
pub fn load_tiles(index: u16, data: &[u32]) {
    set_write_addr(index * 32);
    for &word in data {
        write_data((word >> 16) as u16);
        write_data(word as u16);
    }
}

/// Set a tile in Plane A
///
/// # Arguments
/// * `x` - Tile X position (0-63)
/// * `y` - Tile Y position (0-31)
/// * `attr` - Tile attributes (use [`tile::attr()`])
pub fn set_tile_a(x: u8, y: u8, attr: u16) {
    let addr = vram::PLANE_A + ((y as u16) * 128) + ((x as u16) * 2);
    set_write_addr(addr);
    write_data(attr);
}

/// Set a tile in Plane B
pub fn set_tile_b(x: u8, y: u8, attr: u16) {
    let addr = vram::PLANE_B + ((y as u16) * 128) + ((x as u16) * 2);
    set_write_addr(addr);
    write_data(attr);
}

/// Set background color
///
/// # Arguments
/// * `palette` - Palette number (0-3)
/// * `color` - Color index within palette (0-15)
pub fn set_background(palette: u8, color: u8) {
    set_reg(7, (palette << 4) | color);
}

/// Set horizontal scroll for Plane A
///
/// # Arguments
/// * `scroll` - Scroll value in pixels (negative = scroll right)
pub fn set_hscroll_a(scroll: i16) {
    set_write_addr(vram::HSCROLL);
    write_data(scroll as u16);
}

/// Set horizontal scroll for Plane B
///
/// # Arguments
/// * `scroll` - Scroll value in pixels
pub fn set_hscroll_b(scroll: i16) {
    set_write_addr(vram::HSCROLL + 2);
    write_data(scroll as u16);
}

/// Set vertical scroll for Plane A
///
/// # Arguments
/// * `scroll` - Scroll value in pixels (negative = scroll down)
pub fn set_vscroll_a(scroll: i16) {
    // VSRAM write
    unsafe {
        VDP_CTRL.write_volatile(0x4000);
        VDP_CTRL.write_volatile(0x0010);
        VDP_DATA.write_volatile(scroll as u16);
    }
}

/// Set vertical scroll for Plane B
///
/// # Arguments
/// * `scroll` - Scroll value in pixels
pub fn set_vscroll_b(scroll: i16) {
    // VSRAM write at offset 2
    unsafe {
        VDP_CTRL.write_volatile(0x4002);
        VDP_CTRL.write_volatile(0x0010);
        VDP_DATA.write_volatile(scroll as u16);
    }
}

/// Clear Plane A (fill with tile 0)
pub fn clear_plane_a() {
    set_write_addr(vram::PLANE_A);
    for _ in 0..(64 * 32) {
        write_data(0);
    }
}

/// Clear Plane B (fill with tile 0)
pub fn clear_plane_b() {
    set_write_addr(vram::PLANE_B);
    for _ in 0..(64 * 32) {
        write_data(0);
    }
}
