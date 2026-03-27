//! Library code generation for complex SDK functions

use super::{PSG_PORT, VDP_CTRL, VDP_DATA, YM_ADDR0};
use crate::backend::m68k::m68k::*;

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

            // Sprite library functions
            "sprite_init" => self.gen_sprite_init(),
            "sprite_set" => self.gen_sprite_set(),
            "sprite_set_pos" => self.gen_sprite_set_pos(),
            "sprite_hide" => self.gen_sprite_hide(),
            "sprite_clear" => self.gen_sprite_clear(),
            "sprite_clear_all" => self.gen_sprite_clear_all(),
            "sprite_set_link" => self.gen_sprite_set_link(),

            // Input library functions
            "input_init" => self.gen_input_init(),
            "input_read" => self.gen_input_read(),
            "input_update" => self.gen_input_update(),
            "input_held" => self.gen_input_held(),
            "input_pressed" => self.gen_input_pressed(),
            "input_released" => self.gen_input_released(),
            "input_is_6button" => self.gen_input_is_6button(),

            _ => {
                // For unimplemented functions, generate a stub
                vec![
                    M68kInst::Label(func_name.to_string()),
                    M68kInst::Comment(format!("TODO: implement {func_name}")),
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
        // Note: VRAM is already cleared by startup code, so we just set registers
        // and enable display immediately
        let regs: [(i32, i32); 15] = [
            (0x00, 0x04),
            (0x01, 0x44),
            (0x02, 0x30),
            (0x03, 0x3C), // 0x01=0x44: display ON
            (0x04, 0x07),
            (0x05, 0x78),
            (0x07, 0x00),
            (0x0A, 0x00),
            (0x0B, 0x00),
            (0x0C, 0x81),
            (0x0D, 0x3F),
            (0x0F, 0x02),
            (0x10, 0x11),
            (0x11, 0x00),
            (0x12, 0x00), // 0x10: H64xV32 scroll size
        ];

        for (reg, val) in regs {
            let cmd = 0x8000 | (reg << 8) | val;
            insts.push(M68kInst::Move(
                Size::Word,
                Operand::Imm(cmd),
                Operand::AddrInd(AddrReg::A0),
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

        // Note: Must read status as WORD and test bit in register.
        // BTST on memory does a byte read, which at even address 0xC00004
        // reads the HIGH byte (bits 8-15), missing the VBlank flag in bit 3.
        vec![
            M68kInst::Label("vdp_wait_vblank_start".to_string()),
            M68kInst::Lea(Operand::AbsLong(VDP_CTRL), AddrReg::A0),
            // Wait until NOT in VBlank
            M68kInst::Label(wait_not.clone()),
            M68kInst::Move(
                Size::Word,
                Operand::AddrInd(AddrReg::A0),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Btst(Operand::Imm(3), Operand::DataReg(DataReg::D0)),
            M68kInst::Bcc(Cond::Ne, wait_not),
            // Wait until IN VBlank
            M68kInst::Label(wait_in.clone()),
            M68kInst::Move(
                Size::Word,
                Operand::AddrInd(AddrReg::A0),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Btst(Operand::Imm(3), Operand::DataReg(DataReg::D0)),
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

        // Note: Must read status as WORD and test bit in register.
        // BTST on memory does a byte read, which at even address 0xC00004
        // reads the HIGH byte (bits 8-15), missing the VBlank flag in bit 3.
        vec![
            M68kInst::Label("vdp_wait_vblank_end".to_string()),
            M68kInst::Lea(Operand::AbsLong(VDP_CTRL), AddrReg::A0),
            // Wait until IN VBlank
            M68kInst::Label(wait_in.clone()),
            M68kInst::Move(
                Size::Word,
                Operand::AddrInd(AddrReg::A0),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Btst(Operand::Imm(3), Operand::DataReg(DataReg::D0)),
            M68kInst::Bcc(Cond::Eq, wait_in),
            // Wait until out of VBlank
            M68kInst::Label(wait_out.clone()),
            M68kInst::Move(
                Size::Word,
                Operand::AddrInd(AddrReg::A0),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Btst(Operand::Imm(3), Operand::DataReg(DataReg::D0)),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(8, AddrReg::A6),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Add(
                Size::Long,
                Operand::DataReg(DataReg::D0),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Andi(Size::Word, 0x3FFF, Operand::DataReg(DataReg::D0)),
            M68kInst::Ori(Size::Word, 0xC000_u16 as i32, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_CTRL),
            ),
            M68kInst::Clr(Size::Word, Operand::AbsLong(VDP_CTRL)),
            // Load colors pointer and count
            M68kInst::Move(
                Size::Long,
                Operand::Disp(12, AddrReg::A6),
                Operand::AddrReg(AddrReg::A0),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(16, AddrReg::A6),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Subq(Size::Long, 1, Operand::DataReg(DataReg::D1)),
            M68kInst::Bcc(Cond::Mi, ".vlp_done".to_string()),
            // Loop
            M68kInst::Label(loop_label.clone()),
            M68kInst::Move(
                Size::Word,
                Operand::PostInc(AddrReg::A0),
                Operand::AbsLong(VDP_DATA),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(12, AddrReg::A6),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Lsl(Size::Long, Operand::Imm(5), DataReg::D0),
            // Set write address
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
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_CTRL),
            ),
            // Load tiles pointer
            M68kInst::Move(
                Size::Long,
                Operand::Disp(8, AddrReg::A6),
                Operand::AddrReg(AddrReg::A0),
            ),
            // count * 16 words per tile
            M68kInst::Move(
                Size::Long,
                Operand::Disp(16, AddrReg::A6),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Lsl(Size::Long, Operand::Imm(4), DataReg::D1),
            M68kInst::Subq(Size::Long, 1, Operand::DataReg(DataReg::D1)),
            M68kInst::Bcc(Cond::Mi, ".vlt_done".to_string()),
            // Loop
            M68kInst::Label(loop_label.clone()),
            M68kInst::Move(
                Size::Word,
                Operand::PostInc(AddrReg::A0),
                Operand::AbsLong(VDP_DATA),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(12, AddrReg::A6),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Lsl(Size::Long, Operand::Imm(7), DataReg::D0),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(8, AddrReg::A6),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Add(
                Size::Long,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Add(
                Size::Long,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Addi(Size::Long, 0xC000, Operand::DataReg(DataReg::D0)),
            // Set write address
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
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_CTRL),
            ),
            // Write tile (use low word of 32-bit argument)
            M68kInst::Move(
                Size::Long,
                Operand::Disp(16, AddrReg::A6),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D1),
                Operand::AbsLong(VDP_DATA),
            ),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_set_tile_b(&mut self) -> Vec<M68kInst> {
        // Same as tile_a but with VRAM_PLANE_B = 0xE000
        vec![
            M68kInst::Label("vdp_set_tile_b".to_string()),
            M68kInst::Link(AddrReg::A6, 0),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(12, AddrReg::A6),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Lsl(Size::Long, Operand::Imm(7), DataReg::D0),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(8, AddrReg::A6),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Add(
                Size::Long,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Add(
                Size::Long,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Addi(Size::Long, 0xE000, Operand::DataReg(DataReg::D0)),
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
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_CTRL),
            ),
            // Write tile (use low word of 32-bit argument)
            M68kInst::Move(
                Size::Long,
                Operand::Disp(16, AddrReg::A6),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D1),
                Operand::AbsLong(VDP_DATA),
            ),
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
            M68kInst::Move(
                Size::Word,
                Operand::Imm(2047),
                Operand::DataReg(DataReg::D0),
            ),
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
            M68kInst::Move(
                Size::Word,
                Operand::Imm(2047),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Label(loop_label.clone()),
            M68kInst::Clr(Size::Word, Operand::AbsLong(VDP_DATA)),
            M68kInst::Dbf(DataReg::D0, loop_label),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_set_hscroll_a(&mut self) -> Vec<M68kInst> {
        // VRAM_HSCROLL = 0xFC00
        // Arg on stack as 32-bit long: 4(SP)=value, low word at 6(SP)
        vec![
            M68kInst::Label("vdp_set_hscroll_a".to_string()),
            M68kInst::Move(Size::Word, Operand::Imm(0x7C00), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(Size::Word, Operand::Imm(0x0003), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(
                Size::Word,
                Operand::Disp(6, AddrReg::A7),
                Operand::AbsLong(VDP_DATA),
            ),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_set_hscroll_b(&mut self) -> Vec<M68kInst> {
        // Arg on stack as 32-bit long: 4(SP)=value, low word at 6(SP)
        vec![
            M68kInst::Label("vdp_set_hscroll_b".to_string()),
            M68kInst::Move(Size::Word, Operand::Imm(0x7C02), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(Size::Word, Operand::Imm(0x0003), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(
                Size::Word,
                Operand::Disp(6, AddrReg::A7),
                Operand::AbsLong(VDP_DATA),
            ),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_set_vscroll_a(&mut self) -> Vec<M68kInst> {
        // Arg on stack as 32-bit long: 4(SP)=value, low word at 6(SP)
        vec![
            M68kInst::Label("vdp_set_vscroll_a".to_string()),
            M68kInst::Move(Size::Word, Operand::Imm(0x4000), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(Size::Word, Operand::Imm(0x0010), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(
                Size::Word,
                Operand::Disp(6, AddrReg::A7),
                Operand::AbsLong(VDP_DATA),
            ),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_set_vscroll_b(&mut self) -> Vec<M68kInst> {
        // Arg on stack as 32-bit long: 4(SP)=value, low word at 6(SP)
        vec![
            M68kInst::Label("vdp_set_vscroll_b".to_string()),
            M68kInst::Move(Size::Word, Operand::Imm(0x4002), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(Size::Word, Operand::Imm(0x0010), Operand::AbsLong(VDP_CTRL)),
            M68kInst::Move(
                Size::Word,
                Operand::Disp(6, AddrReg::A7),
                Operand::AbsLong(VDP_DATA),
            ),
            M68kInst::Rts,
        ]
    }

    fn gen_vdp_get_frame_count(&mut self) -> Vec<M68kInst> {
        vec![
            M68kInst::Label("vdp_get_frame_count".to_string()),
            M68kInst::Move(
                Size::Long,
                Operand::Label("__sdk_frame_count".to_string()),
                Operand::DataReg(DataReg::D0),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D7),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Bsr("ym_key_off".to_string()),
            M68kInst::Addq(Size::Long, 1, Operand::DataReg(DataReg::D7)),
            M68kInst::Cmpi(Size::Long, 6, Operand::DataReg(DataReg::D7)),
            M68kInst::Bcc(Cond::Lt, keyoff_loop),
            // Set all TL to 0x7F (max attenuation)
            M68kInst::Clr(Size::Long, Operand::DataReg(DataReg::D7)), // channel
            M68kInst::Label(tl_loop_ch.clone()),
            M68kInst::Clr(Size::Long, Operand::DataReg(DataReg::D6)), // operator
            M68kInst::Label(tl_loop_op.clone()),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D7),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D6),
                Operand::DataReg(DataReg::D1),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(8, AddrReg::A6),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(12, AddrReg::A6),
                Operand::DataReg(DataReg::D2),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(16, AddrReg::A6),
                Operand::DataReg(DataReg::D3),
            ),
            // Check if ch >= 3
            M68kInst::Cmpi(Size::Long, 3, Operand::DataReg(DataReg::D0)),
            M68kInst::Bcc(Cond::Ge, use_port1.clone()),
            // Port 0: reg + ch
            M68kInst::Add(
                Size::Long,
                Operand::DataReg(DataReg::D0),
                Operand::DataReg(DataReg::D2),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D2),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D3),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Bra("ym_write0".to_string()),
            // Port 1: reg + (ch - 3)
            M68kInst::Label(use_port1),
            M68kInst::Subq(Size::Long, 3, Operand::DataReg(DataReg::D0)),
            M68kInst::Add(
                Size::Long,
                Operand::DataReg(DataReg::D0),
                Operand::DataReg(DataReg::D2),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D2),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D3),
                Operand::DataReg(DataReg::D1),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(12, AddrReg::A6),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Lea(Operand::Label("__sdk_op_offsets".to_string()), AddrReg::A0),
            M68kInst::Lsl(Size::Long, Operand::Imm(2), DataReg::D0),
            M68kInst::Move(
                Size::Long,
                Operand::Indexed(0, AddrReg::A0, DataReg::D0),
                Operand::DataReg(DataReg::D4),
            ),
            // Get channel
            M68kInst::Move(
                Size::Long,
                Operand::Disp(8, AddrReg::A6),
                Operand::DataReg(DataReg::D0),
            ),
            // Calculate final register
            M68kInst::Move(
                Size::Long,
                Operand::Disp(16, AddrReg::A6),
                Operand::DataReg(DataReg::D2),
            ),
            M68kInst::Add(
                Size::Long,
                Operand::DataReg(DataReg::D4),
                Operand::DataReg(DataReg::D2),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(20, AddrReg::A6),
                Operand::DataReg(DataReg::D3),
            ),
            // Check port
            M68kInst::Cmpi(Size::Long, 3, Operand::DataReg(DataReg::D0)),
            M68kInst::Bcc(Cond::Ge, use_port1.clone()),
            // Port 0
            M68kInst::Add(
                Size::Long,
                Operand::DataReg(DataReg::D0),
                Operand::DataReg(DataReg::D2),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D2),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D3),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Bra("ym_write0".to_string()),
            // Port 1
            M68kInst::Label(use_port1),
            M68kInst::Subq(Size::Long, 3, Operand::DataReg(DataReg::D0)),
            M68kInst::Add(
                Size::Long,
                Operand::DataReg(DataReg::D0),
                Operand::DataReg(DataReg::D2),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D2),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D3),
                Operand::DataReg(DataReg::D1),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(4, AddrReg::A7),
                Operand::DataReg(DataReg::D1),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(4, AddrReg::A7),
                Operand::DataReg(DataReg::D1),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(4, AddrReg::A7),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Cmpi(Size::Long, 3, Operand::DataReg(DataReg::D1)),
            M68kInst::Bcc(Cond::Ge, ch_hi.clone()),
            M68kInst::Bra(".ykoo_write".to_string()),
            M68kInst::Label(ch_hi),
            M68kInst::Subq(Size::Long, 3, Operand::DataReg(DataReg::D1)),
            M68kInst::Ori(Size::Long, 4, Operand::DataReg(DataReg::D1)),
            M68kInst::Label(".ykoo_write".to_string()),
            // Combine ops << 4 with slot
            M68kInst::Move(
                Size::Long,
                Operand::Disp(8, AddrReg::A7),
                Operand::DataReg(DataReg::D2),
            ),
            M68kInst::Lsl(Size::Long, Operand::Imm(4), DataReg::D2),
            M68kInst::Or(
                Size::Long,
                Operand::DataReg(DataReg::D2),
                Operand::DataReg(DataReg::D1),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(12, AddrReg::A6),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Lsl(Size::Long, Operand::Imm(3), DataReg::D0),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(16, AddrReg::A6),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D2),
            ),
            M68kInst::Lsr(Size::Long, Operand::Imm(8), DataReg::D2),
            M68kInst::Or(
                Size::Long,
                Operand::DataReg(DataReg::D2),
                Operand::DataReg(DataReg::D0),
            ),
            // Save for later
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D0),
                Operand::DataReg(DataReg::D3),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D4),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(8, AddrReg::A6),
                Operand::DataReg(DataReg::D5),
            ),
            // Write freq_hi first (reg 0xA4)
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D5),
                Operand::PreDec(AddrReg::A7),
            ),
            M68kInst::Move(Size::Long, Operand::Imm(0xA4), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D3),
                Operand::PreDec(AddrReg::A7),
            ),
            M68kInst::Bsr("ym_write_ch".to_string()),
            M68kInst::Adda(Size::Long, Operand::Imm(12), AddrReg::A7),
            // Write freq_lo (reg 0xA0)
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D5),
                Operand::PreDec(AddrReg::A7),
            ),
            M68kInst::Move(Size::Long, Operand::Imm(0xA0), Operand::PreDec(AddrReg::A7)),
            M68kInst::Andi(Size::Long, 0xFF, Operand::DataReg(DataReg::D4)),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D4),
                Operand::PreDec(AddrReg::A7),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(16, AddrReg::A6),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Lsl(Size::Long, Operand::Imm(3), DataReg::D0),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(12, AddrReg::A6),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Or(
                Size::Long,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D0),
            ),
            // ym_write_ch(ch, 0xB0, val)
            M68kInst::Move(
                Size::Long,
                Operand::Disp(8, AddrReg::A6),
                Operand::PreDec(AddrReg::A7),
            ),
            M68kInst::Move(Size::Long, Operand::Imm(0xB0), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D0),
                Operand::PreDec(AddrReg::A7),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(8, AddrReg::A6),
                Operand::PreDec(AddrReg::A7),
            ),
            M68kInst::Move(Size::Long, Operand::Imm(0xB4), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(12, AddrReg::A6),
                Operand::PreDec(AddrReg::A7),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(8, AddrReg::A6),
                Operand::PreDec(AddrReg::A7),
            ),
            M68kInst::Move(Size::Long, Operand::Imm(3), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(Size::Long, Operand::Imm(0x40), Operand::PreDec(AddrReg::A7)),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(12, AddrReg::A6),
                Operand::PreDec(AddrReg::A7),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(4, AddrReg::A7),
                Operand::DataReg(DataReg::D1),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(4, AddrReg::A7),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Lsl(Size::Word, Operand::Imm(5), DataReg::D0),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(8, AddrReg::A7),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D2),
            ),
            M68kInst::Andi(Size::Word, 0x0F, Operand::DataReg(DataReg::D2)),
            M68kInst::Or(
                Size::Word,
                Operand::DataReg(DataReg::D2),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Ori(Size::Word, 0x80, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(
                Size::Byte,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(PSG_PORT),
            ),
            // Data byte: (divider >> 4) & 0x3F
            M68kInst::Lsr(Size::Word, Operand::Imm(4), DataReg::D1),
            M68kInst::Andi(Size::Word, 0x3F, Operand::DataReg(DataReg::D1)),
            M68kInst::Move(
                Size::Byte,
                Operand::DataReg(DataReg::D1),
                Operand::AbsLong(PSG_PORT),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(12, AddrReg::A6),
                Operand::DataReg(DataReg::D1),
            ),
            // Clamp freq >= 1
            M68kInst::Tst(Size::Long, Operand::DataReg(DataReg::D1)),
            M68kInst::Bcc(Cond::Gt, ".psf_calc".to_string()),
            M68kInst::Moveq(1, DataReg::D1),
            M68kInst::Label(".psf_calc".to_string()),
            // 32 * freq
            M68kInst::Lsl(Size::Long, Operand::Imm(5), DataReg::D1),
            // 3579545 / (32 * freq)
            M68kInst::Move(
                Size::Long,
                Operand::Imm(3579545),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Divu(Operand::DataReg(DataReg::D1), DataReg::D0),
            M68kInst::Andi(Size::Long, 0xFFFF, Operand::DataReg(DataReg::D0)),
            // Clamp to 1023
            M68kInst::Cmpi(Size::Long, 1023, Operand::DataReg(DataReg::D0)),
            M68kInst::Bcc(Cond::Le, ".psf_ok".to_string()),
            M68kInst::Move(
                Size::Long,
                Operand::Imm(1023),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Label(".psf_ok".to_string()),
            // Call psg_set_tone(channel, divider)
            M68kInst::Move(
                Size::Long,
                Operand::Disp(8, AddrReg::A6),
                Operand::PreDec(AddrReg::A7),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D0),
                Operand::PreDec(AddrReg::A7),
            ),
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
            M68kInst::Move(
                Size::Long,
                Operand::Disp(8, AddrReg::A6),
                Operand::PreDec(AddrReg::A7),
            ),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(12, AddrReg::A6),
                Operand::PreDec(AddrReg::A7),
            ),
            M68kInst::Bsr("psg_set_tone".to_string()),
            M68kInst::Addq(Size::Long, 8, Operand::AddrReg(AddrReg::A7)),
            // psg_set_volume(channel, volume) - inline
            M68kInst::Move(
                Size::Long,
                Operand::Disp(8, AddrReg::A6),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Andi(Size::Word, 0x03, Operand::DataReg(DataReg::D0)),
            M68kInst::Lsl(Size::Word, Operand::Imm(5), DataReg::D0),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(16, AddrReg::A6),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Andi(Size::Word, 0x0F, Operand::DataReg(DataReg::D1)),
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

    // -------------------------------------------------------------------------
    // Sprite Library Functions
    // -------------------------------------------------------------------------

    /// Sprite Attribute Table base address (default at 0xF000 in VRAM)
    const SPRITE_TABLE: u32 = 0xF000;

    fn gen_sprite_init(&mut self) -> Vec<M68kInst> {
        // Initialize sprite table - set all sprites to hidden (y = 0 or offscreen)
        let loop_label = self.next_label("sprite_init");
        vec![
            M68kInst::Label("sprite_init".to_string()),
            // Set up VRAM write address to sprite table
            M68kInst::Move(
                Size::Long,
                Operand::Imm(Self::SPRITE_TABLE as i32),
                Operand::DataReg(DataReg::D0),
            ),
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
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_CTRL),
            ),
            // Clear 80 sprites (80 * 8 = 640 bytes)
            M68kInst::Moveq(79, DataReg::D0),
            M68kInst::Label(loop_label.clone()),
            // Y = 0 (offscreen), size/link = 0, attr = 0, X = 0
            M68kInst::Move(Size::Long, Operand::Imm(0), Operand::AbsLong(VDP_DATA)),
            M68kInst::Move(Size::Long, Operand::Imm(0), Operand::AbsLong(VDP_DATA)),
            M68kInst::Dbf(DataReg::D0, loop_label),
            M68kInst::Rts,
        ]
    }

    fn gen_sprite_set(&mut self) -> Vec<M68kInst> {
        // sprite_set(index, x, y, size, attr)
        // Args on stack as 32-bit longs: 4(SP)=index, 8(SP)=x, 12(SP)=y, 16(SP)=size, 20(SP)=attr
        // On big-endian 68k, to read low word of long at offset N, read from N+2
        vec![
            M68kInst::Label("sprite_set".to_string()),
            // Calculate sprite table address: SPRITE_TABLE + index * 8
            M68kInst::Move(
                Size::Long,
                Operand::Disp(4, AddrReg::A7),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Lsl(Size::Long, Operand::Imm(3), DataReg::D0), // index * 8
            M68kInst::Addi(
                Size::Long,
                Self::SPRITE_TABLE as i32,
                Operand::DataReg(DataReg::D0),
            ),
            // Set VRAM write address
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
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_CTRL),
            ),
            // Write Y position (y + 128) - y is at 12(SP), low word at 14(SP)
            M68kInst::Move(
                Size::Word,
                Operand::Disp(14, AddrReg::A7),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Addi(Size::Word, 128, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_DATA),
            ),
            // Write size/link (size in upper nibble, link = index+1)
            // size is at 16(SP), low word at 18(SP); index is at 4(SP), low word at 6(SP)
            M68kInst::Move(
                Size::Word,
                Operand::Disp(18, AddrReg::A7),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Lsl(Size::Word, Operand::Imm(8), DataReg::D0),
            M68kInst::Move(
                Size::Word,
                Operand::Disp(6, AddrReg::A7),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Addq(Size::Word, 1, Operand::DataReg(DataReg::D1)),
            M68kInst::Or(
                Size::Word,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_DATA),
            ),
            // Write attribute word - attr is at 20(SP), low word at 22(SP)
            M68kInst::Move(
                Size::Word,
                Operand::Disp(22, AddrReg::A7),
                Operand::AbsLong(VDP_DATA),
            ),
            // Write X position (x + 128) - x is at 8(SP), low word at 10(SP)
            M68kInst::Move(
                Size::Word,
                Operand::Disp(10, AddrReg::A7),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Addi(Size::Word, 128, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_DATA),
            ),
            M68kInst::Rts,
        ]
    }

    fn gen_sprite_set_pos(&mut self) -> Vec<M68kInst> {
        // sprite_set_pos(index, x, y)
        // Args on stack as 32-bit longs: 4(SP)=index, 8(SP)=x, 12(SP)=y
        // On big-endian 68k, to read low word of long at offset N, read from N+2
        vec![
            M68kInst::Label("sprite_set_pos".to_string()),
            // Calculate Y position address: SPRITE_TABLE + index * 8 + 0
            M68kInst::Move(
                Size::Long,
                Operand::Disp(4, AddrReg::A7),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Lsl(Size::Long, Operand::Imm(3), DataReg::D0),
            M68kInst::Addi(
                Size::Long,
                Self::SPRITE_TABLE as i32,
                Operand::DataReg(DataReg::D0),
            ),
            // Set VRAM write address
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
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_CTRL),
            ),
            // Write Y position - y is at 12(SP), low word at 14(SP)
            M68kInst::Move(
                Size::Word,
                Operand::Disp(14, AddrReg::A7),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Addi(Size::Word, 128, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_DATA),
            ),
            // Skip size/link word, write attr (need to read-modify-write for X only)
            // For simplicity, update X position at offset +6
            M68kInst::Move(
                Size::Long,
                Operand::Disp(4, AddrReg::A7),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Lsl(Size::Long, Operand::Imm(3), DataReg::D0),
            M68kInst::Addi(
                Size::Long,
                (Self::SPRITE_TABLE + 6) as i32,
                Operand::DataReg(DataReg::D0),
            ),
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
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_CTRL),
            ),
            // Write X position - x is at 8(SP), low word at 10(SP)
            M68kInst::Move(
                Size::Word,
                Operand::Disp(10, AddrReg::A7),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Addi(Size::Word, 128, Operand::DataReg(DataReg::D0)),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_DATA),
            ),
            M68kInst::Rts,
        ]
    }

    fn gen_sprite_hide(&mut self) -> Vec<M68kInst> {
        // sprite_hide(index) - set Y to 0 (offscreen) and link to 0 (end list)
        vec![
            M68kInst::Label("sprite_hide".to_string()),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(4, AddrReg::A7),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Lsl(Size::Long, Operand::Imm(3), DataReg::D0),
            M68kInst::Addi(
                Size::Long,
                Self::SPRITE_TABLE as i32,
                Operand::DataReg(DataReg::D0),
            ),
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
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_CTRL),
            ),
            // Y = 0, link = 0 (end sprite list)
            M68kInst::Clr(Size::Long, Operand::AbsLong(VDP_DATA)),
            M68kInst::Rts,
        ]
    }

    fn gen_sprite_clear(&mut self) -> Vec<M68kInst> {
        // Same as sprite_hide for now
        vec![
            M68kInst::Label("sprite_clear".to_string()),
            M68kInst::Bra("sprite_hide".to_string()),
        ]
    }

    fn gen_sprite_clear_all(&mut self) -> Vec<M68kInst> {
        vec![
            M68kInst::Label("sprite_clear_all".to_string()),
            M68kInst::Bra("sprite_init".to_string()),
        ]
    }

    fn gen_sprite_set_link(&mut self) -> Vec<M68kInst> {
        // sprite_set_link(index, next)
        // Args on stack as 32-bit longs: 4(SP)=index, 8(SP)=next
        // On big-endian 68k, to read low byte of long at offset N, read from N+3
        vec![
            M68kInst::Label("sprite_set_link".to_string()),
            // Address of link byte: SPRITE_TABLE + index * 8 + 3
            M68kInst::Move(
                Size::Long,
                Operand::Disp(4, AddrReg::A7),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Lsl(Size::Long, Operand::Imm(3), DataReg::D0),
            M68kInst::Addi(
                Size::Long,
                (Self::SPRITE_TABLE + 3) as i32,
                Operand::DataReg(DataReg::D0),
            ),
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
            M68kInst::Lsr(Size::Long, Operand::Imm(14), DataReg::D0),
            M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsLong(VDP_CTRL),
            ),
            // Write link value - next is at 8(SP), low byte at 11(SP)
            M68kInst::Move(
                Size::Byte,
                Operand::Disp(11, AddrReg::A7),
                Operand::AbsLong(VDP_DATA),
            ),
            M68kInst::Rts,
        ]
    }

    // -------------------------------------------------------------------------
    // Input Library Functions
    // -------------------------------------------------------------------------

    /// Joystick hardware addresses
    const JOY1_DATA: u32 = 0xA10003;
    const JOY1_CTRL: u32 = 0xA10009;
    const JOY2_DATA: u32 = 0xA10005;
    const JOY2_CTRL: u32 = 0xA1000B;

    fn gen_input_init(&mut self) -> Vec<M68kInst> {
        // Initialize controller ports
        vec![
            M68kInst::Label("input_init".to_string()),
            // Set port 1 control (output TH, input the rest)
            M68kInst::Move(
                Size::Byte,
                Operand::Imm(0x40),
                Operand::AbsLong(Self::JOY1_CTRL),
            ),
            // Set port 2 control
            M68kInst::Move(
                Size::Byte,
                Operand::Imm(0x40),
                Operand::AbsLong(Self::JOY2_CTRL),
            ),
            // Set TH high initially
            M68kInst::Move(
                Size::Byte,
                Operand::Imm(0x40),
                Operand::AbsLong(Self::JOY1_DATA),
            ),
            M68kInst::Move(
                Size::Byte,
                Operand::Imm(0x40),
                Operand::AbsLong(Self::JOY2_DATA),
            ),
            M68kInst::Rts,
        ]
    }

    fn gen_input_read(&mut self) -> Vec<M68kInst> {
        // input_read(port) - read 3-button state
        // Returns buttons active high
        let port1_label = self.next_label("port1");
        let done_label = self.next_label("done");
        vec![
            M68kInst::Label("input_read".to_string()),
            M68kInst::Move(
                Size::Long,
                Operand::Disp(4, AddrReg::A7),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Tst(Size::Long, Operand::DataReg(DataReg::D1)),
            M68kInst::Bcc(Cond::Eq, port1_label.clone()),
            // Port 2
            M68kInst::Move(
                Size::Byte,
                Operand::AbsLong(Self::JOY2_DATA),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Bra(done_label.clone()),
            // Port 1
            M68kInst::Label(port1_label),
            M68kInst::Move(
                Size::Byte,
                Operand::AbsLong(Self::JOY1_DATA),
                Operand::DataReg(DataReg::D0),
            ),
            M68kInst::Label(done_label),
            // Invert (active low -> active high)
            M68kInst::Not(Size::Byte, Operand::DataReg(DataReg::D0)),
            M68kInst::Andi(Size::Long, 0xFF, Operand::DataReg(DataReg::D0)),
            M68kInst::Rts,
        ]
    }

    fn gen_input_update(&mut self) -> Vec<M68kInst> {
        // Placeholder - for more complex input state tracking
        vec![M68kInst::Label("input_update".to_string()), M68kInst::Rts]
    }

    fn gen_input_held(&mut self) -> Vec<M68kInst> {
        // Same as input_read for simple implementation
        vec![
            M68kInst::Label("input_held".to_string()),
            M68kInst::Bra("input_read".to_string()),
        ]
    }

    fn gen_input_pressed(&mut self) -> Vec<M68kInst> {
        // For simple implementation, same as input_read
        vec![
            M68kInst::Label("input_pressed".to_string()),
            M68kInst::Bra("input_read".to_string()),
        ]
    }

    fn gen_input_released(&mut self) -> Vec<M68kInst> {
        // Return 0 for simple implementation
        vec![
            M68kInst::Label("input_released".to_string()),
            M68kInst::Moveq(0, DataReg::D0),
            M68kInst::Rts,
        ]
    }

    fn gen_input_is_6button(&mut self) -> Vec<M68kInst> {
        // Return 0 (not 6-button) for simple implementation
        vec![
            M68kInst::Label("input_is_6button".to_string()),
            M68kInst::Moveq(0, DataReg::D0),
            M68kInst::Rts,
        ]
    }
}

impl Default for SdkLibraryGenerator {
    fn default() -> Self {
        Self::new()
    }
}
