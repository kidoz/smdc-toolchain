//! YM2612 FM Synthesizer interface
//!
//! Complete YM2612 FM synthesis support for Sega Mega Drive including:
//! - Low-level register access
//! - Operator-level control
//! - Frequency and key management
//! - Pre-defined instrument patches
//!
//! # Overview
//!
//! The YM2612 is the FM synthesis chip in the Sega Genesis:
//! - 6 FM channels (0-5)
//! - 4 operators per channel
//! - 8 FM algorithms
//! - Stereo output (L/R per channel)
//! - LFO for vibrato/tremolo
//! - DAC mode on channel 6
//!
//! # Hardware Ports
//!
//! - Port 0 (0xA04000/0xA04001): Channels 0-2 + global registers
//! - Port 1 (0xA04002/0xA04003): Channels 3-5
//!
//! # Example
//!
//! ```no_run
//! use smd::ym2612;
//!
//! ym2612::init();
//! ym2612::patch::dist_guitar(0);
//! ym2612::set_freq(0, 4, ym2612::note::E);
//! ym2612::key_on(0);
//! ```

// ============================================================================
// Hardware Addresses
// ============================================================================

/// YM2612 address port 0 (channels 0-2)
pub const ADDR0: u32 = 0xA04000;
/// YM2612 data port 0
pub const DATA0: u32 = 0xA04001;
/// YM2612 address port 1 (channels 3-5)
pub const ADDR1: u32 = 0xA04002;
/// YM2612 data port 1
pub const DATA1: u32 = 0xA04003;

// ============================================================================
// Status Flags
// ============================================================================

/// YM2612 busy flag
pub const STATUS_BUSY: u8 = 0x80;
/// Maximum wait cycles for busy flag
pub const WAIT_LIMIT: u32 = 0x0400;

// ============================================================================
// Global Registers (Port 0 only)
// ============================================================================

/// Register definitions
pub mod reg {
    /// LFO enable and frequency
    pub const LFO: u8 = 0x22;
    /// Timer A MSB
    pub const TIMER_A_HI: u8 = 0x24;
    /// Timer A LSB
    pub const TIMER_A_LO: u8 = 0x25;
    /// Timer B
    pub const TIMER_B: u8 = 0x26;
    /// Timer control and Ch3 mode
    pub const TIMER_CTRL: u8 = 0x27;
    /// Key on/off control
    pub const KEY_ONOFF: u8 = 0x28;
    /// DAC data
    pub const DAC: u8 = 0x2A;
    /// DAC enable
    pub const DAC_EN: u8 = 0x2B;

    // Per-channel registers (add channel offset 0-2)
    /// Frequency LSB
    pub const FREQ_LO: u8 = 0xA0;
    /// Frequency MSB + Block
    pub const FREQ_HI: u8 = 0xA4;
    /// Algorithm + Feedback
    pub const ALGO_FB: u8 = 0xB0;
    /// Stereo + LFO sensitivity
    pub const STEREO_LFO: u8 = 0xB4;

    // Per-operator registers
    // Operator offsets: Op0=0, Op1=8, Op2=4, Op3=12
    /// Detune + Multiply
    pub const OP_DT_MUL: u8 = 0x30;
    /// Total Level (volume)
    pub const OP_TL: u8 = 0x40;
    /// Rate Scale + Attack Rate
    pub const OP_RS_AR: u8 = 0x50;
    /// AM enable + Decay 1 Rate
    pub const OP_AM_D1R: u8 = 0x60;
    /// Decay 2 Rate
    pub const OP_D2R: u8 = 0x70;
    /// Sustain Level + Release Rate
    pub const OP_D1L_RR: u8 = 0x80;
    /// SSG-EG envelope mode
    pub const OP_SSG_EG: u8 = 0x90;
}

// ============================================================================
// Algorithm Definitions
// ============================================================================

/// FM Algorithms (0-7)
pub mod algo {
    /// Algorithm 0: Serial modulation (M1->M2->M3->C)
    /// Good for: Warm bass, bells
    pub const SERIAL: u8 = 0;

