//! Hybrid ROM rebuild: replace discovered code with compiled C, keep the
//! original image as the substrate.
//!
//! The original ROM stays byte-for-byte in place: its data, tables and even
//! its code provide everything the C does not (absolute data references,
//! undiscovered functions, checksum coverage). The compiled functions are
//! appended after the original image end — outside the original checksum
//! range, so the boot code's self-check still passes — and the exception
//! vectors are repointed at the compiled functions. Interrupt handlers get
//! a full register save/restore wrapper and return via `rte`, matching the
//! interrupt contract.

use super::decompile_rom;
use super::m68k::*;
use super::{Assembler, CodeGenerator};
use crate::common::DiagnosticReporter;
use crate::frontend::{CFrontend, CompileContext, Frontend, FrontendConfig};
use crate::ir::IrModule;
use std::collections::{HashMap, HashSet};

/// Result of a hybrid build.
pub struct HybridRom {
    pub image: Vec<u8>,
    /// The decompiled C source the functions were compiled from.
    pub source: String,
    /// Original address -> compiled address, for every replaced function.
    pub function_map: HashMap<u32, u32>,
}

/// Build a hybrid ROM from an original image.
pub fn build_hybrid_rom(rom: &[u8]) -> Result<HybridRom, String> {
    // 1. Decompile with auto names: functions are `F_%06X` of their original
    // address, the entry is `main`.
    let source = decompile_rom(rom, &HashMap::new(), None);

    // 2. Compile the C to IR.
    let module = compile_c(&source)?;

    // 3. Codegen functions only (no startup stub, no data section).
    let mut codegen = CodeGenerator::new();
    let mut insts = codegen
        .generate_functions_only(&module)
        .map_err(|e| format!("codegen failed: {e}"))?;

    // 4. Interrupt handlers: full register save and `rte` return.
    let entry = rom
        .get(0x4..0x8)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .unwrap_or(0);
    let mut handler_names: HashSet<String> = HashSet::new();
    for idx in 2..64 {
        // Skip idx 1 (offset 4): that is the reset entry PC, not an
        // exception vector — entering it does not push an SR/PC frame.
        let off = idx * 4;
        let Some(b) = rom.get(off..off + 4) else {
            break;
        };
        let v = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        if v >= 0x200 && v % 2 == 0 && (v as usize) < rom.len() {
            handler_names.insert(function_name(v, entry));
        }
    }
    let func_names: HashSet<String> = module.functions.iter().map(|f| f.name.clone()).collect();
    inject_handler_wrappers(&mut insts, &func_names, &handler_names);

    // 5. Assemble the compiled code right after the original image.
    let mut base = rom.len() as u32;
    base = (base + 1) & !1; // even
    let mut assembler = Assembler::new(base);
    let code = assembler
        .assemble(&insts)
        .map_err(|e| format!("assembly failed: {e}"))?;
    let symbols = assembler.symbols().clone();

    // 6. New image: original bytes + compiled code + power-of-two padding.
    let mut image = rom.to_vec();
    image.extend_from_slice(&code);
    let mut size = 0x10000usize; // Mega Drive ROMs are at least 64 KB
    while size < image.len() {
        size *= 2;
    }
    image.resize(size, 0xFF);

    // 7. Repoint every vector at its compiled function (original address
    // kept when there is no compiled counterpart).
    let mut function_map = HashMap::new();
    for idx in 1..64 {
        let off = idx * 4;
        let Some(b) = rom.get(off..off + 4) else {
            break;
        };
        let v = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        let name = function_name(v, entry);
        if let Some(&new) = symbols.get(&name) {
            image[off..off + 4].copy_from_slice(&new.to_be_bytes());
            function_map.insert(v, new);
        }
    }

    Ok(HybridRom {
        image,
        source,
        function_map,
    })
}

/// C name for the function that lived at `addr` (decompiler convention).
fn function_name(addr: u32, entry: u32) -> String {
    if addr == entry {
        "main".to_string()
    } else {
        format!("F_{addr:06X}")
    }
}

fn compile_c(source: &str) -> Result<IrModule, String> {
    let mut reporter = DiagnosticReporter::new();
    let file_id = reporter.add_file("decompiled.c", source);
    let context = CompileContext::new("decompiled.c".to_string(), file_id, &reporter);
    CFrontend::new()
        .compile(source, &context, &FrontendConfig::default())
        .map_err(|e| format!("decompiled C failed to compile: {e}"))
}

/// Wrap interrupt handlers with a full register save/restore and turn their
/// `rts` into `rte`, so an interrupt can land on any compiled code without
/// losing register state.
fn inject_handler_wrappers(
    insts: &mut Vec<M68kInst>,
    func_names: &HashSet<String>,
    handlers: &HashSet<String>,
) {
    let all_regs: Vec<Reg> = vec![
        Reg::Data(DataReg::D0),
        Reg::Data(DataReg::D1),
        Reg::Data(DataReg::D2),
        Reg::Data(DataReg::D3),
        Reg::Data(DataReg::D4),
        Reg::Data(DataReg::D5),
        Reg::Data(DataReg::D6),
        Reg::Data(DataReg::D7),
        Reg::Addr(AddrReg::A0),
        Reg::Addr(AddrReg::A1),
        Reg::Addr(AddrReg::A2),
        Reg::Addr(AddrReg::A3),
        Reg::Addr(AddrReg::A4),
        Reg::Addr(AddrReg::A5),
        Reg::Addr(AddrReg::A6),
    ];
    let full_save = M68kInst::Movem(Size::Long, all_regs.clone(), Operand::PreDec(AddrReg::A7), true);
    let full_restore = M68kInst::Movem(
        Size::Long,
        all_regs,
        Operand::PostInc(AddrReg::A7),
        false,
    );

    let mut out: Vec<M68kInst> = Vec::with_capacity(insts.len() + 32);
    let mut is_handler = false;
    let mut wrapped = false;
    for inst in insts.drain(..) {
        match &inst {
            M68kInst::Label(name) if func_names.contains(name) => {
                is_handler = handlers.contains(name);
                wrapped = false;
                out.push(inst);
            }
            M68kInst::Link(..) if is_handler && !wrapped => {
                out.push(inst);
                out.push(full_save.clone());
                wrapped = true;
            }
            M68kInst::Rts if wrapped => {
                // Full restore, then return from exception (SR + PC frame).
                out.push(full_restore.clone());
                out.push(M68kInst::Rte);
            }
            _ => out.push(inst),
        }
    }
    *insts = out;
}
