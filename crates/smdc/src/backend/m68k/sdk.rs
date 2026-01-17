//! SDK function definitions and code generation for Sega Genesis hardware
//!
//! This module provides built-in implementations for SDK functions,
//! eliminating the need for separate SDK source files.

use std::collections::{HashMap, HashSet};
use super::m68k::*;

// ============================================================================
// Hardware Addresses
// ============================================================================

/// VDP (Video Display Processor) registers
pub const VDP_DATA: u32 = 0xC00000;
pub const VDP_CTRL: u32 = 0xC00004;

/// PSG (Programmable Sound Generator) port
pub const PSG_PORT: u32 = 0xC00011;

/// YM2612 FM synthesizer ports
pub const YM_ADDR0: u32 = 0xA04000;
pub const YM_DATA0: u32 = 0xA04001;
pub const YM_ADDR1: u32 = 0xA04002;
pub const YM_DATA1: u32 = 0xA04003;

// ============================================================================
// SDK Function Classification
// ============================================================================

/// How an SDK function should be generated
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkFunctionKind {
    /// Simple function - inline at call site (1-20 instructions)
    Inline,
    /// Complex function - generate as library function (loops, state, etc.)
    Library,
}

/// SDK function category
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkCategory {
    Vdp,
    Ym2612,
    Psg,
}

/// SDK function definition
#[derive(Debug, Clone)]
pub struct SdkFunction {
    pub name: &'static str,
    pub kind: SdkFunctionKind,
    pub category: SdkCategory,
    pub param_count: usize,
    pub has_return: bool,
}

// ============================================================================
// SDK Registry
// ============================================================================

/// Registry of all SDK functions
pub struct SdkRegistry {
    functions: HashMap<&'static str, SdkFunction>,
}

impl SdkRegistry {
    pub fn new() -> Self {
        let mut functions = HashMap::new();

        // VDP Functions
        Self::register_vdp_functions(&mut functions);

        // YM2612 Functions
        Self::register_ym2612_functions(&mut functions);

        // PSG Functions
        Self::register_psg_functions(&mut functions);

        Self { functions }
    }

    pub fn lookup(&self, name: &str) -> Option<&SdkFunction> {
        self.functions.get(name)
    }

    pub fn is_sdk_function(&self, name: &str) -> bool {
        self.functions.contains_key(name)
    }

    fn register_vdp_functions(map: &mut HashMap<&'static str, SdkFunction>) {
        use SdkFunctionKind::*;
        use SdkCategory::Vdp;

        // Inline VDP functions
        map.insert("vdp_set_reg", SdkFunction { name: "vdp_set_reg", kind: Inline, category: Vdp, param_count: 2, has_return: false });
        map.insert("vdp_get_status", SdkFunction { name: "vdp_get_status", kind: Inline, category: Vdp, param_count: 0, has_return: true });
        map.insert("vdp_set_write_addr", SdkFunction { name: "vdp_set_write_addr", kind: Inline, category: Vdp, param_count: 1, has_return: false });
        map.insert("vdp_set_cram_addr", SdkFunction { name: "vdp_set_cram_addr", kind: Inline, category: Vdp, param_count: 1, has_return: false });
        map.insert("vdp_set_color", SdkFunction { name: "vdp_set_color", kind: Inline, category: Vdp, param_count: 2, has_return: false });
        map.insert("vdp_set_background", SdkFunction { name: "vdp_set_background", kind: Inline, category: Vdp, param_count: 2, has_return: false });
        map.insert("vdp_in_vblank", SdkFunction { name: "vdp_in_vblank", kind: Inline, category: Vdp, param_count: 0, has_return: true });

        // Library VDP functions
        map.insert("vdp_init", SdkFunction { name: "vdp_init", kind: Library, category: Vdp, param_count: 0, has_return: false });
        map.insert("vdp_vsync", SdkFunction { name: "vdp_vsync", kind: Library, category: Vdp, param_count: 0, has_return: false });
        map.insert("vdp_wait_vblank_start", SdkFunction { name: "vdp_wait_vblank_start", kind: Library, category: Vdp, param_count: 0, has_return: false });
        map.insert("vdp_wait_vblank_end", SdkFunction { name: "vdp_wait_vblank_end", kind: Library, category: Vdp, param_count: 0, has_return: false });
        map.insert("vdp_wait_frame", SdkFunction { name: "vdp_wait_frame", kind: Library, category: Vdp, param_count: 0, has_return: false });
        map.insert("vdp_load_palette", SdkFunction { name: "vdp_load_palette", kind: Library, category: Vdp, param_count: 3, has_return: false });
        map.insert("vdp_load_tiles", SdkFunction { name: "vdp_load_tiles", kind: Library, category: Vdp, param_count: 3, has_return: false });
        map.insert("vdp_set_tile_a", SdkFunction { name: "vdp_set_tile_a", kind: Library, category: Vdp, param_count: 3, has_return: false });
        map.insert("vdp_set_tile_b", SdkFunction { name: "vdp_set_tile_b", kind: Library, category: Vdp, param_count: 3, has_return: false });
        map.insert("vdp_clear_plane_a", SdkFunction { name: "vdp_clear_plane_a", kind: Library, category: Vdp, param_count: 0, has_return: false });
        map.insert("vdp_clear_plane_b", SdkFunction { name: "vdp_clear_plane_b", kind: Library, category: Vdp, param_count: 0, has_return: false });
        map.insert("vdp_set_hscroll_a", SdkFunction { name: "vdp_set_hscroll_a", kind: Library, category: Vdp, param_count: 1, has_return: false });
        map.insert("vdp_set_hscroll_b", SdkFunction { name: "vdp_set_hscroll_b", kind: Library, category: Vdp, param_count: 1, has_return: false });
        map.insert("vdp_set_vscroll_a", SdkFunction { name: "vdp_set_vscroll_a", kind: Library, category: Vdp, param_count: 1, has_return: false });
        map.insert("vdp_set_vscroll_b", SdkFunction { name: "vdp_set_vscroll_b", kind: Library, category: Vdp, param_count: 1, has_return: false });
        map.insert("vdp_get_frame_count", SdkFunction { name: "vdp_get_frame_count", kind: Library, category: Vdp, param_count: 0, has_return: true });
        map.insert("vdp_reset_frame_count", SdkFunction { name: "vdp_reset_frame_count", kind: Library, category: Vdp, param_count: 0, has_return: false });
    }

