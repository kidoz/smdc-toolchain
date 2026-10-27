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

/// Build a hybrid ROM from an original image: the original image stays as
/// the substrate and discovered functions are replaced by compiled C.
pub fn build_hybrid_rom(rom: &[u8]) -> Result<HybridRom, String> {
    build(rom, false)
}

/// Build a data-only C rebuild: every flow-verified instruction span of the
/// original image is erased (0xFF), so the surviving original bytes are
/// data only — tables, tiles, strings, header — at their original
/// addresses. All executable code is compiled from the decompiled C. The
/// boot checksum is recomputed over the modified range and patched into the
/// header, so ROM self-checks in the boot code still pass.
pub fn build_data_only_rebuild(rom: &[u8]) -> Result<HybridRom, String> {
    let hybrid = build(rom, true)?;
    Ok(hybrid)
}

fn build(rom: &[u8], strip_code: bool) -> Result<HybridRom, String> {
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
    if strip_code {
        // Erase every flow-verified instruction span except bytes inside a
        // protected data window or a thunk island: what remains of the
        // original image is data only. Bytes the analysis could not prove
        // to be code (undiscovered regions) are kept — they may be data or
        // dead code, and erasing them could break data references.
        let (spans, mut protected) = super::decomp::code_footprint(rom);
        // Thunk calls (`((int (*)(void))0xNNNNNN)()`) target original code
        // that was merged away or never lifted; the block it runs through
        // must survive as original bytes.
        for t in thunk_targets(&source) {
            if let Some(block) = contiguous_block(&spans, t) {
                merge_range(&mut protected, block);
            }
        }
        let is_protected = |a: u32| protected.iter().any(|&(s, e)| a >= s && a < e);
        for (start, end) in spans {
            for a in start..end {
                if !is_protected(a) {
                    image[a as usize] = 0xFF;
                }
            }
        }
    }
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

    if strip_code {
        // Recompute the checksum over the range the boot code actually
        // verifies: words 0x200..=rom_end, where rom_end comes from the
        // header field at 0x1A4 (not the image length — appended compiled
        // code lives beyond it and must not change the sum).
        let rom_end = image
            .get(0x1A4..0x1A8)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize)
            .unwrap_or(image.len() - 1);
        let last = rom_end.min(image.len() - 1);
        let sum = image[0x200..=last]
            .chunks(2)
            .fold(0u16, |acc, c| {
                let w = if c.len() == 2 {
                    u16::from_be_bytes([c[0], c[1]])
                } else {
                    (c[0] as u16) << 8
                };
                acc.wrapping_add(w)
            });
        image[0x18E..0x190].copy_from_slice(&sum.to_be_bytes());
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

/// Addresses called through raw pointers in lifted C
/// (`((int (*)(void))0xNNNNNN)();`) — original code the decompiler did not
/// lift as its own function.
fn thunk_targets(source: &str) -> Vec<u32> {
    const MARKER: &str = "((int (*)(void))0x";
    let mut out = Vec::new();
    for (pos, _) in source.match_indices(MARKER) {
        let rest = &source[pos + MARKER.len()..];
        if let Some(end) = rest.find(")();")
            && let Ok(v) = u32::from_str_radix(rest[..end].trim(), 16)
        {
            out.push(v);
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The contiguous run of instruction spans starting at or covering `t`:
/// from `t` to the first gap in the walked instruction stream.
fn contiguous_block(spans: &[(u32, u32)], t: u32) -> Option<(u32, u32)> {
    let idx = spans.iter().position(|&(s, e)| s <= t && t < e)?;
    let mut end = spans[idx].1;
    for &(s, e) in &spans[idx + 1..] {
        if s != end {
            break;
        }
        end = e;
    }
    Some((t, end))
}

/// Merge a range into a sorted, disjoint protected list.
fn merge_range(ranges: &mut Vec<(u32, u32)>, r: (u32, u32)) {
    let mut merged = false;
    for e in ranges.iter_mut() {
        if e.1 >= r.0 && e.0 <= r.1 {
            e.0 = e.0.min(r.0);
            e.1 = e.1.max(r.1);
            merged = true;
            break;
        }
    }
    if !merged {
        ranges.push(r);
        ranges.sort();
    }
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
                // Save BEFORE linking (canonical handler prologue): pushed
                // after the link, the save block would sit inside the
                // compiled body's local-slot area (-4(a6), -12(a6), ...)
                // and the body's own temp stores would overwrite the saved
                // registers. Before the link it sits between the interrupt
                // frame and the link frame, out of the body's reach; the
                // epilogue's unlk then points sp straight at it.
                out.push(full_save.clone());
                out.push(inst);
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
