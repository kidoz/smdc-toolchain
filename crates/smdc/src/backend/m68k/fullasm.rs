//! Full-coverage ROM disassembler for byte-exact round trips.
//!
//! Produces a `.s` source in the `asmparse` grammar that reassembles to the
//! original ROM byte-for-byte:
//!
//! - The vector table and ROM header (0x000-0x1FF) pass through as `dc.l`.
//! - Code is discovered by a flow walk seeded from every exception vector,
//!   following branches, calls and PC-relative jump tables.
//! - Remaining gaps are filled by a linear sweep that only accepts
//!   instructions which do not overrun a known boundary; everything else is
//!   emitted as `dc.w`/`dc.b` data, which preserves the bytes exactly.
//! - `romend` restores the 0xFF-padded image size.

use super::disasm::decode_at;
use super::m68k::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Disassemble a complete ROM image into re-assemblable source.
pub fn disassemble_full(rom: &[u8], symbols: &HashMap<String, u32>) -> String {
    let mut fx = FullDisasm::new(rom, symbols);
    fx.run();
    fx.render()
}

const CODE_BASE: u32 = 0x200;

struct FullDisasm<'a> {
    rom: &'a [u8],
    /// End of content: everything from here to the image end is 0xFF fill.
    content_end: u32,
    /// Final instruction set (address -> instruction, decoded with names).
    insts: BTreeMap<u32, M68kInst>,
    /// Instruction sizes (address -> byte length) from the discovery passes.
    spans: BTreeMap<u32, usize>,
    /// Addresses that came from the flow walk (trusted control flow).
    flow: BTreeSet<u32>,
    /// Address -> label name.
    labels: BTreeMap<u32, String>,
    /// Function entry addresses (for naming only).
    funcs: BTreeSet<u32>,
    /// Forced-data ranges (jump tables): [start, end).
    forced_data: Vec<(u32, u32)>,
    /// User-supplied symbol names by address.
    sym_names: HashMap<u32, String>,
}

impl<'a> FullDisasm<'a> {
    fn new(rom: &'a [u8], symbols: &HashMap<String, u32>) -> Self {
        // Trim the trailing 0xFF fill.
        let mut end = rom.len() as u32;
        while end > CODE_BASE && rom[end as usize - 1] == 0xFF {
            end -= 1;
        }
        let mut sym_names = HashMap::new();
        for (name, &addr) in symbols {
            sym_names.entry(addr).or_insert_with(|| name.clone());
        }
        FullDisasm {
            rom,
            content_end: end.max(CODE_BASE),
            insts: BTreeMap::new(),
            spans: BTreeMap::new(),
            flow: BTreeSet::new(),
            labels: BTreeMap::new(),
            funcs: BTreeSet::new(),
            forced_data: Vec::new(),
            sym_names,
        }
    }

