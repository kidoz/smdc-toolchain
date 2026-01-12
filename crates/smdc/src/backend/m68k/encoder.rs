//! M68k instruction binary encoder
//!
//! Converts M68k instructions to binary machine code.

use super::m68k::*;
use std::collections::HashMap;

/// Error during instruction encoding
#[derive(Debug, Clone)]
pub enum EncodeError {
    /// Unsupported instruction or addressing mode
    Unsupported(String),
    /// Value out of range for encoding
    OutOfRange(String),
    /// Unresolved symbol reference
    UnresolvedSymbol(String),
    /// Invalid operand combination
    InvalidOperands(String),
}

impl std::fmt::Display for EncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodeError::Unsupported(msg) => write!(f, "unsupported: {}", msg),
            EncodeError::OutOfRange(msg) => write!(f, "out of range: {}", msg),
            EncodeError::UnresolvedSymbol(msg) => write!(f, "unresolved symbol: {}", msg),
            EncodeError::InvalidOperands(msg) => write!(f, "invalid operands: {}", msg),
        }
    }
}

/// Instruction encoder that converts M68k instructions to bytes
pub struct InstructionEncoder {
    /// Current position in output
    pub position: u32,
    /// Symbol table for label resolution
    symbols: HashMap<String, u32>,
    /// Pending relocations (position, symbol_name, is_relative)
    relocations: Vec<(u32, String, bool)>,
}

impl InstructionEncoder {
    pub fn new() -> Self {
        Self {
            position: 0,
            symbols: HashMap::new(),
            relocations: Vec::new(),
        }
    }

    /// Set base address for encoding
    pub fn set_base_address(&mut self, addr: u32) {
        self.position = addr;
    }

    /// Define a symbol at the current position (only if not already defined)
    pub fn define_symbol(&mut self, name: &str) {
        // Don't overwrite existing symbols - they may have been pre-populated
        // with correct addresses from the layout pass
        if !self.symbols.contains_key(name) {
            self.symbols.insert(name.to_string(), self.position);
        }
    }

    /// Get symbol address, if defined
    pub fn get_symbol(&self, name: &str) -> Option<u32> {
        self.symbols.get(name).copied()
    }

    /// Calculate the size of an instruction in bytes (for layout pass)
    pub fn instruction_size(&self, inst: &M68kInst) -> usize {
        match inst {
            // Pseudo-instructions produce no code
            M68kInst::Label(_) | M68kInst::Comment(_) => 0,
            M68kInst::Directive(d) => self.directive_size(d),

            // Fixed 2-byte instructions
            M68kInst::Nop | M68kInst::Rts | M68kInst::Rte => 2,
            M68kInst::Moveq(_, _) => 2,
            M68kInst::Swap(_) => 2,
            M68kInst::Unlk(_) => 2,

            // Fixed 4-byte instructions
            M68kInst::Link(_, _) => 4,

            // Branch instructions
            M68kInst::Bra(_) | M68kInst::Bsr(_) | M68kInst::Bcc(_, _) => 4, // Use word displacement
            M68kInst::Dbf(_, _) => 4, // DBF is 4 bytes

            // Variable length based on operands
            M68kInst::Move(size, src, dst) => 2 + self.operand_extension_size(src, *size) + self.operand_extension_size(dst, *size),
            M68kInst::Lea(src, _) => 2 + self.operand_extension_size(src, Size::Long),
            M68kInst::Pea(op) => 2 + self.operand_extension_size(op, Size::Long),
            M68kInst::Clr(size, op) => 2 + self.operand_extension_size(op, *size),

            M68kInst::Add(size, src, dst) | M68kInst::Sub(size, src, dst) |
            M68kInst::And(size, src, dst) | M68kInst::Or(size, src, dst) |
            M68kInst::Cmp(size, src, dst) => {
                2 + self.ea_operand_extension_size(src, dst, *size)
            }

            M68kInst::Adda(size, src, _) | M68kInst::Suba(size, src, _) |
            M68kInst::Cmpa(size, src, _) => 2 + self.operand_extension_size(src, *size),

            M68kInst::Addq(size, _, op) | M68kInst::Subq(size, _, op) => {
                2 + self.operand_extension_size(op, *size)
            }

            M68kInst::Addi(size, _, op) | M68kInst::Subi(size, _, op) |
            M68kInst::Andi(size, _, op) | M68kInst::Ori(size, _, op) |
            M68kInst::Eori(size, _, op) | M68kInst::Cmpi(size, _, op) => {
                2 + self.immediate_size(*size) + self.operand_extension_size(op, *size)
            }

            M68kInst::Muls(src, _) | M68kInst::Mulu(src, _) |
            M68kInst::Divs(src, _) | M68kInst::Divu(src, _) => {
                2 + self.operand_extension_size(src, Size::Word)
            }

            M68kInst::Neg(size, op) | M68kInst::Not(size, op) | M68kInst::Tst(size, op) => {
                2 + self.operand_extension_size(op, *size)
            }

            M68kInst::Ext(_, _) => 2,

            M68kInst::Eor(size, _, dst) => 2 + self.operand_extension_size(dst, *size),

            M68kInst::Lsl(_, _, _) | M68kInst::Lsr(_, _, _) |
            M68kInst::Asl(_, _, _) | M68kInst::Asr(_, _, _) |
            M68kInst::Rol(_, _, _) | M68kInst::Ror(_, _, _) => 2,

            M68kInst::Btst(bit, op) | M68kInst::Bset(bit, op) |
            M68kInst::Bclr(bit, op) | M68kInst::Bchg(bit, op) => {
                let bit_size = match bit {
                    Operand::Imm(_) => 2,
                    _ => 0,
                };
                2 + bit_size + self.operand_extension_size(op, Size::Byte)
            }

            M68kInst::Jmp(op) | M68kInst::Jsr(op) => 2 + self.operand_extension_size(op, Size::Long),

            M68kInst::Exg(_, _) => 2,

            M68kInst::Movem(_, _, op, _) => 4 + self.operand_extension_size(op, Size::Long),

            M68kInst::Scc(_, op) => 2 + self.operand_extension_size(op, Size::Byte),
        }
    }