    /// Algorithm 4: Three parallel carriers with modulator
    /// Good for: Piano, organ
    pub const PIANO: u8 = 4;

    /// Algorithm 5: Parallel carriers with feedback modulator
    /// Good for: Distorted guitar, heavy tones
    pub const DISTORTION: u8 = 5;

    /// Algorithm 7: All operators in parallel (additive)
    /// Good for: Organ, rich pads
    pub const ORGAN: u8 = 7;
}

// ============================================================================
// Note Frequency Table (F-number values for block 4)
// ============================================================================

/// Note frequencies (F-number values)
pub mod note {
    pub const C: u16 = 644;
    pub const CS: u16 = 682;
    pub const D: u16 = 723;
    pub const DS: u16 = 766;
    pub const E: u16 = 811;
    pub const F: u16 = 859;
    pub const FS: u16 = 910;
    pub const G: u16 = 964;
    pub const GS: u16 = 1021;
    pub const A: u16 = 1081;
    pub const AS: u16 = 1145;
    pub const B: u16 = 1214;
}

// ============================================================================
// Operator offset mapping
// ============================================================================

/// Get operator register offset
/// Operator 0,1,2,3 -> offset 0,8,4,12
#[inline]
fn op_offset(op: u8) -> u8 {
    match op {
        0 => 0,
        1 => 8,
        2 => 4,
        3 => 12,
        _ => 0,
    }
}

// ============================================================================
// Low-Level Functions
// ============================================================================

/// Read YM2612 status register
#[inline]
pub fn read_status() -> u8 {
    unsafe { (ADDR0 as *const u8).read_volatile() }
}

/// Wait until YM2612 is ready
#[inline]
pub fn wait_ready() {
    let mut spins = WAIT_LIMIT;
    while spins > 0 {
        if (read_status() & STATUS_BUSY) == 0 {
            break;
        }
        spins -= 1;
    }
}

/// Write to YM2612 port 0 (channels 0-2, global regs)
#[inline]
pub fn write_port0(reg: u8, val: u8) {
    wait_ready();
    unsafe {
        (ADDR0 as *mut u8).write_volatile(reg);
    }
    wait_ready();
    unsafe {
        (DATA0 as *mut u8).write_volatile(val);
    }
}

/// Write to YM2612 port 1 (channels 3-5)
#[inline]
pub fn write_port1(reg: u8, val: u8) {
    wait_ready();
    unsafe {
        (ADDR1 as *mut u8).write_volatile(reg);
    }
    wait_ready();
    unsafe {
        (DATA1 as *mut u8).write_volatile(val);
    }
}

// ============================================================================
// Channel Functions
// ============================================================================

/// Write to operator register for any channel
///
/// Handles operator offset mapping: op 0,1,2,3 -> offset 0,8,4,12
///
/// # Arguments
/// * `ch` - Channel (0-5)
/// * `op` - Operator (0-3)
/// * `reg` - Base register address (0x30, 0x40, etc.)
/// * `val` - Value to write
pub fn write_op(ch: u8, op: u8, reg: u8, val: u8) {
    let off = op_offset(op);
    let base_ch = if ch > 2 { ch - 3 } else { ch };
    let addr = reg + base_ch + off;

    if ch > 2 {
        write_port1(addr, val);
    } else {
        write_port0(addr, val);
    }
}

/// Key on (start note) for channel
///
/// # Arguments
/// * `ch` - Channel (0-5)
pub fn key_on(ch: u8) {
    let slot = if ch < 3 {
        0xF0 | ch
    } else {
        0xF0 | (ch - 3) | 4
    };
    write_port0(reg::KEY_ONOFF, slot);
}

/// Key off (stop note) for channel
///
/// # Arguments
/// * `ch` - Channel (0-5)
pub fn key_off(ch: u8) {
    let slot = if ch < 3 { ch } else { (ch - 3) | 4 };
    write_port0(reg::KEY_ONOFF, slot);
}

