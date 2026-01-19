//! M68k code emitter

use crate::ir::*;
use crate::common::CompileResult;
use super::m68k::*;
use super::sdk::{
    SdkRegistry, SdkFunctionKind, SdkInlineGenerator, SdkLibraryGenerator,
    resolve_dependencies, generate_static_data,
};
use std::collections::{HashMap, HashSet};

/// Code generator that converts IR to M68k assembly
pub struct CodeGenerator {
    output: Vec<M68kInst>,
    /// Maps IR temps to stack offsets (negative from FP)
    temp_offsets: HashMap<u32, i16>,
    /// Current stack frame size
    frame_size: i16,
    /// Next stack offset
    next_offset: i16,
    /// Total size of global data (for RAM allocation)
    data_size: usize,
    /// SDK function registry
    sdk_registry: SdkRegistry,
    /// Set of SDK library functions that need to be generated
    pending_sdk_functions: HashSet<String>,
    /// Set of user-defined functions (to avoid SDK conflicts)
    defined_functions: HashSet<String>,
}

impl CodeGenerator {
    pub fn new() -> Self {
        Self {
            output: Vec::new(),
            temp_offsets: HashMap::new(),
            frame_size: 0,
            next_offset: -4,
            data_size: 0,
            sdk_registry: SdkRegistry::new(),
            pending_sdk_functions: HashSet::new(),
            defined_functions: HashSet::new(),
        }
    }

    /// Generate M68k assembly from IR module as text
    pub fn generate(&mut self, module: &IrModule) -> CompileResult<String> {
        let instructions = self.generate_instructions(module)?;

        // Format output
        let mut result = String::new();
        for inst in &instructions {
            result.push_str(&inst.format());
            result.push('\n');
        }

        Ok(result)
    }

    /// Generate M68k instructions from IR module (for binary output)
    pub fn generate_instructions(&mut self, module: &IrModule) -> CompileResult<Vec<M68kInst>> {
        self.output.clear();
        self.pending_sdk_functions.clear();
        self.defined_functions.clear();

        // Track all user-defined functions to avoid SDK conflicts
        for func in &module.functions {
            self.defined_functions.insert(func.name.clone());
        }

        // Calculate total data size first (for RAM allocation)
        self.data_size = 0;
        for global in &module.globals {
            let size = global.ty.size();
            // Align to 4 bytes for efficiency
            self.data_size = (self.data_size + 3) & !3;
            self.data_size += size;
        }
        for (_, string) in &module.strings {
            self.data_size += string.len() + 1; // +1 for null terminator
        }
        // Align final size
        self.data_size = (self.data_size + 3) & !3;

        // Emit header
        self.emit(M68kInst::Directive(".section .text".to_string()));
        self.emit(M68kInst::Directive(".align 2".to_string()));

        // Emit startup stub at entry point (0x200)
        // This ensures the ROM starts properly regardless of function order
        self.emit_startup_stub();

        // Emit user functions
        for func in &module.functions {
            self.generate_function(func)?;
        }

        // Emit SDK library functions that were used
        self.emit_sdk_library_functions();

        // Emit data section with ROM initial values and RAM references
        if !module.globals.is_empty() || !module.strings.is_empty() {
            // Emit label for ROM location BEFORE switching to data section
            // This label gets a ROM address (where initial values are stored)
            self.emit(M68kInst::Directive(".align 2".to_string()));
            self.emit(M68kInst::Label("__data_rom_start".to_string()));

            // Now switch to data section - labels get RAM addresses
            self.emit(M68kInst::Directive(".section .data".to_string()));
            self.emit(M68kInst::Directive(".align 2".to_string()));

            // Mark start of data in RAM
            self.emit(M68kInst::Label("__data_ram_start".to_string()));

            for global in &module.globals {
                self.emit(M68kInst::Label(global.name.clone()));
                if let Some(init_bytes) = &global.init {
                    // Emit initialized data
                    self.emit_data_bytes(init_bytes);
                } else {
                    // Zero-initialized
                    let size = global.ty.size();
                    match size {
                        1 => self.emit(M68kInst::Directive(".byte 0".to_string())),
                        2 => self.emit(M68kInst::Directive(".word 0".to_string())),
                        _ => self.emit(M68kInst::Directive(format!(".space {}", size))),
                    }
                }
            }

            for (label, string) in &module.strings {
                self.emit(M68kInst::Label(label.0.clone()));
                // Escape the string for assembly
                let escaped = string
                    .replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
                    .replace('\r', "\\r")
                    .replace('\t', "\\t")
                    .replace('\0', "\\0");
                self.emit(M68kInst::Directive(format!(".asciz \"{}\"", escaped)));
            }

            // Mark end of data in RAM
            self.emit(M68kInst::Directive(".align 2".to_string()));
            self.emit(M68kInst::Label("__data_ram_end".to_string()));
        }

        // Emit SDK static data (frame counter, operator offsets, etc.)
        self.emit_sdk_static_data();

        Ok(std::mem::take(&mut self.output))
    }