    fn directive_size(&self, directive: &str) -> usize {
        if directive.starts_with(".byte ") {
            1
        } else if directive.starts_with(".word ") {
            2
        } else if directive.starts_with(".long ") {
            4
        } else if directive.starts_with(".space ") {
            directive[7..].trim().parse::<usize>().unwrap_or(0)
        } else if directive.starts_with(".ascii ") || directive.starts_with(".asciz ") {
            // Extract string and count characters
            let content = &directive[7..];
            if content.starts_with('"') && content.ends_with('"') {
                let s = &content[1..content.len()-1];
                let len = s.len();
                if directive.starts_with(".asciz") { len + 1 } else { len }
            } else {
                0
            }
        } else {
            0 // Other directives like .section, .align, .global
        }
    }

    fn immediate_size(&self, size: Size) -> usize {
        match size {
            Size::Byte | Size::Word => 2,
            Size::Long => 4,
        }
    }

    fn operand_extension_size(&self, op: &Operand, size: Size) -> usize {
        match op {
            Operand::DataReg(_) | Operand::AddrReg(_) |
            Operand::AddrInd(_) | Operand::PostInc(_) | Operand::PreDec(_) |
            Operand::Sr => 0,
            Operand::Disp(_, _) => 2,
            Operand::Indexed(_, _, _) => 2,
            Operand::AbsShort(_) => 2,
            Operand::AbsLong(_) => 4,
            Operand::Imm(_) => {
                // Immediate size depends on operation size, not value
                match size {
                    Size::Byte | Size::Word => 2,
                    Size::Long => 4,
                }
            }
            Operand::PcRel(_) | Operand::Label(_) => 4, // Assume long for labels
        }
    }

    fn ea_operand_extension_size(&self, src: &Operand, dst: &Operand, size: Size) -> usize {
        // For most two-operand instructions, one operand is a register
        match (src, dst) {
            (Operand::DataReg(_), op) | (op, Operand::DataReg(_)) => {
                self.operand_extension_size(op, size)
            }
            _ => self.operand_extension_size(src, size) + self.operand_extension_size(dst, size),
        }
    }