    fn register_ym2612_functions(map: &mut HashMap<&'static str, SdkFunction>) {
        use SdkFunctionKind::*;
        use SdkCategory::Ym2612;

        // Inline YM2612 functions
        map.insert("ym_read_status", SdkFunction { name: "ym_read_status", kind: Inline, category: Ym2612, param_count: 0, has_return: true });
        map.insert("ym_write0", SdkFunction { name: "ym_write0", kind: Inline, category: Ym2612, param_count: 2, has_return: false });
        map.insert("ym_write1", SdkFunction { name: "ym_write1", kind: Inline, category: Ym2612, param_count: 2, has_return: false });
        map.insert("ym_dac_enable", SdkFunction { name: "ym_dac_enable", kind: Inline, category: Ym2612, param_count: 0, has_return: false });
        map.insert("ym_dac_disable", SdkFunction { name: "ym_dac_disable", kind: Inline, category: Ym2612, param_count: 0, has_return: false });
        map.insert("ym_dac_write", SdkFunction { name: "ym_dac_write", kind: Inline, category: Ym2612, param_count: 1, has_return: false });

        // Library YM2612 functions
        map.insert("ym_init", SdkFunction { name: "ym_init", kind: Library, category: Ym2612, param_count: 0, has_return: false });
        map.insert("ym_reset", SdkFunction { name: "ym_reset", kind: Library, category: Ym2612, param_count: 0, has_return: false });
        map.insert("ym_wait", SdkFunction { name: "ym_wait", kind: Library, category: Ym2612, param_count: 0, has_return: false });
        map.insert("ym_write_ch", SdkFunction { name: "ym_write_ch", kind: Library, category: Ym2612, param_count: 3, has_return: false });
        map.insert("ym_write_op", SdkFunction { name: "ym_write_op", kind: Library, category: Ym2612, param_count: 4, has_return: false });
        map.insert("ym_key_on", SdkFunction { name: "ym_key_on", kind: Library, category: Ym2612, param_count: 1, has_return: false });
        map.insert("ym_key_off", SdkFunction { name: "ym_key_off", kind: Library, category: Ym2612, param_count: 1, has_return: false });
        map.insert("ym_key_on_ops", SdkFunction { name: "ym_key_on_ops", kind: Library, category: Ym2612, param_count: 2, has_return: false });
        map.insert("ym_set_freq", SdkFunction { name: "ym_set_freq", kind: Library, category: Ym2612, param_count: 3, has_return: false });
        map.insert("ym_set_freq_detune", SdkFunction { name: "ym_set_freq_detune", kind: Library, category: Ym2612, param_count: 4, has_return: false });
        map.insert("ym_set_algo", SdkFunction { name: "ym_set_algo", kind: Library, category: Ym2612, param_count: 3, has_return: false });
        map.insert("ym_set_pan", SdkFunction { name: "ym_set_pan", kind: Library, category: Ym2612, param_count: 2, has_return: false });
        map.insert("ym_set_volume", SdkFunction { name: "ym_set_volume", kind: Library, category: Ym2612, param_count: 2, has_return: false });
        map.insert("ym_set_lfo", SdkFunction { name: "ym_set_lfo", kind: Library, category: Ym2612, param_count: 1, has_return: false });
        map.insert("ym_set_lfo_sensitivity", SdkFunction { name: "ym_set_lfo_sensitivity", kind: Library, category: Ym2612, param_count: 3, has_return: false });
        map.insert("ym_load_patch", SdkFunction { name: "ym_load_patch", kind: Library, category: Ym2612, param_count: 2, has_return: false });
        map.insert("ym_load_operator", SdkFunction { name: "ym_load_operator", kind: Library, category: Ym2612, param_count: 3, has_return: false });
        map.insert("ym_dac_play", SdkFunction { name: "ym_dac_play", kind: Library, category: Ym2612, param_count: 3, has_return: false });
        map.insert("ym_set_timer_a", SdkFunction { name: "ym_set_timer_a", kind: Library, category: Ym2612, param_count: 1, has_return: false });
        map.insert("ym_set_timer_b", SdkFunction { name: "ym_set_timer_b", kind: Library, category: Ym2612, param_count: 1, has_return: false });
        map.insert("ym_start_timers", SdkFunction { name: "ym_start_timers", kind: Library, category: Ym2612, param_count: 1, has_return: false });
        map.insert("ym_stop_timers", SdkFunction { name: "ym_stop_timers", kind: Library, category: Ym2612, param_count: 0, has_return: false });
        map.insert("ym_timer_a_overflow", SdkFunction { name: "ym_timer_a_overflow", kind: Library, category: Ym2612, param_count: 0, has_return: true });
        map.insert("ym_timer_b_overflow", SdkFunction { name: "ym_timer_b_overflow", kind: Library, category: Ym2612, param_count: 0, has_return: true });

        // YM2612 Patch functions
        for patch in &[
            "ym_patch_dist_guitar", "ym_patch_palm_mute", "ym_patch_clean_guitar",
            "ym_patch_lead_guitar", "ym_patch_synth_bass", "ym_patch_elec_bass",
            "ym_patch_epiano", "ym_patch_strings", "ym_patch_brass", "ym_patch_organ",
            "ym_patch_synth_lead", "ym_patch_kick", "ym_patch_snare", "ym_patch_tom",
            "ym_patch_hihat"
        ] {
            map.insert(patch, SdkFunction { name: patch, kind: Library, category: Ym2612, param_count: 1, has_return: false });
        }

        // Vibrato functions
        map.insert("ym_vibrato_init", SdkFunction { name: "ym_vibrato_init", kind: Library, category: Ym2612, param_count: 5, has_return: false });
        map.insert("ym_vibrato_update", SdkFunction { name: "ym_vibrato_update", kind: Library, category: Ym2612, param_count: 2, has_return: false });
        map.insert("ym_pitch_bend", SdkFunction { name: "ym_pitch_bend", kind: Library, category: Ym2612, param_count: 4, has_return: true });
    }

    fn register_psg_functions(map: &mut HashMap<&'static str, SdkFunction>) {
        use SdkFunctionKind::*;
        use SdkCategory::Psg;

        // Inline PSG functions
        map.insert("psg_write", SdkFunction { name: "psg_write", kind: Inline, category: Psg, param_count: 1, has_return: false });
        map.insert("psg_set_volume", SdkFunction { name: "psg_set_volume", kind: Inline, category: Psg, param_count: 2, has_return: false });
        map.insert("psg_set_noise", SdkFunction { name: "psg_set_noise", kind: Inline, category: Psg, param_count: 1, has_return: false });
        map.insert("psg_stop_channel", SdkFunction { name: "psg_stop_channel", kind: Inline, category: Psg, param_count: 1, has_return: false });
        map.insert("psg_note_off", SdkFunction { name: "psg_note_off", kind: Inline, category: Psg, param_count: 1, has_return: false });

        // Library PSG functions
        map.insert("psg_init", SdkFunction { name: "psg_init", kind: Library, category: Psg, param_count: 0, has_return: false });
        map.insert("psg_set_tone", SdkFunction { name: "psg_set_tone", kind: Library, category: Psg, param_count: 2, has_return: false });
        map.insert("psg_set_freq", SdkFunction { name: "psg_set_freq", kind: Library, category: Psg, param_count: 2, has_return: false });
        map.insert("psg_stop", SdkFunction { name: "psg_stop", kind: Library, category: Psg, param_count: 0, has_return: false });
        map.insert("psg_beep", SdkFunction { name: "psg_beep", kind: Library, category: Psg, param_count: 3, has_return: false });
        map.insert("psg_note_on", SdkFunction { name: "psg_note_on", kind: Library, category: Psg, param_count: 3, has_return: false });
        map.insert("psg_hihat", SdkFunction { name: "psg_hihat", kind: Library, category: Psg, param_count: 1, has_return: false });
        map.insert("psg_snare_noise", SdkFunction { name: "psg_snare_noise", kind: Library, category: Psg, param_count: 1, has_return: false });
        map.insert("psg_kick", SdkFunction { name: "psg_kick", kind: Library, category: Psg, param_count: 1, has_return: false });
        map.insert("psg_cymbal", SdkFunction { name: "psg_cymbal", kind: Library, category: Psg, param_count: 1, has_return: false });
        map.insert("psg_env_init", SdkFunction { name: "psg_env_init", kind: Library, category: Psg, param_count: 2, has_return: false });
        map.insert("psg_env_attack", SdkFunction { name: "psg_env_attack", kind: Library, category: Psg, param_count: 3, has_return: false });
        map.insert("psg_env_release", SdkFunction { name: "psg_env_release", kind: Library, category: Psg, param_count: 2, has_return: false });
        map.insert("psg_env_update", SdkFunction { name: "psg_env_update", kind: Library, category: Psg, param_count: 1, has_return: true });
    }
}

impl Default for SdkRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Inline Code Generator
// ============================================================================

/// Generates inline M68k instructions for simple SDK functions
pub struct SdkInlineGenerator;

impl SdkInlineGenerator {
    /// Generate inline code for a function call
    /// Arguments are expected to be loaded in D0, D1, D2, D3 in order
    pub fn generate(func_name: &str) -> Vec<M68kInst> {
        match func_name {
            // VDP inline functions
            "vdp_set_reg" => Self::gen_vdp_set_reg(),
            "vdp_get_status" => Self::gen_vdp_get_status(),
            "vdp_set_write_addr" => Self::gen_vdp_set_write_addr(),
            "vdp_set_cram_addr" => Self::gen_vdp_set_cram_addr(),
            "vdp_set_color" => Self::gen_vdp_set_color(),
            "vdp_set_background" => Self::gen_vdp_set_background(),
            "vdp_in_vblank" => Self::gen_vdp_in_vblank(),

            // YM2612 inline functions
            "ym_read_status" => Self::gen_ym_read_status(),
            "ym_write0" => Self::gen_ym_write0(),
            "ym_write1" => Self::gen_ym_write1(),
            "ym_dac_enable" => Self::gen_ym_dac_enable(),
            "ym_dac_disable" => Self::gen_ym_dac_disable(),
            "ym_dac_write" => Self::gen_ym_dac_write(),

            // PSG inline functions
            "psg_write" => Self::gen_psg_write(),
            "psg_set_volume" => Self::gen_psg_set_volume(),
            "psg_set_noise" => Self::gen_psg_set_noise(),
            "psg_stop_channel" => Self::gen_psg_stop_channel(),
            "psg_note_off" => Self::gen_psg_note_off(),

            _ => panic!("Not an inline function: {}", func_name),
        }
    }

    // -------------------------------------------------------------------------
    // VDP Inline Functions
    // -------------------------------------------------------------------------

