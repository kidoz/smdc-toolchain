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

const CODE_BASE: u32 = 0x200;

/// One decoded instruction with its static control-transfer target.
struct Decoded {
    inst: M68kInst,
    target: Option<u32>,
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
        }
    }

    fn run(&mut self) -> String {
        let mut out = String::from("/* Decompiled by smdc --decompile (pseudo-C) */\n");
        self.analyze();
        self.merge_cross_jumping_functions();
        self.emit_functions(&mut out);
        self.emit_data(&mut out);
        out
    }

    /// C labels are function-scoped, so a branch into a neighbouring
    /// function's extent means the call-target partition split one logical
    /// function. Merge such functions: drop the start of the function whose
    /// range contains the foreign target.
    fn merge_cross_jumping_functions(&mut self) {
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
                            removed = Some(owner);
                            break 'outer;
                        }
                    } else if t < a1 {
                        // Backward into an earlier function: merge self in.
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
        let mut work: Vec<u32> = self.entry_points().into_iter().collect();
        self.func_starts = self.entry_points().into_iter().collect();
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
                    break;
                };
                visited.insert(addr);
                self.insts.insert(
                    addr,
                    Decoded {
                        inst,
                        target,
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
                            M68kInst::Rts | M68kInst::Rte | M68kInst::Bra(_)
                        ) =>
                    {
                        break;
                    }
                    _ => {}
                }

                // Computed dispatch through a PC-relative jump table:
                // `jsr/jmp table(pc,dN)`. The table base is static; its word
                // entries are offsets relative to the table start. Follow
                // every entry that lands inside the code section.
                if let M68kInst::Jsr(op) | M68kInst::Jmp(op) = &self.insts[&addr].inst {
                    if let Operand::PcIndexed(base, _) = op {
                        let is_jsr = matches!(self.insts[&addr].inst, M68kInst::Jsr(_));
                        let table = self.resolve_jump_table(*base);
                        for t in table {
                            if is_jsr {
                                self.func_starts.insert(t);
                            } else {
                                self.labels.insert(t);
                            }
                            work.push(t);
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

    /// Resolve a jump table at `base`: word offsets relative to `base`.
    /// Entries are followed while they point at plausible code (forward,
    /// inside the ROM, even); the scan stops at the first implausible entry.
    fn resolve_jump_table(&self, base: i32) -> Vec<u32> {
        let mut targets = Vec::new();
        if base < CODE_BASE as i32 {
            return targets;
        }
        let base = base as u32;
        let mut off = base;
        // Cap: no dispatch table has more than 256 arms.
        for _ in 0..256 {
            let Some(w) = self.read_rom_word(off) else {
                break;
            };
            let target = base.wrapping_add_signed(w as i16 as i32);
            // Offsets are non-negative and must land after the table start.
            if (w as i16) < 0 || !self.in_code(target) || target < base {
                break;
            }
            targets.push(target);
            off += 2;
        }
        targets
    }

    fn read_rom_word(&self, addr: u32) -> Option<u16> {
        let off = addr.checked_sub(CODE_BASE)? as usize;
        let b = self.code.get(off..off + 2)?;
        Some(u16::from_be_bytes([b[0], b[1]]))
    }

    /// Entry points from the vector table: initial PC (0x4), HBlank (0x70)
    /// and VBlank (0x78) autovectors. smdc ROMs point them all at the entry.
    fn entry_points(&self) -> BTreeSet<u32> {
        let read = |off: usize| -> Option<u32> {
            let b = self.rom.get(off..off + 4)?;
            let v = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
            (v >= CODE_BASE && v % 2 == 0).then_some(v)
        };
        [0x4, 0x70, 0x78]
            .iter()
            .filter_map(|&off| read(off))
            .collect()
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

fn is_unconditional(inst: &M68kInst) -> bool {
    // DBF is not in this list: it falls through when the loop exits, so the
    // linear walk must continue past it (the target is queued as a label).
    matches!(inst, M68kInst::Bra(_) | M68kInst::Jmp(_))
}

fn is_call(inst: &M68kInst) -> bool {
    matches!(inst, M68kInst::Jsr(_) | M68kInst::Bsr(_))
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
        let stmts = structure(std::mem::take(&mut self.stmts));
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
                    Operand::Indexed(_, r, i) => {
                        self.used_regs.insert(format!("{r}"));
                        self.used_regs.insert(format!("{i}"));
                    }
                    Operand::PcIndexed(_, i) => {
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
        for addr in addrs {
            if self.decomp.labels.contains(&addr) {
                self.stmts.push(Stmt::Label(addr));
            }
            let d = &self.decomp.insts[&addr];
            let inst = d.inst.clone();
            let target = d.target;
            self.lift_inst(inst, addr, target);
        }
    }

    fn lift_inst(&mut self, inst: M68kInst, _addr: u32, target: Option<u32>) {
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
                self.stmts.push(Stmt::Return(None));
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
            M68kInst::Clr(_, op) => {
                let lv = self.lvalue(&op, &Size::Long);
                self.stmts.push(Stmt::Expr(format!("{lv} = 0;")));
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
            M68kInst::Addq(_, n, dst) => {
                self.lift_binary(&Operand::Imm(n as i32), &dst, "+", &Size::Long)
            }
            M68kInst::Subq(_, n, dst) => {
                self.lift_binary(&Operand::Imm(n as i32), &dst, "-", &Size::Long)
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
            M68kInst::Neg(_, op) => {
                let lv = self.lvalue(&op, &Size::Long);
                let rv = self.rvalue(&op, &Size::Long);
                self.stmts.push(Stmt::Expr(format!("{lv} = -{rv};")));
            }
            M68kInst::Not(_, op) => {
                let lv = self.lvalue(&op, &Size::Long);
                let rv = self.rvalue(&op, &Size::Long);
                self.stmts.push(Stmt::Expr(format!("{lv} = ~{rv};")));
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

            M68kInst::Tst(_, op) => {
                let v = self.rvalue(&op, &Size::Long);
                self.pending = Some((v, CmpRef::Zero));
            }
            M68kInst::Cmp(_, src, dst) => {
                let d = self.rvalue(&dst, &Size::Long);
                let s = self.rvalue(&src, &Size::Long);
                self.pending = Some((d, CmpRef::Expr(s)));
            }
            M68kInst::Cmpa(_, src, dst) => {
                let s = self.rvalue(&src, &Size::Long);
                self.pending = Some((format!("{dst}"), CmpRef::Expr(s)));
            }
            M68kInst::Cmpi(_, v, dst) => {
                let d = self.rvalue(&dst, &Size::Long);
                self.pending = Some((d, CmpRef::Expr(format!("{v}"))));
            }

            M68kInst::Bra(_) => {
                if let Some(t) = target {
                    self.stmts.push(Stmt::Branch { taken: None, target: t });
                }
            }
            M68kInst::Bcc(cond, _) => {
                let taken = self.cond_expr(&cond);
                if let Some(t) = target {
                    self.stmts.push(Stmt::Branch {
                        taken: Some(taken),
                        target: t,
                    });
                }
                self.pending = None;
            }
            M68kInst::Dbf(reg, _) => {
                if let Some(t) = target {
                    self.stmts.push(Stmt::Dbf {
                        reg: format!("{reg}"),
                        target: t,
                    });
                }
                self.pending = None;
            }
            M68kInst::Bsr(_) => {
                if let Some(t) = target {
                    let name = self.target_name(t);
                    self.stmts.push(Stmt::Expr(format!("{name}();")));
                }
            }
            M68kInst::Jsr(op) => {
                // Prefer the analysis-pass call target (named via symbols);
                // only statically-known addresses reach this path.
                if let Some(t) = target {
                    let name = self.target_name(t);
                    self.stmts.push(Stmt::Expr(format!("{name}();")));
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
            M68kInst::Jmp(_) => {
                if let Some(t) = target {
                    self.stmts.push(Stmt::Branch { taken: None, target: t });
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
                self.stmts.push(Stmt::Expr(format!("*--sp = (int)({a});")));
            }
        }
    }

    // -- statement helpers --------------------------------------------------

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
        self.stmts.push(Stmt::Expr(format!("{lv} = {lv} {op} {rv};")));
        self.pending = Some((lv, CmpRef::Zero));
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
            Operand::Indexed(d, r, i) => format!("*({} *)({d} + {r} + {i})", ctype(size)),
            Operand::PcIndexed(base, r) => {
                format!("*({} *)({:#X} + {r})", ctype(size), base)
            }
            Operand::AbsShort(a) => format!("*({} *)0x{:X}", ctype(size), *a as u16),
            Operand::AbsLong(a) => format!("*({} *)0x{a:X}", ctype(size)),
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
            Operand::Indexed(d, r, i) => format!("({d} + {r} + {i})"),
            Operand::PcIndexed(base, r) => format!("({:#X} + {r})", base),
            Operand::AbsShort(a) => format!("0x{:X}", *a as u16),
            Operand::AbsLong(a) => format!("0x{a:X}"),
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
            CmpRef::Expr(e) => e.clone(),
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
        let ret = if self.returns_value { "int" } else { "void" };
        let params: Vec<String> = self.params.iter().map(|p| format!("int p{p}")).collect();
        let plist = if params.is_empty() {
            "void".to_string()
        } else {
            params.join(", ")
        };
        out.push_str(&format!("{ret} {name}({plist}) {{\n"));

        let mut regs: Vec<String> = self.used_regs.iter().cloned().collect();
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

        // Safety net: any label referenced by a goto but never defined
        // (its block was consumed by a structuring pattern) is appended at
        // the function end — a jump to the epilogue falls through to the
        // closing return, matching `rts` semantics closely enough for
        // pseudo-C and keeping the output compilable.
        let mut refs: Vec<u32> = stmts
            .iter()
            .filter_map(|s| {
                match s {
                    Stmt::Branch { target, .. } | Stmt::Dbf { target, .. } => Some(*target),
                    _ => None,
                }
            })
            .collect();
        refs.sort();
        refs.dedup();
        for t in refs {
            let marker = format!("L_{t:06X}:");
            if !out.contains(&marker) {
                out.push_str(&format!("L_{t:06X}: ;\n"));
            }
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

fn for_each_operand(inst: &M68kInst, f: &mut impl FnMut(&Operand)) {
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
        M68kInst::Dbf(r, _) => f(&Operand::DataReg(*r)),
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
