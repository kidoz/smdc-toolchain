//! M68k code emitter

use super::m68k::*;
use super::runtime;
use super::sdk::{
    SdkFunctionKind, SdkInlineGenerator, SdkLibraryGenerator, SdkRegistry, generate_static_data,
    resolve_dependencies,
};
use crate::common::CompileResult;
use crate::ir::*;
use std::collections::{BTreeSet, HashMap, HashSet};

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
    /// Runtime helpers (32-bit multiply/divide) called so far
    pending_runtime: BTreeSet<&'static str>,
    /// Whether to emit debug source comments
    debug_enabled: bool,
    /// Source filename for debug comments
    debug_filename: String,
    /// Source text for byte-offset â†’ line mapping
    debug_source: String,

    // ---- Block-local register cache (D2-D7) ----
    // Maps an IR temp id to a cached data register holding its current value.
    // The stack slot in `temp_offsets` remains the authoritative home location;
    // the cache only avoids redundant load/store traffic within a basic block.
    // It is flushed (written back + cleared) at every block and call boundary.
    reg_cache: HashMap<u32, DataReg>,
    // LRU-ordered list of cached temps (most-recently-used at the back) for eviction.
    reg_cache_lru: Vec<u32>,
    // Free callee-saved data registers available for caching (D2-D7).
    // D0 and D1 stay reserved as scratch; A0/A1 as address scratch.
    free_data_regs: Vec<DataReg>,
    // ---- Alloca fast path ----
    // Alloca temps whose storage offset is known: `Load`/`Store` through the
    // bare temp access the frame slot directly instead of going through a
    // materialized pointer cell. The address itself is only computed when it
    // escapes (e.g. `&local`, member arithmetic), in `load_value`.
    alloca_storage: HashMap<u32, i16>,
    // Remaining reads of each temp id within the block being emitted. The
    // register-targeted paths use this to detect an operand's last use: a
    // count reaching zero means the cached register may be recomputed into.
    block_use_counts: HashMap<u32, u32>,
}