    fn code(&self) -> &'a [u8] {
        &self.rom[CODE_BASE as usize..]
    }

    fn in_code(&self, addr: u32) -> bool {
        addr >= CODE_BASE && addr % 2 == 0 && addr < self.content_end
    }

    fn decode(&self, addr: u32, rev: &HashMap<u32, String>) -> Option<(M68kInst, usize, Option<u32>)> {
        decode_at(self.code(), (addr - CODE_BASE) as usize, CODE_BASE, rev)
    }

    fn run(&mut self) {
        self.flow_walk();
        self.linear_sweep();
        self.demote_overlapped();
        self.assign_names();
        self.final_decode();
    }

    // -- phase A: flow walk ------------------------------------------------

    fn flow_walk(&mut self) {
        let empty = HashMap::new();
        let mut work: Vec<u32> = self.vector_seeds();
        for &s in &work {
            self.funcs.insert(s);
        }
        let mut visited: BTreeSet<u32> = BTreeSet::new();

        while let Some(addr) = work.pop() {
            let mut addr = addr;
            loop {
                if visited.contains(&addr) || !self.in_code(addr) {
                    break;
                }
                let Some((inst, next, target)) = self.decode(addr, &empty) else {
                    break;
                };
                // A control transfer whose target cannot be named (odd
                // address, out of content, inside a jump table) is garbage:
                // stop the walk here and let the sweep treat it as data.
                if Self::is_control(&inst) && target.is_some_and(|t| !self.labelable(t)) {
                    break;
                }
                // Encodings that cannot re-assemble byte-exactly stop the
                // walk; the sweep will keep their bytes as data.
                if Self::ambiguous_encoding(&inst) {
                    break;
                }
                visited.insert(addr);
                self.flow.insert(addr);
                self.spans.insert(addr, next - (addr - CODE_BASE) as usize);
                self.collect_data_refs(&inst);

                let is_uncond = matches!(
                    inst,
                    M68kInst::Bra(_) | M68kInst::BraS(_) | M68kInst::Jmp(_)
                );
                let is_call = matches!(
                    inst,
                    M68kInst::Jsr(_) | M68kInst::Bsr(_) | M68kInst::BsrS(_)
                );
                let is_terminal = matches!(
                    inst,
                    M68kInst::Rts
                        | M68kInst::Rte
                        | M68kInst::Rtr
                        | M68kInst::Jmp(Operand::AddrInd(_) | Operand::PostInc(_) | Operand::PreDec(_) | Operand::Disp(_, _) | Operand::Indexed(..))
                );

                match target {
                    Some(t) if is_uncond => {
                        self.push_label(&mut work, t);
                        break;
                    }
                    Some(t) if is_call => {
                        self.funcs.insert(t);
                        self.push_label(&mut work, t);
                    }
                    Some(t) => {
                        self.push_label(&mut work, t);
                    }
                    None => {}
                }

                // PC-relative jump table dispatch.
                if let (M68kInst::Jsr(op) | M68kInst::Jmp(op), true) = (&inst, target.is_none()) {
                    if let Operand::PcIndexed(disp, ..) = op {
                        // Brief-format d(PC,Xn) references the extension
                        // word's own address (no +2), matching this ROM's
                        // assembler.
                        let base = addr.wrapping_add(2).wrapping_add_signed(*disp as i32);
                        self.resolve_jump_table(base, matches!(inst, M68kInst::Jsr(_)), &mut work);
                    }
                }

                if is_terminal {
                    break;
                }
                addr = CODE_BASE + next as u32;
            }
        }
    }

    fn push_label(&mut self, work: &mut Vec<u32>, t: u32) {
        // Only code addresses become branch labels.
        if self.labelable(t) {
            self.labels.insert(t, String::new()); // placeholder, named later
            work.push(t);
        }
    }

    /// A target a control transfer can legally name: even, inside the
    /// content, and not part of a resolved jump table.
    fn labelable(&self, t: u32) -> bool {
        self.in_code(t) && !self.forced_data.iter().any(|&(s, e)| t >= s && t < e)
    }

    /// Control-transfer instructions (branches, calls, static jumps).
    fn is_control(inst: &M68kInst) -> bool {
        matches!(
            inst,
            M68kInst::Bra(_)
                | M68kInst::BraS(_)
                | M68kInst::Bsr(_)
                | M68kInst::BsrS(_)
                | M68kInst::Bcc(_, _)
                | M68kInst::BccS(_, _)
                | M68kInst::Dbcc(_, _, _)
                | M68kInst::Jsr(_)
                | M68kInst::Jmp(_)
        )
    }

    /// True for encodings that decode fine but cannot re-assemble to the
    /// same bytes: OR/AND in the Dn -> <ea> direction with a register-direct
    /// destination encode canonically as <ea> -> Dn, so the typed model
    /// loses the distinction. The listing keeps such words as `dc.w` data.
    fn ambiguous_encoding(inst: &M68kInst) -> bool {
        matches!(
            inst,
            M68kInst::Or(_, Operand::DataReg(_), Operand::DataReg(_))
                | M68kInst::And(_, Operand::DataReg(_), Operand::DataReg(_))
        )
    }

    /// Every exception vector that points into the image (the whole table,
    /// not just the usual reset/HBlank/VBlank slots). Vector 1 (offset 4)
    /// is the initial PC.
    fn vector_seeds(&self) -> Vec<u32> {
        let mut seeds = Vec::new();
        for idx in 1..64 {
            let off = idx * 4;
            let Some(b) = self.rom.get(off..off + 4) else {
                break;
            };
            let v = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
            if self.in_code(v) {
                seeds.push(v);
            }
        }
        seeds.sort();
        seeds.dedup();
        seeds
    }

    /// Resolve a jump table at `base` and mark its extent as data.
    /// Supports long-absolute and word-offset-relative entry formats.
    fn resolve_jump_table(&mut self, base: u32, is_jsr: bool, work: &mut Vec<u32>) {
        if !self.in_code(base) {
            return;
        }
        let mut size = 2;
        // Try long-absolute entries first.
        let mut long_targets = Vec::new();
        let mut off = base;
        loop {
            let Some(b) = self.rom.get(off as usize..off as usize + 4) else {
                break;
            };
            let v = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
            if v == 0 || v >= self.content_end || v < CODE_BASE || v % 2 != 0 {
                break;
            }
            long_targets.push(v);
            off += 4;
            if long_targets.len() >= 256 {
                break;
            }
        }
        let entries: Vec<u32> = if long_targets.len() >= 2 {
            size = 4;
            long_targets
        } else {
            // Word offsets relative to the table base.
            let mut targets = Vec::new();
            let mut off = base;
            loop {
                let Some(b) = self.rom.get(off as usize..off as usize + 2) else {
                    break;
                };
                let w = u16::from_be_bytes([b[0], b[1]]) as i16;
                let t = base.wrapping_add_signed(w as i32);
                if w < 0 || !self.in_code(t) || t < base {
                    break;
                }
                targets.push(t);
                off += 2;
                if targets.len() >= 256 {
                    break;
                }
            }
            targets
        };

        if entries.is_empty() {
            return;
        }
        self.forced_data.push((base, base + entries.len() as u32 * size));
        self.labels.insert(base, String::new());
        for t in entries {
            if is_jsr {
                self.funcs.insert(t);
            }
            self.push_label(work, t);
        }
    }

    /// Absolute ROM references from instruction operands become labels so
    /// referenced code/data keeps a stable name and the operand re-encodes.
    fn collect_data_refs(&mut self, inst: &M68kInst) {
        let mut refs: Vec<u32> = Vec::new();
        let mut visit = |op: &Operand| {
            match op {
                Operand::AbsLong(a) if *a >= CODE_BASE && *a < self.content_end => {
                    refs.push(*a);
                }
                Operand::AbsShort(a) => {
                    let addr = *a as u32;
                    if addr >= CODE_BASE && addr < self.content_end {
                        refs.push(addr);
                    }
                }
                _ => {}
            }
        };
        super::decomp::for_each_operand(inst, &mut visit);
        for r in refs {
            self.labels.entry(r).or_insert_with(String::new);
        }
    }

    // -- phase B: linear sweep ----------------------------------------------

    fn linear_sweep(&mut self) {
        let empty = HashMap::new();
        let mut addr = CODE_BASE;
        while addr < self.content_end {
            // Skip forced data regions entirely.
            if let Some(&(_, end)) = self
                .forced_data
                .iter()
                .find(|&&(s, e)| addr >= s && addr < e)
            {
                addr = end;
                continue;
            }
            if self.spans.contains_key(&addr) {
                // Known instruction: skip past it.
                addr += self.spans[&addr] as u32;
                continue;
            }
            if addr % 2 != 0 {
                addr += 1;
                continue;
            }
            // Data labels mark table starts: do not decode them as code.
            if self.labels.contains_key(&addr)
                && !self.funcs.contains(&addr)
                && self.is_data_label(addr)
            {
                addr += 2;
                continue;
            }
            match self.decode(addr, &empty) {
                Some((inst, next, target)) => {
                    let len = next - (addr - CODE_BASE) as usize;
                    // Accept only if the instruction does not overrun any
                    // known boundary (label or instruction start), and any
                    // control transfer has a nameable target.
                    let overruns = (addr + 1..addr + len as u32).any(|a| {
                        self.labels.contains_key(&a) || self.spans.contains_key(&a)
                    });
                    let bad_target = Self::is_control(&inst)
                        && target.is_some_and(|t| !self.labelable(t));
                    if overruns || bad_target || Self::ambiguous_encoding(&inst) {
                        addr += 2;
                        continue;
                    }
                    self.spans.insert(addr, len);
                    // Branch targets must have labels so the text re-parses.
                    if let Some(t) = target {
                        if Self::is_control(&inst) && self.labelable(t) {
                            self.labels.entry(t).or_insert_with(String::new);
                        }
                    }
                    addr += len as u32;
                }
                None => {
                    addr += 2;
                }
            }
        }
    }

    fn is_data_label(&self, addr: u32) -> bool {
        !self.funcs.contains(&addr) && !self.flow.contains(&addr)
    }

    /// A label that lands strictly inside an accepted linear instruction
    /// invalidates that instruction: demote it to data. Labels strictly
    /// inside any instruction span (e.g. mid-instruction data references)
    /// can never be emitted, so they are removed; linear control transfers
    /// that lost their target label this way are demoted too.
    fn demote_overlapped(&mut self) {
        let addrs: Vec<u32> = self.spans.keys().copied().collect();
        for addr in addrs {
            if self.flow.contains(&addr) {
                continue;
            }
            let len = self.spans[&addr] as u32;
            let overlapped = (addr + 1..addr + len).any(|a| self.labels.contains_key(&a))
                || self
                    .forced_data
                    .iter()
                    .any(|&(s, e)| addr >= s && addr < e);
            if overlapped {
                self.spans.remove(&addr);
            }
        }

        // Labels strictly inside an instruction span can never be emitted.
        let spans = self.spans.clone();
        let inside_span = |a: u32| -> bool {
            spans
                .range(..a)
                .next_back()
                .is_some_and(|(&s, &len)| a > s && a < s + len as u32)
        };
        let dead: Vec<u32> = self
            .labels
            .keys()
            .copied()
            .filter(|&a| inside_span(a))
            .collect();
        for a in dead {
            self.labels.remove(&a);
        }

        // Demote linear control transfers whose target label vanished.
        let empty = HashMap::new();
        let addrs: Vec<u32> = self.spans.keys().copied().collect();
        for addr in addrs {
            if self.flow.contains(&addr) {
                continue;
            }
            if let Some((inst, _, target)) = self.decode(addr, &empty) {
                if Self::is_control(&inst) && target.is_some_and(|t| !self.labels.contains_key(&t))
                {
                    self.spans.remove(&addr);
                }
            }
        }
    }

    // -- phase C: naming and final decode -----------------------------------

    fn assign_names(&mut self) {
        let addr_list: Vec<u32> = self.labels.keys().copied().collect();
        for addr in addr_list {
            let name = match self.sym_names.get(&addr) {
                Some(n) => sanitize_name(n),
                None => {
                    if self.funcs.contains(&addr) {
                        format!("F_{addr:06X}")
                    } else if self.flow.contains(&addr) || self.is_instr_start(addr) {
                        format!("L_{addr:06X}")
                    } else {
                        format!("DAT_{addr:06X}")
                    }
                }
            };
            self.labels.insert(addr, name);
        }
        // Function entries always get a label line even if nothing branched
        // to them.
        for &f in &self.funcs {
            self.labels.entry(f).or_insert_with(|| format!("F_{f:06X}"));
        }
    }

    fn is_instr_start(&self, addr: u32) -> bool {
        self.spans.contains_key(&addr)
    }

    fn final_decode(&mut self) {
        let rev: HashMap<u32, String> = self
            .labels
            .iter()
            .map(|(&a, n)| (a, n.clone()))
            .collect();
        let addrs: Vec<u32> = self.spans.keys().copied().collect();
        for addr in addrs {
            if let Some((inst, _, _)) = self.decode(addr, &rev) {
                self.insts.insert(addr, inst);
            }
        }
    }

    // -- rendering -----------------------------------------------------------

    fn render(&self) -> String {
        use std::fmt::Write;
        let mut out = String::new();
        let _ = writeln!(out, "; Full-coverage disassembly (smdc --disasm --full)");
        let _ = writeln!(out, "; Reassemble with: smdc <file>.s -o <file>.gen");
        let _ = writeln!(out);

        // Header passthrough: 0x000-0x1FF as dc.l rows.
        let _ = writeln!(out, "; ---- vectors and ROM header (passthrough) ----");
        for row in 0..32 {
            let words: Vec<String> = (0..4)
                .map(|i| {
                    let off = (row * 4 + i) * 4;
                    let v = u32::from_be_bytes([
                        self.rom[off],
                        self.rom[off + 1],
                        self.rom[off + 2],
                        self.rom[off + 3],
                    ]);
                    format!("${v:08X}")
                })
                .collect();
            let _ = writeln!(out, "dc.l    {}", words.join(", "));
        }

        let _ = writeln!(out);
        let _ = writeln!(out, "org ${CODE_BASE:X}");
        let _ = writeln!(out);

        // Code and data interleaved by address.
        let mut pending: Vec<u8> = Vec::new();
        let mut pending_start = CODE_BASE;
        let mut addr = CODE_BASE;
        while addr < self.content_end {
            if let Some(name) = self.labels.get(&addr) {
                if !pending.is_empty() {
                    flush_data(&mut out, &pending, pending_start);
                    pending.clear();
                }
                let _ = writeln!(out, "{name}:");
                pending_start = addr;
            }
            if let Some(inst) = self.insts.get(&addr) {
                if !pending.is_empty() {
                    flush_data(&mut out, &pending, pending_start);
                    pending.clear();
                }
                let len = self.spans[&addr] as u32;
                let text = inst.format();
                // Every instruction line must re-parse: a decode that
                // renders as unparseable text (e.g. a mask-0 movem swept
                // out of data) would break reassembly. Fall back to the
                // instruction's bytes as dc.w words, which are exact.
                if super::asmparse::parse_inst_line(&text).is_err() {
                    let base = (addr - CODE_BASE) as usize;
                    let words: Vec<String> = (0..len as usize)
                        .step_by(2)
                        .map(|i| {
                            format!(
                                "${:04X}",
                                u16::from_be_bytes([
                                    self.code()[base + i],
                                    self.code()[base + i + 1],
                                ])
                            )
                        })
                        .collect();
                    let _ = writeln!(out, "dc.w    {}", words.join(", "));
                } else {
                    let _ = writeln!(out, "{text}");
                }
                addr += len;
                pending_start = addr;
                continue;
            }
            pending.push(self.rom[addr as usize]);
            addr += 1;
        }
        flush_data(&mut out, &pending, pending_start);

        let _ = writeln!(out);
        let _ = writeln!(out, "romend ${:X}", self.rom.len());
        out
    }
}

