//! # SMD - Sega Mega Drive SDK
//!
//! A hardware abstraction layer for Sega Genesis / Mega Drive development in Rust.
//!
//! ## Features
//!
//! - **VDP Control**: Tile graphics, palettes, planes, scrolling
//! - **Sprites**: Up to 80 hardware sprites with various sizes
//! - **Input**: Controller reading with button state tracking
//! - **PSG**: Programmable Sound Generator for simple sound effects
//! - **Types**: Fixed-point math, common type aliases
//!
//! ## Quick Start
//!
//! ```no_run
//! #![no_std]
//! #![no_main]
//!
//! use smd::prelude::*;
//!
//! #[no_mangle]
//! pub extern "C" fn main() {
//!     vdp::init();
//!     sprite::init();
//!     input::init();
//!
//!     vdp::set_color(0, Color::BLACK);
//!     vdp::set_color(1, Color::WHITE);
//!
//!     let mut player_x: i16 = 160;
//!     let mut player_y: i16 = 112;
//!
//!     loop {
//!         vdp::vsync();
//!
//!         let buttons = input::read(0);
//!         if buttons.contains(Button::UP) {
//!             player_y -= 2;
//!         }
//!         if buttons.contains(Button::DOWN) {
//!             player_y += 2;
//!         }
//!
//!         sprite::set(0, player_x, player_y, SpriteSize::Size1x1, 1);
//!     }
//! }
//! ```
//!
//! ## Modules
//!
//! - [`vdp`] - Video Display Processor control
//! - [`sprite`] - Sprite management
//! - [`input`] - Controller input handling
//! - [`psg`] - Programmable Sound Generator
//! - [`types`] - Common types and constants

#![no_std]
#![allow(dead_code)]

pub mod vdp;
pub mod sprite;
pub mod input;
pub mod psg;
pub mod types;

/// Convenient re-exports for common usage
///
/// Import everything you need with:
/// ```
/// use smd::prelude::*;
/// ```
pub mod prelude {
    pub use crate::vdp::{self, Color};
    pub use crate::sprite::{self, SpriteSize};
    pub use crate::input::{self, Button, Buttons};
    pub use crate::psg;
    pub use crate::types::*;
}

/// SDK version information
pub mod version {
    /// Major version number
    pub const MAJOR: u8 = 1;
    /// Minor version number
    pub const MINOR: u8 = 0;
    /// Patch version number
    pub const PATCH: u8 = 0;
    /// Version as string
    pub const STRING: &str = "1.0.0";
}

/// Initialize all SDK subsystems
///
/// Convenience function that calls:
/// - [`vdp::init()`]
/// - [`sprite::init()`]
/// - [`input::init()`]
/// - [`psg::init()`]
pub fn init() {
    vdp::init();
    sprite::init();
    input::init();
    psg::init();
}

/// Check if running on PAL system
///
/// Returns `true` if PAL (50Hz), `false` if NTSC (60Hz).
pub fn is_pal() -> bool {
    // Check version register
    unsafe {
        let version = (0xA10001 as *const u8).read_volatile();
        (version & 0x40) != 0
    }
}
