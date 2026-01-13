//! Z80 CPU control and sound driver interface
//!
//! The Sega Genesis has a Z80 CPU that can be used for sound processing
//! independently from the main 68000 CPU. This module provides:
//! - Z80 bus control (request/release)
//! - Z80 reset control
//! - Sound driver loading and communication
//!
//! # Architecture
//!
//! The Z80 has 8KB of RAM at 0xA00000-0xA01FFF (68000 view).
//! The 68000 must request the bus before accessing Z80 RAM.
//!
//! # Command Protocol
//!
//! Commands are sent via a shared memory area in Z80 RAM:
//! - 0xA01F00: Command byte (write last to trigger)
//! - 0xA01F01+: Command data bytes
//!
//! # Example
//!
//! ```no_run
//! use smd::z80;
//!
//! // Initialize Z80 with sound driver
//! z80::init();
//!
//! // Play a note on FM channel 0
//! z80::play_note(0, z80::Note::E, 4);
//!
//! // Stop the note
//! z80::stop_note(0);
//! ```

// ============================================================================
// Hardware Addresses (68000 view)
// ============================================================================

/// Z80 RAM start address (8KB: 0xA00000-0xA01FFF)
pub const RAM: u32 = 0xA00000;

/// Z80 RAM size in bytes
pub const RAM_SIZE: usize = 8192;

/// Z80 bus request register
pub const BUS_REQ: u32 = 0xA11100;

/// Z80 reset control register
pub const RESET: u32 = 0xA11200;

/// Command byte address in Z80 RAM
pub const CMD_ADDR: u32 = 0xA01F00;

/// Command data address in Z80 RAM
pub const DATA_ADDR: u32 = 0xA01F01;

// ============================================================================
// Driver Commands
// ============================================================================

/// Z80 driver commands
pub mod cmd {
    /// No operation / idle
    pub const NOP: u8 = 0x00;
    /// Play note: ch, note, octave
    pub const PLAY_NOTE: u8 = 0x01;
    /// Stop note: ch
    pub const STOP_NOTE: u8 = 0x02;
    /// Set patch: ch, patch_id
    pub const SET_PATCH: u8 = 0x03;
    /// Set tempo: tempo_value
    pub const SET_TEMPO: u8 = 0x04;
    /// Start sequence playback
    pub const PLAY_SEQ: u8 = 0x10;
    /// Stop sequence playback
    pub const STOP_SEQ: u8 = 0x11;
}

// ============================================================================
// Note Definitions
// ============================================================================

/// Musical note values (0-11)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Note {
    C = 0,
    Cs = 1,
    D = 2,
    Ds = 3,
    E = 4,
    F = 5,
    Fs = 6,
    G = 7,
    Gs = 8,
    A = 9,
    As = 10,
    B = 11,
}

// ============================================================================
// Bus Control Functions
// ============================================================================

/// Request Z80 bus
///
/// The 68000 must request the bus before accessing Z80 RAM.
/// This function blocks until the bus is granted.
#[inline]
pub fn request_bus() {
    unsafe {
        let bus_req = BUS_REQ as *mut u16;
        // Request bus
        bus_req.write_volatile(0x0100);
        // Wait for bus grant (bit 0 becomes 0)
        while (bus_req.read_volatile() & 0x0100) != 0 {}
    }
}

/// Release Z80 bus
///
/// Release the bus so the Z80 can continue executing.
#[inline]
pub fn release_bus() {
    unsafe {
        let bus_req = BUS_REQ as *mut u16;
        bus_req.write_volatile(0x0000);
    }
}

/// Assert Z80 reset
///
/// Holds the Z80 in reset state.
#[inline]
pub fn reset_on() {
    unsafe {
        let reset = RESET as *mut u16;
        reset.write_volatile(0x0000);
    }
}

/// Release Z80 reset
///
/// Allows the Z80 to start executing from address 0.
#[inline]
pub fn reset_off() {
    unsafe {
        let reset = RESET as *mut u16;
        reset.write_volatile(0x0100);
    }
}

// ============================================================================
// Memory Access Functions
// ============================================================================

/// Write a byte to Z80 RAM
///
/// # Safety
/// Caller must have requested the bus first.
#[inline]
pub unsafe fn write_ram(offset: u16, value: u8) {
    let addr = (RAM + offset as u32) as *mut u8;
    addr.write_volatile(value);
}

/// Read a byte from Z80 RAM
///
/// # Safety
/// Caller must have requested the bus first.
#[inline]
pub unsafe fn read_ram(offset: u16) -> u8 {
    let addr = (RAM + offset as u32) as *const u8;
    addr.read_volatile()
}

// ============================================================================
// Driver Functions
// ============================================================================

/// Load Z80 driver to Z80 RAM
///
/// Loads the sound driver binary into Z80 RAM and starts the Z80.
/// Call once at initialization.
pub fn load_driver(driver: &[u8]) {
    request_bus();
    reset_on();

    // Copy driver to Z80 RAM
    unsafe {
        for (i, &byte) in driver.iter().enumerate() {
            if i >= RAM_SIZE {
                break;
            }
            write_ram(i as u16, byte);
        }
    }

    reset_off();
    release_bus();
}

/// Initialize Z80
///
/// Resets the Z80 and prepares it for use.
/// Call this if not loading a custom driver.
pub fn init() {
    request_bus();
    reset_on();

    // Clear command area
    unsafe {
        write_ram(0x1F00, 0x00); // CMD = NOP
    }

    reset_off();
    release_bus();
}

/// Send command to Z80 driver
///
/// Writes data bytes first, then command byte to trigger processing.
///
/// # Arguments
/// * `command` - Command byte (see `cmd` module)
/// * `d1` - Data byte 1
/// * `d2` - Data byte 2
/// * `d3` - Data byte 3
pub fn send_command(command: u8, d1: u8, d2: u8, d3: u8) {
    request_bus();

    unsafe {
        // Write data first
        write_ram(0x1F01, d1);
        write_ram(0x1F02, d2);
        write_ram(0x1F03, d3);
        // Write command last (triggers processing)
        write_ram(0x1F00, command);
    }

    release_bus();
}

/// Play a note via Z80 driver
///
/// # Arguments
/// * `channel` - YM2612 channel (0-5)
/// * `note` - Note to play (C-B)
/// * `octave` - Octave (0-7)
#[inline]
pub fn play_note(channel: u8, note: Note, octave: u8) {
    send_command(cmd::PLAY_NOTE, channel, note as u8, octave);
}

/// Stop a note via Z80 driver
///
/// # Arguments
/// * `channel` - YM2612 channel (0-5)
#[inline]
pub fn stop_note(channel: u8) {
    send_command(cmd::STOP_NOTE, channel, 0, 0);
}

/// Set instrument patch via Z80 driver
///
/// # Arguments
/// * `channel` - YM2612 channel (0-5)
/// * `patch_id` - Patch identifier
#[inline]
pub fn set_patch(channel: u8, patch_id: u8) {
    send_command(cmd::SET_PATCH, channel, patch_id, 0);
}

/// Set tempo via Z80 driver
///
/// # Arguments
/// * `tempo` - Tempo value
#[inline]
pub fn set_tempo(tempo: u8) {
    send_command(cmd::SET_TEMPO, tempo, 0, 0);
}

/// Start sequence playback
#[inline]
pub fn play_sequence() {
    send_command(cmd::PLAY_SEQ, 0, 0, 0);
}

/// Stop sequence playback
#[inline]
pub fn stop_sequence() {
    send_command(cmd::STOP_SEQ, 0, 0, 0);
}
