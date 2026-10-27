//! M68k binary disassembler
//!
//! Decodes machine code back into readable assembly listings for ROM
//! debugging and inspection. Covers the instruction subset produced by
//! `InstructionEncoder`; anything else is emitted as raw `dc.w` data.

use super::m68k::*;
use std::collections::HashMap;

/// Byte cursor over the code being disassembled.
struct Cursor<'a> {
    bytes: &'a [u8],
    pos: usize,
    base: u32,
    /// Reverse symbol map: address -> label name
    rev: &'a HashMap<u32, String>,
}

/// Decode the single instruction at byte offset `pos`.
///
/// Returns the instruction, the offset just past it, and — for static
/// control transfers (branches, DBF, absolute JSR/JMP) — the target address.
/// This is the decompiler's reuse seam into the decoder; the listing path
/// keeps its own cursor.
pub(crate) fn decode_at(
    bytes: &[u8],
    pos: usize,
    base: u32,
    rev: &HashMap<u32, String>,
) -> Option<(M68kInst, usize, Option<u32>)> {
    let mut cursor = Cursor {
        bytes,
        pos,
        base,
        rev,
    };
    let inst = cursor.decode_inst()?;
    let next = cursor.pos;

    // Recover the numeric target the decoder folded into a label string.
    let op = u16::from_be_bytes([*bytes.get(pos)?, *bytes.get(pos + 1)?]);
    let word = |off: usize| -> Option<i32> {
        Some(u16::from_be_bytes([
            *bytes.get(off)?,
            *bytes.get(off + 1)?,
        ]) as i16 as i32)
    };
    let addr = base + pos as u32;
    let target = match op >> 12 {
        0x6 => {
            let disp8 = op & 0xFF;
            if disp8 == 0xFF {
                None // 68020 long form
            } else {
                let disp = if disp8 == 0 {
                    word(pos + 2)?
                } else {
                    disp8 as i8 as i32
                };
                Some(addr.wrapping_add(2).wrapping_add_signed(disp))
            }
        }
        _ if op & 0xF0F8 == 0x50C8 => {
            // DBcc (any condition): displacement word follows the opword.
            Some(addr.wrapping_add(2).wrapping_add_signed(word(pos + 2)?))
        }
        _ if op & 0xFFC0 == 0x4E80 || op & 0xFFC0 == 0x4EC0 => {
            // JSR/JMP with an absolute-long operand (mode 7, reg 1).
            if op & 0x3F == 0x39 {
                Some(u32::from_be_bytes([
                    *bytes.get(pos + 2)?,
                    *bytes.get(pos + 3)?,
                    *bytes.get(pos + 4)?,
                    *bytes.get(pos + 5)?,
                ]))
            } else {
                None // register/memory indirect: not statically known
            }
        }
        _ => None,
    };

    Some((inst, next, target))
}

