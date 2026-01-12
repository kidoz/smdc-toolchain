//! # PSG - Programmable Sound Generator
//!
//! The Genesis includes a TI SN76489 PSG for simple sound effects.
//! It provides 3 square wave tone channels and 1 noise channel.
//!
//! ## Quick Start
//!
//! ```no_run
//! use smd::psg;
//!
//! // Play a beep
//! psg::set_tone(0, 440);    // 440 Hz on channel 0
//! psg::set_volume(0, 0);    // Max volume
//!
//! // Later: silence
//! psg::set_volume(0, volume::OFF);
//! ```
//!
//! ## Channels
//!
//! - Channels 0-2: Square wave tone generators
//! - Channel 3: Noise generator
//!
//! ## Volume
//!
//! Volume ranges from 0 (loudest) to 15 (silent).

/// PSG port address
const PSG_PORT: *mut u8 = 0xC00011 as *mut u8;

/// PSG clock frequency (NTSC)
pub const CLOCK: u32 = 3_579_545;

/// Volume level constants
pub mod volume {
    /// Maximum volume (loudest)
    pub const MAX: u8 = 0;
    /// Silent
    pub const OFF: u8 = 15;
}

/// Channel numbers
pub mod channel {
    /// Tone channel 0
    pub const TONE0: u8 = 0;
    /// Tone channel 1
    pub const TONE1: u8 = 1;
    /// Tone channel 2
    pub const TONE2: u8 = 2;
    /// Noise channel
    pub const NOISE: u8 = 3;
}

/// Noise mode configuration
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum NoiseMode {
    /// Periodic noise, high frequency
    PeriodicHi = 0x00,
    /// Periodic noise, medium frequency
    PeriodicMed = 0x01,
    /// Periodic noise, low frequency
    PeriodicLo = 0x02,
    /// Periodic noise, uses channel 2 frequency
    PeriodicCh2 = 0x03,
    /// White noise, high frequency
    WhiteHi = 0x04,
    /// White noise, medium frequency
    WhiteMed = 0x05,
    /// White noise, low frequency
    WhiteLo = 0x06,
    /// White noise, uses channel 2 frequency
    WhiteCh2 = 0x07,
}

/// Common frequencies for sound effects (Hz)
pub mod freq {
    /// Low beep
    pub const LOW: u16 = 220;
    /// Medium beep
    pub const MED: u16 = 440;
    /// High beep
    pub const HIGH: u16 = 880;
    /// Short blip
    pub const BLIP: u16 = 1760;
}

/// Musical note frequencies (Hz)
pub mod note {
    pub const C4: u16 = 262;
    pub const D4: u16 = 294;
    pub const E4: u16 = 330;
    pub const F4: u16 = 349;
    pub const G4: u16 = 392;
    pub const A4: u16 = 440;
    pub const B4: u16 = 494;
    pub const C5: u16 = 523;
}

/// Write a byte to the PSG
#[inline]
fn write(value: u8) {
    unsafe {
        PSG_PORT.write_volatile(value);
    }
}

/// Initialize PSG (silence all channels)
///
/// Call once at startup to ensure all channels are silent.
pub fn init() {
    stop();
}

/// Silence all PSG channels
///
/// Sets all 4 channels to volume 15 (silent).
pub fn stop() {
    // Silence all 4 channels
    set_volume(0, volume::OFF);
    set_volume(1, volume::OFF);
    set_volume(2, volume::OFF);
    set_volume(3, volume::OFF);
}

/// Set volume for a channel
///
/// # Arguments
/// * `channel` - Channel number (0-3)
/// * `volume` - Volume level (0=loudest, 15=silent)
///
/// # Example
/// ```no_run
/// use smd::psg;
///
/// psg::set_volume(0, 0);   // Max volume
/// psg::set_volume(0, 15);  // Silent
/// ```
pub fn set_volume(channel: u8, volume: u8) {
    let cmd = 0x90 | ((channel & 0x03) << 5) | (volume & 0x0F);
    write(cmd);
}

/// Set tone frequency for a channel
///
/// # Arguments
/// * `channel` - Channel number (0-2)
/// * `freq` - Frequency in Hz (approx. 109-125000 Hz)
///
/// # Example
/// ```no_run
/// use smd::psg;
///
/// psg::set_tone(0, 440);  // A4 note on channel 0
/// ```
pub fn set_tone(channel: u8, freq: u16) {
    if freq == 0 {
        return;
    }
    // Divider = CLOCK / (32 * freq)
    let divider = (CLOCK / (32 * freq as u32)) as u16;
    set_tone_raw(channel, divider);
}

/// Set raw tone value for a channel
///
/// # Arguments
/// * `channel` - Channel number (0-2)
/// * `value` - 10-bit divider value (0-1023)
///
/// Lower values = higher pitch.
/// Frequency = CLOCK / (32 * value)
pub fn set_tone_raw(channel: u8, value: u16) {
    let value = value & 0x03FF; // 10-bit
    let ch = (channel & 0x03) << 5;

    // First byte: 1 CC T DDDD (latch + low 4 bits)
    let byte1 = 0x80 | ch | ((value & 0x0F) as u8);
    // Second byte: 0 0 DDDDDD (high 6 bits)
    let byte2 = ((value >> 4) & 0x3F) as u8;

    write(byte1);
    write(byte2);
}

/// Set noise channel mode
///
/// # Arguments
/// * `mode` - Noise mode (type and frequency)
///
/// # Example
/// ```no_run
/// use smd::psg::{self, NoiseMode};
///
/// psg::set_noise(NoiseMode::WhiteMed);
/// psg::set_volume(3, 0);  // Enable noise at max volume
/// ```
pub fn set_noise(mode: NoiseMode) {
    let cmd = 0xE0 | (mode as u8);
    write(cmd);
}

/// Play a simple beep
///
/// Convenience function to start a tone on a channel.
///
/// # Arguments
/// * `channel` - Channel to use (0-2)
/// * `freq` - Frequency in Hz
/// * `volume` - Volume (0=loudest, 15=silent)
///
/// # Example
/// ```no_run
/// use smd::psg;
///
/// psg::beep(0, 440, 0);  // Play A4 at max volume
/// // Later...
/// psg::stop();           // Silence
/// ```
pub fn beep(channel: u8, freq: u16, volume: u8) {
    set_tone(channel, freq);
    set_volume(channel, volume);
}