/// Set frequency for channel
///
/// # Arguments
/// * `ch` - Channel (0-5)
/// * `block` - Octave block (0-7)
/// * `fnum` - Frequency number (0-2047), use `note::*` constants
pub fn set_freq(ch: u8, block: u8, fnum: u16) {
    let reg_ch = if ch > 2 { ch - 3 } else { ch };
    let freq_h = ((block & 7) << 3) | ((fnum >> 8) as u8 & 7);
    let freq_l = (fnum & 0xFF) as u8;

    if ch < 3 {
        write_port0(reg::FREQ_HI + reg_ch, freq_h);
        write_port0(reg::FREQ_LO + reg_ch, freq_l);
    } else {
        write_port1(reg::FREQ_HI + reg_ch, freq_h);
        write_port1(reg::FREQ_LO + reg_ch, freq_l);
    }
}

/// Set algorithm and feedback for channel
///
/// # Arguments
/// * `ch` - Channel (0-5)
/// * `algorithm` - Algorithm (0-7), use `algo::*` constants
/// * `feedback` - Feedback level (0-7)
pub fn set_algorithm(ch: u8, algorithm: u8, feedback: u8) {
    let reg_ch = if ch > 2 { ch - 3 } else { ch };
    let val = ((feedback & 7) << 3) | (algorithm & 7);

    if ch < 3 {
        write_port0(reg::ALGO_FB + reg_ch, val);
    } else {
        write_port1(reg::ALGO_FB + reg_ch, val);
    }
}

/// Set stereo output for channel
///
/// # Arguments
/// * `ch` - Channel (0-5)
/// * `left` - Left speaker enable
/// * `right` - Right speaker enable
pub fn set_stereo(ch: u8, left: bool, right: bool) {
    let reg_ch = if ch > 2 { ch - 3 } else { ch };
    let val = ((left as u8) << 7) | ((right as u8) << 6);

    if ch < 3 {
        write_port0(reg::STEREO_LFO + reg_ch, val);
    } else {
        write_port1(reg::STEREO_LFO + reg_ch, val);
    }
}

/// Initialize YM2612 to default state
///
/// Disables LFO, timers, DAC, and keys off all channels.
pub fn init() {
    write_port0(reg::LFO, 0x00);       // LFO off
    write_port0(reg::TIMER_CTRL, 0x00); // Timers off
    write_port0(reg::DAC_EN, 0x00);     // DAC off

    // Key off all channels
    for ch in 0..6u8 {
        key_off(ch);
    }
}

// ============================================================================
// Instrument Patch Functions
// ============================================================================

/// Pre-defined instrument patches
pub mod patch {
    use super::*;

    /// Load distorted guitar patch
    ///
    /// Heavy metal guitar with maximum feedback.
    /// Algorithm 5, Feedback 7.
    pub fn dist_guitar(ch: u8) {
        set_algorithm(ch, algo::DISTORTION, 7);
        set_stereo(ch, true, true);

        // Op 0: Detuned modulator
        write_op(ch, 0, reg::OP_DT_MUL, 0x71);  // DT=7, MUL=1
        write_op(ch, 0, reg::OP_TL, 0x1A);
        write_op(ch, 0, reg::OP_RS_AR, 0x1F);   // AR=31
        write_op(ch, 0, reg::OP_AM_D1R, 0x0D);
        write_op(ch, 0, reg::OP_D2R, 0x02);
        write_op(ch, 0, reg::OP_D1L_RR, 0x2A);

        // Op 1: Carrier
        write_op(ch, 1, reg::OP_DT_MUL, 0x01);
        write_op(ch, 1, reg::OP_TL, 0x12);
        write_op(ch, 1, reg::OP_RS_AR, 0x1F);
        write_op(ch, 1, reg::OP_AM_D1R, 0x0A);
        write_op(ch, 1, reg::OP_D2R, 0x02);
        write_op(ch, 1, reg::OP_D1L_RR, 0x2A);

        // Op 2: Detuned carrier
        write_op(ch, 2, reg::OP_DT_MUL, 0x32);  // DT=3, MUL=2
        write_op(ch, 2, reg::OP_TL, 0x1C);
        write_op(ch, 2, reg::OP_RS_AR, 0x1F);
        write_op(ch, 2, reg::OP_AM_D1R, 0x0A);
        write_op(ch, 2, reg::OP_D2R, 0x02);
        write_op(ch, 2, reg::OP_D1L_RR, 0x2A);

        // Op 3: Carrier
        write_op(ch, 3, reg::OP_DT_MUL, 0x01);
        write_op(ch, 3, reg::OP_TL, 0x14);
        write_op(ch, 3, reg::OP_RS_AR, 0x1F);
        write_op(ch, 3, reg::OP_AM_D1R, 0x0A);
        write_op(ch, 3, reg::OP_D2R, 0x02);
        write_op(ch, 3, reg::OP_D1L_RR, 0x2A);
    }

