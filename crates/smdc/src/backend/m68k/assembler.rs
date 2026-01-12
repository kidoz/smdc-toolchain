//! Two-pass assembler for M68k instructions
//!
//! Converts M68k instructions to binary with symbol resolution.

use super::encoder::{EncodeError, InstructionEncoder};
use super::m68k::M68kInst;
use std::collections::HashMap;

/// Assembly error
#[derive(Debug, Clone)]
pub enum AssemblyError {
    /// Encoding error from instruction encoder
    Encode(EncodeError),
    /// Unresolved symbol after assembly
    UnresolvedSymbol(String),
    /// Symbol defined multiple times
    DuplicateSymbol(String),
}

impl std::fmt::Display for AssemblyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssemblyError::Encode(e) => write!(f, "encode error: {}", e),
            AssemblyError::UnresolvedSymbol(s) => write!(f, "unresolved symbol: {}", s),
            AssemblyError::DuplicateSymbol(s) => write!(f, "duplicate symbol: {}", s),
        }
    }
}

impl From<EncodeError> for AssemblyError {
    fn from(e: EncodeError) -> Self {
        AssemblyError::Encode(e)
    }
}

/// Two-pass assembler for M68k instructions
pub struct Assembler {
    /// Symbol table (label -> address)
    symbols: HashMap<String, u32>,
    /// Base address for code
    base_address: u32,
}

impl Assembler {
    /// Create a new assembler with the given base address
    pub fn new(base_address: u32) -> Self {
        Self {
            symbols: HashMap::new(),
            base_address,
        }
    }

    /// Assemble a list of instructions to binary
    pub fn assemble(&mut self, instructions: &[M68kInst]) -> Result<Vec<u8>, AssemblyError> {
        // Pass 1: Calculate all label addresses
        self.layout_pass(instructions)?;

        // Debug: print symbol table for global variables
        if std::env::var("DEBUG_ASM").is_ok() {
            eprintln!("=== Symbol table after layout pass ===");
            let mut syms: Vec<_> = self.symbols.iter().collect();
            syms.sort_by_key(|(_, addr)| *addr);
            for (name, addr) in syms {
                if !name.starts_with(".L") && !name.starts_with("_") {
                    eprintln!("  {} = 0x{:04X}", name, addr);
                }
            }
        }

        // Pass 2: Encode all instructions with resolved addresses
        self.encode_pass(instructions)
    }

    /// Pass 1: Calculate the address of each label
    fn layout_pass(&mut self, instructions: &[M68kInst]) -> Result<(), AssemblyError> {
        self.symbols.clear();
        let mut encoder = InstructionEncoder::new();
        encoder.set_base_address(self.base_address);

        let mut position = self.base_address;

        for inst in instructions {
            // Handle labels
            if let M68kInst::Label(name) = inst {
                if self.symbols.contains_key(name) {
                    return Err(AssemblyError::DuplicateSymbol(name.clone()));
                }
                self.symbols.insert(name.clone(), position);
            }

            // Handle alignment directives
            if let M68kInst::Directive(d) = inst {
                if d.starts_with(".align ") {
                    let align = d[7..].trim().parse::<u32>().unwrap_or(2);
                    let mask = align - 1;
                    position = (position + mask) & !mask;
                    continue;
                }
            }

            // Calculate instruction size
            let size = encoder.instruction_size(inst) as u32;
            position += size;
        }

        Ok(())
    }

    /// Pass 2: Encode all instructions with resolved addresses
    fn encode_pass(&self, instructions: &[M68kInst]) -> Result<Vec<u8>, AssemblyError> {
        let mut output = Vec::new();
        let mut encoder = InstructionEncoder::new();
        encoder.set_base_address(self.base_address);

        // Copy symbol table to encoder
        for (name, _addr) in &self.symbols {
            encoder.define_symbol(name);
            // We need to set the position manually for pre-defined symbols
        }

        // Create a new encoder with symbols pre-populated
        let mut encoder = self.create_encoder_with_symbols();

        for inst in instructions {
            // Handle alignment
            if let M68kInst::Directive(d) = inst {
                if d.starts_with(".align ") {
                    let align = d[7..].trim().parse::<usize>().unwrap_or(2);
                    while output.len() % align != 0 {
                        output.push(0);
                        encoder.position += 1;
                    }
                    continue;
                }
            }

            let bytes = encoder.encode(inst)?;
            output.extend(bytes);
        }

        // Apply relocations
        self.apply_relocations(&mut output, &encoder)?;

        Ok(output)
    }