    /// vdp_set_reg(reg, value) -> *VDP_CTRL = 0x8000 | (reg << 8) | value
    /// Args: D0 = reg, D1 = value
    fn gen_vdp_set_reg() -> Vec<M68kInst> {
        vec![
            // reg << 8
            M68kInst::Lsl(Size::Word, Operand::Imm(8), DataReg::D0),
            // reg | value
            M68kInst::Or(Size::Word, Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D0)),
            // | 0x8000
            M68kInst::Ori(Size::Word, 0x8000, Operand::DataReg(DataReg::D0)),
            // Write to VDP control port
            M68kInst::Move(Size::Word, Operand::DataReg(DataReg::D0), Operand::AbsLong(VDP_CTRL)),
        ]
    }

    /// vdp_get_status() -> return *VDP_CTRL
    fn gen_vdp_get_status() -> Vec<M68kInst> {
        vec![
            M68kInst::Move(Size::Word, Operand::AbsLong(VDP_CTRL), Operand::DataReg(DataReg::D0)),
        ]
    }

    /// vdp_set_write_addr(addr) -> write address command to VDP_CTRL
    /// Args: D0 = addr
    fn gen_vdp_set_write_addr() -> Vec<M68kInst> {
        vec![
            // First word: 0x4000 | (addr & 0x3FFF)
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D0), Operand::DataReg(DataReg::D1)),
            M68kInst::Andi(Size::Word, 0x3FFF, Operand::DataReg(DataReg::D1)),
            M68kInst::Ori(Size::Word, 0x4000, Operand::DataReg(DataReg::D1)),
            M68kInst::Move(Size::Word, Operand::DataReg(DataReg::D1), Operand::AbsLong(VDP_CTRL)),
            // Second word: (addr >> 14) & 0x03
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Word, Operand::DataReg(DataReg::D0), Operand::AbsLong(VDP_CTRL)),
        ]
    }

    /// vdp_set_cram_addr(index) -> set CRAM write address
    /// Args: D0 = index
    fn gen_vdp_set_cram_addr() -> Vec<M68kInst> {
        vec![
            // addr = index * 2
            M68kInst::Add(Size::Long, Operand::DataReg(DataReg::D0), Operand::DataReg(DataReg::D0)),
            // First word: 0xC000 | (addr & 0x3FFF)
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D0), Operand::DataReg(DataReg::D1)),
            M68kInst::Andi(Size::Word, 0x3FFF, Operand::DataReg(DataReg::D1)),
            M68kInst::Ori(Size::Word, 0xC000_u16 as i32, Operand::DataReg(DataReg::D1)),
            M68kInst::Move(Size::Word, Operand::DataReg(DataReg::D1), Operand::AbsLong(VDP_CTRL)),
            // Second word: (addr >> 14) & 0x03
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Word, Operand::DataReg(DataReg::D0), Operand::AbsLong(VDP_CTRL)),
        ]
    }

    /// vdp_set_color(index, color) -> set CRAM addr then write color
    /// Args: D0 = index, D1 = color
    fn gen_vdp_set_color() -> Vec<M68kInst> {
        let mut insts = vec![
            // Save color
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D2)),
        ];
        // Set CRAM address
        insts.extend(Self::gen_vdp_set_cram_addr());
        // Write color
        insts.push(M68kInst::Move(Size::Word, Operand::DataReg(DataReg::D2), Operand::AbsLong(VDP_DATA)));
        insts
    }

    /// vdp_set_background(palette, color)
    /// Args: D0 = palette, D1 = color
    fn gen_vdp_set_background() -> Vec<M68kInst> {
        vec![
            // ((palette & 0x03) << 4) | (color & 0x0F)
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Lsl(Size::Word, Operand::Imm(4), DataReg::D0),
            M68kInst::Andi(Size::Word, 0x0F, Operand::DataReg(DataReg::D1)),
            M68kInst::Or(Size::Word, Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D0)),
            // Set as register 7 value
            M68kInst::Ori(Size::Word, 0x8700, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Word, Operand::DataReg(DataReg::D0), Operand::AbsLong(VDP_CTRL)),
        ]
    }

    /// vdp_in_vblank() -> return (*VDP_CTRL & 0x0008) != 0
    fn gen_vdp_in_vblank() -> Vec<M68kInst> {
        vec![
            M68kInst::Move(Size::Word, Operand::AbsLong(VDP_CTRL), Operand::DataReg(DataReg::D0)),
            M68kInst::Andi(Size::Word, 0x0008, Operand::DataReg(DataReg::D0)),
            // Convert to 0 or 1
            M68kInst::Scc(Cond::Ne, Operand::DataReg(DataReg::D0)),
            M68kInst::Andi(Size::Long, 1, Operand::DataReg(DataReg::D0)),
        ]
    }

    // -------------------------------------------------------------------------
    // YM2612 Inline Functions
    // -------------------------------------------------------------------------

    /// ym_read_status() -> return *YM_ADDR0
    fn gen_ym_read_status() -> Vec<M68kInst> {
        vec![
            M68kInst::Move(Size::Byte, Operand::AbsLong(YM_ADDR0), Operand::DataReg(DataReg::D0)),
            M68kInst::Andi(Size::Long, 0xFF, Operand::DataReg(DataReg::D0)),
        ]
    }

    /// Helper to emit YM timing delay (12 NOPs for ~83 cycles)
    fn emit_ym_delay() -> Vec<M68kInst> {
        vec![
            M68kInst::Nop, M68kInst::Nop, M68kInst::Nop, M68kInst::Nop,
            M68kInst::Nop, M68kInst::Nop, M68kInst::Nop, M68kInst::Nop,
            M68kInst::Nop, M68kInst::Nop, M68kInst::Nop, M68kInst::Nop,
        ]
    }

    /// ym_write0(reg, val) -> write to YM port 0 with timing
    /// Args: D0 = reg, D1 = val
    fn gen_ym_write0() -> Vec<M68kInst> {
        let mut insts = vec![
            M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D0), Operand::AbsLong(YM_ADDR0)),
        ];
        insts.extend(Self::emit_ym_delay());
        insts.push(M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D1), Operand::AbsLong(YM_DATA0)));
        insts.extend(Self::emit_ym_delay());
        insts
    }

    /// ym_write1(reg, val) -> write to YM port 1 with timing
    /// Args: D0 = reg, D1 = val
    fn gen_ym_write1() -> Vec<M68kInst> {
        let mut insts = vec![
            M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D0), Operand::AbsLong(YM_ADDR1)),
        ];
        insts.extend(Self::emit_ym_delay());
        insts.push(M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D1), Operand::AbsLong(YM_DATA1)));
        insts.extend(Self::emit_ym_delay());
        insts
    }

    /// ym_dac_enable() -> write 0x80 to register 0x2B
    fn gen_ym_dac_enable() -> Vec<M68kInst> {
        let mut insts = vec![
            M68kInst::Moveq(0x2B, DataReg::D0),
            M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D0), Operand::AbsLong(YM_ADDR0)),
        ];
        insts.extend(Self::emit_ym_delay());
        insts.push(M68kInst::Move(Size::Byte, Operand::Imm(0x80), Operand::AbsLong(YM_DATA0)));
        insts.extend(Self::emit_ym_delay());
        insts
    }

    /// ym_dac_disable() -> write 0x00 to register 0x2B
    fn gen_ym_dac_disable() -> Vec<M68kInst> {
        let mut insts = vec![
            M68kInst::Moveq(0x2B, DataReg::D0),
            M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D0), Operand::AbsLong(YM_ADDR0)),
        ];
        insts.extend(Self::emit_ym_delay());
        insts.push(M68kInst::Clr(Size::Byte, Operand::AbsLong(YM_DATA0)));
        insts.extend(Self::emit_ym_delay());
        insts
    }

    /// ym_dac_write(sample) -> write sample to DAC register
    /// Args: D0 = sample
    fn gen_ym_dac_write() -> Vec<M68kInst> {
        let mut insts = vec![
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D0), Operand::DataReg(DataReg::D1)),
            M68kInst::Moveq(0x2A, DataReg::D0),
            M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D0), Operand::AbsLong(YM_ADDR0)),
        ];
        insts.extend(Self::emit_ym_delay());
        insts.push(M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D1), Operand::AbsLong(YM_DATA0)));
        insts
    }

    // -------------------------------------------------------------------------
    // PSG Inline Functions
    // -------------------------------------------------------------------------

    /// psg_write(value) -> *PSG_PORT = value
    /// Args: D0 = value
    fn gen_psg_write() -> Vec<M68kInst> {
        vec![
            M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D0), Operand::AbsLong(PSG_PORT)),
        ]
    }

    /// psg_set_volume(channel, volume)
    /// Args: D0 = channel, D1 = volume
    fn gen_psg_set_volume() -> Vec<M68kInst> {
        vec![
            // Clamp volume to 0-15
            M68kInst::Andi(Size::Word, 0x0F, Operand::DataReg(DataReg::D1)),
            // Build latch byte: 0x90 | (channel << 5) | volume
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Lsl(Size::Word, Operand::Imm(5), DataReg::D0),
            M68kInst::Or(Size::Word, Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D0)),
            M68kInst::Ori(Size::Word, 0x90, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D0), Operand::AbsLong(PSG_PORT)),
        ]
    }

    /// psg_set_noise(mode)
    /// Args: D0 = mode
    fn gen_psg_set_noise() -> Vec<M68kInst> {
        vec![
            // Build latch byte: 0xE0 | (mode & 0x07)
            M68kInst::Andi(Size::Word, 0x07, Operand::DataReg(DataReg::D0)),
            M68kInst::Ori(Size::Word, 0xE0, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D0), Operand::AbsLong(PSG_PORT)),
        ]
    }

    /// psg_stop_channel(channel) -> set volume to 15
    /// Args: D0 = channel
    fn gen_psg_stop_channel() -> Vec<M68kInst> {
        vec![
            // Build latch byte: 0x9F | (channel << 5)
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Lsl(Size::Word, Operand::Imm(5), DataReg::D0),
            M68kInst::Ori(Size::Word, 0x9F, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D0), Operand::AbsLong(PSG_PORT)),
        ]
    }

    /// psg_note_off(channel) -> set volume to 15
    /// Args: D0 = channel
    fn gen_psg_note_off() -> Vec<M68kInst> {
        Self::gen_psg_stop_channel()
    }
}

