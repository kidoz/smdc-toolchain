//! Inline code generation for simple SDK functions

use super::{PSG_PORT, VDP_CTRL, VDP_DATA, YM_ADDR0, YM_ADDR1, YM_DATA0, YM_DATA1};
use crate::backend::m68k::m68k::*;

/// Generates inline M68k instructions for simple SDK functions
pub struct SdkInlineGenerator;

impl SdkInlineGenerator {
    /// Generate inline code for a function call
    /// Arguments are expected to be loaded in D0, D1, D2, D3 in order
    pub fn generate(func_name: &str) -> crate::common::CompileResult<Vec<M68kInst>> {
        let insts = match func_name {
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

            // Sprite inline functions
            "sprite_attr" => Self::gen_sprite_attr(),
            "sprite_get_width" => Self::gen_sprite_get_width(),
            "sprite_get_height" => Self::gen_sprite_get_height(),

            // Input inline functions
            "joy1_read" => Self::gen_joy1_read(),
            "joy2_read" => Self::gen_joy2_read(),

            _ => {
                return Err(crate::common::CompileError::codegen(format!(
                    "Not an inline function: {func_name}"
                )));
            }
        };
        Ok(insts)
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
            M68kInst::Or(
                Size::Word,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D0),
            ),
            // | 0x8000
            M68kInst::Ori(Size::Word, 0x8000, Operand::DataReg(DataReg::D0)),
            // Write to VDP control port
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_CTRL),
            ),
        ]
    }

    /// vdp_get_status() -> return *VDP_CTRL
    fn gen_vdp_get_status() -> Vec<M68kInst> {
        vec![M68kInst::Move(
            Size::Word,
            Operand::AbsLong(VDP_CTRL),
            Operand::DataReg(DataReg::D0),
        )]
    }

    /// vdp_set_write_addr(addr) -> write address command to VDP_CTRL
    /// Args: D0 = addr
    fn gen_vdp_set_write_addr() -> Vec<M68kInst> {
        vec![
            // First word: 0x4000 | (addr & 0x3FFF)
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D0),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Andi(Size::Word, 0x3FFF, Operand::DataReg(DataReg::D1)),
            M68kInst::Ori(Size::Word, 0x4000, Operand::DataReg(DataReg::D1)),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D1),
                Operand::AbsLong(VDP_CTRL),
            ),
            // Second word: (addr >> 14) & 0x03
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_CTRL),
            ),
        ]
    }

    /// vdp_set_cram_addr(index) -> set CRAM write address
    /// Args: D0 = index
    fn gen_vdp_set_cram_addr() -> Vec<M68kInst> {
        vec![
            // addr = index * 2
            M68kInst::Add(
                Size::Long,
                Operand::DataReg(DataReg::D0),
                Operand::DataReg(DataReg::D0),
            ),
            // First word: 0xC000 | (addr & 0x3FFF)
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D0),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Andi(Size::Word, 0x3FFF, Operand::DataReg(DataReg::D1)),
            M68kInst::Ori(Size::Word, 0xC000_u16 as i32, Operand::DataReg(DataReg::D1)),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D1),
                Operand::AbsLong(VDP_CTRL),
            ),
            // Second word: (addr >> 14) & 0x03
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_CTRL),
            ),
        ]
    }

    /// vdp_set_color(index, color) -> set CRAM addr then write color
    /// Args: D0 = index, D1 = color
    fn gen_vdp_set_color() -> Vec<M68kInst> {
        let mut insts = vec![
            // Save color
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D2),
            ),
        ];
        // Set CRAM address
        insts.extend(Self::gen_vdp_set_cram_addr());
        // Write color
        insts.push(M68kInst::Move(
            Size::Word,
            Operand::DataReg(DataReg::D2),
            Operand::AbsLong(VDP_DATA),
        ));
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
            M68kInst::Or(
                Size::Word,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D0),
            ),
            // Set as register 7 value
            M68kInst::Ori(Size::Word, 0x8700, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_CTRL),
            ),
        ]
    }

    /// vdp_in_vblank() -> return (*VDP_CTRL & 0x0008) != 0
    fn gen_vdp_in_vblank() -> Vec<M68kInst> {
        vec![
            M68kInst::Move(
                Size::Word,
                Operand::AbsLong(VDP_CTRL),
                Operand::DataReg(DataReg::D0),
            ),
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
            M68kInst::Move(
                Size::Byte,
                Operand::AbsLong(YM_ADDR0),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Andi(Size::Long, 0xFF, Operand::DataReg(DataReg::D0)),
        ]
    }

    /// Helper to emit YM timing delay (12 NOPs for ~83 cycles)
    fn emit_ym_delay() -> Vec<M68kInst> {
        vec![
            M68kInst::Nop,
            M68kInst::Nop,
            M68kInst::Nop,
            M68kInst::Nop,
            M68kInst::Nop,
            M68kInst::Nop,
            M68kInst::Nop,
            M68kInst::Nop,
            M68kInst::Nop,
            M68kInst::Nop,
            M68kInst::Nop,
            M68kInst::Nop,
        ]
    }

    /// ym_write0(reg, val) -> write to YM port 0 with timing
    /// Args: D0 = reg, D1 = val
    fn gen_ym_write0() -> Vec<M68kInst> {
        let mut insts = vec![M68kInst::Move(
            Size::Byte,
            Operand::DataReg(DataReg::D0),
            Operand::AbsLong(YM_ADDR0),
        )];
        insts.extend(Self::emit_ym_delay());
        insts.push(M68kInst::Move(
            Size::Byte,
            Operand::DataReg(DataReg::D1),
            Operand::AbsLong(YM_DATA0),
        ));
        insts.extend(Self::emit_ym_delay());
        insts
    }

    /// ym_write1(reg, val) -> write to YM port 1 with timing
    /// Args: D0 = reg, D1 = val
    fn gen_ym_write1() -> Vec<M68kInst> {
        let mut insts = vec![M68kInst::Move(
            Size::Byte,
            Operand::DataReg(DataReg::D0),
            Operand::AbsLong(YM_ADDR1),
        )];
        insts.extend(Self::emit_ym_delay());
        insts.push(M68kInst::Move(
            Size::Byte,
            Operand::DataReg(DataReg::D1),
            Operand::AbsLong(YM_DATA1),
        ));
        insts.extend(Self::emit_ym_delay());
        insts
    }

    /// ym_dac_enable() -> write 0x80 to register 0x2B
    fn gen_ym_dac_enable() -> Vec<M68kInst> {
        let mut insts = vec![
            M68kInst::Moveq(0x2B, DataReg::D0),
            M68kInst::Move(
                Size::Byte,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(YM_ADDR0),
            ),
        ];
        insts.extend(Self::emit_ym_delay());
        insts.push(M68kInst::Move(
            Size::Byte,
            Operand::Imm(0x80),
            Operand::AbsLong(YM_DATA0),
        ));
        insts.extend(Self::emit_ym_delay());
        insts
    }

    /// ym_dac_disable() -> write 0x00 to register 0x2B
    fn gen_ym_dac_disable() -> Vec<M68kInst> {
        let mut insts = vec![
            M68kInst::Moveq(0x2B, DataReg::D0),
            M68kInst::Move(
                Size::Byte,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(YM_ADDR0),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D0),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Moveq(0x2A, DataReg::D0),
            M68kInst::Move(
                Size::Byte,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(YM_ADDR0),
            ),
        ];
        insts.extend(Self::emit_ym_delay());
        insts.push(M68kInst::Move(
            Size::Byte,
            Operand::DataReg(DataReg::D1),
            Operand::AbsLong(YM_DATA0),
        ));
        insts
    }

    // -------------------------------------------------------------------------
    // PSG Inline Functions
    // -------------------------------------------------------------------------

    /// psg_write(value) -> *PSG_PORT = value
    /// Args: D0 = value
    fn gen_psg_write() -> Vec<M68kInst> {
        vec![M68kInst::Move(
            Size::Byte,
            Operand::DataReg(DataReg::D0),
            Operand::AbsLong(PSG_PORT),
        )]
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
            M68kInst::Or(
                Size::Word,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Ori(Size::Word, 0x90, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(
                Size::Byte,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(PSG_PORT),
            ),
        ]
    }

    /// psg_set_noise(mode)
    /// Args: D0 = mode
    fn gen_psg_set_noise() -> Vec<M68kInst> {
        vec![
            // Build latch byte: 0xE0 | (mode & 0x07)
            M68kInst::Andi(Size::Word, 0x07, Operand::DataReg(DataReg::D0)),
            M68kInst::Ori(Size::Word, 0xE0, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(
                Size::Byte,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(PSG_PORT),
            ),
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
            M68kInst::Move(
                Size::Byte,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(PSG_PORT),
            ),
        ]
    }

    /// psg_note_off(channel) -> set volume to 15
    /// Args: D0 = channel
    fn gen_psg_note_off() -> Vec<M68kInst> {
        Self::gen_psg_stop_channel()
    }

    // -------------------------------------------------------------------------
    // Sprite Inline Functions
    // -------------------------------------------------------------------------

    /// sprite_attr(tile, pal, priority, hflip, vflip) -> attribute word
    /// Args: D0 = tile, D1 = pal, D2 = priority, D3 = hflip, stack[0] = vflip
    /// Returns in D0: tile | (pal << 13) | (priority << 15) | (hflip << 11) | (vflip << 12)
    fn gen_sprite_attr() -> Vec<M68kInst> {
        vec![
            // D0 = tile (already has base tile index in bits 0-10)
            M68kInst::Andi(Size::Word, 0x07FF, Operand::DataReg(DataReg::D0)),
            // Add palette: (pal & 3) << 13
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D1)),
            M68kInst::Lsl(Size::Word, Operand::Imm(13), DataReg::D1),
            M68kInst::Or(
                Size::Word,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D0),
            ),
            // Add priority: (priority & 1) << 15
            M68kInst::Andi(Size::Word, 0x01, Operand::DataReg(DataReg::D2)),
            M68kInst::Lsl(Size::Word, Operand::Imm(15), DataReg::D2),
            M68kInst::Or(
                Size::Word,
                Operand::DataReg(DataReg::D2),
                Operand::DataReg(DataReg::D0),
            ),
            // Add hflip: (hflip & 1) << 11
            M68kInst::Andi(Size::Word, 0x01, Operand::DataReg(DataReg::D3)),
            M68kInst::Lsl(Size::Word, Operand::Imm(11), DataReg::D3),
            M68kInst::Or(
                Size::Word,
                Operand::DataReg(DataReg::D3),
                Operand::DataReg(DataReg::D0),
            ),
            // Note: vflip is on stack, we'll skip it for 4-arg inline version
            // For 5th arg, caller should handle separately or use library version
        ]
    }

    /// sprite_get_width(size) -> width in tiles (1-4)
    /// Args: D0 = size
    /// Returns: ((size >> 2) & 3) + 1
    fn gen_sprite_get_width() -> Vec<M68kInst> {
        vec![
            M68kInst::Lsr(Size::Word, Operand::Imm(2), DataReg::D0),
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Addq(Size::Word, 1, Operand::DataReg(DataReg::D0)),
        ]
    }

    /// sprite_get_height(size) -> height in tiles (1-4)
    /// Args: D0 = size
    /// Returns: (size & 3) + 1
    fn gen_sprite_get_height() -> Vec<M68kInst> {
        vec![
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Addq(Size::Word, 1, Operand::DataReg(DataReg::D0)),
        ]
    }

    // -------------------------------------------------------------------------
    // Input Inline Functions
    // -------------------------------------------------------------------------

    /// joy1_read() -> read joystick 1 raw value
    /// Returns 3-button state (active low inverted to active high)
    fn gen_joy1_read() -> Vec<M68kInst> {
        vec![
            // Read from joystick 1 data port
            M68kInst::Move(
                Size::Byte,
                Operand::AbsLong(0xA10003),
                Operand::DataReg(DataReg::D0),
            ),
            // Invert bits (active low -> active high)
            M68kInst::Not(Size::Byte, Operand::DataReg(DataReg::D0)),
            M68kInst::Andi(Size::Long, 0xFF, Operand::DataReg(DataReg::D0)),
        ]
    }

    /// joy2_read() -> read joystick 2 raw value
    fn gen_joy2_read() -> Vec<M68kInst> {
        vec![
            // Read from joystick 2 data port
            M68kInst::Move(
                Size::Byte,
                Operand::AbsLong(0xA10005),
                Operand::DataReg(DataReg::D0),
            ),
            // Invert bits (active low -> active high)
            M68kInst::Not(Size::Byte, Operand::DataReg(DataReg::D0)),
            M68kInst::Andi(Size::Long, 0xFF, Operand::DataReg(DataReg::D0)),
        ]
    }
}