/// Flush pending data bytes as `dc.b`/`dc.w` rows, byte-precise.
fn flush_data(out: &mut String, data: &[u8], start: u32) {
    use std::fmt::Write;
    if data.is_empty() {
        return;
    }
    let mut i = 0usize;
    // Leading odd byte.
    if start % 2 != 0 && i < data.len() {
        let _ = writeln!(out, "dc.b    ${:02X}", data[i]);
        i += 1;
    }
    // Word pairs.
    while i + 1 < data.len() {
        let mut row: Vec<String> = Vec::new();
        let mut n = 0;
        while i + 1 < data.len() && n < 8 {
            let w = u16::from_be_bytes([data[i], data[i + 1]]);
            row.push(format!("${w:04X}"));
            i += 2;
            n += 1;
        }
        let _ = writeln!(out, "dc.w    {}", row.join(", "));
    }
    // Trailing odd byte.
    if i < data.len() {
        let _ = writeln!(out, "dc.b    ${:02X}", data[i]);
    }
}

fn sanitize_name(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        let ok = c.is_ascii_alphanumeric() || c == '_' || (c == '.' && i > 0);
        if ok {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::m68k::{Assembler, AsmItem};
    use crate::backend::m68k::asmparse::parse_assembly;

    /// Compile C -> ROM -> disassemble -> reassemble == identical bytes.
    /// This is the CI-verifiable pre-gate for the foreign-ROM round trip.
    #[test]
    fn round_trips_an_smdc_built_rom() {
        let source = "int add(int a, int b) { return a + b; }\n\
                      void main() {\n\
                          int i;\n\
                          int total = 0;\n\
                          for (i = 0; i < 4; i = i + 1) { total = total + add(i, 2); }\n\
                          if (total) { total = total + 1; } else { total = 1; }\n\
                      }\n";
        let (rom, symbols) = build_rom(source);

        let src = disassemble_full(&rom, &symbols);
        let items = parse_assembly(&src).expect("disassembly must re-parse");
        let mut asm = Assembler::new(0);
        let rebuilt = asm
            .assemble_items(&items)
            .expect("disassembly must re-assemble");
        let first_diff = rebuilt
            .iter()
            .zip(rom.iter())
            .position(|(a, b)| a != b)
            .map(|i| format!("{i:X}"))
            .unwrap_or_else(|| "?".into());
        assert_eq!(rebuilt, rom, "first difference at {first_diff}");
    }

    fn build_rom(
        source: &str,
    ) -> (Vec<u8>, HashMap<String, u32>) {
        use crate::backend::rom::RomBackend;
        use crate::backend::BackendConfig;
        use crate::backend::OutputFormat;
        use crate::frontend::{CFrontend, CompileContext, Frontend, FrontendConfig};
        use crate::DiagnosticReporter;

        let mut reporter = DiagnosticReporter::new();
        let file_id = reporter.add_file("test.c", source);
        let context = CompileContext::new("test.c".to_string(), file_id, &reporter);
        let module = CFrontend::new()
            .compile(source, &context, &FrontendConfig::default())
            .unwrap();

        let reporter = DiagnosticReporter::new();
        let context = CompileContext::new("test".to_string(), 0, &reporter);
        let config = BackendConfig {
            output_format: OutputFormat::Binary,
            debug_info: true,
            ..BackendConfig::default()
        };
        let (rom, debug) = RomBackend::new().build_rom(&module, &config).unwrap();
        let symbols = debug.expect("debug artifacts").symbols;
        (rom, symbols)
    }

    #[test]
    fn round_trips_a_synthetic_rom() {
        // Build a tiny image: header at 0, code at 0x200, trailing data,
        // 0xFF padding — then take it apart and put it back together.
        let items = vec![
            AsmItem::Org(0x200),
            AsmItem::Label("vectors_common".to_string()),
            AsmItem::Inst(M68kInst::Nop),
            AsmItem::Inst(M68kInst::BraS("vectors_common".to_string())),
            AsmItem::Label("entry".to_string()),
            AsmItem::Inst(M68kInst::Moveq(1, DataReg::D0)),
            AsmItem::Inst(M68kInst::BsrS("helper".to_string())),
            AsmItem::Inst(M68kInst::Bra("entry".to_string())),
            AsmItem::Label("helper".to_string()),
            AsmItem::Inst(M68kInst::Move(
                Size::Word,
                Operand::Imm(0),
                Operand::DataReg(DataReg::D1),
            )),
            AsmItem::Inst(M68kInst::Rts),
            // Raw data right after the code (undecodable word + a valid
            // instruction pattern, both must survive as exact bytes).
            AsmItem::Data(vec![0xF0, 0x00, 0x4E, 0x71, 0xBE, 0xEF]),
        ];
        let mut asm = Assembler::new(0);
        let image = asm.assemble_items(&items).unwrap();
        let mut rom = vec![0xFF; 0x4000];
        rom[..image.len()].copy_from_slice(&image);
        // Point the reset vector at `entry` (0x204).
        rom[4..8].copy_from_slice(&0x204u32.to_be_bytes());

        let src = disassemble_full(&rom, &HashMap::new());
        let items = parse_assembly(&src).expect("disassembly must re-parse");
        let mut asm = Assembler::new(0);
        let rebuilt = asm
            .assemble_items(&items)
            .expect("disassembly must re-assemble");
        let first_diff = rebuilt
            .iter()
            .zip(rom.iter())
            .position(|(a, b)| a != b)
            .map(|i| format!("{i:X}"))
            .unwrap_or_else(|| "?".into());
        assert_eq!(rebuilt, rom, "first difference at {first_diff}\n{src}");
    }
}