    /// Load synth bass patch
    ///
    /// Deep bass with punch.
    /// Algorithm 0, Feedback 5.
    pub fn synth_bass(ch: u8) {
        set_algorithm(ch, algo::SERIAL, 5);
        set_stereo(ch, true, true);

        // Modulator chain
        write_op(ch, 0, reg::OP_DT_MUL, 0x01);
        write_op(ch, 0, reg::OP_TL, 0x20);
        write_op(ch, 0, reg::OP_RS_AR, 0x1F);
        write_op(ch, 0, reg::OP_AM_D1R, 0x08);
        write_op(ch, 0, reg::OP_D2R, 0x04);
        write_op(ch, 0, reg::OP_D1L_RR, 0x1A);

        write_op(ch, 1, reg::OP_DT_MUL, 0x02);
        write_op(ch, 1, reg::OP_TL, 0x28);
        write_op(ch, 1, reg::OP_RS_AR, 0x1F);
        write_op(ch, 1, reg::OP_AM_D1R, 0x0A);
        write_op(ch, 1, reg::OP_D2R, 0x05);
        write_op(ch, 1, reg::OP_D1L_RR, 0x18);

        write_op(ch, 2, reg::OP_TL, 0x7F);  // Off

        // Carrier
        write_op(ch, 3, reg::OP_DT_MUL, 0x01);
        write_op(ch, 3, reg::OP_TL, 0x08);
        write_op(ch, 3, reg::OP_RS_AR, 0x1F);
        write_op(ch, 3, reg::OP_AM_D1R, 0x06);
        write_op(ch, 3, reg::OP_D2R, 0x03);
        write_op(ch, 3, reg::OP_D1L_RR, 0x1C);
    }

    /// Load organ patch
    ///
    /// All operators in parallel for rich harmonics.
    /// Algorithm 7.
    pub fn organ(ch: u8) {
        set_algorithm(ch, algo::ORGAN, 0);
        set_stereo(ch, true, true);

        // All 4 operators output directly
        // Op 0: Fundamental
        write_op(ch, 0, reg::OP_DT_MUL, 0x01);  // MUL=1
        write_op(ch, 0, reg::OP_TL, 0x20);
        write_op(ch, 0, reg::OP_RS_AR, 0x1F);
        write_op(ch, 0, reg::OP_AM_D1R, 0x00);
        write_op(ch, 0, reg::OP_D2R, 0x00);
        write_op(ch, 0, reg::OP_D1L_RR, 0x0F);

        // Op 1: Octave
        write_op(ch, 1, reg::OP_DT_MUL, 0x02);  // MUL=2
        write_op(ch, 1, reg::OP_TL, 0x24);
        write_op(ch, 1, reg::OP_RS_AR, 0x1F);
        write_op(ch, 1, reg::OP_AM_D1R, 0x00);
        write_op(ch, 1, reg::OP_D2R, 0x00);
        write_op(ch, 1, reg::OP_D1L_RR, 0x0F);

        // Op 2: 12th
        write_op(ch, 2, reg::OP_DT_MUL, 0x03);  // MUL=3
        write_op(ch, 2, reg::OP_TL, 0x28);
        write_op(ch, 2, reg::OP_RS_AR, 0x1F);
        write_op(ch, 2, reg::OP_AM_D1R, 0x00);
        write_op(ch, 2, reg::OP_D2R, 0x00);
        write_op(ch, 2, reg::OP_D1L_RR, 0x0F);

        // Op 3: 2 octaves
        write_op(ch, 3, reg::OP_DT_MUL, 0x04);  // MUL=4
        write_op(ch, 3, reg::OP_TL, 0x2C);
        write_op(ch, 3, reg::OP_RS_AR, 0x1F);
        write_op(ch, 3, reg::OP_AM_D1R, 0x00);
        write_op(ch, 3, reg::OP_D2R, 0x00);
        write_op(ch, 3, reg::OP_D1L_RR, 0x0F);
    }