    /// Encode a single instruction to bytes
    pub fn encode(&mut self, inst: &M68kInst) -> Result<Vec<u8>, EncodeError> {
        let mut bytes = Vec::new();

        match inst {
            // Pseudo-instructions
            M68kInst::Label(name) => {
                self.define_symbol(name);
            }
            M68kInst::Comment(_) => {}
            M68kInst::Directive(d) => {
                self.encode_directive(d, &mut bytes)?;
            }

            // Fixed instructions
            M68kInst::Nop => {
                bytes.extend_from_slice(&0x4E71u16.to_be_bytes());
            }
            M68kInst::Rts => {
                bytes.extend_from_slice(&0x4E75u16.to_be_bytes());
            }
            M68kInst::Rte => {
                bytes.extend_from_slice(&0x4E73u16.to_be_bytes());
            }

            // MOVEQ - Move Quick
            M68kInst::Moveq(data, reg) => {
                let opword = 0x7000 | ((reg_num_data(reg) as u16) << 9) | (*data as u8 as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
            }

            // SWAP
            M68kInst::Swap(reg) => {
                let opword = 0x4840 | reg_num_data(reg) as u16;
                bytes.extend_from_slice(&opword.to_be_bytes());
            }

            // LINK
            M68kInst::Link(reg, disp) => {
                let opword = 0x4E50 | reg_num_addr(reg) as u16;
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&(*disp as u16).to_be_bytes());
            }

            // UNLK
            M68kInst::Unlk(reg) => {
                let opword = 0x4E58 | reg_num_addr(reg) as u16;
                bytes.extend_from_slice(&opword.to_be_bytes());
            }

            // EXT - Sign Extend
            M68kInst::Ext(size, reg) => {
                let opcode = match size {
                    Size::Word => 0x4880, // EXT.W
                    Size::Long => 0x48C0, // EXT.L
                    Size::Byte => return Err(EncodeError::InvalidOperands("EXT.B not valid".to_string())),
                };
                let opword = opcode | reg_num_data(reg) as u16;
                bytes.extend_from_slice(&opword.to_be_bytes());
            }

            // Branch instructions
            M68kInst::Bra(label) => {
                self.encode_branch(0x6000, label, &mut bytes)?;
            }
            M68kInst::Bsr(label) => {
                self.encode_branch(0x6100, label, &mut bytes)?;
            }
            M68kInst::Bcc(cond, label) => {
                let base = 0x6000 | ((cond_code(cond) as u16) << 8);
                self.encode_branch(base, label, &mut bytes)?;
            }
            M68kInst::Dbf(reg, label) => {
                // DBF Dn, label (decrement and branch if not -1)
                // Encoding: 0101 0001 1100 1 rrr (rrr = register)
                let opword = 0x51C8 | reg_num_data(reg) as u16;
                bytes.extend_from_slice(&opword.to_be_bytes());

                // Calculate displacement or mark for relocation
                if let Some(target) = self.symbols.get(label) {
                    let current = self.position + 2; // After the opword
                    let disp = (*target as i32) - (current as i32);
                    if disp < -32768 || disp > 32767 {
                        return Err(EncodeError::OutOfRange(format!("DBF to {} out of range", label)));
                    }
                    bytes.extend_from_slice(&(disp as i16 as u16).to_be_bytes());
                } else {
                    self.relocations.push((self.position + 2, label.to_string(), true));
                    bytes.extend_from_slice(&0u16.to_be_bytes()); // Placeholder
                }
            }

            // MOVE
            M68kInst::Move(size, src, dst) => {
                self.encode_move(*size, src, dst, &mut bytes)?;
            }

            // LEA
            M68kInst::Lea(src, dst) => {
                let (src_mode, src_reg, src_ext) = self.encode_ea(src, Size::Long)?;
                let opword = 0x41C0 | ((reg_num_addr(dst) as u16) << 9) | ((src_mode as u16) << 3) | (src_reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&src_ext);
            }

            // PEA
            M68kInst::Pea(op) => {
                let (mode, reg, ext) = self.encode_ea(op, Size::Long)?;
                let opword = 0x4840 | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }

            // CLR
            M68kInst::Clr(size, op) => {
                let (mode, reg, ext) = self.encode_ea(op, *size)?;
                let size_bits = size_bits(*size);
                let opword = 0x4200 | ((size_bits as u16) << 6) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }

            // ADD
            M68kInst::Add(size, src, dst) => {
                self.encode_add_sub(0xD000, *size, src, dst, &mut bytes)?;
            }

            // SUB
            M68kInst::Sub(size, src, dst) => {
                self.encode_add_sub(0x9000, *size, src, dst, &mut bytes)?;
            }

            // ADDA
            M68kInst::Adda(size, src, dst) => {
                self.encode_adda_suba(0xD0C0, *size, src, dst, &mut bytes)?;
            }

            // SUBA
            M68kInst::Suba(size, src, dst) => {
                self.encode_adda_suba(0x90C0, *size, src, dst, &mut bytes)?;
            }

            // ADDQ
            M68kInst::Addq(size, data, op) => {
                self.encode_addq_subq(0x5000, *size, *data, op, &mut bytes)?;
            }

            // SUBQ
            M68kInst::Subq(size, data, op) => {
                self.encode_addq_subq(0x5100, *size, *data, op, &mut bytes)?;
            }

            // ADDI
            M68kInst::Addi(size, imm, op) => {
                self.encode_imm_op(0x0600, *size, *imm, op, &mut bytes)?;
            }

            // SUBI
            M68kInst::Subi(size, imm, op) => {
                self.encode_imm_op(0x0400, *size, *imm, op, &mut bytes)?;
            }

            // AND
            M68kInst::And(size, src, dst) => {
                self.encode_and_or(0xC000, *size, src, dst, &mut bytes)?;
            }

            // OR
            M68kInst::Or(size, src, dst) => {
                self.encode_and_or(0x8000, *size, src, dst, &mut bytes)?;
            }

            // ANDI
            M68kInst::Andi(size, imm, op) => {
                self.encode_imm_op(0x0200, *size, *imm, op, &mut bytes)?;
            }

            // ORI
            M68kInst::Ori(size, imm, op) => {
                self.encode_imm_op(0x0000, *size, *imm, op, &mut bytes)?;
            }

            // EORI
            M68kInst::Eori(size, imm, op) => {
                self.encode_imm_op(0x0A00, *size, *imm, op, &mut bytes)?;
            }

            // EOR
            M68kInst::Eor(size, src, dst) => {
                let (mode, reg, ext) = self.encode_ea(dst, *size)?;
                let size_bits = size_bits(*size);
                let opword = 0xB100 | ((reg_num_data(src) as u16) << 9) | ((size_bits as u16) << 6) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }

            // CMP
            M68kInst::Cmp(size, src, dst) => {
                // CMP <ea>, Dn
                if let Operand::DataReg(dn) = dst {
                    let (mode, reg, ext) = self.encode_ea(src, *size)?;
                    let size_bits = size_bits(*size);
                    let opword = 0xB000 | ((reg_num_data(dn) as u16) << 9) | ((size_bits as u16) << 6) | ((mode as u16) << 3) | (reg as u16);
                    bytes.extend_from_slice(&opword.to_be_bytes());
                    bytes.extend_from_slice(&ext);
                } else {
                    return Err(EncodeError::InvalidOperands("CMP destination must be Dn".to_string()));
                }
            }

            // CMPA
            M68kInst::Cmpa(size, src, dst) => {
                let (mode, reg, ext) = self.encode_ea(src, *size)?;
                let opmode = match size {
                    Size::Word => 3,
                    Size::Long => 7,
                    Size::Byte => return Err(EncodeError::InvalidOperands("CMPA.B not valid".to_string())),
                };
                let opword = 0xB000 | ((reg_num_addr(dst) as u16) << 9) | (opmode << 6) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }

            // CMPI
            M68kInst::Cmpi(size, imm, op) => {
                self.encode_imm_op(0x0C00, *size, *imm, op, &mut bytes)?;
            }

            // NEG
            M68kInst::Neg(size, op) => {
                let (mode, reg, ext) = self.encode_ea(op, *size)?;
                let size_bits = size_bits(*size);
                let opword = 0x4400 | ((size_bits as u16) << 6) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }

            // NOT
            M68kInst::Not(size, op) => {
                let (mode, reg, ext) = self.encode_ea(op, *size)?;
                let size_bits = size_bits(*size);
                let opword = 0x4600 | ((size_bits as u16) << 6) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }

            // TST
            M68kInst::Tst(size, op) => {
                let (mode, reg, ext) = self.encode_ea(op, *size)?;
                let size_bits = size_bits(*size);
                let opword = 0x4A00 | ((size_bits as u16) << 6) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }

            // MULS
            M68kInst::Muls(src, dst) => {
                let (mode, reg, ext) = self.encode_ea(src, Size::Word)?;
                let opword = 0xC1C0 | ((reg_num_data(dst) as u16) << 9) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }

            // MULU
            M68kInst::Mulu(src, dst) => {
                let (mode, reg, ext) = self.encode_ea(src, Size::Word)?;
                let opword = 0xC0C0 | ((reg_num_data(dst) as u16) << 9) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }

            // DIVS
            M68kInst::Divs(src, dst) => {
                let (mode, reg, ext) = self.encode_ea(src, Size::Word)?;
                let opword = 0x81C0 | ((reg_num_data(dst) as u16) << 9) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }

            // DIVU
            M68kInst::Divu(src, dst) => {
                let (mode, reg, ext) = self.encode_ea(src, Size::Word)?;
                let opword = 0x80C0 | ((reg_num_data(dst) as u16) << 9) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }

            // Shift instructions
            M68kInst::Lsl(size, count, reg) => {
                self.encode_shift(0xE108, *size, count, reg, &mut bytes)?;
            }
            M68kInst::Lsr(size, count, reg) => {
                self.encode_shift(0xE008, *size, count, reg, &mut bytes)?;
            }
            M68kInst::Asl(size, count, reg) => {
                self.encode_shift(0xE100, *size, count, reg, &mut bytes)?;
            }
            M68kInst::Asr(size, count, reg) => {
                self.encode_shift(0xE000, *size, count, reg, &mut bytes)?;
            }
            M68kInst::Rol(size, count, reg) => {
                self.encode_shift(0xE118, *size, count, reg, &mut bytes)?;
            }
            M68kInst::Ror(size, count, reg) => {
                self.encode_shift(0xE018, *size, count, reg, &mut bytes)?;
            }

            // Bit operations
            M68kInst::Btst(bit, op) => {
                self.encode_bit_op(0x0100, 0x0800, bit, op, &mut bytes)?;
            }
            M68kInst::Bset(bit, op) => {
                self.encode_bit_op(0x01C0, 0x08C0, bit, op, &mut bytes)?;
            }
            M68kInst::Bclr(bit, op) => {
                self.encode_bit_op(0x0180, 0x0880, bit, op, &mut bytes)?;
            }
            M68kInst::Bchg(bit, op) => {
                self.encode_bit_op(0x0140, 0x0840, bit, op, &mut bytes)?;
            }

            // JMP
            M68kInst::Jmp(op) => {
                let (mode, reg, ext) = self.encode_ea(op, Size::Long)?;
                let opword = 0x4EC0 | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }

            // JSR
            M68kInst::Jsr(op) => {
                let (mode, reg, ext) = self.encode_ea(op, Size::Long)?;
                let opword = 0x4E80 | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }

            // EXG
            M68kInst::Exg(r1, r2) => {
                let opword = match (r1, r2) {
                    (Reg::Data(d1), Reg::Data(d2)) => {
                        0xC140 | ((reg_num_data(d1) as u16) << 9) | reg_num_data(d2) as u16
                    }
                    (Reg::Addr(a1), Reg::Addr(a2)) => {
                        0xC148 | ((reg_num_addr(a1) as u16) << 9) | reg_num_addr(a2) as u16
                    }
                    (Reg::Data(d), Reg::Addr(a)) | (Reg::Addr(a), Reg::Data(d)) => {
                        0xC188 | ((reg_num_data(d) as u16) << 9) | reg_num_addr(a) as u16
                    }
                };
                bytes.extend_from_slice(&opword.to_be_bytes());
            }

            // MOVEM
            M68kInst::Movem(size, regs, op, to_mem) => {
                self.encode_movem(*size, regs, op, *to_mem, &mut bytes)?;
            }

            // Scc
            M68kInst::Scc(cond, op) => {
                let (mode, reg, ext) = self.encode_ea(op, Size::Byte)?;
                let opword = 0x50C0 | ((cond_code(cond) as u16) << 8) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }
        }

        self.position += bytes.len() as u32;
        Ok(bytes)
    }

