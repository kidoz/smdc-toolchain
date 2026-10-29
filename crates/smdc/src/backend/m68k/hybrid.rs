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
use super::disasm::decode_at;
use super::interp::{run_rom, RunConfig, RunTrace};
use super::m68k::*;
use super::{Assembler, CodeGenerator};
use crate::common::DiagnosticReporter;
use crate::frontend::{CFrontend, CompileContext, Frontend, FrontendConfig};
use crate::ir::IrModule;
use std::collections::{BTreeSet, HashMap, HashSet};

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
    build(rom, false, &[])
}

/// Build a data-only C rebuild: every flow-verified instruction span of the
/// original image is erased (0xFF), so the surviving original bytes are
/// data only — tables, tiles, strings, header — at their original
/// addresses. All executable code is compiled from the decompiled C. The
/// boot checksum is recomputed over the modified range and patched into the
/// header, so ROM self-checks in the boot code still pass.
pub fn build_data_only_rebuild(rom: &[u8]) -> Result<HybridRom, String> {
    let hybrid = build(rom, true, &[])?;
    Ok(hybrid)
}

/// Build a data-only rebuild with extra preserved byte ranges (used by the
/// interpreter-in-the-loop convergence below).
pub fn build_data_only_rebuild_seeded(
    rom: &[u8],
    extra_protected: &[(u32, u32)],
) -> Result<HybridRom, String> {
    build(rom, true, extra_protected)
}

/// Build a data-only rebuild that converges empirically: build, run a few
/// frames, and on each "undecodable instruction at $P" fault protect P's
/// original block and rebuild — static flow analysis cannot see computed
/// jumps, so the interpreter itself names the preserved code the program
/// still needs. Returns the image and the number of extension rounds used.
pub fn build_data_only_rebuild_converged(
    rom: &[u8],
    max_rounds: usize,
) -> Result<(HybridRom, usize), String> {
    let mut extra: Vec<(u32, u32)> = Vec::new();
    for round in 0..max_rounds {
        let hybrid = build(rom, true, &extra)?;
        let probe = run_rom(
            &hybrid.image,
            &RunConfig {
                frames: 3,
                frame_instructions: 7_600_000,
                input: Vec::new(),
            },
        );
        let Some(error) = probe.error else {
            return Ok((hybrid, round));
        };
        // The interpreter reports faults as `... at $XXXXXX (...)`; the
        // fault PC is preserved code the static walk could not see.
        let Some(pc) = error
            .split("$")
            .nth(1)
            .and_then(|rest| rest.split([' ', '(']).next())
            .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        else {
            return Err(error);
        };
        extra.push(block_from(rom, pc));
    }
    Err(format!(
        "data-only rebuild did not converge in {max_rounds} rounds"
    ))
}