impl Cursor<'_> {
    fn read_u16(&mut self) -> Option<u16> {
        let b = self.bytes.get(self.pos..self.pos + 2)?;
        self.pos += 2;
        Some(u16::from_be_bytes([b[0], b[1]]))
    }

    fn read_u32(&mut self) -> Option<u32> {
        let b = self.bytes.get(self.pos..self.pos + 4)?;
        self.pos += 4;
        Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Address of the given byte offset.
    fn addr(&self, offset: usize) -> u32 {
        self.base + offset as u32
    }

    /// Format a code address as a known label or `$XXXXXX`.
    fn label_for(&self, target: u32) -> String {
        match self.rev.get(&target) {
            Some(name) => name.clone(),
            None => format!("${target:06X}"),
        }
    }

    /// Turn an absolute long address into a label operand when known.
    fn symbolize(&self, addr: u32) -> Operand {
        match self.rev.get(&addr) {
            Some(name) => Operand::Label(name.clone()),
            None => Operand::AbsLong(addr),
        }
    }

    /// Decode an effective address given mode/register fields.
    fn decode_ea(&mut self, mode: u16, reg: u16, size: Size) -> Option<Operand> {
        Some(match mode {
            0 => Operand::DataReg(data_reg(reg)),
            1 => Operand::AddrReg(addr_reg(reg)),
            2 => Operand::AddrInd(addr_reg(reg)),
            3 => Operand::PostInc(addr_reg(reg)),
            4 => Operand::PreDec(addr_reg(reg)),
            5 => Operand::Disp(self.read_u16()? as i16, addr_reg(reg)),
            6 => {
                let ext = self.read_u16()?;
                // Brief-format extension words only (full format has bit 8
                // set): D/A | reg | W/L | scale | 0 | disp8. The raw high
                // byte rides along so the operand re-encodes exactly.
                if ext & 0x0100 != 0 {
                    return None; // full format (68020)
                }
                let index = if ext & 0x8000 != 0 {
                    Reg::Addr(addr_reg((ext >> 12) & 7))
                } else {
                    Reg::Data(data_reg((ext >> 12) & 7))
                };
                Operand::Indexed(ext as u8 as i8, addr_reg(reg), index, (ext >> 8) as u8)
            }
            7 => match reg {
                0 => Operand::AbsShort(self.read_u16()? as i16),
                1 => {
                    let addr = self.read_u32()?;
                    self.symbolize(addr)
                }
                // d(PC): kept as a raw PC-relative displacement so it
                // re-encodes to the same bytes.
                2 => Operand::PcDisp(self.read_u16()? as i16),
                // d(PC,Xn): brief-format index, kept raw for re-encoding.
                3 => {
                    let ext = self.read_u16()?;
                    if ext & 0x0100 != 0 {
                        return None; // full format (68020)
                    }
                    let index = if ext & 0x8000 != 0 {
                        Reg::Addr(addr_reg((ext >> 12) & 7))
                    } else {
                        Reg::Data(data_reg((ext >> 12) & 7))
                    };
                    Operand::PcIndexed(ext as u8 as i8, index, (ext >> 8) as u8)
                }
                4 => match size {
                    // Byte immediates keep the full extension word so the
                    // bytes re-encode exactly (the high byte is normally 0,
                    // but foreign data can hold anything).
                    Size::Byte => Operand::Imm(self.read_u16()? as i32),
                    Size::Word => Operand::Imm(self.read_u16()? as i16 as i32),
                    Size::Long => Operand::Imm(self.read_u32()? as i32),
                },
                _ => return None,
            },
            _ => return None,
        })
    }

    /// Read a sized immediate (word for byte/word ops, long for long ops).
    /// Byte immediates keep the full extension word (high byte included) so
    /// garbage encodings re-encode byte-exactly.
    fn read_imm(&mut self, size: Size) -> Option<i32> {
        match size {
            Size::Byte => Some(self.read_u16()? as i32),
            Size::Word => Some(self.read_u16()? as i16 as i32),
            Size::Long => Some(self.read_u32()? as i32),
        }
    }

    /// Read a branch displacement and resolve the target label.
    /// `pc_ref` is the address the displacement is relative to.
    fn read_branch_target(&mut self, pc_ref: u32) -> Option<String> {
        let disp = self.read_u16()? as i16 as i32;
        let target = pc_ref.wrapping_add_signed(disp);
        Some(self.label_for(target))
    }

    /// Decode one instruction. On `None` the caller falls back to `dc.w`.
    fn decode_inst(&mut self) -> Option<M68kInst> {
        let start = self.pos;
        let op = self.read_u16()?;

        match op >> 12 {
            0x0 => self.decode_group0(op),
            0x1 => self.decode_move(op, Size::Byte),
            0x2 => self.decode_move(op, Size::Long),
            0x3 => self.decode_move(op, Size::Word),
            0x4 => self.decode_group4(op),
            0x5 => self.decode_group5(op, start),
            0x6 => self.decode_branch(op, start),
            0x7 => {
                if op & 0x0100 != 0 {
                    return None;
                }
                Some(M68kInst::Moveq(op as u8 as i8, data_reg((op >> 9) & 7)))
            }
            0x8 => self.decode_group8(op),
            0x9 => self.decode_add_sub(op, false),
            0xB => self.decode_group_b(op),
            0xC => self.decode_group_c(op),
            0xD => self.decode_add_sub(op, true),
            0xE => self.decode_shift(op),
            _ => None,
        }
    }

    /// Group 0: bit operations, immediate arithmetic/logic, MOVEP.
    fn decode_group0(&mut self, op: u16) -> Option<M68kInst> {
        let mode = (op >> 3) & 7;
        let ea_reg = op & 7;

        // MOVEP Dx,(d16,An) / (d16,An),Dx: bit 8 set with EA mode 001.
        // Must be checked before the dynamic bit ops, which share bit 8;
        // mis-decoding MOVEP drops its extension word and desyncs the cursor.
        if op & 0x0100 != 0 && mode == 1 {
            let disp = self.read_u16()? as i16;
            let size = if op & 0x0040 != 0 {
                Size::Word
            } else {
                Size::Long
            };
            let to_mem = op & 0x0080 == 0;
            return Some(M68kInst::Movep(
                size,
                to_mem,
                disp,
                addr_reg(ea_reg),
                data_reg((op >> 9) & 7),
            ));
        }

        // Immediate-to-CCR/SR forms: single extension word, special opwords.
        // Reserved encodings (SUBI/ADDI/CMPI to CCR/SR) fall through to None.
        match ((op >> 8) & 0x0F, op & 0x00FF) {
            (0x0, 0x3C) => return Some(M68kInst::OriToCcr(self.read_u16()?)),
            (0x0, 0x7C) => return Some(M68kInst::OriToSr(self.read_u16()?)),
            (0x2, 0x3C) => return Some(M68kInst::AndiToCcr(self.read_u16()?)),
            (0x2, 0x7C) => return Some(M68kInst::AndiToSr(self.read_u16()?)),
            (0xA, 0x3C) => return Some(M68kInst::EoriToCcr(self.read_u16()?)),
            (0xA, 0x7C) => return Some(M68kInst::EoriToSr(self.read_u16()?)),
            (0x4, 0x3C) | (0x4, 0x7C) | (0x6, 0x3C) | (0x6, 0x7C) | (0xC, 0x3C) | (0xC, 0x7C) => {
                return None
            }
            _ => {}
        }

        if op & 0x0100 != 0 {
            // Dynamic bit op: bit number in Dn
            let bit = Operand::DataReg(data_reg((op >> 9) & 7));
            let ea = self.decode_ea(mode, ea_reg, Size::Byte)?;
            return Some(match (op >> 6) & 3 {
                0 => M68kInst::Btst(bit, ea),
                1 => M68kInst::Bchg(bit, ea),
                2 => M68kInst::Bclr(bit, ea),
                _ => M68kInst::Bset(bit, ea),
            });
        }

        if op & 0x0F00 == 0x0800 {
            // Static bit op: immediate bit number word, then EA extension
            let bit = Operand::Imm(self.read_u16()? as i32);
            let ea = self.decode_ea(mode, ea_reg, Size::Byte)?;
            return Some(match (op >> 6) & 3 {
                0 => M68kInst::Btst(bit, ea),
                1 => M68kInst::Bchg(bit, ea),
                2 => M68kInst::Bclr(bit, ea),
                _ => M68kInst::Bset(bit, ea),
            });
        }

        // Immediate op: opword, immediate data, then EA extension
        let size = decode_size((op >> 6) & 3)?;
        let imm = self.read_imm(size)?;
        let ea = self.decode_ea(mode, ea_reg, size)?;
        Some(match (op >> 9) & 7 {
            0 => M68kInst::Ori(size, imm, ea),
            1 => M68kInst::Andi(size, imm, ea),
            2 => M68kInst::Subi(size, imm, ea),
            3 => M68kInst::Addi(size, imm, ea),
            5 => M68kInst::Eori(size, imm, ea),
            6 => M68kInst::Cmpi(size, imm, ea),
            _ => return None,
        })
    }

    fn decode_move(&mut self, op: u16, size: Size) -> Option<M68kInst> {
        let src = self.decode_ea((op >> 3) & 7, op & 7, size)?;
        let dst = self.decode_ea((op >> 6) & 7, (op >> 9) & 7, size)?;
        // Immediate/PC destinations can't exist; treat mode 7 reg 4 as invalid
        if matches!(dst, Operand::Imm(_)) {
            return None;
        }
        Some(M68kInst::Move(size, src, dst))
    }

    /// Group 4: miscellaneous (nop/rts/link/jsr/lea/clr/movem/...).
    fn decode_group4(&mut self, op: u16) -> Option<M68kInst> {
        let mode = (op >> 3) & 7;
        let ea_reg = op & 7;

        match op {
            0x4E71 => return Some(M68kInst::Nop),
            0x4E75 => return Some(M68kInst::Rts),
            0x4E73 => return Some(M68kInst::Rte),
            0x4E77 => return Some(M68kInst::Rtr),
            0x4E76 => return Some(M68kInst::Trapv),
            0x4E70 => return Some(M68kInst::Reset),
            0x4AFC => return Some(M68kInst::Illegal),
            _ => {}
        }
        if op & 0xFFF0 == 0x4E40 {
            return Some(M68kInst::Trap((op & 0xF) as u8));
        }
        if op == 0x4E72 {
            let imm = self.read_u16()?;
            return Some(M68kInst::Stop(imm));
        }
        if op & 0xFFF8 == 0x4E60 {
            return Some(M68kInst::MoveUspToAddr(addr_reg(op & 7)));
        }
        if op & 0xFFF8 == 0x4E68 {
            return Some(M68kInst::MoveAddrToUsp(addr_reg(op & 7)));
        }
        if op & 0xFFF8 == 0x4E50 {
            let disp = self.read_u16()? as i16;
            return Some(M68kInst::Link(addr_reg(op & 7), disp));
        }
        if op & 0xFFF8 == 0x4E58 {
            return Some(M68kInst::Unlk(addr_reg(op & 7)));
        }
        if op & 0xFFC0 == 0x4EC0 {
            return Some(M68kInst::Jmp(self.decode_ea(mode, ea_reg, Size::Long)?));
        }
        if op & 0xFFC0 == 0x4E80 {
            return Some(M68kInst::Jsr(self.decode_ea(mode, ea_reg, Size::Long)?));
        }
        if op & 0xFFC0 == 0x46C0 {
            // MOVE <ea>, SR
            let src = self.decode_ea(mode, ea_reg, Size::Word)?;
            return Some(M68kInst::Move(Size::Word, src, Operand::Sr));
        }
        if op & 0xFFC0 == 0x40C0 {
            // MOVE SR, <ea>
            let dst = self.decode_ea(mode, ea_reg, Size::Word)?;
            return Some(M68kInst::Move(Size::Word, Operand::Sr, dst));
        }
        if op & 0xFFC0 == 0x44C0 {
            // MOVE <ea>, CCR
            let src = self.decode_ea(mode, ea_reg, Size::Word)?;
            return Some(M68kInst::MoveToCcr(src));
        }
        if op & 0xF1C0 == 0x4180 {
            // CHK.W <ea>,Dn
            let src = self.decode_ea(mode, ea_reg, Size::Word)?;
            return Some(M68kInst::Chk(src, data_reg((op >> 9) & 7)));
        }
        if op & 0xFFF8 == 0x4840 {
            return Some(M68kInst::Swap(data_reg(op & 7)));
        }
        if op & 0xFFC0 == 0x4840 {
            return Some(M68kInst::Pea(self.decode_ea(mode, ea_reg, Size::Long)?));
        }
        if op & 0xFFC0 == 0x4800 {
            return Some(M68kInst::Nbcd(self.decode_ea(mode, ea_reg, Size::Byte)?));
        }
        if op & 0xFFF8 == 0x4880 {
            return Some(M68kInst::Ext(Size::Word, data_reg(op & 7)));
        }
        if op & 0xFFF8 == 0x48C0 {
            return Some(M68kInst::Ext(Size::Long, data_reg(op & 7)));
        }
        if op & 0xFB80 == 0x4880 {
            return self.decode_movem(op);
        }
        if op & 0xFFC0 == 0x4AC0 {
            return Some(M68kInst::Tas(self.decode_ea(mode, ea_reg, Size::Byte)?));
        }
        if op & 0xFF00 == 0x4000 {
            let size = decode_size((op >> 6) & 3)?;
            return Some(M68kInst::Negx(size, self.decode_ea(mode, ea_reg, size)?));
        }
        if op & 0xFF00 == 0x4200 {
            let size = decode_size((op >> 6) & 3)?;
            return Some(M68kInst::Clr(size, self.decode_ea(mode, ea_reg, size)?));
        }
        if op & 0xFF00 == 0x4400 {
            let size = decode_size((op >> 6) & 3)?;
            return Some(M68kInst::Neg(size, self.decode_ea(mode, ea_reg, size)?));
        }
        if op & 0xFF00 == 0x4600 {
            let size = decode_size((op >> 6) & 3)?;
            return Some(M68kInst::Not(size, self.decode_ea(mode, ea_reg, size)?));
        }
        if op & 0xFF00 == 0x4A00 {
            let size = decode_size((op >> 6) & 3)?;
            return Some(M68kInst::Tst(size, self.decode_ea(mode, ea_reg, size)?));
        }
        if op & 0xF1C0 == 0x41C0 {
            let src = self.decode_ea(mode, ea_reg, Size::Long)?;
            return Some(M68kInst::Lea(src, addr_reg((op >> 9) & 7)));
        }
        None
    }

    fn decode_movem(&mut self, op: u16) -> Option<M68kInst> {
        let to_mem = (op >> 10) & 1 == 0;
        let size = if (op >> 6) & 1 == 0 {
            Size::Word
        } else {
            Size::Long
        };
        let mode = (op >> 3) & 7;
        // Valid MOVEM effective addresses are control-alterable modes:
        // bare registers (0/1), PC-relative (7.2/7.3) and immediate (7.4)
        // are invalid, and encodings using them are data.
        let ea_reg = op & 7;
        if mode == 0 || mode == 1 || (mode == 7 && ea_reg >= 2) {
            return None;
        }
        let mask = self.read_u16()?;
        let ea = self.decode_ea(mode, op & 7, size)?;
        let reverse = to_mem && matches!(ea, Operand::PreDec(_));

        let mut regs = Vec::new();
        for idx in 0u16..16 {
            let bit = if reverse { 15 - idx } else { idx };
            if mask & (1 << bit) != 0 {
                regs.push(if idx < 8 {
                    Reg::Data(data_reg(idx))
                } else {
                    Reg::Addr(addr_reg(idx - 8))
                });
            }
        }
        Some(M68kInst::Movem(size, regs, ea, to_mem))
    }

    /// Group 5: ADDQ/SUBQ, Scc, DBcc.
    fn decode_group5(&mut self, op: u16, start: usize) -> Option<M68kInst> {
        let mode = (op >> 3) & 7;
        if (op >> 6) & 3 == 3 {
            if mode == 1 {
                // DBcc (word displacement only on the 68000)
                let pc_ref = self.addr(start) + 2;
                let label = self.read_branch_target(pc_ref)?;
                return Some(M68kInst::Dbcc(
                    decode_cond((op >> 8) & 0xF),
                    data_reg(op & 7),
                    label,
                ));
            }
            let cond = decode_cond((op >> 8) & 0xF);
            let ea = self.decode_ea(mode, op & 7, Size::Byte)?;
            return Some(M68kInst::Scc(cond, ea));
        }

        let size = decode_size((op >> 6) & 3)?;
        let data = match (op >> 9) & 7 {
            0 => 8u8,
            n => n as u8,
        };
        let ea = self.decode_ea(mode, op & 7, size)?;
        Some(if op & 0x0100 == 0 {
            M68kInst::Addq(size, data, ea)
        } else {
            M68kInst::Subq(size, data, ea)
        })
    }

    /// Group 6: BRA/BSR/Bcc in short (8-bit) and word (16-bit) forms.
    fn decode_branch(&mut self, op: u16, start: usize) -> Option<M68kInst> {
        let disp8 = op & 0xFF;
        let pc_ref = self.addr(start) + 2;
        if disp8 == 0xFF {
            return None; // 68020 long form
        }
        let cond = (op >> 8) & 0xF;
        if disp8 == 0 {
            // Word displacement follows
            let label = self.read_branch_target(pc_ref)?;
            Some(match cond {
                0 => M68kInst::Bra(label),
                1 => M68kInst::Bsr(label),
                c => M68kInst::Bcc(decode_cond(c), label),
            })
        } else {
            let target = pc_ref.wrapping_add_signed(disp8 as u8 as i8 as i32);
            let label = self.label_for(target);
            Some(match cond {
                0 => M68kInst::BraS(label),
                1 => M68kInst::BsrS(label),
                c => M68kInst::BccS(decode_cond(c), label),
            })
        }
    }

    /// Group 8: OR, DIVS, DIVU, SBCD.
    fn decode_group8(&mut self, op: u16) -> Option<M68kInst> {
        if op & 0xF1F0 == 0x8100 {
            // SBCD Dy,Dx (m=0) or -(Ay),-(Ax) (m=1)
            let src = self.bcd_operand((op & 8) != 0, op & 7)?;
            let dst = self.bcd_operand((op & 8) != 0, (op >> 9) & 7)?;
            return Some(M68kInst::Sbcd(src, dst));
        }
        if op & 0xF1C0 == 0x81C0 {
            let src = self.decode_ea((op >> 3) & 7, op & 7, Size::Word)?;
            return Some(M68kInst::Divs(src, data_reg((op >> 9) & 7)));
        }
        if op & 0xF1C0 == 0x80C0 {
            let src = self.decode_ea((op >> 3) & 7, op & 7, Size::Word)?;
            return Some(M68kInst::Divu(src, data_reg((op >> 9) & 7)));
        }
        let size = decode_size((op >> 6) & 3)?;
        let dn = Operand::DataReg(data_reg((op >> 9) & 7));
        let ea = self.decode_ea((op >> 3) & 7, op & 7, size)?;
        Some(if op & 0x0100 == 0 {
            M68kInst::Or(size, ea, dn)
        } else {
            M68kInst::Or(size, dn, ea)
        })
    }

    /// Groups 9/D: SUB/SUBA and ADD/ADDA.
    fn decode_add_sub(&mut self, op: u16, is_add: bool) -> Option<M68kInst> {
        let opmode = (op >> 6) & 7;
        let ea_mode = (op >> 3) & 7;
        let ea_reg = op & 7;

        if opmode == 3 || opmode == 7 {
            let size = if opmode == 3 { Size::Word } else { Size::Long };
            let src = self.decode_ea(ea_mode, ea_reg, size)?;
            let an = addr_reg((op >> 9) & 7);
            return Some(if is_add {
                M68kInst::Adda(size, src, an)
            } else {
                M68kInst::Suba(size, src, an)
            });
        }

        // ADDX/SUBX: memory-direction opmodes (4-6) with EA mode 0 (Dy,Dx)
        // or mode 1 (-(Ay),-(Ax)), which are invalid as normal EAs.
        if (4..=6).contains(&opmode) && (ea_mode == 0 || ea_mode == 1) {
            let size = decode_size(opmode & 3)?;
            let src = self.bcd_operand(ea_mode == 1, ea_reg)?;
            let dst = self.bcd_operand(ea_mode == 1, (op >> 9) & 7)?;
            return Some(if is_add {
                M68kInst::Addx(size, src, dst)
            } else {
                M68kInst::Subx(size, src, dst)
            });
        }

        let size = decode_size(opmode & 3)?;
        let dn = Operand::DataReg(data_reg((op >> 9) & 7));
        let ea = self.decode_ea(ea_mode, ea_reg, size)?;
        Some(match (is_add, opmode < 4) {
            (true, true) => M68kInst::Add(size, ea, dn),
            (true, false) => M68kInst::Add(size, dn, ea),
            (false, true) => M68kInst::Sub(size, ea, dn),
            (false, false) => M68kInst::Sub(size, dn, ea),
        })
    }

    /// Group B: CMP, CMPA, EOR.
    fn decode_group_b(&mut self, op: u16) -> Option<M68kInst> {
        let opmode = (op >> 6) & 7;
        let ea_mode = (op >> 3) & 7;
        let ea_reg = op & 7;
        let reg9 = (op >> 9) & 7;

        match opmode {
            0..=2 => {
                let size = decode_size(opmode)?;
                let src = self.decode_ea(ea_mode, ea_reg, size)?;
                Some(M68kInst::Cmp(size, src, Operand::DataReg(data_reg(reg9))))
            }
            3 | 7 => {
                let size = if opmode == 3 { Size::Word } else { Size::Long };
                let src = self.decode_ea(ea_mode, ea_reg, size)?;
                Some(M68kInst::Cmpa(size, src, addr_reg(reg9)))
            }
            _ => {
                // CMPM (Ay)+,(Ax)+: opmode 4-6 with EA mode 001
                if ea_mode == 1 {
                    let size = decode_size(opmode & 3)?;
                    return Some(M68kInst::Cmpm(size, addr_reg(ea_reg), addr_reg(reg9)));
                }
                let size = decode_size(opmode & 3)?;
                let ea = self.decode_ea(ea_mode, ea_reg, size)?;
                Some(M68kInst::Eor(size, data_reg(reg9), ea))
            }
        }
    }

    /// Register (m=0) or predecrement (m=1) operand for the BCD and
    /// extend-flag arithmetic forms.
    fn bcd_operand(&self, m: bool, reg: u16) -> Option<Operand> {
        Some(if m {
            Operand::PreDec(addr_reg(reg))
        } else {
            Operand::DataReg(data_reg(reg))
        })
    }

    /// Group C: EXG, MULS, MULU, AND, ABCD.
    fn decode_group_c(&mut self, op: u16) -> Option<M68kInst> {
        if op & 0xF1F0 == 0xC100 {
            // ABCD Dy,Dx (m=0) or -(Ay),-(Ax) (m=1)
            let src = self.bcd_operand((op & 8) != 0, op & 7)?;
            let dst = self.bcd_operand((op & 8) != 0, (op >> 9) & 7)?;
            return Some(M68kInst::Abcd(src, dst));
        }
        if op & 0xF1F8 == 0xC140 {
            return Some(M68kInst::Exg(
                Reg::Data(data_reg((op >> 9) & 7)),
                Reg::Data(data_reg(op & 7)),
            ));
        }
        if op & 0xF1F8 == 0xC148 {
            return Some(M68kInst::Exg(
                Reg::Addr(addr_reg((op >> 9) & 7)),
                Reg::Addr(addr_reg(op & 7)),
            ));
        }
        if op & 0xF1F8 == 0xC188 {
            return Some(M68kInst::Exg(
                Reg::Data(data_reg((op >> 9) & 7)),
                Reg::Addr(addr_reg(op & 7)),
            ));
        }
        if op & 0xF1C0 == 0xC1C0 {
            let src = self.decode_ea((op >> 3) & 7, op & 7, Size::Word)?;
            return Some(M68kInst::Muls(src, data_reg((op >> 9) & 7)));
        }
        if op & 0xF1C0 == 0xC0C0 {
            let src = self.decode_ea((op >> 3) & 7, op & 7, Size::Word)?;
            return Some(M68kInst::Mulu(src, data_reg((op >> 9) & 7)));
        }
        let size = decode_size((op >> 6) & 3)?;
        let dn = Operand::DataReg(data_reg((op >> 9) & 7));
        let ea = self.decode_ea((op >> 3) & 7, op & 7, size)?;
        Some(if op & 0x0100 == 0 {
            M68kInst::And(size, ea, dn)
        } else {
            M68kInst::And(size, dn, ea)
        })
    }

    /// Group E: shifts and rotates (register and memory forms).
    fn decode_shift(&mut self, op: u16) -> Option<M68kInst> {
        // Memory form (size bits 11): shift a word EA by one.
        // Type is bits 11-9 (0=AS, 1=LS, 2=ROX, 3=RO), direction bit 8.
        if (op >> 6) & 3 == 3 {
            let ea = self.decode_ea((op >> 3) & 7, op & 7, Size::Word)?;
            return match ((op >> 9) & 7, (op >> 8) & 1) {
                (0, 1) => Some(M68kInst::AslMem(ea)),
                (0, 0) => Some(M68kInst::AsrMem(ea)),
                (1, 1) => Some(M68kInst::LslMem(ea)),
                (1, 0) => Some(M68kInst::LsrMem(ea)),
                (2, 1) => Some(M68kInst::RoxlMem(ea)),
                (2, 0) => Some(M68kInst::RoxrMem(ea)),
                (3, 1) => Some(M68kInst::RolMem(ea)),
                (3, 0) => Some(M68kInst::RorMem(ea)),
                _ => None, // reserved encoding
            };
        }

        let size = decode_size((op >> 6) & 3)?;
        let left = (op >> 8) & 1 == 1;
        let count_field = (op >> 9) & 7;
        let reg = data_reg(op & 7);

        let count = if (op >> 5) & 1 == 0 {
            Operand::Imm(if count_field == 0 {
                8
            } else {
                count_field as i32
            })
        } else {
            Operand::DataReg(data_reg(count_field))
        };

        Some(match ((op >> 3) & 3, left) {
            (0, true) => M68kInst::Asl(size, count, reg),
            (0, false) => M68kInst::Asr(size, count, reg),
            (1, true) => M68kInst::Lsl(size, count, reg),
            (1, false) => M68kInst::Lsr(size, count, reg),
            (2, true) => M68kInst::Roxl(size, count, reg),
            (2, false) => M68kInst::Roxr(size, count, reg),
            (3, true) => M68kInst::Rol(size, count, reg),
            (3, false) => M68kInst::Ror(size, count, reg),
            _ => return None,
        })
    }
}