    fn encode_directive(&self, directive: &str, bytes: &mut Vec<u8>) -> Result<(), EncodeError> {
        if directive.starts_with(".byte ") {
            let val = parse_number(&directive[6..])?;
            bytes.push(val as u8);
        } else if directive.starts_with(".word ") {
            let val = parse_number(&directive[6..])?;
            bytes.extend_from_slice(&(val as u16).to_be_bytes());
        } else if directive.starts_with(".long ") {
            let val = parse_number(&directive[6..])?;
            bytes.extend_from_slice(&(val as u32).to_be_bytes());
        } else if directive.starts_with(".space ") {
            let size = directive[7..].trim().parse::<usize>()
                .map_err(|_| EncodeError::InvalidOperands("invalid .space size".to_string()))?;
            bytes.extend(std::iter::repeat(0u8).take(size));
        } else if directive.starts_with(".ascii ") || directive.starts_with(".asciz ") {
            let content = &directive[7..];
            if content.starts_with('"') && content.ends_with('"') {
                let s = &content[1..content.len()-1];
                bytes.extend(s.bytes());
                if directive.starts_with(".asciz") {
                    bytes.push(0);
                }
            }
        }
        // Other directives like .section, .align, .global are ignored in binary output
        Ok(())
    }

    fn encode_branch(&mut self, base: u16, label: &str, bytes: &mut Vec<u8>) -> Result<(), EncodeError> {
        // Always use word displacement for simplicity
        let opword = base; // Displacement 0 means word displacement follows
        bytes.extend_from_slice(&opword.to_be_bytes());

        // Calculate displacement or mark for relocation
        if let Some(target) = self.symbols.get(label) {
            let current = self.position + 2; // After the opword
            let disp = (*target as i32) - (current as i32);
            if disp < -32768 || disp > 32767 {
                return Err(EncodeError::OutOfRange(format!("branch to {} out of range", label)));
            }
            bytes.extend_from_slice(&(disp as i16 as u16).to_be_bytes());
        } else {
            // Mark for relocation
            self.relocations.push((self.position + 2, label.to_string(), true));
            bytes.extend_from_slice(&0u16.to_be_bytes()); // Placeholder
        }
        Ok(())
    }