    fn emit(&mut self, inst: M68kInst) {
        self.output.push(inst);
    }

    /// Emit the startup stub that runs at entry point (0x200)
    /// This initializes the Genesis hardware and calls main
    fn emit_startup_stub(&mut self) {
        self.emit(M68kInst::Label("_start".to_string()));
        self.emit(M68kInst::Directive(".global _start".to_string()));

        // Disable interrupts during initialization
        // move.w #$2700, sr (set supervisor mode, disable interrupts)
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x2700),
            Operand::Sr,
        ));

        // Clear TMSS (Trademark Security System) - Required for real hardware
        // Write 'SEGA' to $A14000 if version register indicates TMSS
        // lea $A10001, a0  ; Version register
        // btst #0, (a0)    ; Check if TMSS present
        // beq .no_tmss
        // move.l #$53454741, $A14000 ; Write 'SEGA'
        // .no_tmss:
        self.emit(M68kInst::Lea(Operand::AbsLong(0xA10001), AddrReg::A0));
        self.emit(M68kInst::Btst(Operand::Imm(0), Operand::AddrInd(AddrReg::A0)));
        self.emit(M68kInst::Bcc(Cond::Eq, ".no_tmss".to_string()));
        self.emit(M68kInst::Move(
            Size::Long,
            Operand::Imm(0x53454741), // 'SEGA'
            Operand::AbsLong(0xA14000),
        ));
        self.emit(M68kInst::Label(".no_tmss".to_string()));

        // Request Z80 bus and reset Z80 for PSG access
        // Write $0100 to $A11100 to request Z80 bus
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x0100),
            Operand::AbsLong(0xA11100),
        ));
        // Write $0100 to $A11200 to release Z80 reset
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x0100),
            Operand::AbsLong(0xA11200),
        ));
        // Wait for Z80 bus grant
        self.emit(M68kInst::Label(".wait_z80".to_string()));
        self.emit(M68kInst::Btst(Operand::Imm(0), Operand::AbsLong(0xA11100)));
        self.emit(M68kInst::Bcc(Cond::Ne, ".wait_z80".to_string()));

        // Initialize RAM - clear first 64KB of work RAM
        // lea $FF0000, a0  ; Start of RAM
        // move.w #$3FFF, d0 ; 64KB / 4 - 1 = $3FFF longs
        // .clear_ram:
        // clr.l (a0)+
        // dbf d0, .clear_ram
        self.emit(M68kInst::Lea(Operand::AbsLong(0xFF0000), AddrReg::A0));
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x3FFF),
            Operand::DataReg(DataReg::D0),
        ));
        self.emit(M68kInst::Label(".clear_ram".to_string()));
        self.emit(M68kInst::Clr(Size::Long, Operand::PostInc(AddrReg::A0)));
        self.emit(M68kInst::Dbf(DataReg::D0, ".clear_ram".to_string()));

        // Copy initialized data from ROM to RAM
        // Source: __data_rom_start (ROM address)
        // Dest: __data_ram_start (RAM address = 0xFF8000)
        // Count: __data_ram_end - __data_ram_start
        self.emit(M68kInst::Lea(Operand::Label("__data_rom_start".to_string()), AddrReg::A0));  // Source in ROM
        self.emit(M68kInst::Lea(Operand::Label("__data_ram_start".to_string()), AddrReg::A1)); // Dest in RAM
        self.emit(M68kInst::Lea(Operand::Label("__data_ram_end".to_string()), AddrReg::A2));   // End marker
        self.emit(M68kInst::Label(".copy_data".to_string()));
        self.emit(M68kInst::Cmpa(Size::Long, Operand::AddrReg(AddrReg::A1), AddrReg::A2));     // Compare A1 with A2
        self.emit(M68kInst::Bcc(Cond::Le, ".copy_done".to_string()));  // If A2 <= A1, we're done (end reached)
        self.emit(M68kInst::Move(Size::Long, Operand::PostInc(AddrReg::A0), Operand::PostInc(AddrReg::A1)));
        self.emit(M68kInst::Bra(".copy_data".to_string()));
        self.emit(M68kInst::Label(".copy_done".to_string()));

        // Set up stack pointer (already set by vector table, but ensure it's correct)
        self.emit(M68kInst::Move(
            Size::Long,
            Operand::Imm(0x00FFE000u32 as i32),
            Operand::AddrReg(AddrReg::A7),
        ));

        // Initialize VDP - write directly to $C00004
        // This ensures VDP is set up before any C code runs
        self.emit(M68kInst::Lea(Operand::AbsLong(0xC00004), AddrReg::A1));
        // VDP register writes: 0x8000 | (reg << 8) | value
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x8004), Operand::AddrInd(AddrReg::A1))); // Reg 0
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x8104), Operand::AddrInd(AddrReg::A1))); // Reg 1 - display OFF (vdp_init enables after VRAM clear)
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x8230), Operand::AddrInd(AddrReg::A1))); // Reg 2
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x8407), Operand::AddrInd(AddrReg::A1))); // Reg 4
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x8578), Operand::AddrInd(AddrReg::A1))); // Reg 5
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x8700), Operand::AddrInd(AddrReg::A1))); // Reg 7 - backdrop=0 (black)
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x8AFF), Operand::AddrInd(AddrReg::A1))); // Reg 10
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x8B00), Operand::AddrInd(AddrReg::A1))); // Reg 11
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x8C81), Operand::AddrInd(AddrReg::A1))); // Reg 12
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x8D3F), Operand::AddrInd(AddrReg::A1))); // Reg 13
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x8F02), Operand::AddrInd(AddrReg::A1))); // Reg 15
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x9011), Operand::AddrInd(AddrReg::A1))); // Reg 16: H64xV32

        // Clear all VRAM (64KB) from address 0 upward
        // Set VRAM write address to 0x0000
        // Command format: first word = 0x4000 | (addr & 0x3FFF), second word = (addr >> 14)
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x4000), Operand::AddrInd(AddrReg::A1))); // VRAM write @ 0x0000
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x0000), Operand::AddrInd(AddrReg::A1))); // Upper addr = 0
        // Clear 64KB (32K words)
        self.emit(M68kInst::Lea(Operand::AbsLong(0xC00000), AddrReg::A1));
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x7FFF), Operand::DataReg(DataReg::D0)));
        self.emit(M68kInst::Label(".clear_vram".to_string()));
        self.emit(M68kInst::Clr(Size::Word, Operand::AddrInd(AddrReg::A1)));
        self.emit(M68kInst::Dbf(DataReg::D0, ".clear_vram".to_string()));

        // Re-setup A1 for palette write
        self.emit(M68kInst::Lea(Operand::AbsLong(0xC00004), AddrReg::A1));

        // Set up palette - CRAM write
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0xC000u32 as i32), Operand::AddrInd(AddrReg::A1)));
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x0000), Operand::AddrInd(AddrReg::A1)));
        // Write colors to VDP data port ($C00000)
        self.emit(M68kInst::Lea(Operand::AbsLong(0xC00000), AddrReg::A1));
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x0000), Operand::AddrInd(AddrReg::A1))); // Black
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x0EEE), Operand::AddrInd(AddrReg::A1))); // White
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x00E0), Operand::AddrInd(AddrReg::A1))); // Green
        self.emit(M68kInst::Move(Size::Word, Operand::Imm(0x000E), Operand::AddrInd(AddrReg::A1))); // Red

        // Silence all PSG channels
        // PSG is at $C00011, accessed via byte writes
        // Volume command: 1xx1 vvvv where xx=channel, vvvv=volume (F=silent)
        self.emit(M68kInst::Lea(Operand::AbsLong(0xC00011), AddrReg::A2));
        self.emit(M68kInst::Move(Size::Byte, Operand::Imm(0x9F), Operand::AddrInd(AddrReg::A2))); // Ch 0 silent
        self.emit(M68kInst::Move(Size::Byte, Operand::Imm(0xBF), Operand::AddrInd(AddrReg::A2))); // Ch 1 silent
        self.emit(M68kInst::Move(Size::Byte, Operand::Imm(0xDF), Operand::AddrInd(AddrReg::A2))); // Ch 2 silent
        self.emit(M68kInst::Move(Size::Byte, Operand::Imm(0xFF), Operand::AddrInd(AddrReg::A2))); // Ch 3 (noise) silent

        // Enable interrupts
        // move.w #$2000, sr (user mode, enable interrupts)
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x2000),
            Operand::Sr,
        ));

        // Jump to main
        self.emit(M68kInst::Jsr(Operand::Label("main".to_string())));

        // Infinite loop after main returns (shouldn't happen in a game)
        self.emit(M68kInst::Label(".halt".to_string()));
        self.emit(M68kInst::Bra(".halt".to_string()));

        // Add some padding/alignment
        self.emit(M68kInst::Directive(".align 2".to_string()));
    }

    /// Emit initialized data bytes
    fn emit_data_bytes(&mut self, bytes: &[u8]) {
        let mut i = 0;
        let len = bytes.len();

        // Emit 4-byte (long) values where possible
        while i + 4 <= len {
            let val = u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
            self.emit(M68kInst::Directive(format!(".long 0x{:08X}", val)));
            i += 4;
        }

        // Emit 2-byte (word) values
        while i + 2 <= len {
            let val = u16::from_be_bytes([bytes[i], bytes[i + 1]]);
            self.emit(M68kInst::Directive(format!(".word 0x{:04X}", val)));
            i += 2;
        }

        // Emit remaining bytes
        while i < len {
            self.emit(M68kInst::Directive(format!(".byte 0x{:02X}", bytes[i])));
            i += 1;
        }
    }

    fn generate_function(&mut self, func: &IrFunction) -> CompileResult<()> {
        // Reset state
        self.temp_offsets.clear();
        self.next_offset = -4;

        // Count how many temps we need
        let mut max_temp = 0u32;
        for inst in &func.body {
            match inst {
                Inst::Copy { dst, .. }
                | Inst::Unary { dst, .. }
                | Inst::Binary { dst, .. }
                | Inst::Load { dst, .. }
                | Inst::Alloca { dst, .. }
                | Inst::AddrOf { dst, .. } => {
                    max_temp = max_temp.max(dst.0 + 1);
                }
                Inst::Call { dst: Some(dst), .. } => {
                    max_temp = max_temp.max(dst.0 + 1);
                }
                _ => {}
            }
        }

        // Calculate frame size (temps + locals + saved regs)
        self.frame_size = (max_temp as i16) * 4 + 16; // Extra space for saved regs
        // Align to 4 bytes
        self.frame_size = (self.frame_size + 3) & !3;

        // Emit function label
        self.emit(M68kInst::Directive(format!(".global {}", func.name)));
        self.emit(M68kInst::Label(func.name.clone()));

        // Prologue
        self.emit(M68kInst::Link(AddrReg::A6, -self.frame_size));
        // Save callee-saved registers
        self.emit(M68kInst::Movem(
            Size::Long,
            vec![
                Reg::Data(DataReg::D2),
                Reg::Data(DataReg::D3),
                Reg::Data(DataReg::D4),
                Reg::Data(DataReg::D5),
                Reg::Data(DataReg::D6),
                Reg::Data(DataReg::D7),
                Reg::Addr(AddrReg::A2),
                Reg::Addr(AddrReg::A3),
                Reg::Addr(AddrReg::A4),
                Reg::Addr(AddrReg::A5),
            ],
            Operand::PreDec(AddrReg::A7),
            true,
        ));

        // Generate body
        for inst in &func.body {
            self.generate_inst(inst)?;
        }

        Ok(())
    }

    fn get_temp_offset(&mut self, temp: Temp) -> i16 {
        if let Some(&offset) = self.temp_offsets.get(&temp.0) {
            offset
        } else {
            let offset = self.next_offset;
            self.next_offset -= 4;
            self.temp_offsets.insert(temp.0, offset);
            offset
        }
    }

    fn load_value(&mut self, value: &Value, reg: DataReg) -> CompileResult<()> {
        match value {
            Value::IntConst(n) => {
                if *n >= -128 && *n <= 127 {
                    self.emit(M68kInst::Moveq(*n as i8, reg));
                } else {
                    self.emit(M68kInst::Move(
                        Size::Long,
                        Operand::Imm(*n as i32),
                        Operand::DataReg(reg),
                    ));
                }
            }
            Value::Temp(t) => {
                let offset = self.get_temp_offset(*t);
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::Disp(offset, AddrReg::A6),
                    Operand::DataReg(reg),
                ));
            }
            Value::Name(name) => {
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::Label(name.clone()),
                    Operand::DataReg(reg),
                ));
            }
            Value::StringConst(label) => {
                self.emit(M68kInst::Lea(
                    Operand::Label(label.0.clone()),
                    AddrReg::A0,
                ));
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::AddrReg(AddrReg::A0),
                    Operand::DataReg(reg),
                ));
            }
            Value::Mem(addr) => {
                self.load_value(addr, reg)?;
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::AddrReg(AddrReg::A0),
                    Operand::DataReg(DataReg::D0),
                ));
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::DataReg(DataReg::D0),
                    Operand::AddrReg(AddrReg::A0),
                ));
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::AddrInd(AddrReg::A0),
                    Operand::DataReg(reg),
                ));
            }
        }
        Ok(())
    }

    fn store_temp(&mut self, temp: Temp, reg: DataReg) {
        let offset = self.get_temp_offset(temp);
        self.emit(M68kInst::Move(
            Size::Long,
            Operand::DataReg(reg),
            Operand::Disp(offset, AddrReg::A6),
        ));
    }

    fn generate_inst(&mut self, inst: &Inst) -> CompileResult<()> {
        match inst {
            Inst::Label(label) => {
                self.emit(M68kInst::Label(label.0.clone()));
            }

            Inst::Copy { dst, src } => {
                self.load_value(src, DataReg::D0)?;
                self.store_temp(*dst, DataReg::D0);
            }

            Inst::Unary { dst, op, src } => {
                self.load_value(src, DataReg::D0)?;
                match op {
                    UnOp::Neg => {
                        self.emit(M68kInst::Neg(Size::Long, Operand::DataReg(DataReg::D0)));
                    }
                    UnOp::Not => {
                        // Logical not: result is 0 if non-zero, 1 if zero
                        self.emit(M68kInst::Tst(Size::Long, Operand::DataReg(DataReg::D0)));
                        self.emit(M68kInst::Scc(Cond::Eq, Operand::DataReg(DataReg::D0)));
                        self.emit(M68kInst::And(
                            Size::Long,
                            Operand::Imm(1),
                            Operand::DataReg(DataReg::D0),
                        ));
                    }
                    UnOp::BitNot => {
                        self.emit(M68kInst::Not(Size::Long, Operand::DataReg(DataReg::D0)));
                    }
                }
                self.store_temp(*dst, DataReg::D0);
            }

            Inst::Binary { dst, op, left, right } => {
                self.load_value(left, DataReg::D0)?;
                self.load_value(right, DataReg::D1)?;

                match op {
                    BinOp::Add => {
                        self.emit(M68kInst::Add(
                            Size::Long,
                            Operand::DataReg(DataReg::D1),
                            Operand::DataReg(DataReg::D0),
                        ));
                    }
                    BinOp::Sub => {
                        self.emit(M68kInst::Sub(
                            Size::Long,
                            Operand::DataReg(DataReg::D1),
                            Operand::DataReg(DataReg::D0),
                        ));
                    }
                    BinOp::Mul => {
                        // M68000 only has 16x16->32 multiply
                        self.emit(M68kInst::Muls(
                            Operand::DataReg(DataReg::D1),
                            DataReg::D0,
                        ));
                    }
                    BinOp::Div => {
                        // 32/16->16r16 signed divide
                        self.emit(M68kInst::Divs(
                            Operand::DataReg(DataReg::D1),
                            DataReg::D0,
                        ));
                        // Quotient is in low word, sign-extend
                        self.emit(M68kInst::Ext(Size::Long, DataReg::D0));
                    }
                    BinOp::Mod => {
                        // 32/16->16r16 signed divide for remainder
                        self.emit(M68kInst::Divs(
                            Operand::DataReg(DataReg::D1),
                            DataReg::D0,
                        ));
                        // Remainder is in high word
                        self.emit(M68kInst::Swap(DataReg::D0));
                        self.emit(M68kInst::Ext(Size::Long, DataReg::D0));
                    }
                    BinOp::UDiv => {
                        // 32/16->16r16 unsigned divide
                        self.emit(M68kInst::Divu(
                            Operand::DataReg(DataReg::D1),
                            DataReg::D0,
                        ));
                        // Quotient is in low word, zero-extend
                        self.emit(M68kInst::Andi(Size::Long, 0xFFFF, Operand::DataReg(DataReg::D0)));
                    }
                    BinOp::UMod => {
                        // 32/16->16r16 unsigned divide for remainder
                        self.emit(M68kInst::Divu(
                            Operand::DataReg(DataReg::D1),
                            DataReg::D0,
                        ));
                        // Remainder is in high word
                        self.emit(M68kInst::Swap(DataReg::D0));
                        self.emit(M68kInst::Andi(Size::Long, 0xFFFF, Operand::DataReg(DataReg::D0)));
                    }
                    BinOp::And => {
                        self.emit(M68kInst::And(
                            Size::Long,
                            Operand::DataReg(DataReg::D1),
                            Operand::DataReg(DataReg::D0),
                        ));
                    }
                    BinOp::Or => {
                        self.emit(M68kInst::Or(
                            Size::Long,
                            Operand::DataReg(DataReg::D1),
                            Operand::DataReg(DataReg::D0),
                        ));
                    }
                    BinOp::Xor => {
                        self.emit(M68kInst::Eor(
                            Size::Long,
                            DataReg::D1,
                            Operand::DataReg(DataReg::D0),
                        ));
                    }
                    BinOp::Shl => {
                        self.emit(M68kInst::Lsl(
                            Size::Long,
                            Operand::DataReg(DataReg::D1),
                            DataReg::D0,
                        ));
                    }
                    BinOp::Shr => {
                        self.emit(M68kInst::Lsr(
                            Size::Long,
                            Operand::DataReg(DataReg::D1),
                            DataReg::D0,
                        ));
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                        self.emit(M68kInst::Cmp(
                            Size::Long,
                            Operand::DataReg(DataReg::D1),
                            Operand::DataReg(DataReg::D0),
                        ));
                        let cond = match op {
                            BinOp::Eq => Cond::Eq,
                            BinOp::Ne => Cond::Ne,
                            BinOp::Lt => Cond::Lt,
                            BinOp::Le => Cond::Le,
                            BinOp::Gt => Cond::Gt,
                            BinOp::Ge => Cond::Ge,
                            _ => unreachable!(),
                        };
                        self.emit(M68kInst::Scc(cond, Operand::DataReg(DataReg::D0)));
                        self.emit(M68kInst::And(
                            Size::Long,
                            Operand::Imm(1),
                            Operand::DataReg(DataReg::D0),
                        ));
                    }
                }
                self.store_temp(*dst, DataReg::D0);
            }

            Inst::Load { dst, addr, size, volatile: _, signed } => {
                // Note: volatile flag indicates this memory access should not be optimized.
                // For now, we emit the same code (no optimization pass yet).
                self.load_value(addr, DataReg::D0)?;
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::DataReg(DataReg::D0),
                    Operand::AddrReg(AddrReg::A0),
                ));
                let sz = Size::from_bytes(*size);
                self.emit(M68kInst::Move(
                    sz,
                    Operand::AddrInd(AddrReg::A0),
                    Operand::DataReg(DataReg::D0),
                ));
                // Extend to 32-bit: sign-extend for signed types, zero-extend for unsigned
                // On 68000: ext.w extends byte->word, ext.l extends word->long (sign extension)
                // For unsigned, use AND to zero-extend
                if *signed {
                    // Sign extend
                    if *size == 1 {
                        self.emit(M68kInst::Ext(Size::Word, DataReg::D0)); // byte -> word
                        self.emit(M68kInst::Ext(Size::Long, DataReg::D0)); // word -> long
                    } else if *size == 2 {
                        self.emit(M68kInst::Ext(Size::Long, DataReg::D0)); // word -> long
                    }
                } else {
                    // Zero extend using AND
                    if *size == 1 {
                        self.emit(M68kInst::Andi(Size::Long, 0xFF, Operand::DataReg(DataReg::D0)));
                    } else if *size == 2 {
                        self.emit(M68kInst::Andi(Size::Long, 0xFFFF, Operand::DataReg(DataReg::D0)));
                    }
                }
                self.store_temp(*dst, DataReg::D0);
            }

            Inst::Store { addr, src, size, volatile: _ } => {
                // Note: volatile flag indicates this memory access should not be optimized.
                // For now, we emit the same code (no optimization pass yet).
                self.load_value(addr, DataReg::D0)?;
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::DataReg(DataReg::D0),
                    Operand::AddrReg(AddrReg::A0),
                ));
                self.load_value(src, DataReg::D1)?;
                let sz = Size::from_bytes(*size);
                self.emit(M68kInst::Move(
                    sz,
                    Operand::DataReg(DataReg::D1),
                    Operand::AddrInd(AddrReg::A0),
                ));
            }

            Inst::Jump(label) => {
                self.emit(M68kInst::Bra(label.0.clone()));
            }

            Inst::CondJump { cond, target } => {
                self.load_value(cond, DataReg::D0)?;
                self.emit(M68kInst::Tst(Size::Long, Operand::DataReg(DataReg::D0)));
                self.emit(M68kInst::Bcc(Cond::Ne, target.0.clone()));
            }

            Inst::CondJumpFalse { cond, target } => {
                self.load_value(cond, DataReg::D0)?;
                self.emit(M68kInst::Tst(Size::Long, Operand::DataReg(DataReg::D0)));
                self.emit(M68kInst::Bcc(Cond::Eq, target.0.clone()));
            }

            Inst::Call { dst, func, args } => {
                // Check if this is an SDK function (but not if user defined their own)
                let is_sdk = !self.defined_functions.contains(func)
                    && self.sdk_registry.lookup(func).is_some();

                if is_sdk {
                    let sdk_func = self.sdk_registry.lookup(func).unwrap();
                    match sdk_func.kind {
                        SdkFunctionKind::Inline => {
                            // Emit inline code
                            self.emit_sdk_inline_call(func, args, dst)?;
                        }
                        SdkFunctionKind::Library => {
                            // Mark function as needed, emit normal call
                            self.pending_sdk_functions.insert(func.clone());
                            self.emit_standard_call(func, args, dst)?;
                        }
                    }
                } else {
                    // Regular user function call
                    self.emit_standard_call(func, args, dst)?;
                }
            }

            Inst::Return(value) => {
                if let Some(val) = value {
                    self.load_value(val, DataReg::D0)?;
                }

                // Epilogue
                // Restore callee-saved registers
                self.emit(M68kInst::Movem(
                    Size::Long,
                    vec![
                        Reg::Data(DataReg::D2),
                        Reg::Data(DataReg::D3),
                        Reg::Data(DataReg::D4),
                        Reg::Data(DataReg::D5),
                        Reg::Data(DataReg::D6),
                        Reg::Data(DataReg::D7),
                        Reg::Addr(AddrReg::A2),
                        Reg::Addr(AddrReg::A3),
                        Reg::Addr(AddrReg::A4),
                        Reg::Addr(AddrReg::A5),
                    ],
                    Operand::PostInc(AddrReg::A7),
                    false,
                ));
                self.emit(M68kInst::Unlk(AddrReg::A6));
                self.emit(M68kInst::Rts);
            }

            Inst::Alloca { dst, size, .. } => {
                // Allocate on stack by subtracting from SP
                // But we use frame-relative addressing, so just assign an offset
                let offset = self.get_temp_offset(*dst);
                // Reserve additional space for the allocated storage
                // This prevents other temps from overlapping with alloca storage
                let storage_offset = self.next_offset;
                self.next_offset -= ((*size as i16) + 3) & !3; // Align to 4 bytes
                // Store address of allocated space
                self.emit(M68kInst::Lea(
                    Operand::Disp(storage_offset, AddrReg::A6),
                    AddrReg::A0,
                ));
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::AddrReg(AddrReg::A0),
                    Operand::Disp(offset, AddrReg::A6),
                ));
            }

            Inst::AddrOf { dst, name } => {
                self.emit(M68kInst::Lea(
                    Operand::Label(name.clone()),
                    AddrReg::A0,
                ));
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::AddrReg(AddrReg::A0),
                    Operand::DataReg(DataReg::D0),
                ));
                self.store_temp(*dst, DataReg::D0);
            }

            Inst::LoadParam { dst, index, size } => {
                // In M68k cdecl with LINK A6:
                // - 0(a6) = saved old A6
                // - 4(a6) = return address
                // - 8(a6) = first parameter
                // - 12(a6) = second parameter, etc.
                // Store the ADDRESS of the parameter slot, not its value.
                // This matches the Alloca model where temps hold addresses.
                //
                // On big-endian M68K, callers push all values as 32-bit longs.
                // For smaller types, the value is in the LOW bytes of the slot.
                // We need to adjust the offset to point to the actual value:
                // - 4-byte: offset + 0
                // - 2-byte: offset + 2
                // - 1-byte: offset + 3
                let base_offset = 8 + (*index as i16) * 4;
                let size_adjust = match *size {
                    1 => 3,  // byte at end of 4-byte slot
                    2 => 2,  // word at end of 4-byte slot
                    _ => 0,  // long uses full slot
                };
                let offset = base_offset + size_adjust;
                self.emit(M68kInst::Lea(
                    Operand::Disp(offset, AddrReg::A6),
                    AddrReg::A0,
                ));
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::AddrReg(AddrReg::A0),
                    Operand::DataReg(DataReg::D0),
                ));
                self.store_temp(*dst, DataReg::D0);
            }

            Inst::Comment(c) => {
                self.emit(M68kInst::Comment(c.clone()));
            }
        }

        Ok(())
    }

    // =========================================================================
    // SDK Support Methods
    // =========================================================================

    /// Emit inline code for an SDK function call
    fn emit_sdk_inline_call(
        &mut self,
        func: &str,
        args: &[Value],
        dst: &Option<Temp>,
    ) -> CompileResult<()> {
        // Load arguments into D0, D1, D2, D3 in order
        let arg_regs = [DataReg::D0, DataReg::D1, DataReg::D2, DataReg::D3];
        for (i, arg) in args.iter().enumerate() {
            if i < arg_regs.len() {
                self.load_value(arg, arg_regs[i])?;
            }
        }

        // Generate inline instructions
        let inline_code = SdkInlineGenerator::generate(func);
        for inst in inline_code {
            self.emit(inst);
        }

        // Store return value if needed
        if let Some(d) = dst {
            self.store_temp(*d, DataReg::D0);
        }

        Ok(())
    }

    /// Emit a standard function call (push args, JSR, clean stack)
    fn emit_standard_call(
        &mut self,
        func: &str,
        args: &[Value],
        dst: &Option<Temp>,
    ) -> CompileResult<()> {
        // Push arguments right-to-left
        for arg in args.iter().rev() {
            self.load_value(arg, DataReg::D0)?;
            self.emit(M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D0),
                Operand::PreDec(AddrReg::A7),
            ));
        }

        // Call function
        self.emit(M68kInst::Jsr(Operand::Label(func.to_string())));

        // Clean up stack
        let stack_size = (args.len() * 4) as i32;
        if stack_size > 0 {
            if stack_size <= 8 {
                self.emit(M68kInst::Addq(
                    Size::Long,
                    stack_size as u8,
                    Operand::AddrReg(AddrReg::A7),
                ));
            } else {
                self.emit(M68kInst::Adda(
                    Size::Long,
                    Operand::Imm(stack_size),
                    AddrReg::A7,
                ));
            }
        }

        // Store return value
        if let Some(d) = dst {
            self.store_temp(*d, DataReg::D0);
        }

        Ok(())
    }

    /// Emit SDK library functions that were used
    fn emit_sdk_library_functions(&mut self) {
        if self.pending_sdk_functions.is_empty() {
            return;
        }

        // Resolve all dependencies
        let all_functions = resolve_dependencies(&self.pending_sdk_functions);

        // Filter to only library functions
        let mut library_functions: Vec<_> = all_functions
            .iter()
            .filter(|f| {
                self.sdk_registry.lookup(f)
                    .map(|sdk| sdk.kind == SdkFunctionKind::Library)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        // Sort for deterministic output
        library_functions.sort();

        // Generate each function
        let mut generator = SdkLibraryGenerator::new();
        for func_name in library_functions {
            self.emit(M68kInst::Comment(format!("SDK function: {}", func_name)));
            let code = generator.generate(&func_name);
            for inst in code {
                self.emit(inst);
            }
        }
    }

    /// Emit SDK static data (frame counter, operator offsets, etc.)
    fn emit_sdk_static_data(&mut self) {
        if self.pending_sdk_functions.is_empty() {
            return;
        }

        // Resolve all dependencies to check what data is needed
        let all_functions = resolve_dependencies(&self.pending_sdk_functions);

        // Generate static data
        let static_data = generate_static_data(&all_functions);
        for inst in static_data {
            self.emit(inst);
        }
    }
}

impl Default for CodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}