fn build(rom: &[u8], strip_code: bool, extra_protected: &[(u32, u32)]) -> Result<HybridRom, String> {
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

    // Every lifted function's original -> compiled address, from the
    // assembler's symbol table. Dispatch tables in the ROM's data hold
    // original code addresses; the data-only rebuild must repoint them at
    // the compiled copies or dispatch jumps into erased bytes.
    let full_map: HashMap<u32, u32> = symbols
        .iter()
        .filter_map(|(name, &new)| {
            let orig = if name == "main" {
                Some(entry)
            } else {
                name.strip_prefix("F_")
                    .and_then(|hex| u32::from_str_radix(hex, 16).ok())
            };
            orig.map(|o| (o, new))
        })
        .collect();

    // 6. New image: original bytes + compiled code + power-of-two padding.
    let mut image = rom.to_vec();
    let mut relocations: Vec<(usize, u32)> = Vec::new();
    let mut dispatch_targets: Vec<u32> = Vec::new();
    if strip_code {
        // Erase every flow-verified instruction span except bytes inside a
        // protected data window or a thunk island: what remains of the
        // original image is data only. Bytes the analysis could not prove
        // to be code (undiscovered regions) are kept — they may be data or
        // dead code, and erasing them could break data references.
        let (spans, mut protected) = super::decomp::code_footprint(rom);
        for r in extra_protected {
            merge_range(&mut protected, *r);
        }
        // Thunk calls (`((int (*)(void))0xNNNNNN)()`) target original code
        // that was merged away or never lifted; the block it runs through
        // must survive as original bytes.
        for t in thunk_targets(&source) {
            if let Some(block) = contiguous_block(&spans, t) {
                merge_range(&mut protected, block);
            }
        }
        // Dispatch tables: every aligned long word in the original image
        // whose value is an even address in the code region is a candidate
        // function pointer. Pointers to LIFTED functions become relocations
        // (applied after the vectors are repointed); pointers to code that
        // was not lifted seed the preserved-flow walk below — a preserved
        // table still dispatches through its entry, and the target must
        // exist. Candidates inside spans (instruction immediates of
        // functions that are themselves replaced) are skipped: those bytes
        // are erased with their instruction.
        let span_covered = |a: u32| spans.iter().any(|&(s, e)| a >= s && a < e);
        let rom_len = rom.len() as u32;
        for off in (0x200usize..rom.len() - 3).step_by(4) {
            let v = u32::from_be_bytes([rom[off], rom[off + 1], rom[off + 2], rom[off + 3]]);
            if !((0x200..rom_len).contains(&v) && v % 2 == 0) {
                continue;
            }
            if let Some(&new) = full_map.get(&v) {
                if !span_covered(off as u32) {
                    relocations.push((off, new));
                }
            } else if !span_covered(v) && !protected.iter().any(|&(s, e)| v >= s && v < e) {
                dispatch_targets.push(v);
            }
        }
        // Preserved original code keeps executing, through both static
        // transfers AND `rts` unwinds: a preserved `jsr` pushes a return
        // address that compiled or original code later returns through.
        // Walk the full local flow from every preserved entry — thunk
        // targets and unlifted dispatch targets — and pull every reached
        // flow-verified (erased) span back into the preserved set until
        // the fixpoint.
        let mut work: Vec<u32> = Vec::new();
        for t in thunk_targets(&source) {
            work.push(t);
        }
        work.extend(&dispatch_targets);
        let mut walked: BTreeSet<u32> = BTreeSet::new();
        while let Some(seed) = work.pop() {
            if !walked.insert(seed) {
                continue;
            }
            let mut addr = seed;
            let mut guard = 0usize;
            while guard < 8192 {
                guard += 1;
                if addr < 0x200 || addr as usize >= rom.len() - 1 {
                    break;
                }
                if span_covered(addr) {
                    // Reached erased bytes: restore the whole original
                    // block and keep walking from it.
                    let block = contiguous_block(&spans, addr)
                        .unwrap_or_else(|| block_from(rom, addr));
                    merge_range(&mut protected, block);
                    if !walked.contains(&addr) {
                        work.push(addr);
                    }
                    break;
                }
                let Some((inst, next, target)) =
                    decode_at(rom, (addr - 0x200) as usize, 0x200, &HashMap::new())
                else {
                    break;
                };
                let next = 0x200 + next as u32;
                if let Some(t) = target {
                    if !walked.contains(&t) && (0x200..rom_len).contains(&t) {
                        work.push(t);
                    }
                }
                if matches!(inst, M68kInst::Bsr(_) | M68kInst::Jsr(_)) {
                    // The call pushes its return address; an `rts` later
                    // unwinds into it, so the return site is reachable too.
                    if !walked.contains(&next) && (0x200..rom_len).contains(&next) {
                        work.push(next);
                    }
                }
                let unconditional = matches!(
                    inst,
                    M68kInst::Bra(_)
                        | M68kInst::BraS(_)
                        | M68kInst::Jmp(_)
                        | M68kInst::Rts
                        | M68kInst::Rte
                        | M68kInst::Rtr
                );
                if unconditional {
                    break;
                }
                addr = next;
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
        // Apply dispatch-table relocations: preserved data words that held
        // a lifted function's original address now hold the compiled one.
        for (pos, new) in &relocations {
            image[*pos..*pos + 4].copy_from_slice(&new.to_be_bytes());
        }
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

    // 8. Private stack for the compiled code: repoint the initial SP
    // (vector 0) at the top of the largest RAM window the original
    // program never touches. The compiled frames are an order of
    // magnitude deeper than the original's stack budget, and the
    // original's data can start as little as ~1.5 KB below its initial
    // SP (Tetris keeps its VDP line buffer at $FFF7A0) — with the
    // original SP, the compiled call chain grows straight through the
    // program's globals and buffers. One shared stack keeps nested
    // interrupt frames consistent; vector 0 is outside the checksum
    // range, so the boot self-check is unaffected.
    let probe = run_rom(
        rom,
        &RunConfig {
            frames: 3,
            frame_instructions: 400_000,
            input: Vec::new(),
        },
    );
    let stack_top =
        private_stack_top(&probe).ok_or_else(|| "no free work-RAM window for the compiled stack".to_string())?;
    image[0..4].copy_from_slice(&stack_top.to_be_bytes());

    Ok(HybridRom {
        image,
        source,
        function_map,
    })
}

/// Top address for the compiled code's private stack: the top of the
/// largest run of RAM the original leaves untouched, strictly below the
/// original's deepest stack use (measured by a short probe run). The
/// compiled frames grow down from it through memory the program provably
/// does not use. The probe's nonzero set is a lower bound on the
/// program's data: bytes written zero during the probe are not seen.
fn private_stack_top(probe: &RunTrace) -> Option<u32> {
    const RAM_BASE: u32 = 0xFF0000;
    const REQUIRED: u32 = 0x4000; // compiled call-chain headroom
    const MARGIN: u32 = 0x40;

    // Nothing at or above the original's stack floor is safe: the
    // program's own call chain owns that memory.
    let limit = probe.stack_floor.min(RAM_BASE + probe.ram.len() as u32);
    let mut best: Option<(u32, u32)> = None; // (gap top address, gap length)
    let mut prev_nonzero: i64 = -1;
    for (i, &b) in probe.ram.iter().enumerate() {
        let addr = RAM_BASE + i as u32;
        if addr >= limit {
            break;
        }
        if b != 0 {
            let gap = i as i64 - prev_nonzero - 1;
            if gap as u32 >= REQUIRED + 2 * MARGIN
                && best.map_or(true, |(_, len)| (gap as u32) > len)
            {
                best = Some((addr, gap as u32));
            }
            prev_nonzero = i as i64;
        }
    }
    best.map(|(top, _)| top - MARGIN)
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
    let mut module = CFrontend::new()
        .compile(source, &context, &FrontendConfig::default())
        .map_err(|e| format!("decompiled C failed to compile: {e}"))?;
    crate::ir::optimize_module(&mut module);
    Ok(module)
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

/// Decode a local code block starting at `addr` regardless of the global
/// walk's classification: from `addr` forward until an unconditional
/// terminator or an undecodable word, capped at 256 instructions. Used to
/// island-protect dispatch-table targets the walker classified as data.
fn block_from(rom: &[u8], start: u32) -> (u32, u32) {
    let code = &rom[0x200..];
    let mut addr = start;
    let mut guard = 0;
    while guard < 256 {
        let off = match start.checked_sub(0x200).map(|_| (addr - 0x200) as usize) {
            Some(off) if off + 1 < code.len() => off,
            _ => break,
        };
        match decode_at(code, off, 0x200, &HashMap::new()) {
            Some((inst, next, _)) => {
                let done = matches!(
                    inst,
                    M68kInst::Rts
                        | M68kInst::Rte
                        | M68kInst::Rtr
                        | M68kInst::Bra(_)
                        | M68kInst::BraS(_)
                        | M68kInst::Jmp(_)
                );
                addr = 0x200 + next as u32;
                guard += 1;
                if done {
                    break;
                }
            }
            None => break,
        }
    }
    (start, addr)
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