fn data_reg(n: u16) -> DataReg {
    match n & 7 {
        0 => DataReg::D0,
        1 => DataReg::D1,
        2 => DataReg::D2,
        3 => DataReg::D3,
        4 => DataReg::D4,
        5 => DataReg::D5,
        6 => DataReg::D6,
        _ => DataReg::D7,
    }
}

fn addr_reg(n: u16) -> AddrReg {
    match n & 7 {
        0 => AddrReg::A0,
        1 => AddrReg::A1,
        2 => AddrReg::A2,
        3 => AddrReg::A3,
        4 => AddrReg::A4,
        5 => AddrReg::A5,
        6 => AddrReg::A6,
        _ => AddrReg::A7,
    }
}

fn decode_size(bits: u16) -> Option<Size> {
    match bits {
        0 => Some(Size::Byte),
        1 => Some(Size::Word),
        2 => Some(Size::Long),
        _ => None,
    }
}

fn decode_cond(code: u16) -> Cond {
    match code {
        0 => Cond::True,
        1 => Cond::False,
        2 => Cond::Hi,
        3 => Cond::Ls,
        4 => Cond::Cc,
        5 => Cond::Cs,
        6 => Cond::Ne,
        7 => Cond::Eq,
        8 => Cond::Vc,
        9 => Cond::Vs,
        10 => Cond::Pl,
        11 => Cond::Mi,
        12 => Cond::Ge,
        13 => Cond::Lt,
        14 => Cond::Gt,
        _ => Cond::Le,
    }
}