// ============================================================================
// Library Code Generator
// ============================================================================

/// Generates full M68k function bodies for complex SDK functions
pub struct SdkLibraryGenerator {
    label_counter: u32,
}

impl SdkLibraryGenerator {
    pub fn new() -> Self {
        Self { label_counter: 0 }
    }

    fn next_label(&mut self, prefix: &str) -> String {
        let label = format!(".sdk_{}_{}", prefix, self.label_counter);
        self.label_counter += 1;
        label
    }

    /// Generate a complete function with prologue/epilogue
    pub fn generate(&mut self, func_name: &str) -> Vec<M68kInst> {
        match func_name {
            // VDP library functions
            "vdp_init" => self.gen_vdp_init(),
            "vdp_vsync" => self.gen_vdp_vsync(),
            "vdp_wait_vblank_start" => self.gen_vdp_wait_vblank_start(),
            "vdp_wait_vblank_end" => self.gen_vdp_wait_vblank_end(),
            "vdp_wait_frame" => self.gen_vdp_wait_frame(),
            "vdp_load_palette" => self.gen_vdp_load_palette(),
            "vdp_load_tiles" => self.gen_vdp_load_tiles(),
            "vdp_set_tile_a" => self.gen_vdp_set_tile_a(),
            "vdp_set_tile_b" => self.gen_vdp_set_tile_b(),
            "vdp_clear_plane_a" => self.gen_vdp_clear_plane_a(),
            "vdp_clear_plane_b" => self.gen_vdp_clear_plane_b(),
            "vdp_set_hscroll_a" => self.gen_vdp_set_hscroll_a(),
            "vdp_set_hscroll_b" => self.gen_vdp_set_hscroll_b(),
            "vdp_set_vscroll_a" => self.gen_vdp_set_vscroll_a(),
            "vdp_set_vscroll_b" => self.gen_vdp_set_vscroll_b(),
            "vdp_get_frame_count" => self.gen_vdp_get_frame_count(),
            "vdp_reset_frame_count" => self.gen_vdp_reset_frame_count(),

            // YM2612 library functions
            "ym_init" => self.gen_ym_init(),
            "ym_reset" => self.gen_ym_reset(),
            "ym_wait" => self.gen_ym_wait(),
            "ym_write_ch" => self.gen_ym_write_ch(),
            "ym_write_op" => self.gen_ym_write_op(),
            "ym_key_on" => self.gen_ym_key_on(),
            "ym_key_off" => self.gen_ym_key_off(),
            "ym_key_on_ops" => self.gen_ym_key_on_ops(),
            "ym_set_freq" => self.gen_ym_set_freq(),
            "ym_set_algo" => self.gen_ym_set_algo(),
            "ym_set_pan" => self.gen_ym_set_pan(),
            "ym_set_volume" => self.gen_ym_set_volume(),
            "ym_set_lfo" => self.gen_ym_set_lfo(),

            // PSG library functions
            "psg_init" => self.gen_psg_init(),
            "psg_set_tone" => self.gen_psg_set_tone(),
            "psg_set_freq" => self.gen_psg_set_freq(),
            "psg_stop" => self.gen_psg_stop(),
            "psg_beep" => self.gen_psg_beep(),
            "psg_note_on" => self.gen_psg_note_on(),

            _ => {
                // For unimplemented functions, generate a stub
                vec![
                    M68kInst::Label(func_name.to_string()),
                    M68kInst::Comment(format!("TODO: implement {}", func_name)),
                    M68kInst::Rts,
                ]
            }
        }
    }

    // -------------------------------------------------------------------------
    // VDP Library Functions
    // -------------------------------------------------------------------------

    fn gen_vdp_init(&mut self) -> Vec<M68kInst> {
        let mut insts = vec![
            M68kInst::Label("vdp_init".to_string()),
            M68kInst::Lea(Operand::AbsLong(VDP_CTRL), AddrReg::A0),
        ];

        // VDP register initialization table
        let regs: [(i32, i32); 15] = [
            (0x00, 0x04), (0x01, 0x44), (0x02, 0x30), (0x03, 0x3C),
            (0x04, 0x07), (0x05, 0x78), (0x07, 0x00), (0x0A, 0x00),
            (0x0B, 0x00), (0x0C, 0x81), (0x0D, 0x3F), (0x0F, 0x02),
            (0x10, 0x01), (0x11, 0x00), (0x12, 0x00),
        ];

        for (reg, val) in regs {
            let cmd = 0x8000 | (reg << 8) | val;
            insts.push(M68kInst::Move(
                Size::Word,
                Operand::Imm(cmd),
                Operand::AddrInd(AddrReg::A0)
            ));
        }

        // Reset frame counter
        insts.extend([
            M68kInst::Lea(Operand::Label("__sdk_frame_count".to_string()), AddrReg::A0),
            M68kInst::Clr(Size::Long, Operand::AddrInd(AddrReg::A0)),
            M68kInst::Rts,
        ]);

        insts
    }

    fn gen_vdp_vsync(&mut self) -> Vec<M68kInst> {
        vec![
            M68kInst::Label("vdp_vsync".to_string()),
            M68kInst::Bra("vdp_wait_vblank_start".to_string()),
        ]
    }

