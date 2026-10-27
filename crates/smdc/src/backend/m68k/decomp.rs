//! ROM decompiler
//!
//! Lifts M68k machine code back to C-like pseudo-source. Builds on the
//! disassembler's decoder (`decode_at`) and adds:
//! - control-flow analysis from the ROM vector table (functions, blocks),
//! - a pattern lifter that maps instructions to C statements (smdc's cdecl
//!   `LINK A6` frames become named parameters and locals),
//! - opportunistic structuring of `if/else` and DBF loops; anything the
//!   structurer declines is emitted as valid goto-form C.
//!
//! Output is pseudo-C: register and frame slots appear as variables, and
//! hardware accesses appear as typed pointer dereferences. It is meant for
//! reading and diffing, not necessarily for re-compiling foreign ROMs.

use super::disasm::decode_at;
use super::m68k::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;

const CODE_BASE: u32 = 0x200;

/// One decoded instruction with its static control-transfer target and the
/// address of the next instruction in memory order.
struct Decoded {
    inst: M68kInst,
    target: Option<u32>,
    next: u32,
}

/// Decompile a Genesis ROM image to C-like pseudo-source.
///
/// `symbols` maps label name -> address (e.g. from a `.sym` file or the
/// in-process assembler). `data_start` is the ROM offset where inline data
/// begins, when known; decoding stops there.
pub fn decompile_rom(
    rom: &[u8],
    symbols: &HashMap<String, u32>,
    data_start: Option<u32>,
) -> String {
    Decompiler::new(rom, symbols, data_start).run()
}

/// Flow-verified instruction spans (address, end) of a ROM, plus protected
/// data ranges: windows after every absolute reference walked code makes
/// into the image. Over-reads (tables whose loops run into a neighbouring
/// function's prologue) consume those bytes as data, so the data-only
/// rebuild must preserve them.
pub fn code_footprint(rom: &[u8]) -> (Vec<(u32, u32)>, Vec<(u32, u32)>) {
    let mut decomp = Decompiler::new(rom, &HashMap::new(), None);
    decomp.analyze();
    decomp.merge_cross_jumping_functions();
    let code = rom.get(CODE_BASE as usize..).unwrap_or(&[]);
    let mut spans: Vec<(u32, u32)> = Vec::new();
    for &addr in decomp.insts.keys() {
        let off = (addr - CODE_BASE) as usize;
        if let Some((_, next, _)) = decode_at(code, off, CODE_BASE, &HashMap::new()) {
            spans.push((addr, CODE_BASE + next as u32));
        }
    }
    spans.sort();

    // Absolute references from walked code protect a window after the
    // referenced address (tables, and whatever a loop over them touches).
    let mut refs: Vec<u32> = Vec::new();
    for d in decomp.insts.values() {
        let mut visit = |op: &Operand| match op {
            Operand::AbsLong(a) => refs.push(a & 0x00FF_FFFF),
            Operand::AbsShort(a) => {
                let a = (*a as i16 as i32 as u32) & 0x00FF_FFFF;
                refs.push(a);
            }
            _ => {}
        };
        for_each_operand(&d.inst, &mut visit);
    }
    refs.retain(|a| *a >= CODE_BASE && (*a as usize) < rom.len());
    refs.sort();
    refs.dedup();
    let mut protected: Vec<(u32, u32)> = Vec::new();
    for r in refs {
        let end = (r + 64).min(rom.len() as u32);
        match protected.last_mut() {
            Some(last) if last.1 >= r => last.1 = last.1.max(end),
            _ => protected.push((r, end)),
        }
    }
    (spans, protected)
}

struct Decompiler<'a> {
    rom: &'a [u8],
    /// Initial PC from the vector table: named `main` in the output so the
    /// rebuilt ROM's startup stub can call it.
    entry: Option<u32>,
    /// Code bytes: `rom[0x200..]`.
    code: &'a [u8],
    /// Decoded instructions by address.
    insts: BTreeMap<u32, Decoded>,
    /// Function start addresses (entry points + call targets).
    func_starts: BTreeSet<u32>,
    /// Block label addresses (branch targets).
    labels: BTreeSet<u32>,
    /// Address -> user-visible symbol name (functions and data).
    names: HashMap<u32, String>,
    data_start: Option<u32>,
    /// RAM globals (work-RAM absolute addresses) with their access size.
    /// `None` marks an address accessed at conflicting sizes (rendered raw).
    ram_sizes: BTreeMap<u32, Option<Size>>,
    /// Hardware registers referenced by the code (address set).
    hw_used: BTreeSet<u32>,
    /// Function start -> register argument names (foreign register
    /// conventions), in first-read order. Functions with an A6 frame use
    /// frame parameters instead.
    func_args: BTreeMap<u32, Vec<String>>,
    /// Function start -> argument registers the body also WRITES: the 68k
    /// register file is shared, so a callee's mutation is visible to its
    /// caller. Rendered as pointer out-parameters with a write-back before
    /// every return.
    func_arg_out: BTreeMap<u32, BTreeSet<String>>,
    /// Functions whose return value in d0 is live at some `rts`.
    func_ret: BTreeSet<u32>,
    /// Functions entered from the exception vector table (interrupt
    /// handlers and the reset entry): never modeled with arguments.
    hw_vector_funcs: BTreeSet<u32>,
    /// Resolved PC-relative jump tables: dispatch instruction address ->
    /// (index register, entry stride in bytes, case-value offset, is_jsr,
    /// targets).
    jump_tables: BTreeMap<u32, (Reg, u32, u32, bool, Vec<u32>)>,
}

impl<'a> Decompiler<'a> {
    fn new(rom: &'a [u8], symbols: &HashMap<String, u32>, data_start: Option<u32>) -> Self {
        // Reverse the symbol map, demoting compiler-internal names.
        // `_start` is also demoted: the recompiler emits its own startup
        // stub with that label, so keeping it would collide on rebuild.
        let mut names = HashMap::new();
        for (name, &addr) in symbols {
            if name.starts_with(".L") || name.starts_with("__") || name == "_start" {
                continue;
            }
            names.entry(addr).or_insert_with(|| name.clone());
        }
        let entry = rom
            .get(0x4..0x8)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .filter(|&v| v >= CODE_BASE);
        Decompiler {
            rom,
            entry,
            code: rom.get(CODE_BASE as usize..).unwrap_or(&[]),
            insts: BTreeMap::new(),
            func_starts: BTreeSet::new(),
            labels: BTreeSet::new(),
            names,
            data_start,
            ram_sizes: BTreeMap::new(),
            hw_used: BTreeSet::new(),
            func_args: BTreeMap::new(),
            func_arg_out: BTreeMap::new(),
            func_ret: BTreeSet::new(),
            hw_vector_funcs: BTreeSet::new(),
            jump_tables: BTreeMap::new(),
        }
    }

    fn run(&mut self) -> String {
        let mut out = String::from("/* Decompiled by smdc --decompile (pseudo-C) */\n");
        out.push_str("\n/* status-register intrinsics (provided by the runtime) */\n");
        out.push_str("void __set_sr(int value);\n");
        out.push_str("int __get_sr(void);\n");
        self.analyze();
        self.merge_cross_jumping_functions();
        self.scan_globals();
        self.scan_signatures();
        self.emit_defines(&mut out);
        self.emit_functions(&mut out);
        self.emit_data(&mut out);
        out
    }