    fn encode_move(&self, size: Size, src: &Operand, dst: &Operand, bytes: &mut Vec<u8>) -> Result<(), EncodeError> {
        // Special case: MOVE to SR (privileged instruction)
        if matches!(dst, Operand::Sr) {
            // MOVE to SR: 0100 0110 11 mmm rrr
            let (src_mode, src_reg, src_ext) = self.encode_ea(src, Size::Word)?;
            let opword = 0x46C0 | ((src_mode as u16) << 3) | (src_reg as u16);
            bytes.extend_from_slice(&opword.to_be_bytes());
            bytes.extend_from_slice(&src_ext);
            return Ok(());
        }

        // Special case: MOVE from SR
        if matches!(src, Operand::Sr) {
            // MOVE from SR: 0100 0000 11 mmm rrr
            let (dst_mode, dst_reg, dst_ext) = self.encode_ea(dst, Size::Word)?;
            let opword = 0x40C0 | ((dst_mode as u16) << 3) | (dst_reg as u16);
            bytes.extend_from_slice(&opword.to_be_bytes());
            bytes.extend_from_slice(&dst_ext);
            return Ok(());
        }

        let size_bits: u16 = match size {
            Size::Byte => 0b01,
            Size::Word => 0b11,
            Size::Long => 0b10,
        };

        let (src_mode, src_reg, src_ext) = self.encode_ea(src, size)?;
        let (dst_mode, dst_reg, dst_ext) = self.encode_ea(dst, size)?;

        // MOVE encoding: 00 | size | dst_reg | dst_mode | src_mode | src_reg
        let opword = (size_bits << 12) | ((dst_reg as u16) << 9) | ((dst_mode as u16) << 6) | ((src_mode as u16) << 3) | (src_reg as u16);
        bytes.extend_from_slice(&opword.to_be_bytes());
        bytes.extend_from_slice(&src_ext);
        bytes.extend_from_slice(&dst_ext);
        Ok(())
    }