    /// Load electric piano patch
    pub fn epiano(ch: u8) {
        set_algorithm(ch, algo::PIANO, 3);
        set_stereo(ch, true, true);

        write_op(ch, 0, reg::OP_DT_MUL, 0x01);
        write_op(ch, 0, reg::OP_TL, 0x27);
        write_op(ch, 0, reg::OP_RS_AR, 0x1F);
        write_op(ch, 0, reg::OP_AM_D1R, 0x0A);
        write_op(ch, 0, reg::OP_D2R, 0x04);
        write_op(ch, 0, reg::OP_D1L_RR, 0x26);

        write_op(ch, 1, reg::OP_DT_MUL, 0x0E);
        write_op(ch, 1, reg::OP_TL, 0x1E);
        write_op(ch, 1, reg::OP_RS_AR, 0x1F);
        write_op(ch, 1, reg::OP_AM_D1R, 0x0C);
        write_op(ch, 1, reg::OP_D2R, 0x05);
        write_op(ch, 1, reg::OP_D1L_RR, 0x26);

        write_op(ch, 2, reg::OP_DT_MUL, 0x01);
        write_op(ch, 2, reg::OP_TL, 0x18);
        write_op(ch, 2, reg::OP_RS_AR, 0x1F);
        write_op(ch, 2, reg::OP_AM_D1R, 0x08);
        write_op(ch, 2, reg::OP_D2R, 0x04);
        write_op(ch, 2, reg::OP_D1L_RR, 0x26);

        write_op(ch, 3, reg::OP_DT_MUL, 0x01);
        write_op(ch, 3, reg::OP_TL, 0x14);
        write_op(ch, 3, reg::OP_RS_AR, 0x1F);
        write_op(ch, 3, reg::OP_AM_D1R, 0x08);
        write_op(ch, 3, reg::OP_D2R, 0x04);
        write_op(ch, 3, reg::OP_D1L_RR, 0x26);
    }

    /// Load strings pad patch
    pub fn strings(ch: u8) {
        set_algorithm(ch, 2, 4);
        set_stereo(ch, true, true);

        write_op(ch, 0, reg::OP_DT_MUL, 0x01);
        write_op(ch, 0, reg::OP_TL, 0x22);
        write_op(ch, 0, reg::OP_RS_AR, 0x10);  // Slow attack
        write_op(ch, 0, reg::OP_AM_D1R, 0x02);
        write_op(ch, 0, reg::OP_D2R, 0x01);
        write_op(ch, 0, reg::OP_D1L_RR, 0x14);

        write_op(ch, 1, reg::OP_DT_MUL, 0x02);
        write_op(ch, 1, reg::OP_TL, 0x26);
        write_op(ch, 1, reg::OP_RS_AR, 0x12);
        write_op(ch, 1, reg::OP_AM_D1R, 0x02);
        write_op(ch, 1, reg::OP_D2R, 0x01);
        write_op(ch, 1, reg::OP_D1L_RR, 0x14);

        write_op(ch, 2, reg::OP_DT_MUL, 0x01);
        write_op(ch, 2, reg::OP_TL, 0x1C);
        write_op(ch, 2, reg::OP_RS_AR, 0x10);
        write_op(ch, 2, reg::OP_AM_D1R, 0x02);
        write_op(ch, 2, reg::OP_D2R, 0x01);
        write_op(ch, 2, reg::OP_D1L_RR, 0x14);

        write_op(ch, 3, reg::OP_DT_MUL, 0x01);
        write_op(ch, 3, reg::OP_TL, 0x18);
        write_op(ch, 3, reg::OP_RS_AR, 0x0E);
        write_op(ch, 3, reg::OP_AM_D1R, 0x02);
        write_op(ch, 3, reg::OP_D2R, 0x01);
        write_op(ch, 3, reg::OP_D1L_RR, 0x14);
    }