    fn create_encoder_with_symbols(&self) -> InstructionEncoder {
        let mut encoder = InstructionEncoder::new();
        encoder.set_base_address(self.base_address);

        // Pre-populate symbol table
        for (name, &addr) in &self.symbols {
            // Temporarily move position to symbol address, define, then restore
            let saved_pos = encoder.position;
            encoder.position = addr;
            encoder.define_symbol(name);
            encoder.position = saved_pos;
        }

        encoder
    }

    fn apply_relocations(&self, output: &mut [u8], encoder: &InstructionEncoder) -> Result<(), AssemblyError> {
        for (pos, symbol, is_relative) in encoder.relocations() {
            let target = self.symbols.get(symbol)
                .ok_or_else(|| AssemblyError::UnresolvedSymbol(symbol.clone()))?;

            let offset = (*pos - self.base_address) as usize;

            if *is_relative {
                // PC-relative displacement
                let pc = *pos; // PC is at the extension word
                let disp = (*target as i32) - (pc as i32);

                if disp < -32768 || disp > 32767 {
                    return Err(AssemblyError::Encode(EncodeError::OutOfRange(
                        format!("branch displacement {} out of range", disp)
                    )));
                }

                let disp_bytes = (disp as i16).to_be_bytes();
                if offset + 1 < output.len() {
                    output[offset] = disp_bytes[0];
                    output[offset + 1] = disp_bytes[1];
                }
            } else {
                // Absolute address
                let addr_bytes = target.to_be_bytes();
                if offset + 3 < output.len() {
                    output[offset] = addr_bytes[0];
                    output[offset + 1] = addr_bytes[1];
                    output[offset + 2] = addr_bytes[2];
                    output[offset + 3] = addr_bytes[3];
                }
            }
        }
        Ok(())
    }

    /// Get the symbol table
    pub fn symbols(&self) -> &HashMap<String, u32> {
        &self.symbols
    }

    /// Add an external symbol (e.g., for ROM entry point)
    pub fn add_symbol(&mut self, name: &str, addr: u32) {
        self.symbols.insert(name.to_string(), addr);
    }
}

impl Default for Assembler {
    fn default() -> Self {
        Self::new(0x200) // Default Genesis code start
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::m68k::*;

    #[test]
    fn test_assemble_simple() {
        let mut asm = Assembler::new(0x200);
        let instructions = vec![
            M68kInst::Nop,
            M68kInst::Rts,
        ];

        let bytes = asm.assemble(&instructions).unwrap();
        assert_eq!(bytes, vec![0x4E, 0x71, 0x4E, 0x75]);
    }

    #[test]
    fn test_assemble_with_label() {
        let mut asm = Assembler::new(0x200);
        let instructions = vec![
            M68kInst::Label("start".to_string()),
            M68kInst::Nop,
            M68kInst::Bra("start".to_string()),
        ];

        let bytes = asm.assemble(&instructions).unwrap();

        // NOP at 0x200 = 0x4E71
        // BRA.W start at 0x202 = 0x6000 + displacement
        // displacement = 0x200 - 0x204 = -4 = 0xFFFC
        assert_eq!(bytes[0..2], [0x4E, 0x71]); // NOP
        assert_eq!(bytes[2..4], [0x60, 0x00]); // BRA.W
        assert_eq!(bytes[4..6], [0xFF, 0xFC]); // displacement -4
    }

    #[test]
    fn test_assemble_function() {
        let mut asm = Assembler::new(0x200);
        let instructions = vec![
            M68kInst::Label("add".to_string()),
            M68kInst::Link(AddrReg::A6, -4),
            M68kInst::Move(Size::Long, Operand::Disp(8, AddrReg::A6), Operand::DataReg(DataReg::D0)),
            M68kInst::Add(Size::Long, Operand::Disp(12, AddrReg::A6), Operand::DataReg(DataReg::D0)),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Rts,
        ];

        let bytes = asm.assemble(&instructions).unwrap();
        assert!(!bytes.is_empty());

        // Verify LINK A6, #-4
        assert_eq!(bytes[0..4], [0x4E, 0x56, 0xFF, 0xFC]);

        // Verify RTS at the end
        let len = bytes.len();
        assert_eq!(bytes[len-2..], [0x4E, 0x75]);
    }
}