    fn encode_add_sub(&self, base: u16, size: Size, src: &Operand, dst: &Operand, bytes: &mut Vec<u8>) -> Result<(), EncodeError> {
        let size_bits = size_bits(size);

        match (src, dst) {
            // <ea> + Dn -> Dn
            (_, Operand::DataReg(dn)) => {
                let (mode, reg, ext) = self.encode_ea(src, size)?;
                let opword = base | ((reg_num_data(dn) as u16) << 9) | ((size_bits as u16) << 6) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }
            // Dn + <ea> -> <ea>
            (Operand::DataReg(dn), _) => {
                let (mode, reg, ext) = self.encode_ea(dst, size)?;
                let opword = base | ((reg_num_data(dn) as u16) << 9) | (1 << 8) | ((size_bits as u16) << 6) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }
            _ => return Err(EncodeError::InvalidOperands("ADD/SUB requires at least one Dn".to_string())),
        }
        Ok(())
    }

    fn encode_adda_suba(&self, base: u16, size: Size, src: &Operand, dst: &AddrReg, bytes: &mut Vec<u8>) -> Result<(), EncodeError> {
        let (mode, reg, ext) = self.encode_ea(src, size)?;
        let opmode: u16 = match size {
            Size::Word => 0,
            Size::Long => 1,
            Size::Byte => return Err(EncodeError::InvalidOperands("ADDA/SUBA.B not valid".to_string())),
        };
        let opword = base | ((reg_num_addr(dst) as u16) << 9) | (opmode << 8) | ((mode as u16) << 3) | (reg as u16);
        bytes.extend_from_slice(&opword.to_be_bytes());
        bytes.extend_from_slice(&ext);
        Ok(())
    }

    fn encode_addq_subq(&self, base: u16, size: Size, data: u8, op: &Operand, bytes: &mut Vec<u8>) -> Result<(), EncodeError> {
        let (mode, reg, ext) = self.encode_ea(op, size)?;
        let size_bits = size_bits(size);
        let data_bits = if data == 8 { 0 } else { data as u16 };
        let opword = base | (data_bits << 9) | ((size_bits as u16) << 6) | ((mode as u16) << 3) | (reg as u16);
        bytes.extend_from_slice(&opword.to_be_bytes());
        bytes.extend_from_slice(&ext);
        Ok(())
    }

    fn encode_imm_op(&self, base: u16, size: Size, imm: i32, op: &Operand, bytes: &mut Vec<u8>) -> Result<(), EncodeError> {
        let (mode, reg, ext) = self.encode_ea(op, size)?;
        let size_bits = size_bits(size);
        let opword = base | ((size_bits as u16) << 6) | ((mode as u16) << 3) | (reg as u16);
        bytes.extend_from_slice(&opword.to_be_bytes());

        // Immediate data
        match size {
            Size::Byte | Size::Word => bytes.extend_from_slice(&(imm as u16).to_be_bytes()),
            Size::Long => bytes.extend_from_slice(&(imm as u32).to_be_bytes()),
        }
        bytes.extend_from_slice(&ext);
        Ok(())
    }

    fn encode_and_or(&self, base: u16, size: Size, src: &Operand, dst: &Operand, bytes: &mut Vec<u8>) -> Result<(), EncodeError> {
        let size_bits = size_bits(size);

        match (src, dst) {
            (_, Operand::DataReg(dn)) => {
                let (mode, reg, ext) = self.encode_ea(src, size)?;
                let opword = base | ((reg_num_data(dn) as u16) << 9) | ((size_bits as u16) << 6) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }
            (Operand::DataReg(dn), _) => {
                let (mode, reg, ext) = self.encode_ea(dst, size)?;
                let opword = base | ((reg_num_data(dn) as u16) << 9) | (1 << 8) | ((size_bits as u16) << 6) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }
            _ => return Err(EncodeError::InvalidOperands("AND/OR requires at least one Dn".to_string())),
        }
        Ok(())
    }