/// Disassemble a code region into a listing with addresses, raw bytes,
/// mnemonics, and label lines from the symbol table.
///
/// `data_start` marks the ROM address where the inline data section
/// begins; from there on bytes are dumped as `dc.w` rows instead of
/// being decoded as instructions.
pub fn disassemble_listing(
    code: &[u8],
    base_addr: u32,
    symbols: &HashMap<String, u32>,
    data_start: Option<u32>,
) -> String {
    // Reverse map, preferring user-visible names over compiler-internal ones
    let mut rev: HashMap<u32, String> = HashMap::new();
    let mut sorted: Vec<_> = symbols.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    for (name, &addr) in sorted {
        let entry = rev.entry(addr).or_insert_with(|| name.clone());
        let existing_internal = entry.starts_with('.') || entry.starts_with("__");
        let new_internal = name.starts_with('.') || name.starts_with("__");
        if existing_internal && !new_internal {
            entry.clone_from(name);
        }
    }

    let mut out = String::new();
    let mut offset = 0usize;

    while offset < code.len() {
        let addr = base_addr + offset as u32;

        if let Some(ds) = data_start
            && addr >= ds
        {
            out.push_str("\n; ---- data section (raw dump) ----\n");
            dump_data_words(&mut out, &code[offset..], addr);
            break;
        }

        if let Some(name) = rev.get(&addr) {
            out.push_str(name);
            out.push_str(":\n");
        }

        let mut cur = Cursor {
            bytes: code,
            pos: offset,
            base: base_addr,
            rev: &rev,
        };

        let (consumed, text) = match cur.decode_inst() {
            Some(inst) => (cur.pos - offset, inst.format().trim_start().to_string()),
            None => {
                // Undecodable: emit one raw word (or a final odd byte)
                if offset + 2 <= code.len() {
                    let w = u16::from_be_bytes([code[offset], code[offset + 1]]);
                    (2, format!("dc.w    ${w:04X}"))
                } else {
                    (1, format!("dc.b    ${:02X}", code[offset]))
                }
            }
        };

        // Don't let a decoded instruction run past a label or the data
        // section boundary; re-emit as raw data if it would.
        let overruns_boundary = (offset + 1..offset + consumed).any(|o| {
            let a = base_addr + o as u32;
            (o % 2 == 0 && rev.contains_key(&a)) || data_start.is_some_and(|ds| a == ds)
        });
        let (consumed, text) = if overruns_boundary && consumed > 2 {
            let w = u16::from_be_bytes([code[offset], code[offset + 1]]);
            (2, format!("dc.w    ${w:04X}"))
        } else {
            (consumed, text)
        };

        let hex: Vec<String> = code[offset..offset + consumed]
            .chunks(2)
            .map(|c| {
                if c.len() == 2 {
                    format!("{:02X}{:02X}", c[0], c[1])
                } else {
                    format!("{:02X}", c[0])
                }
            })
            .collect();

        use std::fmt::Write;
        let _ = writeln!(out, "{addr:08X}  {:<24}  {text}", hex.join(" "));

        offset += consumed;
    }

    out
}