    /// C labels are function-scoped, so a branch into a neighbouring
    /// function's extent means the call-target partition split one logical
    /// function. Merge such functions: drop the start of the function whose
    /// range contains the foreign target.
    fn merge_cross_jumping_functions(&mut self) {
        // Functions that are actual call targets must survive merging: the
        // lifter emits them as real C functions (with their register-arg
        // signatures), and calls compile consistently. A merged-away call
        // target would be invoked as a raw-address thunk instead, whose
        // foreign register convention corrupts compiled state.
        let call_targets: BTreeSet<u32> = self
            .insts
            .values()
            .filter(|d| is_call(&d.inst))
            .filter_map(|d| d.target)
            .filter(|&t| self.func_starts.contains(&t))
            .collect();
        loop {
            let starts: Vec<u32> = self.func_starts.iter().copied().collect();
            let mut removed = None;
            'outer: for (w, &a1) in starts.iter().enumerate() {
                let a2 = starts.get(w + 1).copied().unwrap_or(u32::MAX);
                for (&addr, d) in self.insts.range(a1..a2.min(u32::MAX)) {
                    let _ = addr;
                    // Calls (jsr/bsr) legitimately target other functions.
                    if is_call(&d.inst) {
                        continue;
                    }
                    let Some(t) = d.target else { continue };
                    if t == a1 || (a1..a2).contains(&t) {
                        continue; // internal target
                    }
                    // Forward into a later function: merge that one in.
                    if t >= a2 {
                        if let Some(&owner) = self.func_starts.range(a2..=t).next() {
                            if call_targets.contains(&owner) {
                                continue; // keep called functions intact
                            }
                            removed = Some(owner);
                            break 'outer;
                        }
                    } else if t < a1 {
                        // Backward into an earlier function: merge self in.
                        if call_targets.contains(&a1) {
                            continue; // keep called functions intact
                        }
                        removed = Some(a1);
                        break 'outer;
                    }
                }
            }
            match removed {
                Some(f) => {
                    self.func_starts.remove(&f);
                }
                None => break,
            }
        }
    }

    // -- analysis -----------------------------------------------------------

    /// Walk control flow from the vector-table entries, decoding instructions
    /// and collecting function starts and block labels.
    fn analyze(&mut self) {
        let entries = self.entry_points();
        let mut work: Vec<u32> = entries.iter().copied().collect();
        self.func_starts = entries.iter().copied().collect();
        self.hw_vector_funcs = entries;
        let mut visited: HashSet<u32> = HashSet::new();

        while let Some(addr) = work.pop() {
            let mut addr = addr;
            loop {
                if visited.contains(&addr) || !self.in_code(addr) {
                    break;
                }
                let off = (addr - CODE_BASE) as usize;
                let Some((inst, next_off, target)) =
                    decode_at(self.code, off, CODE_BASE, &HashMap::new())
                else {
                    if std::env::var("SMDC_DEBUG_SPANS").is_ok() {
                        eprintln!("walk stop: decode failed at {addr:06X}");
                    }
                    break;
                };
                visited.insert(addr);
                let next = CODE_BASE + next_off as u32;
                self.insts.insert(
                    addr,
                    Decoded {
                        inst,
                        target,
                        next,
                    },
                );

                match target {
                    Some(t) if is_unconditional(&self.insts[&addr].inst) => {
                        self.labels.insert(t);
                        work.push(t);
                        break;
                    }
                    Some(t) if is_call(&self.insts[&addr].inst) => {
                        self.func_starts.insert(t);
                        work.push(t);
                    }
                    Some(t) => {
                        self.labels.insert(t);
                        work.push(t);
                    }
                    None
                        if matches!(
                            self.insts[&addr].inst,
                            M68kInst::Rts
                                | M68kInst::Rte
                                | M68kInst::Rtr
                                | M68kInst::Bra(_)
                                | M68kInst::BraS(_)
                        ) =>
                    {
                        break;
                    }
                    _ => {}
                }

                // Computed dispatch through a PC-relative jump table:
                // `jsr/jmp table(pc,dN)`. The instruction address gives the
                // table base (opword + extension, then +2). Resolve entries
                // so the lifter can render a real switch.
                if target.is_none() {
                    let dispatch = match &self.insts[&addr].inst {
                        M68kInst::Jsr(Operand::PcIndexed(d, i, _))
                        | M68kInst::Jmp(Operand::PcIndexed(d, i, _)) => {
                            let is_jsr =
                                matches!(self.insts[&addr].inst, M68kInst::Jsr(_));
                            Some((*i, *d, is_jsr))
                        }
                        _ => None,
                    };
                    if let Some((idx, disp, is_jsr)) = dispatch {
                        // Brief-format d(PC,Xn) references the extension
                        // word's own address (no +2), matching this ROM's
                        // assembler: base = ext_word_addr + disp.
                        let base = addr.wrapping_add(2).wrapping_add_signed(disp as i32);
                        let (stride, case_offset, entries) = self.resolve_jump_table(base);
                        if std::env::var("SMDC_DEBUG_SPANS").is_ok() {
                            eprintln!(
                                "dispatch {addr:06X} base {base:06X} stride {stride} off {case_offset} entries {}",
                                entries.len()
                            );
                        }
                        if !entries.is_empty() {
                            self.jump_tables.insert(
                                addr,
                                (idx, stride, case_offset, is_jsr, entries.clone()),
                            );
                            self.labels.insert(base);
                            for t in entries {
                                if is_jsr {
                                    self.func_starts.insert(t);
                                } else {
                                    self.labels.insert(t);
                                }
                                work.push(t);
                            }
                        }
                    }
                }
                addr = CODE_BASE + next_off as u32;
            }
        }
        for &f in &self.func_starts {
            self.labels.insert(f);
        }
    }

    /// Resolve a jump table at `base`: (entry stride, case-value offset,
    /// targets). The offset is added to `i * stride` to get the switch case
    /// value: it accounts for non-bra words the trampoline scan skipped
    /// before the first entry (the EA base itself may point at padding or
    /// an `rte` no-event slot). Tries long-absolute entries first, then
    /// `bra` trampolines, then word offsets relative to `base`.
    fn resolve_jump_table(&self, base: u32) -> (u32, u32, Vec<u32>) {
        let decodes = |t: u32| -> bool {
            (t as usize) < self.rom.len()
                && decode_at(
                    self.code,
                    (t - CODE_BASE) as usize,
                    CODE_BASE,
                    &HashMap::new(),
                )
                .is_some()
        };
        let mut targets = Vec::new();
        let mut off = base;
        // Long-absolute entries.
        loop {
            let Some(b) = self.rom.get(off as usize..off as usize + 4) else {
                break;
            };
            let v = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
            if v == 0 || !self.in_code(v) || v % 2 != 0 || !decodes(v) {
                break;
            }
            targets.push(v);
            off += 4;
            if targets.len() >= 256 {
                break;
            }
        }
        if targets.len() >= 2 {
            return (4, 0, targets);
        }
        // to the handler (`jsr table(pc,dN)` with the index stepped by 4).
        // Tables may prepend non-bra words (padding or an `rte` no-event
        // default); scan forward from the base for the first bra run.
        targets.clear();
        let mut first_at = None;
        for skip in 0..4 {
            let probe = base + skip * 2;
            if self.read_rom_word(probe).is_some_and(|w| (w & 0xFF00) == 0x6000)
                && self
                    .read_rom_word(probe + 4)
                    .is_some_and(|w| (w & 0xFF00) == 0x6000)
            {
                first_at = Some(probe);
                break;
            }
        }
        let Some(table) = first_at else {
            return (4, 0, targets);
        };
        // The dispatch's computed EA is relative to `base`, so the entry
        // found at `table` answers to the index (table - base), not 0.
        let case_offset = table - base;
        let mut off = table;
        loop {
            let Some(b) = self.rom.get(off as usize..off as usize + 4) else {
                break;
            };
            let op = u16::from_be_bytes([b[0], b[1]]);
            if (op & 0xFF00) != 0x6000 {
                break;
            }
            // bra.s carries the displacement in the opword; bra.w follows
            // it as a word (both are 4-byte slots in a trampoline table).
            let disp = if op & 0xFF == 0 {
                u16::from_be_bytes([b[2], b[3]]) as i16 as i32
            } else {
                op as u8 as i8 as i32
            };
            let target = (off + 2).wrapping_add_signed(disp);
            if disp == 0 || !self.in_code(target) || !decodes(target) {
                break;
            }
            targets.push(target);
            off += 4;
            if targets.len() >= 256 {
                break;
            }
        }
        if targets.len() >= 2 {
            return (4, case_offset, targets);
        }
        // Word offsets relative to the table base.
        targets.clear();
        let mut off = base;
        loop {
            let Some(w) = self.read_rom_word(off) else {
                break;
            };
            let target = base.wrapping_add_signed(w as i16 as i32);
            // Offsets are non-negative and must land inside the code section.
            if (w as i16) < 0 || !self.in_code(target) || target < base || !decodes(target) {
                break;
            }
            targets.push(target);
            off += 2;
            if targets.len() >= 256 {
                break;
            }
        }
        (2, 0, targets)
    }

    fn read_rom_word(&self, addr: u32) -> Option<u16> {
        let off = addr.checked_sub(CODE_BASE)? as usize;
        let b = self.code.get(off..off + 2)?;
        Some(u16::from_be_bytes([b[0], b[1]]))
    }

    // -- global naming -------------------------------------------------------

    /// Collect absolute work-RAM addresses and the hardware registers the
    /// code touches, so they can be rendered as named globals.
    fn scan_globals(&mut self) {
        let insts: Vec<M68kInst> = self.insts.values().map(|d| d.inst.clone()).collect();
        for inst in &insts {
            for (op, size) in operand_sizes(inst) {
                if let Some(a24) = abs_address(op) {
                    if is_hw_register(a24) {
                        self.hw_used.insert(a24);
                    } else if (RAM_BASE..=0xFFFFFF).contains(&a24) {
                        let entry = self.ram_sizes.entry(a24);
                        match entry {
                            std::collections::btree_map::Entry::Occupied(mut e) => {
                                if *e.get() != Some(size) {
                                    *e.get_mut() = None; // conflicting sizes
                                }
                            }
                            std::collections::btree_map::Entry::Vacant(v) => {
                                v.insert(Some(size));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Name for an absolute address access at the given size, if we have a
    /// stable one. Hardware registers must be accessed at their documented
    /// size; RAM globals at their (consistent) observed size.
    fn named_addr(&self, a24: u32, size: Size) -> Option<String> {
        if let Some((_, name, sz)) = HW_REGISTERS.iter().find(|(a, _, _)| *a == a24) {
            return (*sz == size).then(|| name.to_string());
        }
        if let Some(Some(sz)) = self.ram_sizes.get(&a24) {
            if *sz == size {
                return Some(format!("ram_{a24:06X}"));
            }
        }
        None
    }

    /// Emit the `#define` block for named hardware registers and RAM globals.
    fn emit_defines(&self, out: &mut String) {
        let mut defs: Vec<String> = Vec::new();
        let mut hw: Vec<_> = self.hw_used.iter().copied().collect();
        hw.sort();
        for a in hw {
            if let Some((_, name, sz)) = HW_REGISTERS.iter().find(|(addr, _, _)| *addr == a) {
                let t = match sz {
                    Size::Byte => "volatile unsigned char",
                    Size::Word => "volatile unsigned short",
                    Size::Long => "volatile unsigned int",
                };
                defs.push(format!("#define {name} (*({t} *)0x{a:06X})\n"));
            }
        }
        let mut ram: Vec<_> = self.ram_sizes.iter().collect();
        ram.sort_by_key(|(a, _)| **a);
        for (&a, sz) in ram {
            if let Some(sz) = sz {
                let t = match sz {
                    Size::Byte => "unsigned char",
                    Size::Word => "short",
                    Size::Long => "int",
                };
                defs.push(format!("#define ram_{a:06X} (*({t} *)0x{a:06X})\n"));
            }
        }
        if !defs.is_empty() {
            out.push_str("\n/* hardware registers and RAM globals */\n");
            for d in defs {
                out.push_str(&d);
            }
            out.push('\n');
        }
    }

    // -- calling-convention reconstruction ------------------------------------

    /// Per-function register arguments (read-before-write registers) and
    /// return-value liveness, computed once so call sites can use them even
    /// for forward references.
    fn scan_signatures(&mut self) {
        let starts: Vec<u32> = self.func_starts.iter().copied().collect();
        let mut func_args = BTreeMap::new();
        let mut func_arg_out: BTreeMap<u32, BTreeSet<String>> = BTreeMap::new();
        let mut func_ret = BTreeSet::new();
        for (i, &start) in starts.iter().enumerate() {
            let end = starts
                .get(i + 1)
                .copied()
                .unwrap_or_else(|| self.insts.keys().next_back().map_or(start, |k| k + 1));
            let mut insts: Vec<&M68kInst> =
                self.insts.range(start..end).map(|(_, d)| &d.inst).collect();
            if insts.is_empty() {
                continue;
            }            // Return value: d0 written at any point before an `rts`.
            let mut d0_written = false;
            for inst in &insts {
                let (_, writes) = reg_rw(inst);
                if writes.contains(&Reg::Data(DataReg::D0)) {
                    d0_written = true;
                }
                if matches!(inst, M68kInst::Rts) {
                    if d0_written {
                        func_ret.insert(start);
                    }
                    d0_written = false;
                }
            }
            // Register arguments: read before written. Functions with an
            // A6 frame (smdc cdecl) use frame parameters instead; interrupt
            // handlers and the reset entry are called by hardware; a
            // leading register save (`movem` to memory) preserves registers
            // rather than receiving them.
            let has_link = insts
                .iter()
                .any(|i| matches!(i, M68kInst::Link(AddrReg::A6, _)));
            if has_link || self.hw_vector_funcs.contains(&start) {
                continue;
            }
            let mut read_order: Vec<String> = Vec::new();
            let mut written: Vec<Reg> = Vec::new();
            let mut arg_writes: BTreeSet<String> = BTreeSet::new();
            let mut in_prologue = true;
            for inst in &insts {
                // `link a6` writes the frame pointer; `unlk` reads it.
                if let M68kInst::Link(an, _) = inst {
                    written.push(Reg::Addr(*an));
                    continue;
                }
                if in_prologue {
                    if let M68kInst::Movem(_, _, _, true) = inst {
                        // A leading register save stores the incoming
                        // values; it neither defines nor uses them. Skip
                        // it: a genuine later read (e.g. `movem a0..;
                        // move.l a0, a1`) still counts as an argument.
                        continue;
                    }
                    if matches!(inst, M68kInst::Nop) {
                        continue;
                    }
                    in_prologue = false;
                }
                let (reads, writes) = reg_rw(inst);
                for r in reads {
                    if !written.contains(&r) && r != Reg::Addr(AddrReg::A7) {
                        let name = format!("{r}");
                        if !read_order.contains(&name) {
                            read_order.push(name);
                        }
                    }
                }
                written.extend(writes);
            }
            // Argument registers the body writes: the shared 68k register
            // file exposes the mutation to the caller. A write before the
            // argument's first read would have excluded it from the
            // argument list, so any write here is a genuine mutation.
            for r in &written {
                let name = format!("{r}");
                if read_order.contains(&name) {
                    arg_writes.insert(name);
                }
            }
            if !read_order.is_empty() {
                func_args.insert(start, read_order);
                if !arg_writes.is_empty() {
                    func_arg_out.insert(start, arg_writes);
                }
            }
        }
        self.func_args = func_args;
        self.func_arg_out = func_arg_out;
        self.func_ret = func_ret;
    }

    /// Entry points from the vector table: every populated vector that
    /// points into the code section (foreign ROMs often park a generic
    /// handler on unused vectors), deduplicated.
    fn entry_points(&self) -> BTreeSet<u32> {
        let mut out = BTreeSet::new();
        for idx in 1..64 {
            let off = idx * 4;
            let Some(b) = self.rom.get(off..off + 4) else {
                break;
            };
            let v = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
            if v >= CODE_BASE && v % 2 == 0 && self.in_code(v) {
                out.insert(v);
            }
        }
        out
    }

    fn in_code(&self, addr: u32) -> bool {
        addr >= CODE_BASE
            && addr % 2 == 0
            && (addr - CODE_BASE) as usize + 2 <= self.code.len()
            && self.data_start.is_none_or(|d| addr < d)
    }

    // -- emission -----------------------------------------------------------

    fn emit_functions(&mut self, out: &mut String) {
        let starts: Vec<u32> = self.func_starts.iter().copied().collect();
        // The entry may take the name `main` only when no other function
        // already owns it (the recompiler's stub calls `main`).
        let main_taken = self.names.values().any(|n| n == "main");
        let mut bodies: Vec<String> = Vec::new();
        for (i, &start) in starts.iter().enumerate() {
            let end = starts
                .get(i + 1)
                .copied()
                .unwrap_or_else(|| self.insts.keys().next_back().map_or(start, |k| k + 1));
            if start >= end || !self.insts.contains_key(&start) {
                continue;
            }
            let mut name = self
                .names
                .get(&start)
                .cloned()
                .unwrap_or_else(|| {
                    if self.entry == Some(start) && !main_taken {
                        "main".to_string()
                    } else {
                        format!("F_{start:06X}")
                    }
                });

            let lifter = Lifter::new(self, start, end);
            let mut body = String::new();
            lifter.function(&name, &mut body);
            bodies.push(body);
        }
        if starts.is_empty() {
            out.push_str("/* no code discovered */\n");
            return;
        }
        // Forward declarations: calls may precede definitions in the file.
        for body in &bodies {
            if let Some(sig) = body.lines().next() {
                let sig = sig.trim_end();
                out.push_str(sig.trim_end_matches('{'));
                out.push_str(";\n");
            }
        }
        out.push('\n');
        for body in &bodies {
            out.push_str(body);
            out.push('\n');
        }
    }

    fn emit_data(&self, out: &mut String) {
        let Some(data_start) = self.data_start else {
            return;
        };
        let Some(bytes) = self.code.get((data_start - CODE_BASE) as usize..) else {
            return;
        };
        if bytes.is_empty() {
            return;
        }
        let mut named: Vec<(u32, &String)> = self
            .names
            .iter()
            .filter(|(addr, _)| **addr >= data_start)
            .map(|(addr, name)| (*addr, name))
            .collect();
        named.sort();
        if !named.is_empty() {
            out.push_str("/* data symbols */\n");
            for (addr, name) in named {
                out.push_str(&format!(
                    "/* {name} @ ROM 0x{addr:06X} (byte offset {}) */\n",
                    addr - data_start
                ));
            }
        }
        out.push_str("static const unsigned char rom_data[] = {\n");
        for chunk in bytes.chunks(16) {
            let hex: Vec<String> = chunk.iter().map(|b| format!("0x{b:02X}")).collect();
            out.push_str(&format!("    {},\n", hex.join(", ")));
        }
        out.push_str("};\n");
    }
}

/// First address of the 68000 work RAM as seen in ROM addressing.
const RAM_BASE: u32 = 0xFF0000;

/// Well-known Mega Drive hardware registers (address, name, access size).
const HW_REGISTERS: [(u32, &str, Size); 12] = [
    (0xA10001, "IO_VERSION", Size::Byte),
    (0xA10003, "IO_DATA_A", Size::Byte),
    (0xA10005, "IO_DATA_B", Size::Byte),
    (0xA10007, "IO_DATA_C", Size::Byte),
    (0xA10009, "IO_CTRL_A", Size::Byte),
    (0xA1000B, "IO_CTRL_B", Size::Byte),
    (0xA1000D, "IO_CTRL_C", Size::Byte),
    (0xA11100, "Z80_BUSREQ", Size::Word),
    (0xA11200, "Z80_RESET", Size::Word),
    (0xA14000, "TMSS_SEGA", Size::Long),
    (0xC00000, "VDP_DATA", Size::Word),
    (0xC00004, "VDP_CTRL", Size::Word),
];

fn is_hw_register(a: u32) -> bool {
    HW_REGISTERS.iter().any(|(addr, _, _)| *addr == a)
}

/// 24-bit ROM-space address of an absolute operand, with absolute-short
/// sign extension applied (0xF610.w addresses 0xFFF610).
fn abs_address(op: &Operand) -> Option<u32> {
    match op {
        Operand::AbsLong(a) => Some(a & 0x00FFFFFF),
        Operand::AbsShort(a) => Some(((*a as i16 as i32) as u32) & 0x00FFFFFF),
        _ => None,
    }
}

/// Dereferenced operands with their access size, for global typing.
fn operand_sizes(inst: &M68kInst) -> Vec<(&Operand, Size)> {
    match inst {
        M68kInst::Move(s, a, b) => vec![(a, *s), (b, *s)],
        M68kInst::MoveToCcr(a) => vec![(a, Size::Word)],
        M68kInst::Clr(s, a)
        | M68kInst::Neg(s, a)
        | M68kInst::Negx(s, a)
        | M68kInst::Not(s, a)
        | M68kInst::Tst(s, a) => vec![(a, *s)],
        M68kInst::Add(s, a, b)
        | M68kInst::Sub(s, a, b)
        | M68kInst::And(s, a, b)
        | M68kInst::Or(s, a, b)
        | M68kInst::Cmp(s, a, b) => vec![(a, *s), (b, *s)],
        M68kInst::Adda(s, a, _) | M68kInst::Suba(s, a, _) | M68kInst::Cmpa(s, a, _) => {
            vec![(a, *s)]
        }
        M68kInst::Addq(s, _, a) | M68kInst::Subq(s, _, a) => vec![(a, *s)],
        M68kInst::Addi(s, _, a)
        | M68kInst::Subi(s, _, a)
        | M68kInst::Andi(s, _, a)
        | M68kInst::Ori(s, _, a)
        | M68kInst::Eori(s, _, a)
        | M68kInst::Cmpi(s, _, a) => vec![(a, *s)],
        M68kInst::Movem(s, _, a, _) => vec![(a, *s)],
        M68kInst::Btst(_, a)
        | M68kInst::Bset(_, a)
        | M68kInst::Bclr(_, a)
        | M68kInst::Bchg(_, a) => vec![(a, Size::Byte)],
        M68kInst::Scc(_, a) | M68kInst::Tas(a) | M68kInst::Nbcd(a) => vec![(a, Size::Byte)],
        M68kInst::Muls(a, _) | M68kInst::Mulu(a, _) | M68kInst::Chk(a, _) => vec![(a, Size::Word)],
        M68kInst::Divs(a, _) | M68kInst::Divu(a, _) => vec![(a, Size::Word)],
        _ => Vec::new(),
    }
}

/// Register-direct value uses and address-register bases of an operand.
fn reg_reads_of(op: &Operand) -> Vec<Reg> {
    match op {
        Operand::DataReg(d) => vec![Reg::Data(*d)],
        Operand::AddrReg(a) => vec![Reg::Addr(*a)],
        Operand::AddrInd(a)
        | Operand::PostInc(a)
        | Operand::PreDec(a)
        | Operand::Disp(_, a) => vec![Reg::Addr(*a)],
        Operand::Indexed(_, a, i, _) => vec![Reg::Addr(*a), *i],
        Operand::PcIndexed(_, i, _) => vec![*i],
        _ => Vec::new(),
    }
}

/// Registers written by an operand used as a destination (register direct,
/// or an address register that the addressing mode advances).
fn reg_writes_of(op: &Operand) -> Vec<Reg> {
    match op {
        Operand::DataReg(d) => vec![Reg::Data(*d)],
        Operand::AddrReg(a) => vec![Reg::Addr(*a)],
        Operand::PostInc(a) | Operand::PreDec(a) => vec![Reg::Addr(*a)],
        _ => Vec::new(),
    }
}

/// Registers (reads, writes) touched by an instruction, for the
/// read-before-write argument analysis. Unknown instructions conservatively
/// read every register they mention.
fn reg_rw(inst: &M68kInst) -> (Vec<Reg>, Vec<Reg>) {
    fn src(reads: &mut Vec<Reg>, op: &Operand) {
        reads.extend(reg_reads_of(op));
    }
    fn dst(reads: &mut Vec<Reg>, writes: &mut Vec<Reg>, op: &Operand, also_read: bool) {
        if also_read {
            reads.extend(reg_reads_of(op));
        }
        writes.extend(reg_writes_of(op));
    }

    let mut reads = Vec::new();
    let mut writes = Vec::new();
    match inst {
        M68kInst::Move(_, a, b) => {
            src(&mut reads, a);
            dst(&mut reads, &mut writes, b, false);
        }
        M68kInst::Moveq(_, d) => writes.push(Reg::Data(*d)),
        M68kInst::Lea(a, r) => {
            src(&mut reads, a);
            writes.push(Reg::Addr(*r));
        }
        M68kInst::Pea(a) => src(&mut reads, a),
        M68kInst::Clr(_, a) => dst(&mut reads, &mut writes, a, false),
        M68kInst::Exg(a, b) => {
            reads.extend([*a, *b]);
            writes.extend([*a, *b]);
        }
        M68kInst::Add(_, a, b)
        | M68kInst::Sub(_, a, b)
        | M68kInst::And(_, a, b)
        | M68kInst::Or(_, a, b) => {
            src(&mut reads, a);
            dst(&mut reads, &mut writes, b, true);
        }
        M68kInst::Adda(_, a, r) => {
            src(&mut reads, a);
            writes.push(Reg::Addr(*r));
        }
        M68kInst::Suba(_, a, r) => {
            src(&mut reads, a);
            writes.push(Reg::Addr(*r));
        }
        M68kInst::Cmpa(_, a, r) => {
            src(&mut reads, a);
            reads.push(Reg::Addr(*r));
        }
        M68kInst::Cmp(_, a, b) => {
            src(&mut reads, a);
            src(&mut reads, b);
        }
        M68kInst::Addq(_, _, a) | M68kInst::Subq(_, _, a) => {
            dst(&mut reads, &mut writes, a, true)
        }
        M68kInst::Addi(_, _, a)
        | M68kInst::Subi(_, _, a)
        | M68kInst::Andi(_, _, a)
        | M68kInst::Ori(_, _, a)
        | M68kInst::Eori(_, _, a) => dst(&mut reads, &mut writes, a, true),
        M68kInst::Cmpi(_, _, a) => src(&mut reads, a),
        M68kInst::Neg(_, a) | M68kInst::Negx(_, a) | M68kInst::Not(_, a) => {
            dst(&mut reads, &mut writes, a, true)
        }
        M68kInst::Tst(_, a) | M68kInst::Chk(a, _) | M68kInst::Tas(a) | M68kInst::Nbcd(a) => {
            src(&mut reads, a)
        }
        M68kInst::MoveToCcr(a) => src(&mut reads, a),
        M68kInst::Ext(_, d) | M68kInst::Swap(d) => {
            reads.push(Reg::Data(*d));
            writes.push(Reg::Data(*d));
        }
        M68kInst::Muls(a, d) | M68kInst::Mulu(a, d) | M68kInst::Divs(a, d) | M68kInst::Divu(a, d) => {
            src(&mut reads, a);
            reads.push(Reg::Data(*d));
            writes.push(Reg::Data(*d));
        }
        M68kInst::Lsl(_, c, d)
        | M68kInst::Lsr(_, c, d)
        | M68kInst::Asl(_, c, d)
        | M68kInst::Asr(_, c, d)
        | M68kInst::Rol(_, c, d)
        | M68kInst::Ror(_, c, d)
        | M68kInst::Roxl(_, c, d)
        | M68kInst::Roxr(_, c, d) => {
            src(&mut reads, c);
            reads.push(Reg::Data(*d));
            writes.push(Reg::Data(*d));
        }
        M68kInst::Btst(a, b) => {
            src(&mut reads, a);
            src(&mut reads, b);
        }
        M68kInst::Bset(a, b) | M68kInst::Bclr(a, b) | M68kInst::Bchg(a, b) => {
            src(&mut reads, a);
            dst(&mut reads, &mut writes, b, true);
        }
        M68kInst::Scc(_, a) => dst(&mut reads, &mut writes, a, false),
        M68kInst::Movem(_, regs, a, to_mem) => {
            src(&mut reads, a);
            if *to_mem {
                reads.extend(regs.iter().copied());
            } else {
                writes.extend(regs.iter().copied());
            }
        }
        M68kInst::Dbcc(_, d, _) => {
            reads.push(Reg::Data(*d));
            writes.push(Reg::Data(*d));
        }
        M68kInst::MoveUspToAddr(r) => writes.push(Reg::Addr(*r)),
        M68kInst::MoveAddrToUsp(r) => reads.push(Reg::Addr(*r)),
        M68kInst::Link(r, _) => writes.push(Reg::Addr(*r)),
        M68kInst::Unlk(r) => reads.push(Reg::Addr(*r)),
        M68kInst::Movep(_, _, _, a, d) => {
            reads.push(Reg::Addr(*a));
            reads.push(Reg::Data(*d));
            writes.push(Reg::Data(*d));
        }
        // Conservatively treat anything unlisted as reading all mentioned
        // registers, so no argument is missed.
        other => {
            let mut f = |op: &Operand| reads.extend(reg_reads_of(op));
            for_each_operand(other, &mut f);
        }
    }
    (reads, writes)
}

fn is_unconditional(inst: &M68kInst) -> bool {    // DBcc is not in this list: it falls through when the loop exits, so the
    // linear walk must continue past it (the target is queued as a label).
    matches!(
        inst,
        M68kInst::Bra(_) | M68kInst::BraS(_) | M68kInst::Jmp(_)
    )
}

fn is_call(inst: &M68kInst) -> bool {
    matches!(inst, M68kInst::Jsr(_) | M68kInst::Bsr(_) | M68kInst::BsrS(_))
}

// ---------------------------------------------------------------------------
// Lifter: instructions -> C statements
// ---------------------------------------------------------------------------


impl Stmt {
    fn branch_target(&self) -> u32 {
        match self {
            Stmt::Branch { target, .. } | Stmt::Dbf { target, .. } => *target,
            _ => 0,
        }
    }
}

/// A lifted statement.
#[derive(Debug, Clone)]
enum Stmt {
    /// Plain expression statement (`d0 = d1 + 2;`).
    Expr(String),
    /// Control line emitted by the structurer (`if (...) {`, `}`, ...).
    Control(String),
    /// Conditional or unconditional branch. `taken` is the C condition under
    /// which the branch executes; `None` means unconditional.
    Branch { taken: Option<String>, target: u32 },
    /// DBF loop back-edge.
    Dbf { reg: String, target: u32 },
    Return(Option<String>),
    Label(u32),
    Comment(String),
}

/// Condition source for the next branch.
#[derive(Debug, Clone)]
enum CmpRef {
    /// Value was only tested (tst / arithmetic result).
    Zero,
    /// Value was compared against this expression (cmp/cmpi).
    Expr(String),
    /// Value is a captured carry-out bit (shift by 1); bcc/bcs test it.
    Carry,
}

struct Lifter<'a, 'b> {
    decomp: &'a Decompiler<'b>,
    start: u32,
    end: u32,
    /// Registers currently holding a frame-slot address (`lea slot(a6), rX`
    /// followed by address copies): register name -> slot name. Lets the
    /// lifter collapse the codegen's address-shuffle sequences into direct
    /// slot reads/writes.
    addr_regs: HashMap<String, String>,
    /// Condition for the next branch.
    pending: Option<(String, CmpRef)>,
    /// Deferred `(an)+` store advance: (register, byte step).
    post_store_inc: Option<(String, u32)>,
    stmts: Vec<Stmt>,
    params: BTreeSet<u32>,
    locals: BTreeSet<u32>,
    used_regs: BTreeSet<String>,
    needs_tmp: bool,
    returns_value: bool,
}

impl<'a, 'b> Lifter<'a, 'b> {
    fn new(decomp: &'a Decompiler<'b>, start: u32, end: u32) -> Self {
        Lifter {
            decomp,
            start,
            end,
            addr_regs: HashMap::new(),
            pending: None,
            post_store_inc: None,
            stmts: Vec::new(),
            params: BTreeSet::new(),
            locals: BTreeSet::new(),
            used_regs: BTreeSet::new(),
            needs_tmp: false,
            returns_value: false,
        }
    }

    fn function(mut self, name: &str, out: &mut String) {
        self.prescan();
        self.lift_body();
        let mut stmts = structure(std::mem::take(&mut self.stmts));
        // Publish out-argument registers before every return, and when the
        // body can run off the end.
        let out_args: Vec<String> = self
            .decomp
            .func_arg_out
            .get(&self.start)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default();
        if !out_args.is_empty() {
            let mut rewritten: Vec<Stmt> = Vec::with_capacity(stmts.len() + 8);
            for s in stmts.drain(..) {
                match s {
                    Stmt::Return(v) => {
                        for a in &out_args {
                            rewritten.push(Stmt::Expr(format!("*{a}p = {a};")));
                        }
                        rewritten.push(Stmt::Return(v));
                    }
                    other => rewritten.push(other),
                }
            }
            for a in &out_args {
                rewritten.push(Stmt::Expr(format!("*{a}p = {a};")));
            }
            stmts = rewritten;
        }
        self.render(name, &stmts, out);
    }

    /// Discover frame layout and register usage before lifting.
    fn prescan(&mut self) {
        let insts: Vec<M68kInst> = self
            .decomp
            .insts
            .range(self.start..self.end)
            .map(|(_, d)| d.inst.clone())
            .collect();
        for inst in &insts {
            let mut frame = |op: &Operand| {
                if let Operand::Disp(d, AddrReg::A6) = op {
                    if *d >= 8 && (*d - 8) % 4 == 0 {
                        self.params.insert((*d - 8) as u32 / 4);
                    } else if *d < 0 {
                        self.locals.insert((*d as i32).unsigned_abs());
                    }
                }
                match op {
                    Operand::DataReg(r) => {
                        self.used_regs.insert(format!("{r}"));
                    }
                    Operand::AddrReg(r) => {
                        // A7 renders as `sp`; A6 is the frame pointer in
                        // smdc frames but a plain register in foreign code.
                        self.used_regs.insert(format!("{r}"));
                    }
                    // Base registers of memory operands.
                    Operand::AddrInd(r)
                    | Operand::PostInc(r)
                    | Operand::PreDec(r)
                    | Operand::Disp(_, r) => {
                        self.used_regs.insert(format!("{r}"));
                    }
                    Operand::Indexed(_, r, i, _) => {
                        self.used_regs.insert(format!("{r}"));
                        self.used_regs.insert(format!("{i}"));
                    }
                    Operand::PcIndexed(_, i, _) => {
                        self.used_regs.insert(format!("{i}"));
                    }
                    Operand::Sr => {
                        self.used_regs.insert("sr".to_string());
                    }
                    _ => {}
                }
            };
            for_each_operand(inst, &mut frame);
        }
    }

    fn lift_body(&mut self) {
        let addrs: Vec<u32> = self
            .decomp
            .insts
            .range(self.start..self.end)
            .map(|(&a, _)| a)
            .collect();
        for addr in &addrs {
            if self.decomp.labels.contains(addr) {
                self.stmts.push(Stmt::Label(*addr));
            }
            let d = &self.decomp.insts[addr];
            let inst = d.inst.clone();
            let target = d.target;
            self.lift_inst(inst, *addr, target);
        }
        // Tail fall-through: the extent's last instruction flows on into the
        // next function's start (a shared-body entry such as F_000506's
        // second controller-port pass through F_000526). The callee's `rts`
        // returns to THIS function's caller, so a call followed by this
        // function's own return is the faithful rendering.
        if let Some(&last) = addrs.last() {
            let d = &self.decomp.insts[&last];
            let terminator = matches!(
                d.inst,
                M68kInst::Rts
                    | M68kInst::Rte
                    | M68kInst::Rtr
                    | M68kInst::Bra(_)
                    | M68kInst::BraS(_)
                    | M68kInst::Jmp(_)
                    | M68kInst::Stop(_)
                    | M68kInst::Illegal
            );
            if !terminator
                && d.next == self.end
                && self.decomp.func_starts.contains(&self.end)
                && self.end != self.start
            {
                let text = self.call_text(self.end);
                self.stmts.push(Stmt::Expr(text));
            }
        }
    }

    fn lift_inst(&mut self, inst: M68kInst, addr: u32, target: Option<u32>) {
        match inst {
            M68kInst::Comment(c) => self.stmts.push(Stmt::Comment(format!("/* {c} */"))),
            M68kInst::Directive(_) | M68kInst::Label(_) | M68kInst::Nop => {}

            // Frame plumbing: elided; params/locals are named variables.
            M68kInst::Link(..) | M68kInst::Movem(..) | M68kInst::Unlk(_) => {}

            M68kInst::Rts | M68kInst::Rte => {
                // Fold a preceding `d0 = X;` into `return X;`.
                if let Some(Stmt::Expr(e)) = self.stmts.last() {
                    if let Some(val) = e.strip_prefix("d0 = ").and_then(|v| v.strip_suffix(';')) {
                        let val = val.to_string();
                        self.stmts.pop();
                        self.stmts.push(Stmt::Return(Some(val)));
                        self.returns_value = true;
                        return;
                    }
                }
                if self.decomp.func_ret.contains(&self.start) {
                    // A value-returning function's plain `rts` returns d0.
                    self.used_regs.insert("d0".to_string());
                    self.stmts.push(Stmt::Return(Some("d0".to_string())));
                } else {
                    self.stmts.push(Stmt::Return(None));
                }
            }

            // Status-register accesses become intrinsics so the compiled
            // hybrid actually changes the interrupt mask.
            M68kInst::Move(Size::Word, src, Operand::Sr) => {
                let v = self.rvalue(&src, &Size::Word);
                self.stmts.push(Stmt::Expr(format!("__set_sr({v});")));
            }
            M68kInst::Move(Size::Word, Operand::Sr, dst) => {
                let lv = self.lvalue(&dst, &Size::Word);
                self.stmts.push(Stmt::Expr(format!("{lv} = __get_sr();")));
                self.pending = Some((lv, CmpRef::Zero));
            }
            M68kInst::Move(size, src, dst) => self.lift_move(size, &src, &dst),
            M68kInst::Moveq(v, reg) => {
                self.stmts.push(Stmt::Expr(format!("{reg} = {v};")));
                self.addr_regs.remove(&format!("{reg}"));
            }
            M68kInst::Lea(src, reg) => {
                let r = format!("{reg}");
                if let Some(slot) = frame_slot(&src) {
                    self.addr_regs.insert(r, slot);
                    return; // folds into the next access through the register
                }
                let expr = self.operand_addr(&src);
                self.stmts.push(Stmt::Expr(format!("{r} = {expr};")));
                self.addr_regs.remove(&r);
            }
            M68kInst::Clr(size, op) => {
                let lv = self.lvalue(&op, &size);
                self.stmts.push(Stmt::Expr(format!("{lv} = 0;")));
                self.pending = Some(("0".to_string(), CmpRef::Zero));
            }

            M68kInst::Add(size, src, dst) => self.lift_binary(&src, &dst, "+", &size),
            M68kInst::Sub(size, src, dst) => self.lift_binary(&src, &dst, "-", &size),
            M68kInst::And(size, src, dst) => self.lift_binary(&src, &dst, "&", &size),
            M68kInst::Or(size, src, dst) => self.lift_binary(&src, &dst, "|", &size),
            M68kInst::Adda(_, src, dst) => {
                self.lift_binary(&src, &Operand::AddrReg(dst), "+", &Size::Long)
            }
            M68kInst::Suba(_, src, dst) => {
                self.lift_binary(&src, &Operand::AddrReg(dst), "-", &Size::Long)
            }
            M68kInst::Addq(size, n, dst) => {
                self.lift_binary(&Operand::Imm(n as i32), &dst, "+", &size)
            }
            M68kInst::Subq(size, n, dst) => {
                self.lift_binary(&Operand::Imm(n as i32), &dst, "-", &size)
            }
            M68kInst::Addi(size, v, dst) => {
                self.lift_binary(&Operand::Imm(v), &dst, "+", &size)
            }
            M68kInst::Subi(size, v, dst) => {
                self.lift_binary(&Operand::Imm(v), &dst, "-", &size)
            }
            M68kInst::Andi(size, v, dst) => {
                self.lift_binary(&Operand::Imm(v), &dst, "&", &size)
            }
            M68kInst::Ori(size, v, dst) => self.lift_binary(&Operand::Imm(v), &dst, "|", &size),
            M68kInst::Eori(size, v, dst) => {
                self.lift_binary(&Operand::Imm(v), &dst, "^", &size)
            }
            M68kInst::Eor(_, src, dst) => {
                self.lift_binary(
                    &Operand::DataReg(src),
                    &dst,
                    "^",
                    &Size::Long,
                )
            }
            M68kInst::Muls(src, dst) | M68kInst::Mulu(src, dst) => {
                self.lift_binary(&src, &Operand::DataReg(dst), "*", &Size::Word)
            }
            M68kInst::Divs(src, dst) | M68kInst::Divu(src, dst) => {
                self.lift_binary(&src, &Operand::DataReg(dst), "/", &Size::Word)
            }
            M68kInst::Neg(size, op) => {
                self.lift_unary(&op, "-", &size);
            }
            M68kInst::Not(size, op) => {
                self.lift_unary(&op, "~", &size);
            }
            M68kInst::Ext(_, reg) => {
                self.stmts
                    .push(Stmt::Expr(format!("{reg} = (int)(short){reg};")));
            }
            M68kInst::Swap(reg) => {
                self.stmts.push(Stmt::Expr(format!(
                    "{reg} = ({reg} >> 16) | ({reg} << 16);"
                )));
            }
            M68kInst::Exg(a, b) => {
                let (x, y) = (reg_name(&a), reg_name(&b));
                self.needs_tmp = true;
                self.stmts.push(Stmt::Expr(format!(
                    "__tmp = {x}; {x} = {y}; {y} = __tmp;"
                )));
            }

            M68kInst::Lsl(_, cnt, dst) => {
                self.lift_shift(&cnt, &Operand::DataReg(dst), "<<")
            }
            M68kInst::Asl(_, cnt, dst) => {
                self.lift_shift(&cnt, &Operand::DataReg(dst), "<<")
            }
            M68kInst::Lsr(_, cnt, dst) => {
                self.lift_shift(&cnt, &Operand::DataReg(dst), ">>")
            }
            M68kInst::Asr(_, cnt, dst) => {
                self.lift_shift(&cnt, &Operand::DataReg(dst), ">>")
            }
            M68kInst::Rol(_, cnt, dst) => {
                self.lift_shift(&cnt, &Operand::DataReg(dst), "<<")
            }
            M68kInst::Ror(_, cnt, dst) => {
                self.lift_shift(&cnt, &Operand::DataReg(dst), ">>")
            }

            M68kInst::Tst(size, op) => {
                let v = self.rvalue(&op, &size);
                let v = masked_compare_value(v, size);
                self.pending = Some((v, CmpRef::Zero));
            }
            M68kInst::Cmp(size, src, dst) => {
                // Memory operands render at the compare's width: cmp.w reads
                // the word AT the address, not the low half of a long.
                let d = self.rvalue(&dst, &size);
                let s = self.rvalue(&src, &size);
                let (d, s) = if size == Size::Long {
                    (d, s)
                } else {
                    (masked_compare_value(d, size), masked_compare_value(s, size))
                };
                self.pending = Some((d, CmpRef::Expr(s)));
            }
            M68kInst::Cmpa(_, src, dst) => {
                let s = self.rvalue(&src, &Size::Long);
                self.pending = Some((format!("{dst}"), CmpRef::Expr(s)));
            }
            M68kInst::Cmpi(size, v, dst) => {
                let d = self.rvalue(&dst, &size);
                let s = masked_compare_value(format!("{v}"), size);
                let d = if size == Size::Long {
                    d
                } else {
                    masked_compare_value(d, size)
                };
                self.pending = Some((d, CmpRef::Expr(s)));
            }

            M68kInst::Bra(_) | M68kInst::BraS(_) => {
                if let Some(t) = target {
                    self.stmts.push(Stmt::Branch { taken: None, target: t });
                }
            }
            M68kInst::Bcc(cond, _) | M68kInst::BccS(cond, _) => {
                let taken = self.cond_expr(&cond);
                if let Some(t) = target {
                    self.stmts.push(Stmt::Branch {
                        taken: Some(taken),
                        target: t,
                    });
                }
                // Branches do not affect flags: keep the pending condition
                // so a following branch on the same comparison (beq+bmi on
                // one move) still renders a real condition.
            }
            M68kInst::Dbcc(cond, reg, _) => {
                if let Some(t) = target {
                    if matches!(cond, Cond::False) {
                        self.stmts.push(Stmt::Dbf {
                            reg: format!("{reg}"),
                            target: t,
                        });
                    } else {
                        // Foreign-only loop form: render as a plain branch.
                        let taken = format!("/* dbcc {cond} */ 1");
                        self.stmts.push(Stmt::Branch {
                            taken: Some(taken),
                            target: t,
                        });
                    }
                }
                self.pending = None;
            }
            M68kInst::Bsr(_) | M68kInst::BsrS(_) => {
                if let Some(t) = target {
                    self.push_call(t);
                }
            }
            M68kInst::Jsr(op) => {
                // Prefer the analysis-pass call target (named via symbols);
                // only statically-known addresses reach this path.
                if let Some(t) = target {
                    self.push_call(t);
                } else if let Some((idx, stride, case_offset, is_jsr, targets)) =
                    self.decomp.jump_tables.get(&addr).cloned()
                {
                    self.emit_dispatch_switch(&idx, stride, case_offset, is_jsr, &targets);
                } else {
                    match op {
                        Operand::Label(name) => {
                            self.stmts.push(Stmt::Expr(format!("{name}();")));
                        }
                        Operand::AddrInd(r) => {
                            self.stmts
                                .push(Stmt::Expr(format!("((int (*)(void)){r})();")));
                        }
                        other => {
                            let a = self.operand_addr(&other);
                            self.stmts
                                .push(Stmt::Expr(format!("((int (*)(void)){a})();")));
                        }
                    }
                }
            }
            M68kInst::Jmp(op) => {
                if let Some(t) = target {
                    self.stmts.push(Stmt::Branch { taken: None, target: t });
                } else if let Some((idx, stride, case_offset, is_jsr, targets)) =
                    self.decomp.jump_tables.get(&addr).cloned()
                {
                    self.emit_dispatch_switch(&idx, stride, case_offset, is_jsr, &targets);
                }
            }
            M68kInst::Scc(cond, dst) => {
                let c = self.cond_expr(&cond);
                let lv = self.lvalue(&dst, &Size::Byte);
                self.stmts
                    .push(Stmt::Expr(format!("{lv} = ({c}) ? 1 : 0;")));
                self.pending = None;
            }

            M68kInst::Btst(bit, ea) => {
                // Z = (bit == 0): model the tested bit so a following
                // beq/bne renders a real condition.
                let v = self.rvalue(&ea, &Size::Byte);
                let n = match &bit {
                    Operand::Imm(n) => format!("{n}"),
                    other => self.rvalue(other, &Size::Long),
                };
                self.pending = Some((format!("(({v} >> {n}) & 1)"), CmpRef::Zero));
            }
            M68kInst::Bset(..) | M68kInst::Bclr(..) | M68kInst::Bchg(..) => {
                let text = inst.format();
                self.stmts.push(Stmt::Comment(format!("/*{text}*/")));
                self.pending = None;
            }

            M68kInst::Pea(op) => {
                let a = self.operand_addr(&op);
                self.used_regs.insert("sp".to_string());
                self.stmts
                    .push(Stmt::Expr(format!("sp = sp - 4; *(int *)sp = (int)({a});")));
            }

            // Foreign-ROM instructions without a C-level model yet: keep the
            // text as a comment so nothing is lost.
            other => {
                let text = other.format();
                self.stmts.push(Stmt::Comment(format!("/*{text}*/")));
                self.pending = None;
            }
        }
    }

    // -- statement helpers --------------------------------------------------

    /// Render a resolved jump-table dispatch as a C `switch` over the index
    /// register: `case` values are byte offsets into the table, arms call
    /// the target functions (jsr tables) or branch to labels (jmp tables).
    fn emit_dispatch_switch(
        &mut self,
        idx: &Reg,
        stride: u32,
        case_offset: u32,
        is_jsr: bool,
        targets: &[u32],
    ) {
        let idx_name = format!("{idx}");
        self.used_regs.insert(idx_name.clone());
        self.pending = None;
        self.stmts.push(Stmt::Control(format!("switch ({idx_name}) {{")));
        for (i, t) in targets.iter().enumerate() {
            let arm = if is_jsr {
                format!("{} break;", self.call_text(*t))
            } else {
                format!("goto L_{t:06X};")
            };
            self.stmts.push(Stmt::Control(format!(
                "case {}: {arm}",
                case_offset + i as u32 * stride
            )));
        }
        self.stmts.push(Stmt::Control("}".to_string()));
    }

    /// Call expression for a statically-known target with its reconstructed
    /// register arguments; a callee whose return value is live assigns d0.
    /// Functions that are not independently emitted (merged into a
    /// neighbour) are called through their original address: the hybrid
    /// keeps the original image, so that code still exists.
    fn call_text(&mut self, target: u32) -> String {
        if !self.decomp.func_starts.contains(&target)
            || !self.decomp.insts.contains_key(&target)
        {
            return format!("((int (*)(void))0x{target:X})();");
        }
        let name = self.target_name(target);
        let args = self
            .decomp
            .func_args
            .get(&target)
            .cloned()
            .unwrap_or_default();
        let callee_out = self
            .decomp
            .func_arg_out
            .get(&target)
            .cloned()
            .unwrap_or_default();
        for a in &args {
            self.used_regs.insert(a.clone());
        }
        let rendered: Vec<String> = args
            .iter()
            .map(|a| {
                if callee_out.contains(a) {
                    format!("&{a}")
                } else {
                    a.clone()
                }
            })
            .collect();
        let call = if rendered.is_empty() {
            format!("{name}()")
        } else {
            format!("{name}({})", rendered.join(", "))
        };
        if self.decomp.func_ret.contains(&target) {
            self.used_regs.insert("d0".to_string());
            format!("d0 = {call};")
        } else {
            format!("{call};")
        }
    }

    /// Emit a call to a statically-known target with the callee's
    /// reconstructed register arguments; a callee whose return value is
    /// live gets its result assigned to d0.
    fn push_call(&mut self, target: u32) {
        let returns = self.decomp.func_ret.contains(&target);
        let text = self.call_text(target);
        self.stmts.push(Stmt::Expr(text));
        if returns {
            self.pending = Some(("d0".to_string(), CmpRef::Zero));
        }
    }

    fn lift_move(&mut self, size: Size, src: &Operand, dst: &Operand) {
        // Track pure address copies between registers: the codegen moves
        // slot addresses through `lea -> a0 -> d0 -> a0` chains before the
        // actual dereference. Elide the copies and remember where the
        // address lives so the deref renders as a direct slot access.
        if let (Some(s), Some(d)) = (reg_name_of(src), reg_name_of(dst)) {
            if let Some(slot) = self.addr_regs.get(&s).cloned() {
                self.addr_regs.insert(d, slot);
                return; // pure address shuffle (incl. cache parking)
            }
        }
        let rv = self.rvalue(src, &size);
        // Register destinations model partial-register semantics: a byte
        // move keeps the upper 24 bits, a word move zero-extends (m68k
        // MOVE.W), a long move replaces.
        if let Some(d) = reg_name_of(dst) && matches!(dst, Operand::DataReg(_)) {
            // The pending flag value carries the moved value's SIGN domain:
            // N comes from bit 7 / bit 15, so a following bmi/bpl must test
            // the sign-extended sub-word, not the zero-extended mask.
            let (assign, flags_value) = match size {
                Size::Byte => (
                    format!("{d} = ({d} & -256) | ({rv} & 255);"),
                    format!("((int)(char)({rv} & 255))"),
                ),
                Size::Word => (
                    format!("{d} = {rv} & 65535;"),
                    format!("((int)(short)({rv} & 65535))"),
                ),
                Size::Long => (
                    format!("{d} = {rv};"),
                    rv.clone(),
                ),
            };
            self.stmts.push(Stmt::Expr(assign));
            self.pending = Some((flags_value, CmpRef::Zero));
            self.addr_regs.remove(&d);
            return;
        }
        let lv = self.lvalue(dst, &size);
        self.stmts.push(Stmt::Expr(format!("{lv} = {rv};")));
        if let Some((r, n)) = self.post_store_inc.take() {
            self.stmts.push(Stmt::Expr(format!("{r} = {r} + {n};")));
        }
        self.pending = Some((lv, CmpRef::Zero));
        // The destination register no longer holds a tracked slot address.
        if let Some(d) = reg_name_of(dst) {
            self.addr_regs.remove(&d);
        }
    }

    fn lift_binary(&mut self, src: &Operand, dst: &Operand, op: &str, size: &Size) {
        let lv = self.lvalue(dst, size);
        let rv = self.rvalue(src, size);
        // Sub-word arithmetic into a data register confines the result to
        // the low byte/word, preserving the upper bits (m68k semantics).
        if let Some(d) = reg_name_of(dst)
            && matches!(dst, Operand::DataReg(_))
            && *size != Size::Long
        {
            let (m, keep) = match size {
                Size::Byte => ("255", "-256"),
                _ => ("65535", "-65536"),
            };
            let expr = format!("(({lv} {op} {rv}) & {m})");
            self.stmts
                .push(Stmt::Expr(format!("{d} = ({d} & {keep}) | {expr};")));
            self.pending = Some((expr, CmpRef::Zero));
            self.addr_regs.remove(&d);
            return;
        }
        self.stmts.push(Stmt::Expr(format!("{lv} = {lv} {op} {rv};")));
        self.pending = Some((lv, CmpRef::Zero));
        if let Some(d) = reg_name_of(dst) {
            self.addr_regs.remove(&d);
        }
    }

    /// Unary ALU op (NEG, NOT) at the instruction's size: memory operands
    /// render at that width, and a sub-word result into a data register is
    /// confined to the low byte/word, preserving the upper bits.
    fn lift_unary(&mut self, dst: &Operand, op: &str, size: &Size) {
        let lv = self.lvalue(dst, size);
        let rv = self.rvalue(dst, size);
        if let Some(d) = reg_name_of(dst)
            && matches!(dst, Operand::DataReg(_))
            && *size != Size::Long
        {
            let (m, keep, cast) = match size {
                Size::Byte => ("255", "-256", "(int)(char)"),
                _ => ("65535", "-65536", "(int)(short)"),
            };
            let expr = format!("({op}({rv}) & {m})");
            self.stmts
                .push(Stmt::Expr(format!("{d} = ({d} & {keep}) | {expr};")));
            // The N flag reads the signed sub-word result: sign-cast so a
            // following bmi/bpl tests a negative value.
            self.pending = Some((format!("{cast}({expr})"), CmpRef::Zero));
            self.addr_regs.remove(&d);
            return;
        }
        let expr = format!("{op}({rv})");
        self.stmts.push(Stmt::Expr(format!("{lv} = {expr};")));
        self.pending = Some((expr, CmpRef::Zero));
        if let Some(d) = reg_name_of(dst) {
            self.addr_regs.remove(&d);
        }
    }

    fn lift_shift(&mut self, cnt: &Operand, dst: &Operand, op: &str) {
        let lv = self.lvalue(dst, &Size::Long);
        let n = match cnt {
            Operand::Imm(n) => format!("{n}"),
            other => self.rvalue(other, &Size::Long),
        };
        // A shift-by-1 whose carry-out a following bcc/bcs tests: capture
        // the outgoing bit (the sign bit before the shift) so the branch
        // renders as a real condition instead of a `/* cc */` fallback.
        if matches!(cnt, Operand::Imm(1)) {
            self.stmts
                .push(Stmt::Expr(format!("__cc = ({lv} >> 31) & 1;")));
            self.pending = Some(("__cc".to_string(), CmpRef::Carry));
            self.used_regs.insert("__cc".to_string());
        }
        self.stmts
            .push(Stmt::Expr(format!("{lv} = {lv} {op} {n};")));
    }

    /// C rvalue for an operand.
    fn rvalue(&mut self, op: &Operand, size: &Size) -> String {
        self.value(op, size, false)
    }

    /// C lvalue for a destination operand.
    fn lvalue(&mut self, op: &Operand, size: &Size) -> String {
        self.value(op, size, true)
    }

    fn value(&mut self, op: &Operand, size: &Size, _lv: bool) -> String {
        match op {
            Operand::DataReg(r) => format!("{r}"),
            Operand::AddrReg(r) => format!("{r}"),
            Operand::AddrInd(r) => {
                // Fold `lea slot(a6), rX` (+ address copies) + `(rX)` into
                // a direct slot access.
                if let Some(slot) = self.addr_regs.get(&format!("{r}")) {
                    return slot.clone();
                }
                format!("*({} *){r}", ctype(size))
            }
            Operand::PostInc(r) => {
                // (an)+ / -(an): the register advances by the ACCESS SIZE —
                // int variables get no C pointer scaling, so decompose.
                let n = size_of_bytes(size);
                if _lv {
                    // Store at an; the advance is emitted after the store.
                    self.post_store_inc = Some((format!("{r}"), n));
                    format!("*({} *){r}", ctype(size))
                } else {
                    self.stmts
                        .push(Stmt::Expr(format!("__m = *({} *){r};", ctype(size))));
                    self.stmts.push(Stmt::Expr(format!("{r} = {r} + {n};")));
                    self.used_regs.insert("__m".to_string());
                    "__m".to_string()
                }
            }
            Operand::PreDec(r) => {
                let n = size_of_bytes(size);
                self.stmts.push(Stmt::Expr(format!("{r} = {r} - {n};")));
                if _lv {
                    format!("*({} *){r}", ctype(size))
                } else {
                    self.stmts
                        .push(Stmt::Expr(format!("__m = *({} *){r};", ctype(size))));
                    self.used_regs.insert("__m".to_string());
                    "__m".to_string()
                }
            }
            Operand::Disp(_, AddrReg::A6) => frame_slot(op).unwrap_or_else(|| "0".into()),
            Operand::Disp(d, r) => format!("*({} *)({d} + {r})", ctype(size)),
            Operand::Indexed(d, r, i, _) => format!("*({} *)({d} + {r} + {i})", ctype(size)),
            Operand::PcIndexed(d, r, _) => {
                format!("*({} *)({r} + {d} /*pc*/)", ctype(size))
            }
            Operand::PcDisp(d) => format!("*({} *)({d} /*pc*/)", ctype(size)),
            Operand::AbsShort(_) | Operand::AbsLong(_) => {
                // Sign-extended 24-bit address, named when we know it.
                let a24 = abs_address(op).unwrap_or(0);
                match self.decomp.named_addr(a24, *size) {
                    Some(name) => name,
                    None => format!("*({} *)0x{a24:06X}", ctype(size)),
                }
            }
            Operand::Label(name) => format!("*({} *)&{name}", ctype(size)),
            Operand::Imm(v) => {
                if _lv {
                    // Invalid as an M68k destination — this only arises from
                    // garbage-decoded data; render compilable pseudo-C.
                    format!("*({} *)({v})", ctype(size))
                } else {
                    format!("{v}")
                }
            }
            Operand::Sr => "sr".to_string(),
        }
    }

    /// Address expression for LEA/PEA sources.
    fn operand_addr(&mut self, op: &Operand) -> String {
        match op {
            Operand::DataReg(r) => format!("{r}"),
            Operand::AddrReg(r) => format!("{r}"),
            Operand::AddrInd(r) => format!("{r}"),
            Operand::Disp(_, AddrReg::A6) => frame_slot(op)
                .map(|s| format!("&{s}"))
                .unwrap_or_else(|| "0".into()),
            Operand::Disp(d, r) => format!("({d} + {r})"),
            Operand::Indexed(d, r, i, _) => format!("({d} + {r} + {i})"),
            Operand::PcIndexed(d, r, _) => format!("({r} + {d} /*pc*/)"),
            Operand::PcDisp(d) => format!("({d} /*pc*/)"),
            Operand::AbsShort(_) | Operand::AbsLong(_) => {
                let a24 = abs_address(op).unwrap_or(0);
                match self.decomp.named_addr(a24, Size::Long) {
                    Some(name) => name,
                    None => format!("0x{a24:06X}"),
                }
            }
            Operand::Label(name) => format!("&{name}"),
            Operand::Imm(v) => format!("{v}"),
            Operand::Sr => "sr".to_string(),
            Operand::PostInc(r) | Operand::PreDec(r) => format!("{r}"),
        }
    }

    /// C condition under which a branch with this condition code is taken.
    fn cond_expr(&self, cond: &Cond) -> String {
        let Some((v, r)) = &self.pending else {
            return format!("/* cc {cond} */ 1");
        };
        // Both sides are parenthesized: lifted values are compound
        // expressions (`(d0 & 15) & 255`), and C comparison operators bind
        // tighter than bitwise `&`/`|`.
        let v = format!("({v})");
        if matches!(r, CmpRef::Carry) {
            // Only carry-sense branches are expressible for a captured bit.
            return match cond {
                Cond::Cc => format!("{v} == 0"),
                Cond::Cs => format!("{v} != 0"),
                _ => format!("/* cc {cond} */ 1"),
            };
        }
        let cmp = match r {
            CmpRef::Zero => "0".to_string(),
            CmpRef::Expr(e) => format!("({e})"),
            CmpRef::Carry => unreachable!(),
        };
        match cond {
            Cond::Eq => format!("{v} == {cmp}"),
            Cond::Ne => format!("{v} != {cmp}"),
            Cond::Lt => format!("{v} < {cmp}"),
            Cond::Ge => format!("{v} >= {cmp}"),
            Cond::Gt => format!("{v} > {cmp}"),
            Cond::Le => format!("{v} <= {cmp}"),
            // Unsigned senses of a preceding cmp.
            Cond::Cc => format!("{v} >= {cmp}"),
            Cond::Cs => format!("{v} < {cmp}"),
            Cond::Hi => format!("{v} > {cmp}"),
            Cond::Ls => format!("{v} <= {cmp}"),
            Cond::Pl => format!("{v} >= 0"),
            Cond::Mi => format!("{v} < 0"),
            _ => format!("/* cc {cond} */ 1"),
        }
    }

    fn target_name(&self, addr: u32) -> String {
        self.decomp
            .names
            .get(&addr)
            .cloned()
            .unwrap_or_else(|| format!("F_{addr:06X}"))
    }

    // -- rendering ----------------------------------------------------------

    fn render(&self, name: &str, stmts: &[Stmt], out: &mut String) {
        let body_start = out.len();
        let ret = if self.returns_value || self.decomp.func_ret.contains(&self.start) {
            "int"
        } else {
            "void"
        };
        // smdc cdecl frames use named parameters; foreign register
        // conventions pass the registers the callee reads first. Argument
        // registers the body writes become pointer out-parameters: the 68k
        // register file is shared, so the caller observes the mutation.
        let out_args: BTreeSet<String> = self
            .decomp
            .func_arg_out
            .get(&self.start)
            .cloned()
            .unwrap_or_default();
        let arg_names: Vec<String> = if !self.params.is_empty() {
            self.params.iter().map(|p| format!("int p{p}")).collect()
        } else {
            self.decomp
                .func_args
                .get(&self.start)
                .map(|args| {
                    args.iter()
                        .map(|a| {
                            if out_args.contains(a) {
                                format!("int *{a}p")
                            } else {
                                format!("int {a}")
                            }
                        })
                        .collect()
                })
                .unwrap_or_default()
        };
        let plist = if arg_names.is_empty() {
            "void".to_string()
        } else {
            arg_names.join(", ")
        };
        out.push_str(&format!("{ret} {name}({plist}) {{\n"));

        // Out-arguments alias their incoming cell; the write-back before
        // every return publishes the final register value.
        for a in &out_args {
            out.push_str(&format!("    int {a} = *{a}p;\n"));
        }

        // Locals: every used register except the parameter registers.
        let param_names: Vec<String> = self
            .decomp
            .func_args
            .get(&self.start)
            .cloned()
            .unwrap_or_default();
        let mut regs: Vec<String> = self
            .used_regs
            .iter()
            .filter(|r| !param_names.contains(r))
            .filter(|r| !out_args.contains(*r))
            .cloned()
            .collect();
        regs.sort_by(|a, b| {
            (a.starts_with('a'), a.to_string()).cmp(&(b.starts_with('a'), b.to_string()))
        });
        if self.needs_tmp {
            regs.push("__tmp".to_string());
        }
        if !regs.is_empty() {
            out.push_str(&format!("    int {};\n", regs.join(", ")));
        }
        let locals: Vec<String> = self.locals.iter().map(|v| format!("v{v}")).collect();
        if !locals.is_empty() {
            out.push_str(&format!("    int {};\n", locals.join(", ")));
        }

        for s in stmts {
            match s {
                Stmt::Expr(e) | Stmt::Control(e) => out.push_str(&format!("    {e}\n")),
                Stmt::Comment(c) => out.push_str(&format!("    {c}\n")),
                Stmt::Return(Some(v)) => out.push_str(&format!("    return {v};\n")),
                Stmt::Return(None) => out.push_str(&format!("    return;\n")),
                Stmt::Branch {
                    taken: Some(c),
                    target,
                } => out.push_str(&format!("    if ({c}) goto L_{target:06X};\n")),
                Stmt::Branch { taken: None, target } => {
                    out.push_str(&format!("    goto L_{target:06X};\n"))
                }
                Stmt::Dbf { reg, target } => out.push_str(&format!(
                    "    {reg} = {reg} - 1; if ({reg} >= 0) goto L_{target:06X};\n"
                )),
                Stmt::Label(a) => out.push_str(&format!("L_{a:06X}:\n")),
            }
        }

        // Scope block labels to this function: C labels are function-local
        // but the assembler's symbol table is global, and kept
        // cross-jumping functions can reference the same block address.
        // `L_000788` becomes `L004000_000788` (prefixed with the owning
        // function's start) everywhere in this body.
        {
            let start = self.start;
            let mut renamed = String::with_capacity(out.len() - body_start);
            let source = out[body_start..].to_string();
            let bytes = source.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'L'
                    && i + 8 <= bytes.len()
                    && &bytes[i..i + 2] == b"L_"
                    && bytes[i + 2..i + 8].iter().all(|b| b.is_ascii_hexdigit())
                {
                    let _ = write!(renamed, "L{start:06X}_");
                    renamed.push_str(&source[i + 2..i + 8]);
                    i += 8;
                } else {
                    renamed.push(source[i..].chars().next().unwrap());
                    let ch_len = source[i..].chars().next().unwrap().len_utf8();
                    i += ch_len;
                }
            }
            out.truncate(body_start);
            out.push_str(&renamed);
        }

        // Safety net: any label referenced by a goto but never defined
        // (its block was consumed by a structuring pattern or emitted by
        // a switch arm) is appended at the function end — a jump to the
        // epilogue falls through to the closing return, matching `rts`
        // semantics closely enough for pseudo-C and keeping the output
        // compilable. Scanning this function's rendered text catches every
        // emission path; labels in OTHER functions must not suppress it.
        let start = self.start;
        let prefix = format!("L{start:06X}_");
        let body = out[body_start..].to_string();
        let mut missing: Vec<String> = Vec::new();
        let needle = format!("goto {prefix}");
        let mut rest = body.as_str();
        while let Some(p) = rest.find(&needle) {
            let after = &rest[p + needle.len()..];
            let hex: String = after.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            if hex.len() == 6
                && u32::from_str_radix(&hex, 16).is_ok()
                && !body.contains(&format!("{prefix}{hex}:"))
                && !missing.contains(&hex)
            {
                missing.push(hex);
            }
            rest = after;
        }
        for hex in missing {
            let _ = write!(out, "{prefix}{hex}: ;\n");
        }

        out.push_str("}\n");
    }
}

/// Register name of an operand, if it is a plain register.
fn reg_name_of(op: &Operand) -> Option<String> {
    match op {
        Operand::DataReg(r) => Some(format!("{r}")),
        Operand::AddrReg(r) => Some(format!("{r}")),
        _ => None,
    }
}

/// C expression for a sub-word compare operand: masked to the operation
/// size and cast for signed comparison semantics.
fn masked_compare_value(expr: String, size: Size) -> String {
    match size {
        Size::Byte => format!("(char)(({expr}) & 255)"),
        Size::Word => format!("(short)(({expr}) & 65535)"),
        Size::Long => expr,
    }
}

/// Frame-slot name for `d(a6)`: params `pK` (8+), locals `vN` (negative).
fn frame_slot(op: &Operand) -> Option<String> {
    if let Operand::Disp(d, AddrReg::A6) = op {
        if *d >= 8 && (*d - 8) % 4 == 0 {
            return Some(format!("p{}", (*d - 8) / 4));
        }
        if *d < 0 {
            return Some(format!("v{}", (*d as i32).unsigned_abs()));
        }
    }
    None
}

fn reg_name(r: &Reg) -> String {
    match r {
        Reg::Data(d) => format!("{d}"),
        Reg::Addr(a) => format!("{a}"),
    }
}

fn size_of_bytes(size: &Size) -> u32 {
    match size {
        Size::Byte => 1,
        Size::Word => 2,
        Size::Long => 4,
    }
}

fn ctype(size: &Size) -> &'static str {
    match size {
        Size::Byte => "unsigned char",
        Size::Word => "short",
        Size::Long => "int",
    }
}

pub(crate) fn for_each_operand(inst: &M68kInst, f: &mut impl FnMut(&Operand)) {
    match inst {
        M68kInst::Move(_, a, b)
        | M68kInst::Add(_, a, b)
        | M68kInst::Sub(_, a, b)
        | M68kInst::And(_, a, b)
        | M68kInst::Or(_, a, b)
        | M68kInst::Cmp(_, a, b) => {
            f(a);
            f(b);
        }
        M68kInst::Eor(_, a, b) => {
            f(&Operand::DataReg(*a));
            f(b);
        }
        // Destination registers are visited too: the prescan must declare
        // every register the function touches, including LEA/ADDA targets.
        M68kInst::Lea(a, r) => {
            f(a);
            f(&Operand::AddrReg(*r));
        }
        M68kInst::Adda(_, a, r) | M68kInst::Suba(_, a, r) | M68kInst::Cmpa(_, a, r) => {
            f(a);
            f(&Operand::AddrReg(*r));
        }
        M68kInst::Pea(a)
        | M68kInst::Clr(_, a)
        | M68kInst::Not(_, a)
        | M68kInst::Addq(_, _, a)
        | M68kInst::Subq(_, _, a)
        | M68kInst::Addi(_, _, a)
        | M68kInst::Subi(_, _, a)
        | M68kInst::Andi(_, _, a)
        | M68kInst::Ori(_, _, a)
        | M68kInst::Eori(_, _, a)
        | M68kInst::Cmpi(_, _, a)
        | M68kInst::Muls(a, _)
        | M68kInst::Mulu(a, _)
        | M68kInst::Divs(a, _)
        | M68kInst::Divu(a, _)
        | M68kInst::Tst(_, a)
        | M68kInst::Jmp(a)
        | M68kInst::Jsr(a)
        | M68kInst::Scc(_, a) => f(a),
        M68kInst::Moveq(_, r) => f(&Operand::DataReg(*r)),
        M68kInst::Swap(r) | M68kInst::Ext(_, r) => f(&Operand::DataReg(*r)),
        M68kInst::Dbcc(_, r, _) => f(&Operand::DataReg(*r)),
        M68kInst::Lsl(_, c, d)
        | M68kInst::Lsr(_, c, d)
        | M68kInst::Asl(_, c, d)
        | M68kInst::Asr(_, c, d)
        | M68kInst::Rol(_, c, d)
        | M68kInst::Ror(_, c, d) => {
            f(c);
            f(&Operand::DataReg(*d));
        }
        M68kInst::Btst(a, b) | M68kInst::Bset(a, b) | M68kInst::Bclr(a, b) | M68kInst::Bchg(a, b) => {
            f(a);
            f(b);
        }
        M68kInst::Movem(_, _, a, _) => f(a),
        M68kInst::Exg(a, b) => {
            let as_op = |r: &Reg| match r {
                Reg::Data(d) => Operand::DataReg(*d),
                Reg::Addr(a) => Operand::AddrReg(*a),
            };
            f(&as_op(a));
            f(&as_op(b));
        }
        M68kInst::Link(r, _) | M68kInst::Unlk(r) => f(&Operand::AddrReg(*r)),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Structurer: statements -> structured C with goto fallback
// ---------------------------------------------------------------------------

/// Opportunistic structuring pass. Recognizes the shapes the smdc codegen
/// emits for if/else and DBF loops; everything else stays in goto-form
/// (still valid C).
fn structure(stmts: Vec<Stmt>) -> Vec<Stmt> {
    // Split into basic blocks at Label markers AND after terminators (a
    // conditional branch's fallthrough starts a new unlabeled block).
    let mut blocks: Vec<(u32, Vec<Stmt>)> = Vec::new();
    for s in stmts {
        let is_term = matches!(s, Stmt::Branch { .. } | Stmt::Dbf { .. } | Stmt::Return(_));
        if let Stmt::Label(a) = s {
            blocks.push((a, Vec::new()));
        } else if let Some((_, b)) = blocks.last_mut() {
            b.push(s);
        } else {
            blocks.push((u32::MAX, vec![s]));
        }
        if is_term {
            blocks.push((u32::MAX, Vec::new()));
        }
    }
    blocks.retain(|(_, b)| !b.is_empty());

    let index_of: HashMap<u32, usize> = blocks
        .iter()
        .enumerate()
        .map(|(i, (a, _))| (*a, i))
        .collect();

    if std::env::var("DEBUG_STRUCT").is_ok() {
        for (i, (a, b)) in blocks.iter().enumerate() {
            eprintln!("blk {i} @{a:06X}: {:?}", b.iter().map(|s| match s {
                Stmt::Expr(e) => e.clone(),
                Stmt::Control(c) => c.clone(),
                Stmt::Branch { taken, target } => format!("BR({taken:?})->{target:06X}"),
                Stmt::Dbf { reg, target } => format!("DBF {reg}->{target:06X}"),
                Stmt::Return(v) => format!("RET {v:?}"),
                Stmt::Label(l) => format!("LAB {l:06X}"),
                Stmt::Comment(c) => c.clone(),
            }).collect::<Vec<_>>());
        }
    }

    let mut out: Vec<Stmt> = Vec::new();
    let mut consumed: HashSet<usize> = HashSet::new();

    for i in 0..blocks.len() {
        let (addr, body) = &blocks[i];
        if consumed.contains(&i) {
            continue;
        }
        consumed.insert(i);

        let term = body.iter().position(|s| {
            matches!(s, Stmt::Branch { .. } | Stmt::Dbf { .. } | Stmt::Return(_))
        });

        // ---- Detect patterns first (no emission), so the prologue can be
        // ordered correctly. ----

        // `if (c) goto ELSE` ... fallthrough chain ends `goto END`, ELSE
        // chain merges at END  =>  if/else.
        let m_if_else = match term.map(|t| &body[t]) {
            Some(Stmt::Branch {
                taken: Some(cond),
                target,
            }) => {
                let else_i = index_of.get(target).copied();
                let end = chain_goto_target(&blocks, i + 1, else_i.unwrap_or(0));
                match (else_i, end) {
                    (Some(ei), Some(end))
                        if ei > i
                            && block_chain_ends_in_goto(&blocks, i + 1, ei)
                            // The merge point must lie at/after the else arm;
                            // a target inside or before it is a loop back edge.
                            && index_of.get(&end).is_none_or(|&e| e >= ei) =>
                    {
                        Some((ei, end, cond.clone()))
                    }
                    _ => None,
                }
            }
            _ => None,
        };

        // Do-while: this block is the head of a DBF loop when a nearby
        // following block ends with `dbf -> this block`. Detected at the
        // HEAD so the whole region renders once, here.
        let m_do_while = if m_if_else.is_none() && *addr != u32::MAX {
            // Single-block loop: `label: ...; dbf -> label`.
            if matches!(
                term.map(|t| &body[t]),
                Some(Stmt::Dbf { target, .. }) if *target == *addr
            ) {
                Some(i)
            } else {
            let mut j = i + 1;
            let mut found = None;
            while j < blocks.len() && j < i + 64 {
                match blocks[j].1.last() {
                    Some(Stmt::Dbf { target, .. }) if *target == *addr => {
                        found = Some(j);
                        break;
                    }
                    Some(Stmt::Return(_)) => break,
                    Some(Stmt::Branch { taken: None, target }) => {
                        // An unconditional goto leaving the candidate region
                        // means this is not a simple DBF loop.
                        match index_of.get(target) {
                            Some(&t) if t >= i && t <= j => {}
                            _ => break,
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            found
            }
        } else {
            None
        };

        // `if (c) goto EXIT` ... body ... `goto HEAD` (back edge to this
        // block)  =>  while. Canonical `while`/`for` loop shape.
        let m_while = if m_if_else.is_none() && m_do_while.is_none() {
            match term.map(|t| &body[t]) {
                Some(Stmt::Branch {
                    taken: Some(cond),
                    target,
                }) => {
                    let exit_i = index_of.get(target).copied();
                    if let Some(exit_i) = exit_i.filter(|&e| e > i + 1) {
                        let head = *addr;
                        if *addr != u32::MAX
                            && matches!(
                                blocks.get(exit_i - 1).map(|b| b.1.last()),
                                Some(Some(Stmt::Branch { taken: None, target })) if *target == head
                            )
                        {
                            Some((exit_i, cond.clone()))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            }
        } else {
            None
        };

        // ---- Prologue: the block's label and statements before its
        // terminator always precede the construct. The do-while pattern
        // re-renders its tail block fully, so nothing emits here for it. ----
        let lead = term.unwrap_or(body.len());
        // do-while: the construct renders its tail block fully (label
        // included), so the prologue emits nothing. while: the label emits
        // here, its head statements re-execute inside the loop. Otherwise:
        // label plus leading statements precede the construct.
        if m_do_while.is_none() {
            if *addr != u32::MAX {
                out.push(Stmt::Label(*addr));
            }
            if m_while.is_none() {
                out.extend(body[..lead].iter().cloned());
            }
        }

        // ---- Emit. ----
        if let Some((ei, end, cond)) = &m_if_else {
            emit_if_else(&blocks, i + 1, *ei, *end, cond, &mut consumed, &mut out);
        } else if let Some(tail) = m_do_while {
            let (reg, target) = match &blocks[tail].1.last() {
                Some(Stmt::Dbf { reg, target }) => (reg.clone(), *target),
                _ => unreachable!(),
            };
            emit_do_while(&blocks, i, tail, &reg, target, &mut consumed, &mut out);
        } else if let Some((exit_i, taken)) = &m_while {
            let exit_target = body[term.unwrap()].branch_target();
            emit_while(
                &blocks, i, exit_target, *exit_i, taken, &body[..lead], &mut consumed, &mut out,
            );
        } else {
            out.extend(body[lead..].iter().cloned());
        }
    }
    out
}

/// Whether the block chain `[start, stop)` ends in an unconditional goto.
fn block_chain_ends_in_goto(blocks: &[(u32, Vec<Stmt>)], start: usize, stop: usize) -> bool {
    let mut i = start;
    while i < stop.min(blocks.len()) {
        match blocks[i].1.last() {
            Some(Stmt::Branch {
                taken: None,
                ..
            }) => return true,
            Some(Stmt::Return(_)) => return false,
            _ => i += 1,
        }
    }
    false
}

/// Target of the unconditional goto ending the chain from `start`
/// (bounded by `stop`), if any.
fn chain_goto_target(blocks: &[(u32, Vec<Stmt>)], start: usize, stop: usize) -> Option<u32> {
    let mut i = start;
    while i < stop.min(blocks.len()) {
        match blocks[i].1.last() {
            Some(Stmt::Branch {
                taken: None,
                target,
            }) => return Some(*target),
            Some(Stmt::Return(_)) => return None,
            _ => i += 1,
        }
    }
    None
}

/// Inline a statement into a structured body, preserving control flow:
/// labels become `L_x: ;` (empty statement keeps the label valid C) and
/// branches/DBF become explicit gotos so no jump target is ever lost.
fn inline_stmt(s: &Stmt, out: &mut Vec<Stmt>) {
    match s {
        Stmt::Expr(e) | Stmt::Control(e) => out.push(Stmt::Control(e.clone())),
        Stmt::Comment(c) => out.push(Stmt::Control(c.clone())),
        Stmt::Return(Some(v)) => out.push(Stmt::Control(format!("return {v};"))),
        Stmt::Return(None) => out.push(Stmt::Control("return;".into())),
        Stmt::Label(a) => out.push(Stmt::Control(format!("L_{a:06X}: ;"))),
        Stmt::Branch {
            taken: Some(c),
            target,
        } => out.push(Stmt::Control(format!("if ({c}) goto L_{target:06X};"))),
        Stmt::Branch { taken: None, target } => {
            out.push(Stmt::Control(format!("goto L_{target:06X};")))
        }
        Stmt::Dbf { reg, target } => out.push(Stmt::Control(format!(
            "{reg} = {reg} - 1; if ({reg} >= 0) goto L_{target:06X};"
        ))),
    }
}

/// `HEAD: <lead>; if (<taken>) goto EXIT; <body>; goto HEAD`  ->
/// `while (1) { <lead>; if (<taken>) goto EXIT; <body> }`.
/// The head statements re-execute every iteration, as in the original.
fn emit_while(
    blocks: &[(u32, Vec<Stmt>)],
    head: usize,
    exit_target: u32,
    exit_i: usize,
    taken: &str,
    lead: &[Stmt],
    consumed: &mut HashSet<usize>,
    out: &mut Vec<Stmt>,
) {
    let _ = head;
    out.push(Stmt::Control("while (1) {".into()));
    for s in lead {
        out.push(s.clone());
    }
    out.push(Stmt::Control(format!(
        "if ({taken}) goto L_{exit_target:06X};"
    )));
    for i in head + 1..exit_i {
        emit_block_inline(&blocks[i], out);
        consumed.insert(i);
    }
    out.push(Stmt::Control("}".into()));
}

/// Inline one block into a structured body, preserving its label.
fn emit_block_inline(block: &(u32, Vec<Stmt>), out: &mut Vec<Stmt>) {
    let (addr, body) = block;
    if *addr != u32::MAX {
        out.push(Stmt::Control(format!("L_{addr:06X}: ;")));
    }
    for s in body {
        inline_stmt(s, out);
    }
}

fn body_text(block: &(u32, Vec<Stmt>)) -> Vec<String> {
    let mut inlined = Vec::new();
    emit_block_inline(block, &mut inlined);
    inlined
        .into_iter()
        .filter_map(|s| match s {
            Stmt::Expr(e) | Stmt::Control(e) => Some(e),
            _ => None,
        })
        .collect()
}

fn emit_if_else(
    blocks: &[(u32, Vec<Stmt>)],
    then_start: usize,
    else_start: usize,
    end: u32,
    cond: &str,
    consumed: &mut HashSet<usize>,
    out: &mut Vec<Stmt>,
) {
    out.push(Stmt::Control(format!("if ({}) {{", negate(cond))));
    for i in then_start..else_start {
        for t in body_text(&blocks[i]) {
            out.push(Stmt::Control(t));
        }
        consumed.insert(i);
    }
    out.push(Stmt::Control("} else {".into()));
    let mut i = else_start;
    while i < blocks.len() {
        if blocks[i].0 == end {
            break;
        }
        for t in body_text(&blocks[i]) {
            out.push(Stmt::Control(t));
        }
        consumed.insert(i);
        match blocks[i].1.last() {
            Some(Stmt::Return(_)) => break,
            _ => i += 1,
        }
    }
    out.push(Stmt::Control("}".into()));
}

fn emit_do_while(
    blocks: &[(u32, Vec<Stmt>)],
    body_start: usize,
    tail: usize,
    reg: &str,
    target: u32,
    consumed: &mut HashSet<usize>,
    out: &mut Vec<Stmt>,
) {
    out.push(Stmt::Control("do {".into()));
    for i in body_start..=tail {
        let (addr_b, body_b) = &blocks[i];
        if *addr_b != u32::MAX {
            out.push(Stmt::Control(format!("L_{addr_b:06X}: ;")));
        }
        // The tail block's DBF is expressed by the closing `while`; skip it.
        let stop = if i == tail {
            body_b.len().saturating_sub(1)
        } else {
            body_b.len()
        };
        for s in &body_b[..stop] {
            inline_stmt(s, out);
        }
        consumed.insert(i);
    }
    out.push(Stmt::Control(format!("}} while (--{reg} >= 0);")));
    let _ = target;
}

/// Negate a simple comparison expression (`a == b` -> `a != b`).
/// Two-character operators must be tested before their one-character
/// prefixes (`<=` before `<`).
fn negate(cond: &str) -> String {
    for (a, b) in [
        ("==", "!="),
        ("!=", "=="),
        ("<=", ">"),
        (">=", "<"),
        ("<", ">="),
        (">", "<="),
    ] {
        if let Some(pos) = cond.find(a) {
            let (l, r) = cond.split_at(pos);
            return format!("{l}{b}{}", &r[a.len()..]);
        }
    }
    format!("!({cond})")
}