    fn gen_vdp_wait_vblank_start(&mut self) -> Vec<M68kInst> {
        let wait_not = self.next_label("vwvs_not");
        let wait_in = self.next_label("vwvs_in");

        vec![
            M68kInst::Label("vdp_wait_vblank_start".to_string()),
            M68kInst::Lea(Operand::AbsLong(VDP_CTRL), AddrReg::A0),
            // Wait until NOT in VBlank
            M68kInst::Label(wait_not.clone()),
            M68kInst::Btst(Operand::Imm(3), Operand::AddrInd(AddrReg::A0)),
            M68kInst::Bcc(Cond::Ne, wait_not),
            // Wait until IN VBlank
            M68kInst::Label(wait_in.clone()),
            M68kInst::Btst(Operand::Imm(3), Operand::AddrInd(AddrReg::A0)),
            M68kInst::Bcc(Cond::Eq, wait_in),
            // Increment frame counter
            M68kInst::Lea(Operand::Label("__sdk_frame_count".to_string()), AddrReg::A0),
            M68kInst::Addq(Size::Long, 1, Operand::AddrInd(AddrReg::A0)),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_wait_vblank_end(&mut self) -> Vec<M68kInst> {
        let wait_in = self.next_label("vwve_in");
        let wait_out = self.next_label("vwve_out");

        vec![
            M68kInst::Label("vdp_wait_vblank_end".to_string()),
            M68kInst::Lea(Operand::AbsLong(VDP_CTRL), AddrReg::A0),
            // Wait until IN VBlank
            M68kInst::Label(wait_in.clone()),
            M68kInst::Btst(Operand::Imm(3), Operand::AddrInd(AddrReg::A0)),
            M68kInst::Bcc(Cond::Eq, wait_in),
            // Wait until out of VBlank
            M68kInst::Label(wait_out.clone()),
            M68kInst::Btst(Operand::Imm(3), Operand::AddrInd(AddrReg::A0)),
            M68kInst::Bcc(Cond::Ne, wait_out),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_wait_frame(&mut self) -> Vec<M68kInst> {
        vec![
            M68kInst::Label("vdp_wait_frame".to_string()),
            M68kInst::Bsr("vdp_wait_vblank_start".to_string()),
            M68kInst::Bra("vdp_wait_vblank_end".to_string()),
        ]
    }

    fn gen_vdp_load_palette(&mut self) -> Vec<M68kInst> {
        // Args: 8(a6)=index, 12(a6)=colors, 16(a6)=count
        let loop_label = self.next_label("vlp_loop");

        vec![
            M68kInst::Label("vdp_load_palette".to_string()),
            M68kInst::Link(AddrReg::A6, 0),
            // Set CRAM address
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A6), Operand::DataReg(DataReg::D0)),
            M68kInst::Add(Size::Long, Operand::DataReg(DataReg::D0), Operand::DataReg(DataReg::D0)),
            M68kInst::Andi(Size::Word, 0x3FFF, Operand::DataReg(DataReg::D0)),
            M68kInst::Ori(Size::Word, 0xC000_u16 as i32, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Word, Operand::DataReg(DataReg::D0), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Clr(Size::Word, Operand::AbsLong(VDP_CTRL)),
            // Load colors pointer and count
            M68kInst::Move(Size::Long, Operand::Disp(12, AddrReg::A6), Operand::AddrReg(AddrReg::A0)),
            M68kInst::Move(Size::Long, Operand::Disp(16, AddrReg::A6), Operand::DataReg(DataReg::D1)),
            M68kInst::Subq(Size::Long, 1, Operand::DataReg(DataReg::D1)),
            M68kInst::Bcc(Cond::Mi, ".vlp_done".to_string()),
            // Loop
            M68kInst::Label(loop_label.clone()),
            M68kInst::Move(Size::Word, Operand::PostInc(AddrReg::A0), Operand::AbsLong(VDP_DATA)),
            M68kInst::Dbf(DataReg::D1, loop_label),
            M68kInst::Label(".vlp_done".to_string()),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_load_tiles(&mut self) -> Vec<M68kInst> {
        // Args: 8(a6)=tiles, 12(a6)=index, 16(a6)=count
        let loop_label = self.next_label("vlt_loop");

        vec![
            M68kInst::Label("vdp_load_tiles".to_string()),
            M68kInst::Link(AddrReg::A6, 0),
            // Calculate VRAM address: index * 32
            M68kInst::Move(Size::Long, Operand::Disp(12, AddrReg::A6), Operand::DataReg(DataReg::D0)),
            M68kInst::Lsl(Size::Long, Operand::Imm(5), DataReg::D0),
            // Set write address
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D0), Operand::DataReg(DataReg::D1)),
            M68kInst::Andi(Size::Word, 0x3FFF, Operand::DataReg(DataReg::D1)),
            M68kInst::Ori(Size::Word, 0x4000, Operand::DataReg(DataReg::D1)),
            M68kInst::Move(Size::Word, Operand::DataReg(DataReg::D1), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Word, Operand::DataReg(DataReg::D0), Operand::AbsLong(VDP_CTRL)),
            // Load tiles pointer
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A6), Operand::AddrReg(AddrReg::A0)),
            // count * 16 words per tile
            M68kInst::Move(Size::Long, Operand::Disp(16, AddrReg::A6), Operand::DataReg(DataReg::D1)),
            M68kInst::Lsl(Size::Long, Operand::Imm(4), DataReg::D1),
            M68kInst::Subq(Size::Long, 1, Operand::DataReg(DataReg::D1)),
            M68kInst::Bcc(Cond::Mi, ".vlt_done".to_string()),
            // Loop
            M68kInst::Label(loop_label.clone()),
            M68kInst::Move(Size::Word, Operand::PostInc(AddrReg::A0), Operand::AbsLong(VDP_DATA)),
            M68kInst::Dbf(DataReg::D1, loop_label),
            M68kInst::Label(".vlt_done".to_string()),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_set_tile_a(&mut self) -> Vec<M68kInst> {
        // Args: 8(a6)=x, 12(a6)=y, 16(a6)=tile
        vec![
            M68kInst::Label("vdp_set_tile_a".to_string()),
            M68kInst::Link(AddrReg::A6, 0),
            // addr = VRAM_PLANE_A + (y * 128) + (x * 2)
            // VRAM_PLANE_A = 0xC000
            M68kInst::Move(Size::Long, Operand::Disp(12, AddrReg::A6), Operand::DataReg(DataReg::D0)),
            M68kInst::Lsl(Size::Long, Operand::Imm(7), DataReg::D0),
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A6), Operand::DataReg(DataReg::D1)),
            M68kInst::Add(Size::Long, Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D0)),
            M68kInst::Add(Size::Long, Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D0)),
            M68kInst::Addi(Size::Long, 0xC000, Operand::DataReg(DataReg::D0)),
            // Set write address
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D0), Operand::DataReg(DataReg::D1)),
            M68kInst::Andi(Size::Word, 0x3FFF, Operand::DataReg(DataReg::D1)),
            M68kInst::Ori(Size::Word, 0x4000, Operand::DataReg(DataReg::D1)),
            M68kInst::Move(Size::Word, Operand::DataReg(DataReg::D1), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Word, Operand::DataReg(DataReg::D0), Operand::AbsLong(VDP_CTRL)),
            // Write tile
            M68kInst::Move(Size::Word, Operand::Disp(16, AddrReg::A6), Operand::AbsLong(VDP_DATA)),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_set_tile_b(&mut self) -> Vec<M68kInst> {
        // Same as tile_a but with VRAM_PLANE_B = 0xE000
        vec![
            M68kInst::Label("vdp_set_tile_b".to_string()),
            M68kInst::Link(AddrReg::A6, 0),
            M68kInst::Move(Size::Long, Operand::Disp(12, AddrReg::A6), Operand::DataReg(DataReg::D0)),
            M68kInst::Lsl(Size::Long, Operand::Imm(7), DataReg::D0),
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A6), Operand::DataReg(DataReg::D1)),
            M68kInst::Add(Size::Long, Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D0)),
            M68kInst::Add(Size::Long, Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D0)),
            M68kInst::Addi(Size::Long, 0xE000, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D0), Operand::DataReg(DataReg::D1)),
            M68kInst::Andi(Size::Word, 0x3FFF, Operand::DataReg(DataReg::D1)),
            M68kInst::Ori(Size::Word, 0x4000, Operand::DataReg(DataReg::D1)),
            M68kInst::Move(Size::Word, Operand::DataReg(DataReg::D1), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Word, Operand::DataReg(DataReg::D0), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(Size::Word, Operand::Disp(16, AddrReg::A6), Operand::AbsLong(VDP_DATA)),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_clear_plane_a(&mut self) -> Vec<M68kInst> {
        let loop_label = self.next_label("vcpa_loop");

        vec![
            M68kInst::Label("vdp_clear_plane_a".to_string()),
            // Set write address to VRAM_PLANE_A (0xC000)
            M68kInst::Move(Size::Word, Operand::Imm(0x4000), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(Size::Word, Operand::Imm(0x0003), Operand::AbsLong(VDP_CTRL)),
            // Clear 2048 words
            M68kInst::Move(Size::Word, Operand::Imm(2047), Operand::DataReg(DataReg::D0)),
            M68kInst::Label(loop_label.clone()),
            M68kInst::Clr(Size::Word, Operand::AbsLong(VDP_DATA)),
            M68kInst::Dbf(DataReg::D0, loop_label),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_clear_plane_b(&mut self) -> Vec<M68kInst> {
        let loop_label = self.next_label("vcpb_loop");

        vec![
            M68kInst::Label("vdp_clear_plane_b".to_string()),
            // Set write address to VRAM_PLANE_B (0xE000)
            M68kInst::Move(Size::Word, Operand::Imm(0x6000), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(Size::Word, Operand::Imm(0x0003), Operand::AbsLong(VDP_CTRL)),
            // Clear 2048 words
            M68kInst::Move(Size::Word, Operand::Imm(2047), Operand::DataReg(DataReg::D0)),
            M68kInst::Label(loop_label.clone()),
            M68kInst::Clr(Size::Word, Operand::AbsLong(VDP_DATA)),
            M68kInst::Dbf(DataReg::D0, loop_label),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_set_hscroll_a(&mut self) -> Vec<M68kInst> {
        // VRAM_HSCROLL = 0xFC00
        vec![
            M68kInst::Label("vdp_set_hscroll_a".to_string()),
            M68kInst::Move(Size::Word, Operand::Imm(0x7C00), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(Size::Word, Operand::Imm(0x0003), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(Size::Word, Operand::Disp(4, AddrReg::A7), Operand::AbsLong(VDP_DATA)),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_set_hscroll_b(&mut self) -> Vec<M68kInst> {
        vec![
            M68kInst::Label("vdp_set_hscroll_b".to_string()),
            M68kInst::Move(Size::Word, Operand::Imm(0x7C02), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(Size::Word, Operand::Imm(0x0003), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(Size::Word, Operand::Disp(4, AddrReg::A7), Operand::AbsLong(VDP_DATA)),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_set_vscroll_a(&mut self) -> Vec<M68kInst> {
        vec![
            M68kInst::Label("vdp_set_vscroll_a".to_string()),
            M68kInst::Move(Size::Word, Operand::Imm(0x4000), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(Size::Word, Operand::Imm(0x0010), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(Size::Word, Operand::Disp(4, AddrReg::A7), Operand::AbsLong(VDP_DATA)),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_set_vscroll_b(&mut self) -> Vec<M68kInst> {
        vec![
            M68kInst::Label("vdp_set_vscroll_b".to_string()),
            M68kInst::Move(Size::Word, Operand::Imm(0x4002), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(Size::Word, Operand::Imm(0x0010), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(Size::Word, Operand::Disp(4, AddrReg::A7), Operand::AbsLong(VDP_DATA)),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_get_frame_count(&mut self) -> Vec<M68kInst> {
        vec![
            M68kInst::Label("vdp_get_frame_count".to_string()),
            M68kInst::Move(Size::Long, Operand::Label("__sdk_frame_count".to_string()), Operand::DataReg(DataReg::D0)),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_reset_frame_count(&mut self) -> Vec<M68kInst> {
        vec![
            M68kInst::Label("vdp_reset_frame_count".to_string()),
            M68kInst::Clr(Size::Long, Operand::Label("__sdk_frame_count".to_string())),
            M68kInst::Rts,
        ]
    }

    // -------------------------------------------------------------------------
    // YM2612 Library Functions
    // -------------------------------------------------------------------------

    fn gen_ym_init(&mut self) -> Vec<M68kInst> {
        let keyoff_loop = self.next_label("ymi_keyoff");
        let tl_loop_ch = self.next_label("ymi_tl_ch");
        let tl_loop_op = self.next_label("ymi_tl_op");

        vec![
            M68kInst::Label("ym_init".to_string()),
            M68kInst::Link(AddrReg::A6, -8),
            // Disable LFO
            M68kInst::Moveq(0x22, DataReg::D0),
            M68kInst::Clr(Size::Long, Operand::DataReg(DataReg::D1)),
            M68kInst::Bsr("ym_write0".to_string()),
            // Disable timer control
            M68kInst::Moveq(0x27, DataReg::D0),
            M68kInst::Clr(Size::Long, Operand::DataReg(DataReg::D1)),
            M68kInst::Bsr("ym_write0".to_string()),
            // Disable DAC
            M68kInst::Moveq(0x2B, DataReg::D0),
            M68kInst::Clr(Size::Long, Operand::DataReg(DataReg::D1)),
            M68kInst::Bsr("ym_write0".to_string()),
            // Key off all channels
            M68kInst::Clr(Size::Long, Operand::DataReg(DataReg::D7)),
            M68kInst::Label(keyoff_loop.clone()),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D7), Operand::DataReg(DataReg::D0)),
            M68kInst::Bsr("ym_key_off".to_string()),
            M68kInst::Addq(Size::Long, 1, Operand::DataReg(DataReg::D7)),
            M68kInst::Cmpi(Size::Long, 6, Operand::DataReg(DataReg::D7)),
            M68kInst::Bcc(Cond::Lt, keyoff_loop),
            // Set all TL to 0x7F (max attenuation)
            M68kInst::Clr(Size::Long, Operand::DataReg(DataReg::D7)), // channel
            M68kInst::Label(tl_loop_ch.clone()),
            M68kInst::Clr(Size::Long, Operand::DataReg(DataReg::D6)), // operator
            M68kInst::Label(tl_loop_op.clone()),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D7), Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D6), Operand::DataReg(DataReg::D1)),
            M68kInst::Moveq(0x40, DataReg::D2), // TL register base
            M68kInst::Moveq(0x7F, DataReg::D3), // max attenuation
            M68kInst::Bsr("ym_write_op".to_string()),
            M68kInst::Addq(Size::Long, 1, Operand::DataReg(DataReg::D6)),
            M68kInst::Cmpi(Size::Long, 4, Operand::DataReg(DataReg::D6)),
            M68kInst::Bcc(Cond::Lt, tl_loop_op),
            M68kInst::Addq(Size::Long, 1, Operand::DataReg(DataReg::D7)),
            M68kInst::Cmpi(Size::Long, 6, Operand::DataReg(DataReg::D7)),
            M68kInst::Bcc(Cond::Lt, tl_loop_ch),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Rts,
        ]
    }

    fn gen_ym_reset(&mut self) -> Vec<M68kInst> {
        vec![
            M68kInst::Label("ym_reset".to_string()),
            M68kInst::Bra("ym_init".to_string()),
        ]
    }

    fn gen_ym_wait(&mut self) -> Vec<M68kInst> {
        let wait_loop = self.next_label("ymw_loop");

        vec![
            M68kInst::Label("ym_wait".to_string()),
            M68kInst::Label(wait_loop.clone()),
            M68kInst::Btst(Operand::Imm(7), Operand::AbsLong(YM_ADDR0)),
            M68kInst::Bcc(Cond::Ne, wait_loop),
            M68kInst::Rts,
        ]
    }

    fn gen_ym_write_ch(&mut self) -> Vec<M68kInst> {
        // Args: D0=ch, D1=reg, D2=val
        let use_port1 = self.next_label("ywc_p1");

        vec![
            M68kInst::Label("ym_write_ch".to_string()),
            M68kInst::Link(AddrReg::A6, 0),
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A6), Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Long, Operand::Disp(12, AddrReg::A6), Operand::DataReg(DataReg::D2)),
            M68kInst::Move(Size::Long, Operand::Disp(16, AddrReg::A6), Operand::DataReg(DataReg::D3)),
            // Check if ch >= 3
            M68kInst::Cmpi(Size::Long, 3, Operand::DataReg(DataReg::D0)),
            M68kInst::Bcc(Cond::Ge, use_port1.clone()),
            // Port 0: reg + ch
            M68kInst::Add(Size::Long, Operand::DataReg(DataReg::D0), Operand::DataReg(DataReg::D2)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D2), Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D3), Operand::DataReg(DataReg::D1)),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Bra("ym_write0".to_string()),
            // Port 1: reg + (ch - 3)
            M68kInst::Label(use_port1),
            M68kInst::Subq(Size::Long, 3, Operand::DataReg(DataReg::D0)),
            M68kInst::Add(Size::Long, Operand::DataReg(DataReg::D0), Operand::DataReg(DataReg::D2)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D2), Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D3), Operand::DataReg(DataReg::D1)),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Bra("ym_write1".to_string()),
        ]
    }

    fn gen_ym_write_op(&mut self) -> Vec<M68kInst> {
        // Args: 8(a6)=ch, 12(a6)=op, 16(a6)=reg, 20(a6)=val
        let use_port1 = self.next_label("ywo_p1");

        vec![
            M68kInst::Label("ym_write_op".to_string()),
            M68kInst::Link(AddrReg::A6, 0),
            // Get operator offset: [0, 8, 4, 12]
            M68kInst::Move(Size::Long, Operand::Disp(12, AddrReg::A6), Operand::DataReg(DataReg::D0)),
            M68kInst::Lea(Operand::Label("__sdk_op_offsets".to_string()), AddrReg::A0),
            M68kInst::Lsl(Size::Long, Operand::Imm(2), DataReg::D0),
            M68kInst::Move(Size::Long, Operand::Indexed(0, AddrReg::A0, DataReg::D0), Operand::DataReg(DataReg::D4)),
            // Get channel
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A6), Operand::DataReg(DataReg::D0)),
            // Calculate final register
            M68kInst::Move(Size::Long, Operand::Disp(16, AddrReg::A6), Operand::DataReg(DataReg::D2)),
            M68kInst::Add(Size::Long, Operand::DataReg(DataReg::D4), Operand::DataReg(DataReg::D2)),
            M68kInst::Move(Size::Long, Operand::Disp(20, AddrReg::A6), Operand::DataReg(DataReg::D3)),
            // Check port
            M68kInst::Cmpi(Size::Long, 3, Operand::DataReg(DataReg::D0)),
            M68kInst::Bcc(Cond::Ge, use_port1.clone()),
            // Port 0
            M68kInst::Add(Size::Long, Operand::DataReg(DataReg::D0), Operand::DataReg(DataReg::D2)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D2), Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D3), Operand::DataReg(DataReg::D1)),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Bra("ym_write0".to_string()),
            // Port 1
            M68kInst::Label(use_port1),
            M68kInst::Subq(Size::Long, 3, Operand::DataReg(DataReg::D0)),
            M68kInst::Add(Size::Long, Operand::DataReg(DataReg::D0), Operand::DataReg(DataReg::D2)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D2), Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D3), Operand::DataReg(DataReg::D1)),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Bra("ym_write1".to_string()),
        ]
    }

    fn gen_ym_key_on(&mut self) -> Vec<M68kInst> {
        // Arg: D0=ch
        let ch_hi = self.next_label("ykon_hi");

        vec![
            M68kInst::Label("ym_key_on".to_string()),
            // Calculate slot: ch < 3 ? ch : (ch - 3) | 4
            M68kInst::Move(Size::Long, Operand::Disp(4, AddrReg::A7), Operand::DataReg(DataReg::D1)),
            M68kInst::Cmpi(Size::Long, 3, Operand::DataReg(DataReg::D1)),
            M68kInst::Bcc(Cond::Ge, ch_hi.clone()),
            M68kInst::Bra(".ykon_write".to_string()),
            M68kInst::Label(ch_hi),
            M68kInst::Subq(Size::Long, 3, Operand::DataReg(DataReg::D1)),
            M68kInst::Ori(Size::Long, 4, Operand::DataReg(DataReg::D1)),
            M68kInst::Label(".ykon_write".to_string()),
            // Set all operators on: 0xF0 | slot
            M68kInst::Ori(Size::Long, 0xF0, Operand::DataReg(DataReg::D1)),
            M68kInst::Moveq(0x28, DataReg::D0),
            M68kInst::Bra("ym_write0".to_string()),
        ]
    }

    fn gen_ym_key_off(&mut self) -> Vec<M68kInst> {
        let ch_hi = self.next_label("ykof_hi");

        vec![
            M68kInst::Label("ym_key_off".to_string()),
            M68kInst::Move(Size::Long, Operand::Disp(4, AddrReg::A7), Operand::DataReg(DataReg::D1)),
            M68kInst::Cmpi(Size::Long, 3, Operand::DataReg(DataReg::D1)),
            M68kInst::Bcc(Cond::Ge, ch_hi.clone()),
            M68kInst::Bra(".ykof_write".to_string()),
            M68kInst::Label(ch_hi),
            M68kInst::Subq(Size::Long, 3, Operand::DataReg(DataReg::D1)),
            M68kInst::Ori(Size::Long, 4, Operand::DataReg(DataReg::D1)),
            M68kInst::Label(".ykof_write".to_string()),
            // All operators off: just slot, no 0xF0
            M68kInst::Moveq(0x28, DataReg::D0),
            M68kInst::Bra("ym_write0".to_string()),
        ]
    }

    fn gen_ym_key_on_ops(&mut self) -> Vec<M68kInst> {
        let ch_hi = self.next_label("ykoo_hi");

        vec![
            M68kInst::Label("ym_key_on_ops".to_string()),
            // Args: 4(a7)=ch, 8(a7)=ops
            M68kInst::Move(Size::Long, Operand::Disp(4, AddrReg::A7), Operand::DataReg(DataReg::D1)),
            M68kInst::Cmpi(Size::Long, 3, Operand::DataReg(DataReg::D1)),
            M68kInst::Bcc(Cond::Ge, ch_hi.clone()),
            M68kInst::Bra(".ykoo_write".to_string()),
            M68kInst::Label(ch_hi),
            M68kInst::Subq(Size::Long, 3, Operand::DataReg(DataReg::D1)),
            M68kInst::Ori(Size::Long, 4, Operand::DataReg(DataReg::D1)),
            M68kInst::Label(".ykoo_write".to_string()),
            // Combine ops << 4 with slot
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A7), Operand::DataReg(DataReg::D2)),
            M68kInst::Lsl(Size::Long, Operand::Imm(4), DataReg::D2),
            M68kInst::Or(Size::Long, Operand::DataReg(DataReg::D2), Operand::DataReg(DataReg::D1)),
            M68kInst::Moveq(0x28, DataReg::D0),
            M68kInst::Bra("ym_write0".to_string()),
        ]
    }

    fn gen_ym_set_freq(&mut self) -> Vec<M68kInst> {
        // Args: 8(a6)=ch, 12(a6)=block, 16(a6)=fnum
        vec![
            M68kInst::Label("ym_set_freq".to_string()),
            M68kInst::Link(AddrReg::A6, 0),
            // Build freq_hi: (block << 3) | (fnum >> 8)
            M68kInst::Move(Size::Long, Operand::Disp(12, AddrReg::A6), Operand::DataReg(DataReg::D0)),
            M68kInst::Lsl(Size::Long, Operand::Imm(3), DataReg::D0),
            M68kInst::Move(Size::Long, Operand::Disp(16, AddrReg::A6), Operand::DataReg(DataReg::D1)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D2)),
            M68kInst::Lsr(Size::Long, Operand::Imm(8), DataReg::D2),
            M68kInst::Or(Size::Long, Operand::DataReg(DataReg::D2), Operand::DataReg(DataReg::D0)),
            // Save for later
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D0), Operand::DataReg(DataReg::D3)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D4)),
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A6), Operand::DataReg(DataReg::D5)),
            // Write freq_hi first (reg 0xA4)
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D5), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(Size::Long, Operand::Imm(0xA4), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D3), Operand::PreDec(AddrReg::A7)),
            M68kInst::Bsr("ym_write_ch".to_string()),
            M68kInst::Adda(Size::Long, Operand::Imm(12), AddrReg::A7),
            // Write freq_lo (reg 0xA0)
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D5), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(Size::Long, Operand::Imm(0xA0), Operand::PreDec(AddrReg::A7)),
            M68kInst::Andi(Size::Long, 0xFF, Operand::DataReg(DataReg::D4)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D4), Operand::PreDec(AddrReg::A7)),
            M68kInst::Bsr("ym_write_ch".to_string()),
            M68kInst::Adda(Size::Long, Operand::Imm(12), AddrReg::A7),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Rts,
        ]
    }

    fn gen_ym_set_algo(&mut self) -> Vec<M68kInst> {
        // Args: 8(a6)=ch, 12(a6)=algo, 16(a6)=feedback
        vec![
            M68kInst::Label("ym_set_algo".to_string()),
            M68kInst::Link(AddrReg::A6, 0),
            // val = (feedback << 3) | algo
            M68kInst::Move(Size::Long, Operand::Disp(16, AddrReg::A6), Operand::DataReg(DataReg::D0)),
            M68kInst::Lsl(Size::Long, Operand::Imm(3), DataReg::D0),
            M68kInst::Move(Size::Long, Operand::Disp(12, AddrReg::A6), Operand::DataReg(DataReg::D1)),
            M68kInst::Or(Size::Long, Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D0)),
            // ym_write_ch(ch, 0xB0, val)
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A6), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(Size::Long, Operand::Imm(0xB0), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D0), Operand::PreDec(AddrReg::A7)),
            M68kInst::Bsr("ym_write_ch".to_string()),
            M68kInst::Adda(Size::Long, Operand::Imm(12), AddrReg::A7),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Rts,
        ]
    }

    fn gen_ym_set_pan(&mut self) -> Vec<M68kInst> {
        // Args: 8(a6)=ch, 12(a6)=pan
        vec![
            M68kInst::Label("ym_set_pan".to_string()),
            M68kInst::Link(AddrReg::A6, 0),
            // ym_write_ch(ch, 0xB4, pan)
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A6), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(Size::Long, Operand::Imm(0xB4), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(Size::Long, Operand::Disp(12, AddrReg::A6), Operand::PreDec(AddrReg::A7)),
            M68kInst::Bsr("ym_write_ch".to_string()),
            M68kInst::Adda(Size::Long, Operand::Imm(12), AddrReg::A7),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Rts,
        ]
    }

    fn gen_ym_set_volume(&mut self) -> Vec<M68kInst> {
        // Args: 8(a6)=ch, 12(a6)=vol -> set TL of operator 3 (carrier)
        vec![
            M68kInst::Label("ym_set_volume".to_string()),
            M68kInst::Link(AddrReg::A6, 0),
            // ym_write_op(ch, 3, 0x40, vol)
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A6), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(Size::Long, Operand::Imm(3), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(Size::Long, Operand::Imm(0x40), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(Size::Long, Operand::Disp(12, AddrReg::A6), Operand::PreDec(AddrReg::A7)),
            M68kInst::Bsr("ym_write_op".to_string()),
            M68kInst::Adda(Size::Long, Operand::Imm(16), AddrReg::A7),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Rts,
        ]
    }

    fn gen_ym_set_lfo(&mut self) -> Vec<M68kInst> {
        // Arg: 4(a7)=mode
        vec![
            M68kInst::Label("ym_set_lfo".to_string()),
            M68kInst::Moveq(0x22, DataReg::D0),
            M68kInst::Move(Size::Long, Operand::Disp(4, AddrReg::A7), Operand::DataReg(DataReg::D1)),
            M68kInst::Bra("ym_write0".to_string()),
        ]
    }

    // -------------------------------------------------------------------------
    // PSG Library Functions
    // -------------------------------------------------------------------------

    fn gen_psg_init(&mut self) -> Vec<M68kInst> {
        vec![
            M68kInst::Label("psg_init".to_string()),
            M68kInst::Bra("psg_stop".to_string()),
        ]
    }

    fn gen_psg_set_tone(&mut self) -> Vec<M68kInst> {
        // Args: 4(a7)=channel, 8(a7)=divider
        vec![
            M68kInst::Label("psg_set_tone".to_string()),
            // Latch byte: 0x80 | (channel << 5) | (divider & 0x0F)
            M68kInst::Move(Size::Long, Operand::Disp(4, AddrReg::A7), Operand::DataReg(DataReg::D0)),
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Lsl(Size::Word, Operand::Imm(5), DataReg::D0),
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A7), Operand::DataReg(DataReg::D1)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D2)),
            M68kInst::Andi(Size::Word, 0x0F, Operand::DataReg(DataReg::D2)),
            M68kInst::Or(Size::Word, Operand::DataReg(DataReg::D2), Operand::DataReg(DataReg::D0)),
            M68kInst::Ori(Size::Word, 0x80, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D0), Operand::AbsLong(PSG_PORT)),
            // Data byte: (divider >> 4) & 0x3F
            M68kInst::Lsr(Size::Word, Operand::Imm(4), DataReg::D1),
            M68kInst::Andi(Size::Word, 0x3F, Operand::DataReg(DataReg::D1)),
            M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D1), Operand::AbsLong(PSG_PORT)),
            M68kInst::Rts,
        ]
    }

    fn gen_psg_set_freq(&mut self) -> Vec<M68kInst> {
        // Args: 4(a7)=channel, 8(a7)=freq
        // divider = 3579545 / (32 * freq)
        vec![
            M68kInst::Label("psg_set_freq".to_string()),
            M68kInst::Link(AddrReg::A6, 0),
            // freq in D1
            M68kInst::Move(Size::Long, Operand::Disp(12, AddrReg::A6), Operand::DataReg(DataReg::D1)),
            // Clamp freq >= 1
            M68kInst::Tst(Size::Long, Operand::DataReg(DataReg::D1)),
            M68kInst::Bcc(Cond::Gt, ".psf_calc".to_string()),
            M68kInst::Moveq(1, DataReg::D1),
            M68kInst::Label(".psf_calc".to_string()),
            // 32 * freq
            M68kInst::Lsl(Size::Long, Operand::Imm(5), DataReg::D1),
            // 3579545 / (32 * freq)
            M68kInst::Move(Size::Long, Operand::Imm(3579545), Operand::DataReg(DataReg::D0)),
            M68kInst::Divu(Operand::DataReg(DataReg::D1), DataReg::D0),
            M68kInst::Andi(Size::Long, 0xFFFF, Operand::DataReg(DataReg::D0)),
            // Clamp to 1023
            M68kInst::Cmpi(Size::Long, 1023, Operand::DataReg(DataReg::D0)),
            M68kInst::Bcc(Cond::Le, ".psf_ok".to_string()),
            M68kInst::Move(Size::Long, Operand::Imm(1023), Operand::DataReg(DataReg::D0)),
            M68kInst::Label(".psf_ok".to_string()),
            // Call psg_set_tone(channel, divider)
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A6), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(Size::Long, Operand::DataReg(DataReg::D0), Operand::PreDec(AddrReg::A7)),
            M68kInst::Bsr("psg_set_tone".to_string()),
            M68kInst::Addq(Size::Long, 8, Operand::AddrReg(AddrReg::A7)),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Rts,
        ]
    }

    fn gen_psg_stop(&mut self) -> Vec<M68kInst> {
        vec![
            M68kInst::Label("psg_stop".to_string()),
            // Set all 4 channels to volume 15 (silent)
            M68kInst::Move(Size::Byte, Operand::Imm(0x9F), Operand::AbsLong(PSG_PORT)), // Ch 0
            M68kInst::Move(Size::Byte, Operand::Imm(0xBF), Operand::AbsLong(PSG_PORT)), // Ch 1
            M68kInst::Move(Size::Byte, Operand::Imm(0xDF), Operand::AbsLong(PSG_PORT)), // Ch 2
            M68kInst::Move(Size::Byte, Operand::Imm(0xFF), Operand::AbsLong(PSG_PORT)), // Noise
            M68kInst::Rts,
        ]
    }

    fn gen_psg_beep(&mut self) -> Vec<M68kInst> {
        // Args: 4(a7)=channel, 8(a7)=divider, 12(a7)=volume
        vec![
            M68kInst::Label("psg_beep".to_string()),
            M68kInst::Link(AddrReg::A6, 0),
            // psg_set_tone(channel, divider)
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A6), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(Size::Long, Operand::Disp(12, AddrReg::A6), Operand::PreDec(AddrReg::A7)),
            M68kInst::Bsr("psg_set_tone".to_string()),
            M68kInst::Addq(Size::Long, 8, Operand::AddrReg(AddrReg::A7)),
            // psg_set_volume(channel, volume) - inline
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A6), Operand::DataReg(DataReg::D0)),
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Lsl(Size::Word, Operand::Imm(5), DataReg::D0),
            M68kInst::Move(Size::Long, Operand::Disp(16, AddrReg::A6), Operand::DataReg(DataReg::D1)),
            M68kInst::Andi(Size::Word, 0x0F, Operand::DataReg(DataReg::D1)),
            M68kInst::Or(Size::Word, Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D0)),
            M68kInst::Ori(Size::Word, 0x90, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Byte, Operand::DataReg(DataReg::D0), Operand::AbsLong(PSG_PORT)),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Rts,
        ]
    }

    fn gen_psg_note_on(&mut self) -> Vec<M68kInst> {
        // Same as psg_beep
        vec![
            M68kInst::Label("psg_note_on".to_string()),
            M68kInst::Bra("psg_beep".to_string()),
        ]
    }
}