/// Dump raw bytes as `dc.w` rows (8 words per line) with addresses.
fn dump_data_words(out: &mut String, data: &[u8], base_addr: u32) {
    use std::fmt::Write;
    for (i, row) in data.chunks(16).enumerate() {
        let addr = base_addr + (i * 16) as u32;
        let words: Vec<String> = row
            .chunks(2)
            .map(|c| {
                if c.len() == 2 {
                    format!("${:02X}{:02X}", c[0], c[1])
                } else {
                    format!("${:02X}", c[0])
                }
            })
            .collect();
        let _ = writeln!(out, "{addr:08X}  dc.w    {}", words.join(", "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::m68k::{Assembler, InstructionEncoder};

    /// Encode a single instruction and decode it back; the formatted text
    /// must match the original.
    fn roundtrip(inst: M68kInst) {
        let mut encoder = InstructionEncoder::new();
        let bytes = encoder.encode(&inst).unwrap();
        let rev = HashMap::new();
        let mut cur = Cursor {
            bytes: &bytes,
            pos: 0,
            base: 0,
            rev: &rev,
        };
        let decoded = cur
            .decode_inst()
            .unwrap_or_else(|| panic!("failed to decode {inst:?} ({bytes:02X?})"));
        assert_eq!(
            decoded.format(),
            inst.format(),
            "roundtrip mismatch for {inst:?} ({bytes:02X?})"
        );
        assert_eq!(cur.pos, bytes.len(), "length mismatch for {inst:?}");
    }

    #[test]
    fn roundtrip_data_movement() {
        roundtrip(M68kInst::Nop);
        roundtrip(M68kInst::Rts);
        roundtrip(M68kInst::Rte);
        roundtrip(M68kInst::Moveq(-5, DataReg::D3));
        roundtrip(M68kInst::Swap(DataReg::D2));
        roundtrip(M68kInst::Move(
            Size::Long,
            Operand::DataReg(DataReg::D0),
            Operand::DataReg(DataReg::D1),
        ));
        roundtrip(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x1234),
            Operand::Disp(-8, AddrReg::A6),
        ));
        roundtrip(M68kInst::Move(
            Size::Byte,
            Operand::PostInc(AddrReg::A0),
            Operand::PreDec(AddrReg::A1),
        ));
        roundtrip(M68kInst::Move(
            Size::Long,
            Operand::AbsLong(0x00FF8000),
            Operand::DataReg(DataReg::D0),
        ));
        roundtrip(M68kInst::Move(
            Size::Word,
            Operand::DataReg(DataReg::D0),
            Operand::Sr,
        ));
        roundtrip(M68kInst::Move(
            Size::Word,
            Operand::Sr,
            Operand::DataReg(DataReg::D1),
        ));
        roundtrip(M68kInst::Lea(Operand::AbsLong(0x00C00004), AddrReg::A1));
        roundtrip(M68kInst::Lea(Operand::Disp(16, AddrReg::A6), AddrReg::A0));
        roundtrip(M68kInst::Pea(Operand::Disp(-4, AddrReg::A6)));
        roundtrip(M68kInst::Clr(Size::Long, Operand::DataReg(DataReg::D0)));
        roundtrip(M68kInst::Exg(
            Reg::Data(DataReg::D0),
            Reg::Data(DataReg::D1),
        ));
        roundtrip(M68kInst::Exg(
            Reg::Addr(AddrReg::A0),
            Reg::Addr(AddrReg::A1),
        ));
        roundtrip(M68kInst::Exg(
            Reg::Data(DataReg::D2),
            Reg::Addr(AddrReg::A3),
        ));
    }

    #[test]
    fn roundtrip_arithmetic() {
        roundtrip(M68kInst::Add(
            Size::Long,
            Operand::Disp(8, AddrReg::A6),
            Operand::DataReg(DataReg::D0),
        ));
        roundtrip(M68kInst::Add(
            Size::Word,
            Operand::DataReg(DataReg::D1),
            Operand::AddrInd(AddrReg::A0),
        ));
        roundtrip(M68kInst::Sub(
            Size::Long,
            Operand::DataReg(DataReg::D1),
            Operand::DataReg(DataReg::D0),
        ));
        roundtrip(M68kInst::Adda(
            Size::Long,
            Operand::DataReg(DataReg::D0),
            AddrReg::A2,
        ));
        roundtrip(M68kInst::Suba(Size::Word, Operand::Imm(4), AddrReg::A7));
        roundtrip(M68kInst::Addq(Size::Long, 8, Operand::AddrReg(AddrReg::A7)));
        roundtrip(M68kInst::Subq(Size::Word, 1, Operand::DataReg(DataReg::D2)));
        roundtrip(M68kInst::Addi(
            Size::Long,
            100,
            Operand::DataReg(DataReg::D0),
        ));
        roundtrip(M68kInst::Subi(
            Size::Word,
            -5,
            Operand::DataReg(DataReg::D1),
        ));
        roundtrip(M68kInst::Cmpi(
            Size::Long,
            0x123456,
            Operand::DataReg(DataReg::D0),
        ));
        roundtrip(M68kInst::Muls(Operand::DataReg(DataReg::D1), DataReg::D0));
        roundtrip(M68kInst::Mulu(Operand::Disp(8, AddrReg::A6), DataReg::D2));
        roundtrip(M68kInst::Divs(Operand::DataReg(DataReg::D3), DataReg::D0));
        roundtrip(M68kInst::Divu(Operand::DataReg(DataReg::D4), DataReg::D1));
        roundtrip(M68kInst::Neg(Size::Long, Operand::DataReg(DataReg::D0)));
        roundtrip(M68kInst::Ext(Size::Word, DataReg::D0));
        roundtrip(M68kInst::Ext(Size::Long, DataReg::D5));
        roundtrip(M68kInst::Cmp(
            Size::Long,
            Operand::Disp(-12, AddrReg::A6),
            Operand::DataReg(DataReg::D1),
        ));
        roundtrip(M68kInst::Cmpa(
            Size::Long,
            Operand::AddrReg(AddrReg::A1),
            AddrReg::A0,
        ));
        roundtrip(M68kInst::Tst(Size::Byte, Operand::DataReg(DataReg::D0)));
    }

    #[test]
    fn roundtrip_logic_shift_bits() {
        roundtrip(M68kInst::And(
            Size::Long,
            Operand::DataReg(DataReg::D1),
            Operand::DataReg(DataReg::D0),
        ));
        roundtrip(M68kInst::Or(
            Size::Word,
            Operand::Disp(4, AddrReg::A0),
            Operand::DataReg(DataReg::D2),
        ));
        roundtrip(M68kInst::Andi(
            Size::Long,
            0xFF,
            Operand::DataReg(DataReg::D0),
        ));
        roundtrip(M68kInst::Ori(
            Size::Word,
            0x8000 - 0x10000,
            Operand::DataReg(DataReg::D1),
        ));
        roundtrip(M68kInst::Eor(
            Size::Long,
            DataReg::D1,
            Operand::DataReg(DataReg::D0),
        ));
        roundtrip(M68kInst::Eori(
            Size::Word,
            0x00FF,
            Operand::DataReg(DataReg::D3),
        ));
        roundtrip(M68kInst::Not(Size::Word, Operand::DataReg(DataReg::D4)));
        roundtrip(M68kInst::Lsl(Size::Long, Operand::Imm(4), DataReg::D0));
        roundtrip(M68kInst::Lsr(
            Size::Word,
            Operand::DataReg(DataReg::D1),
            DataReg::D2,
        ));
        roundtrip(M68kInst::Asl(Size::Long, Operand::Imm(8), DataReg::D3));
        roundtrip(M68kInst::Asr(Size::Long, Operand::Imm(2), DataReg::D0));
        roundtrip(M68kInst::Rol(Size::Word, Operand::Imm(1), DataReg::D5));
        roundtrip(M68kInst::Ror(
            Size::Long,
            Operand::DataReg(DataReg::D6),
            DataReg::D7,
        ));
        roundtrip(M68kInst::Btst(
            Operand::Imm(3),
            Operand::DataReg(DataReg::D0),
        ));
        roundtrip(M68kInst::Bset(
            Operand::DataReg(DataReg::D1),
            Operand::AddrInd(AddrReg::A0),
        ));
        roundtrip(M68kInst::Bclr(
            Operand::Imm(7),
            Operand::Disp(2, AddrReg::A1),
        ));
        roundtrip(M68kInst::Bchg(
            Operand::Imm(0),
            Operand::DataReg(DataReg::D2),
        ));
        roundtrip(M68kInst::Scc(Cond::Eq, Operand::DataReg(DataReg::D0)));
        roundtrip(M68kInst::Scc(Cond::Lt, Operand::DataReg(DataReg::D3)));
    }

    /// Foreign-ROM instruction extensions: everything added for the byte-exact
    /// round trip must decode and re-encode to identical bytes.
    #[test]
    fn roundtrip_foreign_instruction_extensions() {
        roundtrip(M68kInst::Negx(Size::Long, Operand::DataReg(DataReg::D0)));
        roundtrip(M68kInst::Negx(Size::Byte, Operand::AddrInd(AddrReg::A1)));
        roundtrip(M68kInst::Addx(
            Size::Long,
            Operand::DataReg(DataReg::D1),
            Operand::DataReg(DataReg::D0),
        ));
        roundtrip(M68kInst::Subx(
            Size::Byte,
            Operand::PreDec(AddrReg::A1),
            Operand::PreDec(AddrReg::A0),
        ));
        roundtrip(M68kInst::Abcd(
            Operand::DataReg(DataReg::D1),
            Operand::DataReg(DataReg::D0),
        ));
        roundtrip(M68kInst::Sbcd(
            Operand::PreDec(AddrReg::A2),
            Operand::PreDec(AddrReg::A0),
        ));
        roundtrip(M68kInst::Nbcd(Operand::AddrInd(AddrReg::A0)));
        roundtrip(M68kInst::Cmpm(Size::Word, AddrReg::A1, AddrReg::A0));
        roundtrip(M68kInst::Chk(Operand::AddrInd(AddrReg::A0), DataReg::D2));
        roundtrip(M68kInst::Movep(Size::Word, true, 4, AddrReg::A1, DataReg::D0));
        roundtrip(M68kInst::Movep(Size::Long, false, -2, AddrReg::A2, DataReg::D3));
        roundtrip(M68kInst::MoveToCcr(Operand::DataReg(DataReg::D0)));
        roundtrip(M68kInst::OriToCcr(0x27));
        roundtrip(M68kInst::AndiToCcr(0xF8));
        roundtrip(M68kInst::EoriToCcr(0x10));
        roundtrip(M68kInst::OriToSr(0x0700));
        roundtrip(M68kInst::AndiToSr(0xF8FF));
        roundtrip(M68kInst::EoriToSr(0x0001));
        roundtrip(M68kInst::MoveUspToAddr(AddrReg::A0));
        roundtrip(M68kInst::MoveAddrToUsp(AddrReg::A3));
        roundtrip(M68kInst::Tas(Operand::AddrInd(AddrReg::A0)));
        roundtrip(M68kInst::Rtr);
        roundtrip(M68kInst::Trapv);
        roundtrip(M68kInst::Reset);
        roundtrip(M68kInst::Illegal);
        roundtrip(M68kInst::Trap(7));
        roundtrip(M68kInst::Stop(0x2700));
        roundtrip(M68kInst::Roxl(Size::Word, Operand::Imm(4), DataReg::D1));
        roundtrip(M68kInst::Roxr(
            Size::Long,
            Operand::DataReg(DataReg::D2),
            DataReg::D0,
        ));
        roundtrip(M68kInst::AslMem(Operand::AddrInd(AddrReg::A0)));
        roundtrip(M68kInst::AsrMem(Operand::PostInc(AddrReg::A3)));
        roundtrip(M68kInst::LslMem(Operand::Disp(4, AddrReg::A1)));
        roundtrip(M68kInst::LsrMem(Operand::PreDec(AddrReg::A5)));
        roundtrip(M68kInst::RolMem(Operand::AbsShort(0x10)));
        roundtrip(M68kInst::RorMem(Operand::AbsLong(0x00C00000)));
        roundtrip(M68kInst::RoxlMem(Operand::AddrInd(AddrReg::A2)));
        roundtrip(M68kInst::RoxrMem(Operand::Disp(-2, AddrReg::A6)));
    }

    /// PC-relative and indexed operands must survive a decode/encode cycle
    /// without being folded into absolute addresses.
    #[test]
    fn roundtrip_pc_relative_and_indexed_operands() {
        roundtrip(M68kInst::Move(
            Size::Byte,
            Operand::PcDisp(-4),
            Operand::DataReg(DataReg::D0),
        ));
        roundtrip(M68kInst::Move(
            Size::Word,
            // hi byte encodes the same register as the Reg field (D2 -> 0x20).
            Operand::PcIndexed(2, Reg::Data(DataReg::D2), 0x20),
            Operand::DataReg(DataReg::D1),
        ));
        roundtrip(M68kInst::Lea(
            // A3 -> hi = D/A | reg 3 = 0xB0.
            Operand::PcIndexed(0, Reg::Addr(AddrReg::A3), 0xB0),
            AddrReg::A0,
        ));
        roundtrip(M68kInst::Jsr(Operand::PcIndexed(6, Reg::Data(DataReg::D0), 0)));
        roundtrip(M68kInst::Move(
            Size::Byte,
            Operand::Indexed(0, AddrReg::A0, Reg::Data(DataReg::D0), 0),
            Operand::DataReg(DataReg::D3),
        ));
        roundtrip(M68kInst::Move(
            Size::Word,
            // A2 index -> hi = 0xA0.
            Operand::Indexed(-1, AddrReg::A5, Reg::Addr(AddrReg::A2), 0xA0),
            Operand::AddrInd(AddrReg::A1),
        ));
    }

    /// Short (8-bit) branches encode with the displacement in the opword and
    /// re-decode to the same short form.
    #[test]
    fn short_branches_roundtrip() {
        let mut asm = Assembler::new(0x200);
        let instructions = vec![
            M68kInst::Label("loop".to_string()),
            M68kInst::BraS("loop".to_string()),
            M68kInst::BccS(Cond::Eq, "loop".to_string()),
            M68kInst::BsrS("loop".to_string()),
            M68kInst::BraS("end".to_string()),
            M68kInst::Nop, // keeps `end` two bytes past the bra.s PC reference
            M68kInst::Label("end".to_string()),
        ];
        let bytes = asm.assemble(&instructions).unwrap();
        assert_eq!(bytes[0..2], [0x60, 0xFE]); // bra.s -2
        assert_eq!(bytes[2..4], [0x67, 0xFC]); // beq.s: pc_ref 0x204 -> 0x200
        assert_eq!(bytes[4..6], [0x61, 0xFA]); // bsr.s: pc_ref 0x206 -> 0x200
        assert_eq!(bytes[6..8], [0x60, 0x02]); // bra.s: pc_ref 0x208 -> 0x20A

        let listing = disassemble_listing(&bytes, 0x200, asm.symbols(), None);
        assert!(listing.contains("bra.s   loop"), "listing:\n{listing}");
        assert!(listing.contains("beq.s    loop"), "listing:\n{listing}");
        assert!(listing.contains("bsr.s   loop"), "listing:\n{listing}");
        assert!(listing.contains("bra.s   end"), "listing:\n{listing}");
        assert!(
            !listing.contains("bra.w"),
            "short forms must not widen:\n{listing}"
        );
    }

    /// MOVEP and immediate-to-CCR opcodes previously desynced the cursor by
    /// consuming the wrong number of extension words.
    #[test]
    fn previously_desyncing_opcodes_stay_in_sync() {
        // movep.w d0, 4(a1) = 0149 0004 ; then a nop must decode at +4
        let code = [0x01, 0x49, 0x00, 0x04, 0x4E, 0x71];
        let listing = disassemble_listing(&code, 0x200, &HashMap::new(), None);
        assert!(listing.contains("movep.w d0, 4(a1)"), "listing:\n{listing}");
        assert!(listing.contains("nop"), "listing:\n{listing}");

        // ori.b #$23, ccr = 003C 0023, then a nop
        let code = [0x00, 0x3C, 0x00, 0x23, 0x4E, 0x71];
        let listing = disassemble_listing(&code, 0x200, &HashMap::new(), None);
        assert!(listing.contains("ori.b   #$23, ccr"), "listing:\n{listing}");
        assert!(listing.contains("nop"), "listing:\n{listing}");
    }

    #[test]
    fn roundtrip_stack_and_jumps() {
        roundtrip(M68kInst::Link(AddrReg::A6, -64));
        roundtrip(M68kInst::Unlk(AddrReg::A6));
        roundtrip(M68kInst::Jmp(Operand::AddrInd(AddrReg::A0)));
        roundtrip(M68kInst::Jsr(Operand::AbsLong(0x00000400)));
        roundtrip(M68kInst::Movem(
            Size::Long,
            vec![
                Reg::Data(DataReg::D2),
                Reg::Data(DataReg::D3),
                Reg::Addr(AddrReg::A2),
            ],
            Operand::PreDec(AddrReg::A7),
            true,
        ));
        roundtrip(M68kInst::Movem(
            Size::Long,
            vec![
                Reg::Data(DataReg::D2),
                Reg::Data(DataReg::D3),
                Reg::Addr(AddrReg::A2),
            ],
            Operand::PostInc(AddrReg::A7),
            false,
        ));
    }

    #[test]
    fn decodes_branches_with_labels() {
        let mut asm = Assembler::new(0x200);
        let instructions = vec![
            M68kInst::Label("start".to_string()),
            M68kInst::Nop,
            M68kInst::Bcc(Cond::Ne, "start".to_string()),
            M68kInst::Bra("start".to_string()),
            M68kInst::Dbcc(Cond::False, DataReg::D0, "start".to_string()),
            M68kInst::Bsr("start".to_string()),
        ];
        let bytes = asm.assemble(&instructions).unwrap();
        let listing = disassemble_listing(&bytes, 0x200, asm.symbols(), None);

        assert!(listing.contains("start:"), "listing:\n{listing}");
        assert!(listing.contains("bne.w    start"), "listing:\n{listing}");
        assert!(listing.contains("bra.w   start"), "listing:\n{listing}");
        assert!(listing.contains("dbf    d0, start"), "listing:\n{listing}");
        assert!(listing.contains("bsr.w   start"), "listing:\n{listing}");
    }

    #[test]
    fn unknown_opcode_becomes_dc_w() {
        // 0xFxxx (line F) is never emitted by the encoder
        let listing = disassemble_listing(&[0xF0, 0x00], 0, &HashMap::new(), None);
        assert!(listing.contains("dc.w    $F000"), "listing:\n{listing}");
    }

    #[test]
    fn odd_trailing_byte_becomes_dc_b() {
        let listing = disassemble_listing(&[0x4E, 0x71, 0xAB], 0, &HashMap::new(), None);
        assert!(listing.contains("nop"), "listing:\n{listing}");
        assert!(listing.contains("dc.b    $AB"), "listing:\n{listing}");
    }

    #[test]
    fn absolute_long_uses_symbol_name() {
        let mut symbols = HashMap::new();
        symbols.insert("player_x".to_string(), 0x00FF8000u32);
        // move.l player_x, d0 -> 2030 39 00FF8000
        let mut encoder = InstructionEncoder::new();
        let bytes = encoder
            .encode(&M68kInst::Move(
                Size::Long,
                Operand::AbsLong(0x00FF8000),
                Operand::DataReg(DataReg::D0),
            ))
            .unwrap();
        let listing = disassemble_listing(&bytes, 0x200, &symbols, None);
        assert!(listing.contains("player_x"), "listing:\n{listing}");
    }

    #[test]
    fn data_section_dumped_raw() {
        let code = [0x4E, 0x71, 0x12, 0x34, 0x56, 0x78];
        let listing = disassemble_listing(&code, 0x200, &HashMap::new(), Some(0x202));
        assert!(listing.contains("nop"), "listing:\n{listing}");
        assert!(listing.contains("data section"), "listing:\n{listing}");
        assert!(listing.contains("$1234"), "listing:\n{listing}");
        assert!(
            !listing.contains("dc.w    $1234\n00000204"),
            "no decode past data start"
        );
    }

    #[test]
    fn listing_has_addresses_and_bytes() {
        let listing = disassemble_listing(&[0x4E, 0x71], 0x200, &HashMap::new(), None);
        assert!(listing.starts_with("00000200  4E71"), "listing:\n{listing}");
    }

    #[test]
    fn shift_over_eight_decodes_as_two_instructions() {
        let mut encoder = InstructionEncoder::new();
        let bytes = encoder
            .encode(&M68kInst::Lsl(Size::Long, Operand::Imm(12), DataReg::D0))
            .unwrap();
        assert_eq!(bytes.len(), 4);
        let listing = disassemble_listing(&bytes, 0, &HashMap::new(), None);
        assert!(listing.contains("lsl.l  #$8, d0"), "listing:\n{listing}");
        assert!(listing.contains("lsl.l  #$4, d0"), "listing:\n{listing}");
    }
}