    /// Load brass patch
    pub fn brass(ch: u8) {
        set_algorithm(ch, 2, 5);
        set_stereo(ch, true, true);

        write_op(ch, 0, reg::OP_DT_MUL, 0x01);
        write_op(ch, 0, reg::OP_TL, 0x1E);
        write_op(ch, 0, reg::OP_RS_AR, 0x18);
        write_op(ch, 0, reg::OP_AM_D1R, 0x06);
        write_op(ch, 0, reg::OP_D2R, 0x03);
        write_op(ch, 0, reg::OP_D1L_RR, 0x1A);

        write_op(ch, 1, reg::OP_DT_MUL, 0x01);
        write_op(ch, 1, reg::OP_TL, 0x22);
        write_op(ch, 1, reg::OP_RS_AR, 0x18);
        write_op(ch, 1, reg::OP_AM_D1R, 0x06);
        write_op(ch, 1, reg::OP_D2R, 0x03);
        write_op(ch, 1, reg::OP_D1L_RR, 0x1A);

        write_op(ch, 2, reg::OP_DT_MUL, 0x01);
        write_op(ch, 2, reg::OP_TL, 0x14);
        write_op(ch, 2, reg::OP_RS_AR, 0x1A);
        write_op(ch, 2, reg::OP_AM_D1R, 0x05);
        write_op(ch, 2, reg::OP_D2R, 0x03);
        write_op(ch, 2, reg::OP_D1L_RR, 0x1A);

        write_op(ch, 3, reg::OP_DT_MUL, 0x01);
        write_op(ch, 3, reg::OP_TL, 0x10);
        write_op(ch, 3, reg::OP_RS_AR, 0x1A);
        write_op(ch, 3, reg::OP_AM_D1R, 0x05);
        write_op(ch, 3, reg::OP_D2R, 0x03);
        write_op(ch, 3, reg::OP_D1L_RR, 0x1A);
    }

    /// Load synth lead patch
    pub fn synth_lead(ch: u8) {
        set_algorithm(ch, algo::DISTORTION, 6);
        set_stereo(ch, true, true);

        write_op(ch, 0, reg::OP_DT_MUL, 0x31);
        write_op(ch, 0, reg::OP_TL, 0x1C);
        write_op(ch, 0, reg::OP_RS_AR, 0x1F);
        write_op(ch, 0, reg::OP_AM_D1R, 0x08);
        write_op(ch, 0, reg::OP_D2R, 0x02);
        write_op(ch, 0, reg::OP_D1L_RR, 0x1F);

        write_op(ch, 1, reg::OP_DT_MUL, 0x01);
        write_op(ch, 1, reg::OP_TL, 0x14);
        write_op(ch, 1, reg::OP_RS_AR, 0x1F);
        write_op(ch, 1, reg::OP_AM_D1R, 0x06);
        write_op(ch, 1, reg::OP_D2R, 0x02);
        write_op(ch, 1, reg::OP_D1L_RR, 0x1F);

        write_op(ch, 2, reg::OP_DT_MUL, 0x02);
        write_op(ch, 2, reg::OP_TL, 0x18);
        write_op(ch, 2, reg::OP_RS_AR, 0x1F);
        write_op(ch, 2, reg::OP_AM_D1R, 0x06);
        write_op(ch, 2, reg::OP_D2R, 0x02);
        write_op(ch, 2, reg::OP_D1L_RR, 0x1F);

        write_op(ch, 3, reg::OP_DT_MUL, 0x01);
        write_op(ch, 3, reg::OP_TL, 0x10);
        write_op(ch, 3, reg::OP_RS_AR, 0x1F);
        write_op(ch, 3, reg::OP_AM_D1R, 0x06);
        write_op(ch, 3, reg::OP_D2R, 0x02);
        write_op(ch, 3, reg::OP_D1L_RR, 0x1F);
    }
}