impl Default for SdkLibraryGenerator {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Dependency Resolution
// ============================================================================

/// Get the set of SDK functions that a given function depends on
pub fn get_sdk_dependencies(func_name: &str) -> &'static [&'static str] {
    match func_name {
        // VDP dependencies
        "vdp_vsync" => &["vdp_wait_vblank_start"],
        "vdp_wait_frame" => &["vdp_wait_vblank_start", "vdp_wait_vblank_end"],

        // YM2612 dependencies
        "ym_reset" => &["ym_init"],
        "ym_init" => &["ym_write0", "ym_key_off", "ym_write_op"],
        "ym_write_ch" => &["ym_write0", "ym_write1"],
        "ym_write_op" => &["ym_write0", "ym_write1"],
        "ym_set_freq" => &["ym_write_ch"],
        "ym_set_algo" => &["ym_write_ch"],
        "ym_set_pan" => &["ym_write_ch"],
        "ym_set_volume" => &["ym_write_op"],
        "ym_key_on" => &["ym_write0"],
        "ym_key_off" => &["ym_write0"],
        "ym_key_on_ops" => &["ym_write0"],
        "ym_set_lfo" => &["ym_write0"],

        // PSG dependencies
        "psg_init" => &["psg_stop"],
        "psg_set_freq" => &["psg_set_tone"],
        "psg_beep" => &["psg_set_tone"],
        "psg_note_on" => &["psg_beep", "psg_set_tone"],

        _ => &[],
    }
}