/// Callee-saved data registers usable by the block-local cache, in preference order.
/// D0/D1 are caller-saved scratch and intentionally excluded.
/// The MOVEM prologue already saves/restores these, so caching in them needs no
/// prologue change.
const CACHE_REGS: [DataReg; 6] = [
    DataReg::D2,
    DataReg::D3,
    DataReg::D4,
    DataReg::D5,
    DataReg::D6,
    DataReg::D7,
];

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
            pending_runtime: BTreeSet::new(),
            debug_enabled: false,
            debug_filename: String::new(),
            debug_source: String::new(),
            reg_cache: HashMap::new(),
            reg_cache_lru: Vec::new(),
            free_data_regs: CACHE_REGS.to_vec(),
            alloca_storage: HashMap::new(),
            block_use_counts: HashMap::new(),
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

    /// Enable debug output (source comments in assembly)
    pub fn set_debug_info(&mut self, filename: String, source: String) {
        self.debug_enabled = true;
        self.debug_filename = filename;
        self.debug_source = source;
    }

    /// Generate only the module's functions (plus any runtime/SDK helpers
    /// they use): no startup stub, no vector/header stub, no data section.
    /// This is the codegen mode for hybrid ROM rebuilds, where the original
    /// image provides the header, vectors and data.
    pub fn generate_functions_only(&mut self, module: &IrModule) -> CompileResult<Vec<M68kInst>> {
        self.output.clear();
        self.pending_sdk_functions.clear();
        self.defined_functions.clear();
        self.pending_runtime.clear();

        for func in &module.functions {
            self.defined_functions.insert(func.name.clone());
        }
        self.data_size = 0;

        self.emit(M68kInst::Directive(".section .text".to_string()));
        self.emit(M68kInst::Directive(".align 2".to_string()));

        for func in &module.functions {
            self.generate_function(func)?;
        }

        self.emit_sdk_library_functions()?;
        self.emit_runtime_helpers();

        let mut out = std::mem::take(&mut self.output);
        peephole(&mut out);
        Ok(out)
    }

    /// Generate M68k instructions from IR module (for binary output)
    pub fn generate_instructions(&mut self, module: &IrModule) -> CompileResult<Vec<M68kInst>> {
        self.output.clear();
        self.pending_sdk_functions.clear();
        self.defined_functions.clear();
        self.pending_runtime.clear();

        // Debug info is configured externally via set_debug_info() before calling this

        // Track all user-defined functions to avoid SDK conflicts
        for func in &module.functions {
            self.defined_functions.insert(func.name.clone());
        }

        // Calculate total data size first (for RAM allocation)
        self.data_size = 0;
        for global in &module.globals {
            let size = global.ty.size;
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
        self.emit_sdk_library_functions()?;

        // Emit runtime helpers that were used
        self.emit_runtime_helpers();

        // Emit data section with ROM initial values and RAM references.
        // The startup stub always references these symbols, so an empty module
        // still needs zero-length section boundaries.
        self.emit(M68kInst::Directive(".align 2".to_string()));
        self.emit(M68kInst::Label("__data_rom_start".to_string()));

        // Now switch to data section - labels get RAM addresses
        self.emit(M68kInst::Directive(".section .data".to_string()));
        self.emit(M68kInst::Directive(".align 2".to_string()));

        // Mark start of data in RAM
        self.emit(M68kInst::Label("__data_ram_start".to_string()));

        for global in &module.globals {
            // Word and long accesses to an odd address raise an address
            // error on the 68000, so anything but byte data starts even
            if global.ty.align >= 2 {
                self.emit(M68kInst::Directive(".align 2".to_string()));
            }
            self.emit(M68kInst::Label(global.name.clone()));
            if let Some(init_bytes) = &global.init {
                // Emit initialized data
                self.emit_data_bytes(init_bytes);
            } else {
                // Zero-initialized
                let size = global.ty.size;
                match size {
                    1 => self.emit(M68kInst::Directive(".byte 0".to_string())),
                    2 => self.emit(M68kInst::Directive(".word 0".to_string())),
                    _ => self.emit(M68kInst::Directive(format!(".space {size}"))),
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
            self.emit(M68kInst::Directive(format!(".asciz \"{escaped}\"")));
        }

        // Mark end of data in RAM
        self.emit(M68kInst::Directive(".align 2".to_string()));
        self.emit(M68kInst::Label("__data_ram_end".to_string()));

        // Emit SDK static data (frame counter, operator offsets, etc.)
        self.emit_sdk_static_data();

        let mut out = std::mem::take(&mut self.output);
        peephole(&mut out);
        Ok(out)
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
        self.emit(M68kInst::Btst(
            Operand::Imm(0),
            Operand::AddrInd(AddrReg::A0),
        ));
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
        self.emit(M68kInst::Dbcc(Cond::False, DataReg::D0, ".clear_ram".to_string()));

        // Copy initialized data from ROM to RAM
        // Source: __data_rom_start (ROM address)
        // Dest: __data_ram_start (RAM address = 0xFF8000)
        // Count: __data_ram_end - __data_ram_start
        self.emit(M68kInst::Lea(
            Operand::Label("__data_rom_start".to_string()),
            AddrReg::A0,
        )); // Source in ROM
        self.emit(M68kInst::Lea(
            Operand::Label("__data_ram_start".to_string()),
            AddrReg::A1,
        )); // Dest in RAM
        self.emit(M68kInst::Lea(
            Operand::Label("__data_ram_end".to_string()),
            AddrReg::A2,
        )); // End marker
        self.emit(M68kInst::Label(".copy_data".to_string()));
        self.emit(M68kInst::Cmpa(
            Size::Long,
            Operand::AddrReg(AddrReg::A1),
            AddrReg::A2,
        )); // Compare A1 with A2
        self.emit(M68kInst::Bcc(Cond::Le, ".copy_done".to_string())); // If A2 <= A1, we're done (end reached)
        self.emit(M68kInst::Move(
            Size::Long,
            Operand::PostInc(AddrReg::A0),
            Operand::PostInc(AddrReg::A1),
        ));
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
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x8004),
            Operand::AddrInd(AddrReg::A1),
        )); // Reg 0
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x8104),
            Operand::AddrInd(AddrReg::A1),
        )); // Reg 1 - display OFF (vdp_init enables after VRAM clear)
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x8230),
            Operand::AddrInd(AddrReg::A1),
        )); // Reg 2
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x8407),
            Operand::AddrInd(AddrReg::A1),
        )); // Reg 4
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x8578),
            Operand::AddrInd(AddrReg::A1),
        )); // Reg 5
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x8700),
            Operand::AddrInd(AddrReg::A1),
        )); // Reg 7 - backdrop=0 (black)
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x8AFF),
            Operand::AddrInd(AddrReg::A1),
        )); // Reg 10
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x8B00),
            Operand::AddrInd(AddrReg::A1),
        )); // Reg 11
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x8C81),
            Operand::AddrInd(AddrReg::A1),
        )); // Reg 12
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x8D3F),
            Operand::AddrInd(AddrReg::A1),
        )); // Reg 13
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x8F02),
            Operand::AddrInd(AddrReg::A1),
        )); // Reg 15
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x9011),
            Operand::AddrInd(AddrReg::A1),
        )); // Reg 16: H64xV32

        // Clear all VRAM (64KB) from address 0 upward
        // Set VRAM write address to 0x0000
        // Command format: first word = 0x4000 | (addr & 0x3FFF), second word = (addr >> 14)
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x4000),
            Operand::AddrInd(AddrReg::A1),
        )); // VRAM write @ 0x0000
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x0000),
            Operand::AddrInd(AddrReg::A1),
        )); // Upper addr = 0
        // Clear 64KB (32K words)
        self.emit(M68kInst::Lea(Operand::AbsLong(0xC00000), AddrReg::A1));
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x7FFF),
            Operand::DataReg(DataReg::D0),
        ));
        self.emit(M68kInst::Label(".clear_vram".to_string()));
        // CLR performs a read-modify-write on the 68000; reading the VDP data
        // port while it is set up for writes locks the bus, so use MOVE #0.
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0),
            Operand::AddrInd(AddrReg::A1),
        ));
        self.emit(M68kInst::Dbcc(Cond::False, DataReg::D0, ".clear_vram".to_string()));

        // Re-setup A1 for palette write
        self.emit(M68kInst::Lea(Operand::AbsLong(0xC00004), AddrReg::A1));

        // Set up palette - CRAM write
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0xC000u32 as i32),
            Operand::AddrInd(AddrReg::A1),
        ));
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x0000),
            Operand::AddrInd(AddrReg::A1),
        ));
        // Write colors to VDP data port ($C00000)
        self.emit(M68kInst::Lea(Operand::AbsLong(0xC00000), AddrReg::A1));
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x0000),
            Operand::AddrInd(AddrReg::A1),
        )); // Black
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x0EEE),
            Operand::AddrInd(AddrReg::A1),
        )); // White
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x00E0),
            Operand::AddrInd(AddrReg::A1),
        )); // Green
        self.emit(M68kInst::Move(
            Size::Word,
            Operand::Imm(0x000E),
            Operand::AddrInd(AddrReg::A1),
        )); // Red

        // Silence all PSG channels
        // PSG is at $C00011, accessed via byte writes
        // Volume command: 1xx1 vvvv where xx=channel, vvvv=volume (F=silent)
        self.emit(M68kInst::Lea(Operand::AbsLong(0xC00011), AddrReg::A2));
        self.emit(M68kInst::Move(
            Size::Byte,
            Operand::Imm(0x9F),
            Operand::AddrInd(AddrReg::A2),
        )); // Ch 0 silent
        self.emit(M68kInst::Move(
            Size::Byte,
            Operand::Imm(0xBF),
            Operand::AddrInd(AddrReg::A2),
        )); // Ch 1 silent
        self.emit(M68kInst::Move(
            Size::Byte,
            Operand::Imm(0xDF),
            Operand::AddrInd(AddrReg::A2),
        )); // Ch 2 silent
        self.emit(M68kInst::Move(
            Size::Byte,
            Operand::Imm(0xFF),
            Operand::AddrInd(AddrReg::A2),
        )); // Ch 3 (noise) silent

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
            self.emit(M68kInst::Directive(format!(".long 0x{val:08X}")));
            i += 4;
        }

        // Emit 2-byte (word) values
        while i + 2 <= len {
            let val = u16::from_be_bytes([bytes[i], bytes[i + 1]]);
            self.emit(M68kInst::Directive(format!(".word 0x{val:04X}")));
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
        self.alloca_storage.clear();
        // Reset the block-local register cache (should already be empty between fns).
        self.reg_cache.clear();
        self.reg_cache_lru.clear();
        self.free_data_regs = CACHE_REGS.to_vec();

        // Count how many temps we need and how much alloca storage they reserve
        let mut max_temp = 0u32;
        let mut alloca_bytes = 0i16;
        for block in &func.blocks {
            for sinst in &block.insts {
                match &sinst.inst {
                    Inst::Copy { dst, .. }
                    | Inst::Unary { dst, .. }
                    | Inst::Binary { dst, .. }
                    | Inst::Load { dst, .. }
                    | Inst::LoadParam { dst, .. }
                    | Inst::AddrOf { dst, .. } => {
                        max_temp = max_temp.max(dst.0 + 1);
                    }
                    Inst::Alloca { dst, size, .. } => {
                        max_temp = max_temp.max(dst.0 + 1);
                        alloca_bytes += ((*size as i16) + 3) & !3;
                    }
                    Inst::Call { dst: Some(dst), .. }
                    | Inst::CallIndirect { dst: Some(dst), .. } => {
                        max_temp = max_temp.max(dst.0 + 1);
                    }
                    _ => {}
                }
            }
        }

        // Calculate frame size (temps + alloca storage + saved regs)
        self.frame_size = (max_temp as i16) * 4 + alloca_bytes + 16; // Extra space for saved regs
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
        let mut last_debug_line: usize = 0;
        for block in &func.blocks {
            // Block boundary: write back any cached registers BEFORE the label so
            // the stores execute on the fall-through path. This keeps the cache
            // strictly intra-block â€” blocks reached via jumps (whose runtime
            // registers don't hold this path's cached values) always load from
            // the authoritative stack slots.
            self.flush_cache();
            // Remaining-use counts for this block, consumed by the
            // register-targeted paths: a count reaching zero marks the
            // operand's last read, so its cache register is reusable.
            self.block_use_counts.clear();
            for si in &block.insts {
                note_reads(&si.inst, &mut self.block_use_counts);
            }
            self.emit(M68kInst::Label(block.label.0.clone()));
            for sinst in &block.insts {
                // Emit source line comment when the line changes
                if self.debug_enabled {
                    if let Some(span) = sinst.span {
                        let line =
                            crate::common::byte_offset_to_line(&self.debug_source, span.start);
                        if line != last_debug_line {
                            last_debug_line = line;
                            self.emit(M68kInst::Comment(format!(
                                "{}:{}",
                                self.debug_filename, line
                            )));
                        }
                    }
                }
                self.generate_inst(&sinst.inst)?;
            }
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

    /// Consume one read of temp `t`: true when this was its last use in the
    /// current block, so a register holding its value is freely recomputable.
    fn consume_use(&mut self, t: u32) -> bool {
        match self.block_use_counts.get_mut(&t) {
            Some(c) if *c > 0 => {
                *c -= 1;
                *c == 0
            }
            _ => false,
        }
    }

    /// Undo a speculative `consume_use` when the fast path declines.
    fn restore_use(&mut self, t: u32) {
        *self.block_use_counts.entry(t).or_insert(0) += 1;
    }

    /// Record `reg` as the cache location of a freshly defined `temp`
    /// (register-targeted paths), replacing any stale mapping.
    fn adopt_cache_reg(&mut self, temp: Temp, reg: DataReg) {
        self.get_temp_offset(temp);
        self.reg_cache.insert(temp.0, reg);
        self.cache_touch(temp.0);
    }

    /// Widen a sub-word load result in `reg` to a full 32-bit value:
    /// sign-extend for signed types, zero-extend for unsigned.
    fn emit_extend(&mut self, signed: bool, size: usize, reg: DataReg) {
        if signed {
            if size == 1 {
                self.emit(M68kInst::Ext(Size::Word, reg)); // byte -> word
                self.emit(M68kInst::Ext(Size::Long, reg)); // word -> long
            } else if size == 2 {
                self.emit(M68kInst::Ext(Size::Long, reg)); // word -> long
            }
        } else if size == 1 {
            self.emit(M68kInst::Andi(
                Size::Long,
                0xFF,
                Operand::DataReg(reg),
            ));
        } else if size == 2 {
            self.emit(M68kInst::Andi(
                Size::Long,
                0xFFFF,
                Operand::DataReg(reg),
            ));
        }
    }

    /// Source operand for a register-targeted ALU op: the value must be
    /// directly addressable (immediate, cached register, or home-slot
    /// displacement). Shift counts additionally accept only immediates in
    /// the encodable 1..=8 range or registers; XOR has no memory ea form.
    /// Alloca temps hold addresses (nothing writes their home slots), so
    /// they are excluded here â€” `load_value` gives them a `lea`.
    fn alu_source(&mut self, op: BinOp, v: &Value) -> Option<Operand> {
        let shift = matches!(op, BinOp::Shl | BinOp::Shr | BinOp::Sar);
        let xor = op == BinOp::Xor;
        match v {
            Value::IntConst(n) => {
                let n = *n as i32;
                if shift && !(1..=8).contains(&n) {
                    return None;
                }
                Some(Operand::Imm(n))
            }
            Value::Temp(t) => {
                if self.alloca_storage.contains_key(&t.0) {
                    return None;
                }
                if let Some(&r) = self.reg_cache.get(&t.0) {
                    return Some(Operand::DataReg(r));
                }
                if shift || xor {
                    None
                } else {
                    let off = self.get_temp_offset(*t);
                    Some(Operand::Disp(off, AddrReg::A6))
                }
            }
            Value::Name(_) | Value::StringConst(_) => None,
        }
    }

    /// Emit `dst = dst op src` directly into `dst` using the tightest
    /// available form (immediate variants where they exist).
    fn emit_alu(&mut self, op: BinOp, src: Operand, dst: DataReg) {
        let d = Operand::DataReg(dst);
        match (op, src) {
            (BinOp::Add, Operand::Imm(n)) => {
                self.emit(M68kInst::Addi(Size::Long, n, d));
            }
            (BinOp::Sub, Operand::Imm(n)) => {
                self.emit(M68kInst::Subi(Size::Long, n, d));
            }
            (BinOp::And, Operand::Imm(n)) => {
                self.emit(M68kInst::Andi(Size::Long, n, d));
            }
            (BinOp::Or, Operand::Imm(n)) => {
                self.emit(M68kInst::Ori(Size::Long, n, d));
            }
            (BinOp::Xor, Operand::Imm(n)) => {
                self.emit(M68kInst::Eori(Size::Long, n, d));
            }
            (BinOp::Xor, Operand::DataReg(r)) => {
                self.emit(M68kInst::Eor(Size::Long, r, d));
            }
            (BinOp::Shl, src) => self.emit(M68kInst::Lsl(Size::Long, src, dst)),
            (BinOp::Shr, src) => self.emit(M68kInst::Lsr(Size::Long, src, dst)),
            (BinOp::Sar, src) => self.emit(M68kInst::Asr(Size::Long, src, dst)),
            (op, src) => {
                let inst = match op {
                    BinOp::Add => M68kInst::Add(Size::Long, src, d),
                    BinOp::Sub => M68kInst::Sub(Size::Long, src, d),
                    BinOp::And => M68kInst::And(Size::Long, src, d),
                    BinOp::Or => M68kInst::Or(Size::Long, src, d),
                    _ => unreachable!("non-ALU op in emit_alu"),
                };
                self.emit(inst);
            }
        }
    }

    /// Register-targeted emission for the block-local ALU ops: compute
    /// straight into the dying left operand's cache register, or (for
    /// commutative ops) the dying right operand's, or into a fresh cache
    /// register for the destination. Returns false when the operands need
    /// the general D0/D1 path. Sound without global liveness: a register is
    /// only recomputed into when `block_use_counts` says the value in it is
    /// never read again in this block.
    fn try_reg_binary(
        &mut self,
        dst: Temp,
        op: BinOp,
        left: &Value,
        right: &Value,
    ) -> CompileResult<bool> {
        let arith = matches!(
            op,
            BinOp::Add | BinOp::Sub | BinOp::And | BinOp::Or | BinOp::Xor
        );
        let shift = matches!(op, BinOp::Shl | BinOp::Shr | BinOp::Sar);
        if !arith && !shift {
            return Ok(false); // helpers and comparison sequences keep their shape
        }

        // Case 1: the left operand dies here and sits in a cache register â€”
        // compute into it (`add.l <right>,rl` semantics: dst is second).
        if let Value::Temp(t) = left
            && let Some(&rl) = self.reg_cache.get(&t.0)
            && !self.alloca_storage.contains_key(&t.0)
            && self.consume_use(t.0)
        {
            match self.alu_source(op, right) {
                Some(src) => {
                    self.emit_alu(op, src, rl);
                    self.reg_cache.remove(&t.0);
                    self.reg_cache_lru.retain(|&x| x != t.0);
                    self.adopt_cache_reg(dst, rl);
                    return Ok(true);
                }
                None => self.restore_use(t.0),
            }
        }

        // Case 2 (commutative only): the right operand dies in a cache
        // register â€” compute into it with the left value as source.
        let commutative = matches!(op, BinOp::Add | BinOp::And | BinOp::Or | BinOp::Xor);
        if commutative
            && let Value::Temp(t) = right
            && let Some(&rr) = self.reg_cache.get(&t.0)
            && !self.alloca_storage.contains_key(&t.0)
            && self.consume_use(t.0)
        {
            match self.alu_source(op, left) {
                Some(src) => {
                    self.emit_alu(op, src, rr);
                    self.reg_cache.remove(&t.0);
                    self.reg_cache_lru.retain(|&x| x != t.0);
                    self.adopt_cache_reg(dst, rr);
                    return Ok(true);
                }
                None => self.restore_use(t.0),
            }
        }

        // Case 3: compute into a fresh cache register for the destination.
        let Some(src) = self.alu_source(op, right) else {
            return Ok(false);
        };
        let rl = self.alloc_cache_reg(dst.0);
        self.load_value(left, rl)?;
        self.emit_alu(op, src, rl);
        self.adopt_cache_reg(dst, rl);
        Ok(true)
    }

    /// Register-targeted NEG/BitNot: apply in place to a dying cached
    /// operand, or into a fresh cache register for the destination.
    fn try_reg_unary(&mut self, dst: Temp, op: UnOp, src: &Value) -> CompileResult<bool> {
        if !matches!(op, UnOp::Neg | UnOp::BitNot) {
            return Ok(false);
        }
        if let Value::Temp(t) = src
            && let Some(&rs) = self.reg_cache.get(&t.0)
            && !self.alloca_storage.contains_key(&t.0)
            && self.consume_use(t.0)
        {
            match op {
                UnOp::Neg => self.emit(M68kInst::Neg(Size::Long, Operand::DataReg(rs))),
                UnOp::BitNot => self.emit(M68kInst::Not(Size::Long, Operand::DataReg(rs))),
                _ => unreachable!(),
            }
            self.reg_cache.remove(&t.0);
            self.reg_cache_lru.retain(|&x| x != t.0);
            self.adopt_cache_reg(dst, rs);
            return Ok(true);
        }
        let rl = self.alloc_cache_reg(dst.0);
        self.load_value(src, rl)?;
        match op {
            UnOp::Neg => self.emit(M68kInst::Neg(Size::Long, Operand::DataReg(rl))),
            UnOp::BitNot => self.emit(M68kInst::Not(Size::Long, Operand::DataReg(rl))),
            _ => unreachable!(),
        }
        self.adopt_cache_reg(dst, rl);
        Ok(true)
    }

    // ---- Block-local register cache (D2-D7) helpers ----

    /// Write back every cached register to its home stack slot and clear the cache.
    /// Called at every block and call boundary so the stack is authoritative
    /// across basic blocks and across calls (matching the original stack model).
    fn flush_cache(&mut self) {
        // Iterate in LRU order (oldest first) for determinism.
        let lru = std::mem::take(&mut self.reg_cache_lru);
        for temp in &lru {
            if let Some(&reg) = self.reg_cache.get(temp) {
                if let Some(&offset) = self.temp_offsets.get(temp) {
                    self.emit(M68kInst::Move(
                        Size::Long,
                        Operand::DataReg(reg),
                        Operand::Disp(offset, AddrReg::A6),
                    ));
                }
            }
        }
        self.reg_cache.clear();
        // Restore canonical preference order so allocation is deterministic.
        self.free_data_regs = CACHE_REGS.to_vec();
    }

    /// Clear the cache WITHOUT writing values back. Used at `Return`: the frame
    /// is about to be torn down (and callee-saved registers restored by MOVEM),
    /// so writebacks would be dead stores â€” but the compile-time state must
    /// still be reset so later blocks aren't emitted against stale mappings.
    fn drop_cache(&mut self) {
        self.reg_cache.clear();
        self.reg_cache_lru.clear();
        self.free_data_regs = CACHE_REGS.to_vec();
    }

    /// Move `temp` to the most-recently-used position in the LRU list.
    fn cache_touch(&mut self, temp: u32) {
        self.reg_cache_lru.retain(|&t| t != temp);
        self.reg_cache_lru.push(temp);
    }

    /// Allocate a callee-saved data register for caching `temp`.
    /// Pops a free register if available; otherwise LRU-evicts the coldest
    /// cached temp (writing it back to its home slot) and reuses its register.
    fn alloc_cache_reg(&mut self, _temp: u32) -> DataReg {
        if let Some(reg) = self.free_data_regs.pop() {
            return reg;
        }
        // Evict the least-recently-used temp (front of the LRU list).
        let victim = self.reg_cache_lru.first().copied();
        if let Some(victim) = victim {
            if let Some(&vreg) = self.reg_cache.get(&victim) {
                if let Some(&offset) = self.temp_offsets.get(&victim) {
                    self.emit(M68kInst::Move(
                        Size::Long,
                        Operand::DataReg(vreg),
                        Operand::Disp(offset, AddrReg::A6),
                    ));
                }
                self.reg_cache.remove(&victim);
                self.reg_cache_lru.retain(|&t| t != victim);
                return vreg;
            }
        }
        // Unreachable: the cache is non-empty whenever free_data_regs is empty.
        DataReg::D7
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
                // An alloca temp used as a general value is the ADDRESS of a
                // local (e.g. `&local`, member/index arithmetic): materialize
                // the frame-relative address.
                if let Some(&storage) = self.alloca_storage.get(&t.0) {
                    self.emit(M68kInst::Lea(
                        Operand::Disp(storage, AddrReg::A6),
                        AddrReg::A0,
                    ));
                    self.emit(M68kInst::Move(
                        Size::Long,
                        Operand::AddrReg(AddrReg::A0),
                        Operand::DataReg(reg),
                    ));
                    return Ok(());
                }
                if let Some(&cached) = self.reg_cache.get(&t.0) {
                    // Cache hit: copy register-to-register instead of a stack load.
                    self.cache_touch(t.0);
                    self.emit(M68kInst::Move(
                        Size::Long,
                        Operand::DataReg(cached),
                        Operand::DataReg(reg),
                    ));
                } else {
                    let offset = self.get_temp_offset(*t);
                    self.emit(M68kInst::Move(
                        Size::Long,
                        Operand::Disp(offset, AddrReg::A6),
                        Operand::DataReg(reg),
                    ));
                }
            }
            Value::Name(name) => {
                // A symbol used as a value is its ADDRESS (function pointers,
                // label references), not the memory at the label.
                self.emit(M68kInst::Lea(Operand::Label(name.clone()), AddrReg::A0));
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::AddrReg(AddrReg::A0),
                    Operand::DataReg(reg),
                ));
            }
            Value::StringConst(label) => {
                self.emit(M68kInst::Lea(Operand::Label(label.0.clone()), AddrReg::A0));
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::AddrReg(AddrReg::A0),
                    Operand::DataReg(reg),
                ));
            }
        }
        Ok(())
    }

    /// Record a temp's new value. When a free cache register is available, the
    /// value is moved into it and the stack write is deferred until flush/eviction.
    /// Otherwise (cache full and caller used a scratch reg we can't reuse) we fall
    /// back to writing straight to the home slot.
    ///
    /// `reg` must be a scratch register (D0/D1), never a cache register: caching
    /// a value whose source is another temp's cache register would alias two
    /// temps to one register.
    fn store_temp(&mut self, temp: Temp, reg: DataReg) {
        debug_assert!(
            !CACHE_REGS.contains(&reg),
            "store_temp source must be a scratch register"
        );
        // Ensure the home slot exists (for later write-back on flush/eviction).
        self.get_temp_offset(temp);

        // If this temp is already cached, update the cached register in place.
        if let Some(&cached) = self.reg_cache.get(&temp.0) {
            if cached != reg {
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::DataReg(reg),
                    Operand::DataReg(cached),
                ));
            }
            self.cache_touch(temp.0);
            return;
        }

        // Try to park the result in a fresh cache register.
        let cache_reg = self.alloc_cache_reg(temp.0);
        if cache_reg != reg {
            self.emit(M68kInst::Move(
                Size::Long,
                Operand::DataReg(reg),
                Operand::DataReg(cache_reg),
            ));
        }
        self.reg_cache.insert(temp.0, cache_reg);
        self.reg_cache_lru.push(temp.0);
    }

    /// Call a runtime helper (operands in D0/D1, result in D0) and record
    /// that it must be emitted
    fn call_runtime(&mut self, helper: &'static str) {
        self.pending_runtime.insert(helper);
        self.emit(M68kInst::Jsr(Operand::Label(helper.to_string())));
    }

    /// Emit cheaper code for a multiply or unsigned divide/modulo by a
    /// constant, leaving the result in D0. Returns false (emitting nothing)
    /// when the operation has no constant special case.
    fn try_emit_const_binary(
        &mut self,
        op: BinOp,
        left: &Value,
        right: &Value,
    ) -> CompileResult<bool> {
        let as_i32 = |v: &Value| match v {
            Value::IntConst(n) => i32::try_from(*n).ok(),
            _ => None,
        };
        match op {
            BinOp::Mul => {
                let (value, factor) = if let Some(c) = as_i32(right) {
                    (left, c)
                } else if let Some(c) = as_i32(left) {
                    (right, c)
                } else {
                    return Ok(false);
                };
                self.load_value(value, DataReg::D0)?;
                self.emit_mul_const(factor);
                Ok(true)
            }
            // Unsigned division/modulo by a power of two is a shift/mask
            BinOp::UDiv | BinOp::UMod => {
                let Some(divisor) = as_i32(right)
                    .and_then(|c| u32::try_from(c).ok())
                    .filter(|c| c.is_power_of_two())
                else {
                    return Ok(false);
                };
                self.load_value(left, DataReg::D0)?;
                if op == BinOp::UDiv {
                    self.emit_shift_const(divisor.trailing_zeros(), false);
                } else {
                    self.emit(M68kInst::Andi(
                        Size::Long,
                        (divisor - 1) as i32,
                        Operand::DataReg(DataReg::D0),
                    ));
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// D0 = D0 * factor (mod 2^32), using D1 as scratch
    fn emit_mul_const(&mut self, factor: i32) {
        let magnitude = factor.unsigned_abs();
        if magnitude == 0 {
            self.emit(M68kInst::Moveq(0, DataReg::D0));
            return;
        }
        if magnitude.is_power_of_two() {
            self.emit_shift_const(magnitude.trailing_zeros(), true);
        } else if magnitude <= 0xFFFF {
            // x * m = xl*m + ((xh*m) << 16) for a 16-bit m
            let m = Operand::Imm(magnitude as i32);
            self.emit(M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D0),
                Operand::DataReg(DataReg::D1),
            ));
            self.emit(M68kInst::Swap(DataReg::D1));
            self.emit(M68kInst::Mulu(m.clone(), DataReg::D1));
            self.emit(M68kInst::Swap(DataReg::D1));
            self.emit(M68kInst::Clr(Size::Word, Operand::DataReg(DataReg::D1)));
            self.emit(M68kInst::Mulu(m, DataReg::D0));
            self.emit(M68kInst::Add(
                Size::Long,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D0),
            ));
        } else {
            self.emit(M68kInst::Move(
                Size::Long,
                Operand::Imm(factor),
                Operand::DataReg(DataReg::D1),
            ));
            self.call_runtime(runtime::MULSI3);
            return;
        }
        if factor < 0 {
            self.emit(M68kInst::Neg(Size::Long, Operand::DataReg(DataReg::D0)));
        }
    }

    /// Shift D0 left (or logically right) by a constant, using D1 as scratch
    /// for counts the immediate form can't encode
    fn emit_shift_const(&mut self, count: u32, left: bool) {
        if count == 0 {
            return;
        }
        let count_op = if count <= 8 {
            Operand::Imm(count as i32)
        } else {
            self.emit(M68kInst::Moveq(count as i8, DataReg::D1));
            Operand::DataReg(DataReg::D1)
        };
        self.emit(if left {
            M68kInst::Lsl(Size::Long, count_op, DataReg::D0)
        } else {
            M68kInst::Lsr(Size::Long, count_op, DataReg::D0)
        });
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
                if self.try_reg_unary(*dst, *op, src)? {
                    return Ok(());
                }
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

            Inst::Binary {
                dst,
                op,
                left,
                right,
            } => {
                if self.try_emit_const_binary(*op, left, right)? {
                    self.store_temp(*dst, DataReg::D0);
                    return Ok(());
                }

                if self.try_reg_binary(*dst, *op, left, right)? {
                    return Ok(());
                }

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
                    // The 68000 only multiplies 16x16 and divides 32/16, so
                    // full 32-bit operations call the runtime helpers
                    BinOp::Mul => self.call_runtime(runtime::MULSI3),
                    BinOp::Div => self.call_runtime(runtime::DIVSI3),
                    BinOp::UDiv => self.call_runtime(runtime::UDIVSI3),
                    BinOp::Mod | BinOp::UMod => {
                        let helper = if *op == BinOp::Mod {
                            runtime::DIVSI3
                        } else {
                            runtime::UDIVSI3
                        };
                        self.call_runtime(helper);
                        // Remainder comes back in D1
                        self.emit(M68kInst::Move(
                            Size::Long,
                            Operand::DataReg(DataReg::D1),
                            Operand::DataReg(DataReg::D0),
                        ));
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
                    BinOp::Sar => {
                        self.emit(M68kInst::Asr(
                            Size::Long,
                            Operand::DataReg(DataReg::D1),
                            DataReg::D0,
                        ));
                    }
                    BinOp::Eq
                    | BinOp::Ne
                    | BinOp::Lt
                    | BinOp::Le
                    | BinOp::Gt
                    | BinOp::Ge
                    | BinOp::ULt
                    | BinOp::ULe
                    | BinOp::UGt
                    | BinOp::UGe => {
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
                            BinOp::ULt => Cond::Cs,
                            BinOp::ULe => Cond::Ls,
                            BinOp::UGt => Cond::Hi,
                            BinOp::UGe => Cond::Cc,
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

            Inst::Load {
                dst,
                addr,
                size,
                volatile: _,
                signed,
            } => {
                // Note: volatile flag indicates this memory access should not be optimized.
                // For now, we emit the same code (no optimization pass yet).
                // Loads compute directly into a cache register for the
                // destination (no D0 round trip).
                // Alloca fast path: a load through a bare local-variable temp
                // reads the known frame slot directly.
                if let Value::Temp(t) = addr
                    && let Some(&storage) = self.alloca_storage.get(&t.0)
                {
                    let rl = self.alloc_cache_reg(dst.0);
                    let sz = Size::from_bytes(*size);
                    self.emit(M68kInst::Move(
                        sz,
                        Operand::Disp(storage, AddrReg::A6),
                        Operand::DataReg(rl),
                    ));
                    self.emit_extend(*signed, *size, rl);
                    self.adopt_cache_reg(*dst, rl);
                    return Ok(());
                }
                let rl = self.alloc_cache_reg(dst.0);
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
                    Operand::DataReg(rl),
                ));
                // Extend to 32-bit: sign-extend for signed types, zero-extend for unsigned
                // On 68000: ext.w extends byte->word, ext.l extends word->long (sign extension)
                // For unsigned, use AND to zero-extend
                self.emit_extend(*signed, *size, rl);
                self.adopt_cache_reg(*dst, rl);
            }

            Inst::Store {
                addr,
                src,
                size,
                volatile: _,
            } => {
                // Note: volatile flag indicates this memory access should not be optimized.
                // For now, we emit the same code (no optimization pass yet).
                // Alloca fast path: a store through a bare local-variable temp
                // writes the known frame slot directly.
                if let Value::Temp(t) = addr
                    && let Some(&storage) = self.alloca_storage.get(&t.0)
                {
                    self.load_value(src, DataReg::D1)?;
                    let sz = Size::from_bytes(*size);
                    self.emit(M68kInst::Move(
                        sz,
                        Operand::DataReg(DataReg::D1),
                        Operand::Disp(storage, AddrReg::A6),
                    ));
                    return Ok(());
                }
                // Load the source BEFORE moving the address into A0: loading a
                // Name/StringConst value uses A0 as LEA scratch and would
                // otherwise clobber the destination address.
                self.load_value(src, DataReg::D1)?;
                self.load_value(addr, DataReg::D0)?;
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::DataReg(DataReg::D0),
                    Operand::AddrReg(AddrReg::A0),
                ));
                let sz = Size::from_bytes(*size);
                self.emit(M68kInst::Move(
                    sz,
                    Operand::DataReg(DataReg::D1),
                    Operand::AddrInd(AddrReg::A0),
                ));
            }

            // Terminators flush BEFORE branching: the jump path exits here, so
            // this is its only chance to write cached values back for any
            // successor block that reads them. (Fall-through blocks without a
            // terminator are covered by the label flush in `generate_function`.)
            Inst::Jump(label) => {
                self.flush_cache();
                self.emit(M68kInst::Bra(label.0.clone()));
            }

            Inst::CondJump { cond, target } => {
                // Flush before the test: MOVE clobbers the Z/N flags on M68k,
                // so no stores may be emitted between TST and the branch.
                self.flush_cache();
                self.load_value(cond, DataReg::D0)?;
                self.emit(M68kInst::Tst(Size::Long, Operand::DataReg(DataReg::D0)));
                self.emit(M68kInst::Bcc(Cond::Ne, target.0.clone()));
            }

            Inst::CondJumpFalse { cond, target } => {
                self.flush_cache();
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
                            // Inline SDK calls clobber D0-D3 as argument regs.
                            self.flush_cache();
                            self.emit_sdk_inline_call(func, args, dst)?;
                        }
                        SdkFunctionKind::Library => {
                            // Mark function as needed, emit normal call
                            self.pending_sdk_functions.insert(func.clone());
                            self.flush_cache();
                            self.emit_standard_call(func, args, dst)?;
                        }
                    }
                } else {
                    // Runtime helper calls (decompiler intrinsics like
                    // __set_sr) keep their bodies pending emission.
                    if let Some(helper) = super::runtime::helper_static(func) {
                        self.pending_runtime.insert(helper);
                    }
                    // Regular user function call
                    self.flush_cache();
                    self.emit_standard_call(func, args, dst)?;
                }
            }

            Inst::CallIndirect { dst, target, args } => {
                self.flush_cache();
                // Push arguments right-to-left, same convention as direct calls
                for arg in args.iter().rev() {
                    self.load_value(arg, DataReg::D0)?;
                    self.emit(M68kInst::Move(
                        Size::Long,
                        Operand::DataReg(DataReg::D0),
                        Operand::PreDec(AddrReg::A7),
                    ));
                }

                // Call through the pointer value
                self.load_value(target, DataReg::D0)?;
                self.emit(M68kInst::Move(
                    Size::Long,
                    Operand::DataReg(DataReg::D0),
                    Operand::AddrReg(AddrReg::A0),
                ));
                self.emit(M68kInst::Jsr(Operand::AddrInd(AddrReg::A0)));

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

                if let Some(d) = dst {
                    self.store_temp(*d, DataReg::D0);
                }
            }

            Inst::Return(value) => {
                if let Some(val) = value {
                    self.load_value(val, DataReg::D0)?;
                }
                // The frame dies at RTS and MOVEM restores the callee-saved
                // registers, so writebacks would be dead stores â€” just drop
                // the compile-time cache state.
                self.drop_cache();

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
                // Reserve storage BELOW the already-assigned slots. next_offset
                // is the top of the next free 4-byte slot, so the free region
                // ends at next_offset + 3; the buffer base (lowest address) is
                // that end minus the aligned size.
                let aligned = ((*size as i16) + 3) & !3;
                let storage_offset = self.next_offset + 4 - aligned;
                self.next_offset -= aligned;
                // Record the storage offset instead of materializing a pointer
                // cell: Load/Store through this temp access the frame slot
                // directly, and `load_value` computes the address (lea) only
                // when it escapes into general value flow.
                self.alloca_storage.insert(dst.0, storage_offset);
            }

            Inst::AddrOf { dst, name } => {
                self.emit(M68kInst::Lea(Operand::Label(name.clone()), AddrReg::A0));
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
                    1 => 3, // byte at end of 4-byte slot
                    2 => 2, // word at end of 4-byte slot
                    _ => 0, // long uses full slot
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
        let inline_code = SdkInlineGenerator::generate(func)?;
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
    fn emit_sdk_library_functions(&mut self) -> CompileResult<()> {
        if self.pending_sdk_functions.is_empty() {
            return Ok(());
        }

        // Resolve all dependencies
        let all_functions = resolve_dependencies(&self.pending_sdk_functions);

        // Filter to only library functions
        let mut library_functions: Vec<_> = all_functions
            .iter()
            .filter(|f| {
                self.sdk_registry
                    .lookup(f)
                    .is_some_and(|sdk| sdk.kind == SdkFunctionKind::Library)
            })
            .cloned()
            .collect();

        // Sort for deterministic output
        library_functions.sort();

        // Generate each function
        let mut generator = SdkLibraryGenerator::new();
        for func_name in library_functions {
            self.emit(M68kInst::Comment(format!("SDK function: {func_name}")));
            let code = generator.generate(&func_name)?;
            for inst in code {
                self.emit(inst);
            }
        }

        Ok(())
    }

    /// Emit the runtime helpers that were called, with their dependencies
    fn emit_runtime_helpers(&mut self) {
        let mut helpers = self.pending_runtime.clone();
        for helper in &self.pending_runtime {
            helpers.extend(runtime::dependencies(helper));
        }
        for helper in helpers {
            self.emit(M68kInst::Comment(format!("Runtime helper: {helper}")));
            for inst in runtime::generate(helper) {
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

/// Adjacent-move peephole: the emitter's D0/D1 scratch convention produces
/// reg-to-reg copy chains (`move X,d0; move d0,Y`). Fold `move A,M` followed
/// immediately by `move M,B` into `move A,B` when every operand is
/// side-effect-free (plain registers, frame slots, immediates), neither
/// instruction touches A7 or a pre/post-increment addressing mode, and the
/// intermediate M is D0/D1 â€” the never-cached scratch registers. D2-D7 hold
/// block-cached temps whose values the emitter assumes persist, so removing
/// a write to them is not sound. Moves set N/Z, but merging adjacent moves
/// leaves exactly the final move's flags in place, so no flags consumer can
/// observe the difference.
/// Count each instruction's value reads, for last-use tracking: an entry
/// reaching zero in `counts` marks the temp's final read within the block.
fn note_reads(inst: &Inst, counts: &mut HashMap<u32, u32>) {
    fn note(v: &Value, counts: &mut HashMap<u32, u32>) {
        if let Value::Temp(t) = v {
            *counts.entry(t.0).or_insert(0) += 1;
        }
    }
    match inst {
        Inst::Copy { src, .. } | Inst::Unary { src, .. } => note(src, counts),
        Inst::Binary { left, right, .. } => {
            note(left, counts);
            note(right, counts);
        }
        Inst::Load { addr, .. } => note(addr, counts),
        Inst::Store { addr, src, .. } => {
            note(addr, counts);
            note(src, counts);
        }
        Inst::CondJump { cond, .. } | Inst::CondJumpFalse { cond, .. } => note(cond, counts),
        Inst::Call { args, .. } => {
            for a in args {
                note(a, counts);
            }
        }
        Inst::CallIndirect { target, args, .. } => {
            note(target, counts);
            for a in args {
                note(a, counts);
            }
        }
        Inst::Return(Some(v)) => note(v, counts),
        _ => {}
    }
}

fn peephole(insts: &mut Vec<M68kInst>) {
    let scratch = |r: &DataReg| matches!(r, DataReg::D0 | DataReg::D1);
    let foldable_src = |op: &Operand| {
        matches!(
            op,
            Operand::DataReg(_)
                | Operand::AddrReg(_)
                | Operand::Disp(_, AddrReg::A6)
                | Operand::Imm(_)
        )
    };
    let foldable_dst = |op: &Operand| {
        matches!(
            op,
            Operand::DataReg(_) | Operand::AddrReg(_) | Operand::Disp(_, AddrReg::A6)
        )
    };
    for _ in 0..4 {
        let mut changed = false;
        let mut out: Vec<M68kInst> = Vec::with_capacity(insts.len());
        let mut i = 0;
        while i < insts.len() {
            let pair = match (&insts[i], insts.get(i + 1)) {
                (
                    M68kInst::Move(s1, src, Operand::DataReg(mid)),
                    Some(M68kInst::Move(s2, Operand::DataReg(src2), dst)),
                ) if s1 == s2
                    && mid == src2
                    && scratch(mid)
                    && foldable_src(src)
                    && foldable_dst(dst) =>
                {
                    Some(M68kInst::Move(*s1, src.clone(), dst.clone()))
                }
                _ => None,
            };
            match pair {
                Some(merged) => {
                    out.push(merged);
                    i += 2;
                    changed = true;
                }
                None => {
                    out.push(insts[i].clone());
                    i += 1;
                }
            }
        }
        *insts = out;
        if !changed {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_module_emits_data_section_boundaries() {
        let instructions = CodeGenerator::new()
            .generate_instructions(&IrModule::new())
            .unwrap();

        for expected in ["__data_rom_start", "__data_ram_start", "__data_ram_end"] {
            assert!(
                instructions
                    .iter()
                    .any(|inst| matches!(inst, M68kInst::Label(label) if label == expected)),
                "missing required startup symbol {expected}"
            );
        }
    }
}