    fn encode_shift(&self, base: u16, size: Size, count: &Operand, reg: &DataReg, bytes: &mut Vec<u8>) -> Result<(), EncodeError> {
        let size_bits = size_bits(size);

        let opword = match count {
            Operand::Imm(n) => {
                let cnt = if *n == 8 { 0 } else { *n as u16 & 7 };
                base | (cnt << 9) | ((size_bits as u16) << 6) | reg_num_data(reg) as u16
            }
            Operand::DataReg(cnt_reg) => {
                base | ((reg_num_data(cnt_reg) as u16) << 9) | (1 << 5) | ((size_bits as u16) << 6) | reg_num_data(reg) as u16
            }
            _ => return Err(EncodeError::InvalidOperands("shift count must be immediate or Dn".to_string())),
        };
        bytes.extend_from_slice(&opword.to_be_bytes());
        Ok(())
    }

    fn encode_bit_op(&self, reg_base: u16, imm_base: u16, bit: &Operand, op: &Operand, bytes: &mut Vec<u8>) -> Result<(), EncodeError> {
        let (mode, reg, ext) = self.encode_ea(op, Size::Byte)?;

        match bit {
            Operand::DataReg(dn) => {
                let opword = reg_base | ((reg_num_data(dn) as u16) << 9) | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&ext);
            }
            Operand::Imm(n) => {
                let opword = imm_base | ((mode as u16) << 3) | (reg as u16);
                bytes.extend_from_slice(&opword.to_be_bytes());
                bytes.extend_from_slice(&(*n as u16).to_be_bytes());
                bytes.extend_from_slice(&ext);
            }
            _ => return Err(EncodeError::InvalidOperands("bit number must be immediate or Dn".to_string())),
        }
        Ok(())
    }

    fn encode_movem(&self, size: Size, regs: &[Reg], op: &Operand, to_mem: bool, bytes: &mut Vec<u8>) -> Result<(), EncodeError> {
        let (mode, reg, ext) = self.encode_ea(op, size)?;
        let size_bit: u16 = match size {
            Size::Word => 0,
            Size::Long => 1,
            Size::Byte => return Err(EncodeError::InvalidOperands("MOVEM.B not valid".to_string())),
        };
        let dir_bit: u16 = if to_mem { 0 } else { 1 };

        let opword = 0x4880 | (dir_bit << 10) | (size_bit << 6) | ((mode as u16) << 3) | (reg as u16);
        bytes.extend_from_slice(&opword.to_be_bytes());

        // Register list mask
        let mask = self.make_register_mask(regs, to_mem && matches!(op, Operand::PreDec(_)));
        bytes.extend_from_slice(&mask.to_be_bytes());

        bytes.extend_from_slice(&ext);
        Ok(())
    }

    fn make_register_mask(&self, regs: &[Reg], reverse: bool) -> u16 {
        let mut mask = 0u16;
        for reg in regs {
            let bit = match reg {
                Reg::Data(d) => reg_num_data(d),
                Reg::Addr(a) => 8 + reg_num_addr(a),
            };
            if reverse {
                mask |= 1 << (15 - bit);
            } else {
                mask |= 1 << bit;
            }
        }
        mask
    }

    /// Encode an effective address operand
    /// Returns (mode, register, extension_words)
    fn encode_ea(&self, op: &Operand, size: Size) -> Result<(u8, u8, Vec<u8>), EncodeError> {
        let mut ext = Vec::new();

        let (mode, reg) = match op {
            Operand::DataReg(d) => (0b000, reg_num_data(d)),
            Operand::AddrReg(a) => (0b001, reg_num_addr(a)),
            Operand::AddrInd(a) => (0b010, reg_num_addr(a)),
            Operand::PostInc(a) => (0b011, reg_num_addr(a)),
            Operand::PreDec(a) => (0b100, reg_num_addr(a)),
            Operand::Disp(d, a) => {
                ext.extend_from_slice(&(*d as u16).to_be_bytes());
                (0b101, reg_num_addr(a))
            }
            Operand::Indexed(d, a, idx) => {
                // Brief extension word: D/A | reg | W/L | scale | 0 | disp
                let brief = ((reg_num_data(idx) as u16) << 12) | ((*d as u8 as u16) & 0xFF);
                ext.extend_from_slice(&brief.to_be_bytes());
                (0b110, reg_num_addr(a))
            }
            Operand::AbsShort(addr) => {
                ext.extend_from_slice(&(*addr as u16).to_be_bytes());
                (0b111, 0b000)
            }
            Operand::AbsLong(addr) => {
                ext.extend_from_slice(&addr.to_be_bytes());
                (0b111, 0b001)
            }
            Operand::Imm(val) => {
                match size {
                    Size::Byte | Size::Word => ext.extend_from_slice(&(*val as u16).to_be_bytes()),
                    Size::Long => ext.extend_from_slice(&(*val as u32).to_be_bytes()),
                }
                (0b111, 0b100)
            }
            Operand::PcRel(_label) => {
                // PC-relative with word displacement
                // TODO: Handle symbol resolution
                ext.extend_from_slice(&0u16.to_be_bytes()); // Placeholder
                (0b111, 0b010)
            }
            Operand::Label(label) => {
                // Treat as absolute long
                if let Some(addr) = self.symbols.get(label) {
                    ext.extend_from_slice(&addr.to_be_bytes());
                } else {
                    ext.extend_from_slice(&0u32.to_be_bytes()); // Placeholder for relocation
                }
                (0b111, 0b001)
            }
            Operand::Sr => {
                // SR is not a standard EA mode, it's handled specially in MOVE to/from SR
                return Err(EncodeError::InvalidOperands("SR cannot be encoded as EA".to_string()));
            }
        };

        Ok((mode, reg, ext))
    }

    /// Get pending relocations
    pub fn relocations(&self) -> &[(u32, String, bool)] {
        &self.relocations
    }
}