/// Resolve all transitive dependencies for a set of SDK functions
pub fn resolve_dependencies(functions: &HashSet<String>) -> HashSet<String> {
    let mut all = functions.clone();
    let mut changed = true;

    while changed {
        changed = false;
        let current: Vec<_> = all.iter().cloned().collect();
        for func in current {
            for dep in get_sdk_dependencies(&func) {
                if all.insert(dep.to_string()) {
                    changed = true;
                }
            }
        }
    }

    all
}

// ============================================================================
// Static Data
// ============================================================================

/// Check if any functions in the set need the frame counter
pub fn needs_frame_counter(functions: &HashSet<String>) -> bool {
    functions.iter().any(|f| {
        matches!(f.as_str(), "vdp_wait_vblank_start" | "vdp_get_frame_count" | "vdp_reset_frame_count" | "vdp_init")
    })
}

/// Check if any functions need the operator offset table
pub fn needs_op_offsets(functions: &HashSet<String>) -> bool {
    functions.iter().any(|f| {
        matches!(f.as_str(), "ym_write_op" | "ym_load_operator" | "ym_init")
    })
}

/// Generate SDK static data section
pub fn generate_static_data(functions: &HashSet<String>) -> Vec<M68kInst> {
    let mut insts = Vec::new();

    if needs_frame_counter(functions) || needs_op_offsets(functions) {
        insts.push(M68kInst::Directive(".section .bss".to_string()));
        insts.push(M68kInst::Directive(".align 4".to_string()));
    }

    if needs_frame_counter(functions) {
        insts.push(M68kInst::Label("__sdk_frame_count".to_string()));
        insts.push(M68kInst::Directive(".space 4".to_string()));
    }

    if needs_op_offsets(functions) {
        insts.push(M68kInst::Directive(".section .rodata".to_string()));
        insts.push(M68kInst::Directive(".align 4".to_string()));
        insts.push(M68kInst::Label("__sdk_op_offsets".to_string()));
        insts.push(M68kInst::Directive(".long 0, 8, 4, 12".to_string()));
    }

    insts
}