impl Default for InstructionEncoder {
    fn default() -> Self {
        Self::new()
    }
}

// Helper functions

fn reg_num_data(r: &DataReg) -> u8 {
    match r {
        DataReg::D0 => 0, DataReg::D1 => 1, DataReg::D2 => 2, DataReg::D3 => 3,
        DataReg::D4 => 4, DataReg::D5 => 5, DataReg::D6 => 6, DataReg::D7 => 7,
    }
}

fn reg_num_addr(r: &AddrReg) -> u8 {
    match r {
        AddrReg::A0 => 0, AddrReg::A1 => 1, AddrReg::A2 => 2, AddrReg::A3 => 3,
        AddrReg::A4 => 4, AddrReg::A5 => 5, AddrReg::A6 => 6, AddrReg::A7 => 7,
    }
}

fn size_bits(size: Size) -> u8 {
    match size {
        Size::Byte => 0b00,
        Size::Word => 0b01,
        Size::Long => 0b10,
    }
}

fn cond_code(cond: &Cond) -> u8 {
    match cond {
        Cond::True => 0,  Cond::False => 1, Cond::Hi => 2,  Cond::Ls => 3,
        Cond::Cc => 4,    Cond::Cs => 5,    Cond::Ne => 6,  Cond::Eq => 7,
        Cond::Vc => 8,    Cond::Vs => 9,    Cond::Pl => 10, Cond::Mi => 11,
        Cond::Ge => 12,   Cond::Lt => 13,   Cond::Gt => 14, Cond::Le => 15,
    }
}

fn parse_number(s: &str) -> Result<i32, EncodeError> {
    let s = s.trim();
    if s.starts_with("0x") || s.starts_with("0X") {
        i32::from_str_radix(&s[2..], 16)
            .map_err(|_| EncodeError::InvalidOperands(format!("invalid hex number: {}", s)))
    } else if s.starts_with('$') {
        i32::from_str_radix(&s[1..], 16)
            .map_err(|_| EncodeError::InvalidOperands(format!("invalid hex number: {}", s)))
    } else {
        s.parse::<i32>()
            .map_err(|_| EncodeError::InvalidOperands(format!("invalid number: {}", s)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_nop() {
        let mut encoder = InstructionEncoder::new();
        let bytes = encoder.encode(&M68kInst::Nop).unwrap();
        assert_eq!(bytes, vec![0x4E, 0x71]);
    }

    #[test]
    fn test_encode_rts() {
        let mut encoder = InstructionEncoder::new();
        let bytes = encoder.encode(&M68kInst::Rts).unwrap();
        assert_eq!(bytes, vec![0x4E, 0x75]);
    }

    #[test]
    fn test_encode_moveq() {
        let mut encoder = InstructionEncoder::new();
        let bytes = encoder.encode(&M68kInst::Moveq(5, DataReg::D0)).unwrap();
        assert_eq!(bytes, vec![0x70, 0x05]); // MOVEQ #5, D0
    }

    #[test]
    fn test_encode_link() {
        let mut encoder = InstructionEncoder::new();
        let bytes = encoder.encode(&M68kInst::Link(AddrReg::A6, -64)).unwrap();
        assert_eq!(bytes, vec![0x4E, 0x56, 0xFF, 0xC0]); // LINK A6, #-64
    }

    #[test]
    fn test_encode_move_reg_to_reg() {
        let mut encoder = InstructionEncoder::new();
        let bytes = encoder.encode(&M68kInst::Move(
            Size::Long,
            Operand::DataReg(DataReg::D0),
            Operand::DataReg(DataReg::D1),
        )).unwrap();
        assert_eq!(bytes, vec![0x22, 0x00]); // MOVE.L D0, D1
    }

    #[test]
    fn test_encode_add_imm() {
        let mut encoder = InstructionEncoder::new();
        let bytes = encoder.encode(&M68kInst::Addi(
            Size::Long,
            100,
            Operand::DataReg(DataReg::D0),
        )).unwrap();
        // ADDI.L #100, D0
        assert_eq!(bytes, vec![0x06, 0x80, 0x00, 0x00, 0x00, 0x64]);
    }
}
