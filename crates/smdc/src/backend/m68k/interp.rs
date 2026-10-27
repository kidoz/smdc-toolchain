//! Deterministic M68k interpreter for semantic ROM comparison.
//!
//! Executes the decoded instruction stream (shared `decode_at` seam) against
//! a synthetic Mega Drive memory model: work RAM at 0xFF0000, logged VDP
//! writes, deterministic IO ports, and a level-6 interrupt in the last tenth
//! of every virtual frame. Because the SAME harness runs both ROMs, absolute
//! hardware fidelity matters less than determinism: equal traces mean equal
//! behavior. This is the oracle for the C-recompile milestone ladder.

use super::disasm::decode_at;
use super::m68k::*;
use std::collections::BTreeMap;
use std::collections::HashMap;

/// Virtual time and run length.
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// How many virtual frames (VBlank interrupts) to execute.
    pub frames: u32,
    /// Instructions per virtual frame.
    pub frame_instructions: u32,
}

impl Default for RunConfig {
    fn default() -> Self {
        RunConfig {
            frames: 6,
            frame_instructions: 400_000,
        }
    }
}

/// Observable outcome of a run.
#[derive(Debug, Clone)]
pub struct RunTrace {
    /// Final 64 KB work RAM.
    pub ram: Vec<u8>,
    /// VDP write sequence: (port, value). Timing excluded on purpose.
    pub vdp_writes: Vec<(u32, u32)>,
    /// Instructions executed before termination.
    pub instructions: u64,
    /// Lowest address the stack pointer reached. RAM above this is stale
    /// stack residue and is excluded from comparisons.
    pub stack_floor: u32,
    /// First execution error (unknown instruction, runaway, etc.).
    pub error: Option<String>,
}

/// Run a ROM image deterministically.
pub fn run_rom(rom: &[u8], cfg: &RunConfig) -> RunTrace {
    let mut cpu = Cpu::new(rom, cfg.frame_instructions);
    cpu.run(cfg.frames);
    RunTrace {
        ram: cpu.ram,
        vdp_writes: cpu.vdp_writes,
        instructions: cpu.instr_count,
        stack_floor: cpu.min_sp & 0x00FF_FFFF,
        error: cpu.error,
    }
}

/// Compare two traces: both must be error-free, with equal final RAM and
/// equal VDP write sequences.
pub fn compare_traces(a: &RunTrace, b: &RunTrace) -> Result<(), String> {
    if let Some(e) = &a.error {
        return Err(format!(
            "trace A errored after {} instructions: {e}",
            a.instructions
        ));
    }
    if let Some(e) = &b.error {
        return Err(format!(
            "trace B errored after {} instructions: {e}",
            b.instructions
        ));
    }
    if a.ram != b.ram {
        // Stale stack residue (from differing call depths) above the
        // deepest stack either run touched is not semantic state.
        let floor = a.stack_floor.min(b.stack_floor);
        let mut first: Option<usize> = None;
        let mut count = 0usize;
        for (i, (x, y)) in a.ram.iter().zip(b.ram.iter()).enumerate() {
            if x != y && (0xFF0000 + i as u32) < floor {
                count += 1;
                if first.is_none() {
                    first = Some(i);
                }
            }
        }
        if let Some(off) = first {
            return Err(format!(
                "work RAM differs at 0x{:06X}: {:02X} vs {:02X} \
                 (stack floor 0x{floor:06X}, {count} differing bytes below it)",
                0xFF0000 + off,
                a.ram[off],
                b.ram[off]
            ));
        }
    }
    if a.vdp_writes != b.vdp_writes {
        let (i, (pa, va), (pb, vb)) = a
            .vdp_writes
            .iter()
            .zip(b.vdp_writes.iter())
            .enumerate()
            .find(|(_, (x, y))| x != y)
            .map(|(i, (x, y))| (i, *x, *y))
            .unwrap_or((
                a.vdp_writes.len().min(b.vdp_writes.len()),
                (0, 0),
                (0, 0),
            ));
        return Err(format!(
            "VDP write sequences differ at index {i}: ({pa:06X},{va:X}) vs ({pb:06X},{vb:X}) \
             (lengths {} vs {})",
            a.vdp_writes.len(),
            b.vdp_writes.len()
        ));
    }
    Ok(())
}

const RAM_BASE: u32 = 0xFF0000;
const RAM_SIZE: usize = 0x10000;
const CODE_BASE: u32 = 0x200;

struct Cpu<'a> {
    rom: &'a [u8],
    ram: Vec<u8>,
    d: [u32; 8],
    a: [u32; 8],
    usp: u32,
    pc: u32,
    x: bool,
    n: bool,
    z: bool,
    v: bool,
    c: bool,
    imask: u8,
    halted: bool,
    /// Mirrors of controller ctrl-port writes ($A10009/0B/0D).
    ctrl_latch: [u8; 3],
    cache: BTreeMap<u32, (M68kInst, usize, Option<u32>)>,
    vdp_writes: Vec<(u32, u32)>,
    instr_count: u64,
    /// Lowest SP seen (24-bit); RAM above it is stack residue.
    min_sp: u32,
    frame_instructions: u32,
    next_irq_at: u64,
    pending_irq: bool,
    error: Option<String>,
}

impl<'a> Cpu<'a> {
    fn new(rom: &'a [u8], frame_instructions: u32) -> Self {
        let read32 = |off: usize| -> u32 {
            rom.get(off..off + 4)
                .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
                .unwrap_or(0)
        };
        let sp = read32(0);
        let pc = read32(4);
        let mut cpu = Cpu {
            rom,
            ram: vec![0; RAM_SIZE],
            d: [0; 8],
            a: [0; 8],
            usp: 0,
            pc,
            x: false,
            n: false,
            z: false,
            v: false,
            c: false,
            imask: 7,
            halted: false,
            ctrl_latch: [0; 3],
            cache: BTreeMap::new(),
            vdp_writes: Vec::new(),
            instr_count: 0,
            min_sp: sp & 0x00FF_FFFF,
            frame_instructions,
            // Interrupts fire at the start of each frame's VBlank window
            // (the last tenth of the frame), matching the status read.
            next_irq_at: (frame_instructions as u64 * 9 / 10).max(1),
            pending_irq: false,
            error: None,
        };
        cpu.a[7] = sp;
        cpu
    }

    fn run(&mut self, frames: u32) {
        let total = frames as u64 * self.frame_instructions as u64;
        while self.instr_count < total && self.error.is_none() {
            self.service_interrupts();
            if self.halted {
                self.instr_count += 1;
                continue;
            }
            self.step();
            let sp24 = self.a[7] & 0x00FF_FFFF;
            if sp24 < self.min_sp {
                self.min_sp = sp24;
            }
        }
    }

    fn service_interrupts(&mut self) {
        if self.instr_count >= self.next_irq_at {
            self.pending_irq = true;
            self.next_irq_at += self.frame_instructions as u64;
        }
        if self.pending_irq && self.imask < 6 {
            // Autovector level 6 (VBlank): vector slot 0x78.
            let handler = self.vector(0x78 / 4);
            let sr = self.sr();
            self.push32(self.pc);
            self.push16(sr);
            self.imask = 7;
            self.pc = handler;
            self.pending_irq = false;
            self.halted = false;
        }
    }

    fn vector(&self, idx: usize) -> u32 {
        let off = idx * 4;
        self.rom
            .get(off..off + 4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .unwrap_or(0)
    }

    fn sr(&self) -> u16 {
        let ccr = (self.x as u16) << 4
            | (self.n as u16) << 3
            | (self.z as u16) << 2
            | (self.v as u16) << 1
            | self.c as u16;
        ((self.imask as u16) << 8) | ccr
    }

    fn set_sr(&mut self, value: u16) {
        self.imask = ((value >> 8) & 7) as u8;
        self.x = value & 0x10 != 0;
        self.n = value & 0x08 != 0;
        self.z = value & 0x04 != 0;
        self.v = value & 0x02 != 0;
        self.c = value & 0x01 != 0;
    }

    fn set_ccr(&mut self, value: u16) {
        let sr = self.sr() & 0xFF00 | value & 0x1F;
        self.set_sr(sr);
    }

    // -- memory --------------------------------------------------------------

    fn read8(&self, addr: u32) -> u8 {
        let addr = addr & 0x00FF_FFFF;
        let rom_addr = addr as usize;
        if rom_addr < self.rom.len() {
            return self.rom[rom_addr];
        }
        match addr {
            RAM_BASE..=0xFFFFFF => self.ram[(addr - RAM_BASE) as usize],
            0xA10001 => 0x01,                       // domestic, no TMSS
            0xA10003 | 0xA10005 | 0xA10007 => 0x3F, // no buttons pressed
            0xA10009 => self.ctrl_latch[0],
            0xA1000B => self.ctrl_latch[1],
            0xA1000D => self.ctrl_latch[2],
            0xA11100 => 0,                          // Z80 bus granted
            0xC00004 => {
                // VBlank status window: the last tenth of each frame. The
                // status bits live in the high byte of the word bus.
                let phase = self.instr_count % self.frame_instructions as u64;
                let vblank = phase * 10 >= self.frame_instructions as u64;
                if vblank {
                    0x20
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    fn read16(&self, addr: u32) -> u16 {
        (self.read8(addr) as u16) << 8 | self.read8(addr.wrapping_add(1)) as u16
    }

    fn read32(&self, addr: u32) -> u32 {
        (self.read16(addr) as u32) << 16 | self.read16(addr.wrapping_add(2)) as u32
    }

    fn write8(&mut self, addr: u32, value: u8) {
        let addr = addr & 0x00FF_FFFF;
        if (RAM_BASE..=0xFFFFFF).contains(&addr) {
            if let Ok(watch) = std::env::var("SMDC_WATCH") {
                if let Ok(w) = u32::from_str_radix(watch.trim_start_matches("0x"), 16) {
                    if addr & 0xFFFFFC == w & 0xFFFFFC || addr == w {
                        eprintln!(
                            "[{:>8}] write {:02X} to {:06X} from pc={:06X}",
                            self.instr_count, value, addr, self.pc
                        );
                    }
                }
            }
            self.ram[(addr - RAM_BASE) as usize] = value;
            return;
        }
        match addr {
            0xA10009 => self.ctrl_latch[0] = value,
            0xA1000B => self.ctrl_latch[1] = value,
            0xA1000D => self.ctrl_latch[2] = value,
            _ => {} // VDP word accesses go through write16; rest ignored
        }
    }

    fn write16(&mut self, addr: u32, value: u16) {
        let addr = addr & 0x00FF_FFFF;
        match addr {
            0xC00000 | 0xC00002 | 0xC00004 | 0xC00006 => {
                self.vdp_writes.push((addr, value as u32));
            }
            0xC00011 => {} // PSG: ignored
            _ => {
                self.write8(addr, (value >> 8) as u8);
                self.write8(addr.wrapping_add(1), value as u8);
            }
        }
    }

    fn write32(&mut self, addr: u32, value: u32) {
        match addr & 0x00FF_FFFF {
            0xC00000 | 0xC00004 => {
                // Long VDP access: two word write cycles, high word first.
                self.write16(addr, (value >> 16) as u16);
                self.write16(addr, value as u16);
            }
            _ => {
                self.write16(addr, (value >> 16) as u16);
                self.write16(addr.wrapping_add(2), value as u16);
            }
        }
    }

    fn push16(&mut self, value: u16) {
        self.a[7] = self.a[7].wrapping_sub(2);
        self.write16(self.a[7], value);
    }

    fn push32(&mut self, value: u32) {
        self.a[7] = self.a[7].wrapping_sub(4);
        self.write32(self.a[7], value);
    }

    fn pop16(&mut self) -> u16 {
        let v = self.read16(self.a[7]);
        self.a[7] = self.a[7].wrapping_add(2);
        v
    }

    fn pop32(&mut self) -> u32 {
        let v = self.read32(self.a[7]);
        self.a[7] = self.a[7].wrapping_add(4);
        v
    }

    // -- decode cache ---------------------------------------------------------

    fn decode_cached(&mut self, addr: u32) -> Option<(M68kInst, usize, Option<u32>)> {
        if let Some(hit) = self.cache.get(&addr) {
            return Some(hit.clone());
        }
        let off = addr.checked_sub(CODE_BASE)? as usize;
        let empty: HashMap<u32, String> = HashMap::new();
        let code = self.rom.get(CODE_BASE as usize..)?;
        let decoded = decode_at(code, off, CODE_BASE, &empty)?;
        let entry = (decoded.0, decoded.1, decoded.2);
        self.cache.insert(addr, entry.clone());
        Some(entry)
    }

    // -- execution -------------------------------------------------------------

    fn step(&mut self) {
        let addr = self.pc;
        if std::env::var("SMDC_TRACE").is_ok() {
            eprintln!("[{:>8}] ${addr:06X}", self.instr_count);
        }
        let Some((inst, next, target)) = self.decode_cached(addr) else {
            let bytes = self.read32(addr);
            self.error = Some(format!(
                "undecodable instruction at ${addr:06X} (${bytes:08X})"
            ));
            return;
        };
        self.pc = CODE_BASE + next as u32;
        self.execute(&inst, addr, target);
        self.instr_count += 1;
    }

    fn err(&mut self, addr: u32, what: &str) {
        if self.error.is_none() {
            self.error = Some(format!("{what} at ${addr:06X}"));
        }
    }

    fn execute(&mut self, inst: &M68kInst, addr: u32, target: Option<u32>) {
        match inst {
            M68kInst::Label(_) | M68kInst::Comment(_) | M68kInst::Directive(_) => {}
            M68kInst::Nop => {}
            M68kInst::Reset => {}
            M68kInst::Illegal => self.exception(self.vector(4)),
            M68kInst::Trap(n) => self.exception(self.vector(32 + *n as usize)),
            M68kInst::Trapv => {
                if self.v {
                    self.exception(self.vector(7));
                }
            }
            M68kInst::Stop(v) => {
                self.set_sr(*v);
                self.halted = true;
            }
            M68kInst::Rts => {
                if std::env::var("SMDC_SPY").is_ok() {
                    eprintln!(
                        "[{:>8}] rts  sp={:08X} -> [sp]={:08X}",
                        self.instr_count,
                        self.a[7],
                        self.read32(self.a[7])
                    );
                }
                self.pc = self.pop32();
            }
            M68kInst::Rtr => {
                let ccr = self.pop16();
                self.pc = self.pop32();
                self.set_ccr(ccr);
            }
            M68kInst::Rte => {
                let sr = self.pop16();
                self.pc = self.pop32();
                self.set_sr(sr);
            }

            // Branches and jumps.
            M68kInst::BraS(_) | M68kInst::Bra(_) => {
                if let Some(t) = target {
                    self.pc = t;
                }
            }
            M68kInst::BccS(c, _) | M68kInst::Bcc(c, _) => {
                if self.cond(*c) && let Some(t) = target {
                    self.pc = t;
                }
            }
            M68kInst::BsrS(_) | M68kInst::Bsr(_) => {
                self.push32(self.pc);
                if let Some(t) = target {
                    self.pc = t;
                }
            }
            M68kInst::Dbcc(c, reg, _) => {
                if !self.cond(*c) {
                    let i = *reg as usize;
                    let new = ((self.d[i] & 0xFFFF) as u16).wrapping_sub(1);
                    self.d[i] = self.d[i] & !0xFFFF | new as u32;
                    if new != 0xFFFF && let Some(t) = target {
                        self.pc = t;
                    }
                }
            }
            M68kInst::Jmp(op) => {
                if let Some(t) = target {
                    self.pc = t;
                } else if let Some(ea) = self.ea_addr(op, Size::Long, addr) {
                    self.pc = ea;
                }
            }
            M68kInst::Jsr(op) => {
                if std::env::var("SMDC_SPY").is_ok() {
                    eprintln!(
                        "[{:>8}] jsr ${:06X}  sp={:08X} ret={:08X}",
                        self.instr_count,
                        target.unwrap_or(0),
                        self.a[7],
                        self.pc
                    );
                }
                if let Some(dest) = target.or_else(|| self.ea_addr(op, Size::Long, addr)) {
                    self.push32(self.pc);
                    self.pc = dest;
                }
            }

            // Moves.
            M68kInst::Move(size, src, dst) => {
                let value = self.ea_read(src, *size, addr);
                if matches!(dst, Operand::Sr) {
                    self.set_sr(value as u16);
                } else {
                    self.move_write(dst, *size, value, addr);
                    self.set_nz(value, *size);
                    self.v = false;
                    self.c = false;
                }
            }
            M68kInst::Moveq(v, reg) => {
                self.d[*reg as usize] = *v as i32 as u32;
                self.set_nz(self.d[*reg as usize], Size::Long);
                self.v = false;
                self.c = false;
            }
            M68kInst::Lea(src, reg) => {
                if let Some(ea) = self.ea_addr(src, Size::Long, addr) {
                    self.a[*reg as usize] = ea;
                }
            }
            M68kInst::Pea(op) => {
                if let Some(ea) = self.ea_addr(op, Size::Long, addr) {
                    self.push32(ea);
                }
            }
            M68kInst::Clr(size, op) => {
                self.ea_write(op, *size, 0, addr);
                self.n = false;
                self.z = true;
                self.v = false;
                self.c = false;
            }
            M68kInst::Exg(r1, r2) => {
                let (x, y) = (self.reg_get(r1), self.reg_get(r2));
                self.reg_set(r1, y);
                self.reg_set(r2, x);
            }
            M68kInst::Swap(r) => {
                let v = self.d[*r as usize];
                self.d[*r as usize] = v.rotate_left(16);
                self.set_nz(self.d[*r as usize], Size::Long);
                self.v = false;
                self.c = false;
            }
            M68kInst::MoveToCcr(op) => {
                let v = self.ea_read(op, Size::Word, addr) as u16;
                self.set_ccr(v);
            }
            M68kInst::OriToCcr(v) => self.set_ccr(self.sr() | v),
            M68kInst::AndiToCcr(v) => self.set_ccr(self.sr() & v),
            M68kInst::EoriToCcr(v) => self.set_ccr(self.sr() ^ v),
            M68kInst::OriToSr(v) => self.set_sr(self.sr() | v),
            M68kInst::AndiToSr(v) => self.set_sr(self.sr() & v),
            M68kInst::EoriToSr(v) => self.set_sr(self.sr() ^ v),
            M68kInst::MoveUspToAddr(r) => self.a[*r as usize] = self.usp,
            M68kInst::MoveAddrToUsp(r) => self.usp = self.a[*r as usize],
            M68kInst::Movep(size, to_mem, disp, an, dn) => {
                let base = self.a[*an as usize].wrapping_add_signed(*disp as i32);
                let v = self.d[*dn as usize];
                match (*size, *to_mem) {
                    (Size::Word, true) => {
                        self.write8(base, (v >> 8) as u8);
                        self.write8(base.wrapping_add(2), v as u8);
                    }
                    (Size::Long, true) => {
                        for (i, shift) in [24u32, 16, 8, 0].into_iter().enumerate() {
                            self.write8(base.wrapping_add(2 * i as u32), (v >> shift) as u8);
                        }
                    }
                    (Size::Word, false) => {
                        let hi = self.read8(base) as u32;
                        let lo = self.read8(base.wrapping_add(2)) as u32;
                        self.d[*dn as usize] = self.d[*dn as usize] & !0xFFFF | hi << 8 | lo;
                    }
                    (Size::Long, false) => {
                        let mut v = 0u32;
                        for i in 0..4 {
                            v = v << 8 | self.read8(base.wrapping_add(2 * i)) as u32;
                        }
                        self.d[*dn as usize] = v;
                    }
                    (Size::Byte, _) => {} // not producible by the decoder
                }
            }

            // Arithmetic.
            M68kInst::Add(size, src, dst) => {
                let a = self.ea_read(src, *size, addr);
                let b = self.ea_read(dst, *size, addr);
                let (r, carry, overflow) = calc_add(a, b, *size);
                self.set_arith_flags(r, carry, overflow, *size);
                self.ea_write(dst, *size, r, addr);
            }
            M68kInst::Adda(_, src, dst) => {
                let v = sign_extend_word(self.ea_read(src, Size::Word, addr));
                self.a[*dst as usize] = self.a[*dst as usize].wrapping_add(v);
            }
            M68kInst::Addq(size, n, op) => {
                if let Operand::AddrReg(r) = op {
                    self.a[*r as usize] = self.a[*r as usize].wrapping_add(*n as u32);
                } else {
                    let b = self.ea_read(op, *size, addr);
                    let (r, carry, overflow) = calc_add(b, *n as u32, *size);
                    self.set_arith_flags(r, carry, overflow, *size);
                    self.ea_write(op, *size, r, addr);
                }
            }
            M68kInst::Addi(size, v, op) => {
                let b = self.ea_read(op, *size, addr);
                let (r, carry, overflow) = calc_add(b, *v as u32, *size);
                self.set_arith_flags(r, carry, overflow, *size);
                self.ea_write(op, *size, r, addr);
            }
            M68kInst::Sub(size, src, dst) => {
                let a = self.ea_read(src, *size, addr);
                let b = self.ea_read(dst, *size, addr);
                let (r, borrow, overflow) = calc_sub(b, a, *size);
                self.set_arith_flags(r, borrow, overflow, *size);
                self.ea_write(dst, *size, r, addr);
            }
            M68kInst::Suba(_, src, dst) => {
                let v = sign_extend_word(self.ea_read(src, Size::Word, addr));
                self.a[*dst as usize] = self.a[*dst as usize].wrapping_sub(v);
            }
            M68kInst::Subq(size, n, op) => {
                if let Operand::AddrReg(r) = op {
                    self.a[*r as usize] = self.a[*r as usize].wrapping_sub(*n as u32);
                } else {
                    let b = self.ea_read(op, *size, addr);
                    let (r, borrow, overflow) = calc_sub(b, *n as u32, *size);
                    self.set_arith_flags(r, borrow, overflow, *size);
                    self.ea_write(op, *size, r, addr);
                }
            }
            M68kInst::Subi(size, v, op) => {
                let b = self.ea_read(op, *size, addr);
                let (r, borrow, overflow) = calc_sub(b, *v as u32, *size);
                self.set_arith_flags(r, borrow, overflow, *size);
                self.ea_write(op, *size, r, addr);
            }
            M68kInst::Neg(size, op) => {
                let b = self.ea_read(op, *size, addr);
                let (r, borrow, overflow) = calc_sub(0, b, *size);
                self.set_arith_flags(r, borrow, overflow, *size);
                self.ea_write(op, *size, r, addr);
            }
            M68kInst::Negx(size, op) => {
                let b = self.ea_read(op, *size, addr);
                let xin = self.x as u32;
                let (r, borrow, overflow) = calc_sub(calc_sub(0, b, *size).0, xin, *size);
                self.set_arith_flags(r, borrow, overflow, *size);
                if r != 0 {
                    self.z = false;
                }
                self.ea_write(op, *size, r, addr);
            }
            M68kInst::Addx(size, src, dst) => {
                let a = self.ea_read(src, *size, addr);
                let b = self.ea_read(dst, *size, addr);
                let xin = self.x as u32;
                let (s, c1, o1) = calc_add(a, b, *size);
                let (r, c2, o2) = calc_add(s, xin, *size);
                self.set_arith_flags(r, c1 || c2, o1 || o2, *size);
                if r != 0 {
                    self.z = false;
                }
                self.ea_write(dst, *size, r, addr);
            }
            M68kInst::Subx(size, src, dst) => {
                let a = self.ea_read(src, *size, addr);
                let b = self.ea_read(dst, *size, addr);
                let xin = self.x as u32;
                let (s, b1, o1) = calc_sub(b, a, *size);
                let (r, b2, o2) = calc_sub(s, xin, *size);
                self.set_arith_flags(r, b1 || b2, o1 || o2, *size);
                if r != 0 {
                    self.z = false;
                }
                self.ea_write(dst, *size, r, addr);
            }
            M68kInst::Abcd(src, dst) => self.bcd_op(true, src, dst, addr),
            M68kInst::Sbcd(src, dst) => self.bcd_op(false, src, dst, addr),
            M68kInst::Nbcd(op) => {
                let b = self.ea_read(op, Size::Byte, addr) as u8;
                let (r, borrow) = bcd_sub_byte(0x00, b, false);
                if r != 0 {
                    self.z = false;
                }
                self.c = borrow;
                self.x = borrow;
                self.ea_write(op, Size::Byte, r as u32, addr);
            }
            M68kInst::Cmpm(size, src, dst) => {
                let ay = self.a[*src as usize].wrapping_add(size_bytes(*size));
                let ax = self.a[*dst as usize].wrapping_add(size_bytes(*size));
                let a = self.read_sized(self.a[*src as usize], *size);
                let b = self.read_sized(self.a[*dst as usize], *size);
                self.a[*src as usize] = ay;
                self.a[*dst as usize] = ax;
                let (r, borrow, overflow) = calc_sub(b, a, *size);
                self.set_cmp_flags(r, borrow, overflow, *size);
            }

            // Multiply / divide.
            M68kInst::Muls(src, dst) => {
                let v = self.ea_read(src, Size::Word, addr) as u16 as i16 as i32;
                let dn = (self.d[*dst as usize] & 0xFFFF) as u16 as i16 as i32;
                let r = (v.wrapping_mul(dn)) as u32;
                self.d[*dst as usize] = r;
                self.set_nz(r, Size::Long);
                self.v = false;
                self.c = false;
            }
            M68kInst::Mulu(src, dst) => {
                let v = self.ea_read(src, Size::Word, addr) & 0xFFFF;
                let dn = self.d[*dst as usize] & 0xFFFF;
                let r = v.wrapping_mul(dn);
                self.d[*dst as usize] = r;
                self.set_nz(r, Size::Long);
                self.v = false;
                self.c = false;
            }
            M68kInst::Divs(src, dst) => self.do_div(src, *dst, addr, true),
            M68kInst::Divu(src, dst) => self.do_div(src, *dst, addr, false),

            M68kInst::Ext(size, reg) => {
                let v = self.d[*reg as usize];
                self.d[*reg as usize] = match size {
                    Size::Word => (v & 0xFF) as u8 as i8 as i32 as u32,
                    _ => (v & 0xFFFF) as u16 as i16 as i32 as u32,
                };
                self.set_nz(self.d[*reg as usize], Size::Long);
                self.v = false;
                self.c = false;
            }

            // Logic.
            M68kInst::And(size, src, dst) => {
                let a = self.ea_read(src, *size, addr);
                let b = self.ea_read(dst, *size, addr);
                let r = a & b;
                self.set_logic_flags(r, *size);
                self.ea_write(dst, *size, r, addr);
            }
            M68kInst::Andi(size, v, op) => {
                let b = self.ea_read(op, *size, addr);
                let r = b & *v as u32;
                self.set_logic_flags(r, *size);
                self.ea_write(op, *size, r, addr);
            }
            M68kInst::Or(size, src, dst) => {
                let a = self.ea_read(src, *size, addr);
                let b = self.ea_read(dst, *size, addr);
                let r = a | b;
                self.set_logic_flags(r, *size);
                self.ea_write(dst, *size, r, addr);
            }
            M68kInst::Ori(size, v, op) => {
                let b = self.ea_read(op, *size, addr);
                let r = b | *v as u32;
                self.set_logic_flags(r, *size);
                self.ea_write(op, *size, r, addr);
            }
            M68kInst::Eor(size, src, dst) => {
                let dn = self.d[*src as usize];
                let b = self.ea_read(dst, *size, addr);
                let r = b ^ dn;
                self.set_logic_flags(r, *size);
                self.ea_write(dst, *size, r, addr);
            }
            M68kInst::Eori(size, v, op) => {
                let b = self.ea_read(op, *size, addr);
                let r = b ^ *v as u32;
                self.set_logic_flags(r, *size);
                self.ea_write(op, *size, r, addr);
            }
            M68kInst::Not(size, op) => {
                let b = self.ea_read(op, *size, addr);
                let r = !b & mask(*size);
                self.set_logic_flags(r, *size);
                self.ea_write(op, *size, r, addr);
            }
            M68kInst::Tst(size, op) => {
                let v = self.ea_read(op, *size, addr);
                self.set_nz(v, *size);
                self.v = false;
                self.c = false;
            }
            M68kInst::Cmp(size, src, dst) => {
                let a = self.ea_read(src, *size, addr);
                let b = self.ea_read(dst, *size, addr);
                let (r, borrow, overflow) = calc_sub(b, a, *size);
                self.set_cmp_flags(r, borrow, overflow, *size);
            }
            M68kInst::Cmpa(_, src, dst) => {
                let a = sign_extend_word(self.ea_read(src, Size::Word, addr));
                let b = self.a[*dst as usize];
                let (r, borrow, overflow) = calc_sub(b, a, Size::Long);
                self.set_cmp_flags(r, borrow, overflow, Size::Long);
            }
            M68kInst::Cmpi(size, v, op) => {
                let b = self.ea_read(op, *size, addr);
                let (r, borrow, overflow) = calc_sub(b, *v as u32, *size);
                self.set_cmp_flags(r, borrow, overflow, *size);
            }
            M68kInst::Chk(src, dst) => {
                let a = self.ea_read(src, Size::Word, addr) as u16 as i16;
                let b = (self.d[*dst as usize] & 0xFFFF) as u16 as i16;
                if a < 0 || a > b {
                    self.n = a < 0;
                    self.exception(self.vector(6));
                }
            }

            // Shifts and rotates.
            M68kInst::Lsl(size, cnt, reg) => self.shift(ShiftOp::Lsl, *size, cnt, *reg),
            M68kInst::Lsr(size, cnt, reg) => self.shift(ShiftOp::Lsr, *size, cnt, *reg),
            M68kInst::Asl(size, cnt, reg) => self.shift(ShiftOp::Asl, *size, cnt, *reg),
            M68kInst::Asr(size, cnt, reg) => self.shift(ShiftOp::Asr, *size, cnt, *reg),
            M68kInst::Rol(size, cnt, reg) => self.shift(ShiftOp::Rol, *size, cnt, *reg),
            M68kInst::Ror(size, cnt, reg) => self.shift(ShiftOp::Ror, *size, cnt, *reg),
            M68kInst::Roxl(size, cnt, reg) => self.shift(ShiftOp::Roxl, *size, cnt, *reg),
            M68kInst::Roxr(size, cnt, reg) => self.shift(ShiftOp::Roxr, *size, cnt, *reg),
            M68kInst::AslMem(op) => self.shift_mem(ShiftOp::Asl, op, addr),
            M68kInst::AsrMem(op) => self.shift_mem(ShiftOp::Asr, op, addr),
            M68kInst::LslMem(op) => self.shift_mem(ShiftOp::Lsl, op, addr),
            M68kInst::LsrMem(op) => self.shift_mem(ShiftOp::Lsr, op, addr),
            M68kInst::RolMem(op) => self.shift_mem(ShiftOp::Rol, op, addr),
            M68kInst::RorMem(op) => self.shift_mem(ShiftOp::Ror, op, addr),
            M68kInst::RoxlMem(op) => self.shift_mem(ShiftOp::Roxl, op, addr),
            M68kInst::RoxrMem(op) => self.shift_mem(ShiftOp::Roxr, op, addr),

            // Bits.
            M68kInst::Btst(bit, op) => {
                let (n, size) = self.bit_target(bit, op, addr);
                let v = self.ea_read(op, size, addr);
                self.z = v >> (n % bits_of(size)) & 1 == 0;
            }
            M68kInst::Bset(bit, op) => {
                let (n, size) = self.bit_target(bit, op, addr);
                let v = self.ea_read(op, size, addr);
                self.z = v >> (n % bits_of(size)) & 1 == 0;
                self.ea_write(op, size, v | 1 << (n % bits_of(size)), addr);
            }
            M68kInst::Bclr(bit, op) => {
                let (n, size) = self.bit_target(bit, op, addr);
                let v = self.ea_read(op, size, addr);
                self.z = v >> (n % bits_of(size)) & 1 == 0;
                self.ea_write(op, size, v & !(1 << (n % bits_of(size))), addr);
            }
            M68kInst::Bchg(bit, op) => {
                let (n, size) = self.bit_target(bit, op, addr);
                let v = self.ea_read(op, size, addr);
                self.z = v >> (n % bits_of(size)) & 1 == 0;
                self.ea_write(op, size, v ^ 1 << (n % bits_of(size)), addr);
            }

            // Stack and block moves.
            M68kInst::Link(r, disp) => {
                // LINK An: push An; An = SP; SP += disp (disp <= 0).
                let old = self.a[*r as usize];
                self.push32(old);
                self.a[*r as usize] = self.a[7];
                self.a[7] = self.a[7].wrapping_add_signed(*disp as i32);
            }
            M68kInst::Unlk(r) => {
                self.a[7] = self.a[*r as usize];
                self.a[*r as usize] = self.pop32();
            }
            M68kInst::Movem(size, regs, op, to_mem) => self.movem(*size, regs, op, *to_mem, addr),
            M68kInst::Scc(c, op) => {
                let v = if self.cond(*c) { 0xFF } else { 0 };
                self.ea_write(op, Size::Byte, v, addr);
            }
            M68kInst::Tas(op) => {
                let v = self.ea_read(op, Size::Byte, addr);
                self.n = v & 0x80 != 0;
                self.z = v == 0;
                self.ea_write(op, Size::Byte, v | 0x80, addr);
            }

            other => {
                let msg = format!("interpreter does not implement {other:?}");
                self.err(addr, &msg);
            }
        }
    }

    fn exception(&mut self, handler: u32) {
        let sr = self.sr();
        self.push32(self.pc);
        self.push16(sr);
        self.imask = 7;
        self.pc = handler;
    }

    // -- helpers over operands -------------------------------------------------

    fn reg_get(&self, r: &Reg) -> u32 {
        match r {
            Reg::Data(d) => self.d[*d as usize],
            Reg::Addr(a) => self.a[*a as usize],
        }
    }

    fn reg_set(&mut self, r: &Reg, v: u32) {
        match r {
            Reg::Data(d) => self.d[*d as usize] = v,
            Reg::Addr(a) => self.a[*a as usize] = v,
        }
    }

    /// Effective value of a brief-format index register: word indexes are
    /// sign-extended, then the scale (1/2/4/8) is applied.
    fn index_value(&self, r: &Reg, hi: u8) -> u32 {
        let raw = self.reg_get(r);
        let v = if hi & 0x08 != 0 {
            raw
        } else {
            (raw & 0xFFFF) as u16 as i16 as i32 as u32
        };
        v << ((hi >> 1) & 3)
    }

    /// Effective address for the memory modes. Predecrement applies its side
    /// effect at the given access size; None for non-addressable operands.
    fn ea_addr(&mut self, op: &Operand, size: Size, inst_addr: u32) -> Option<u32> {
        Some(match op {
            Operand::AddrInd(r) => self.a[*r as usize],
            Operand::PreDec(r) => {
                let v = self.a[*r as usize].wrapping_sub(size_bytes(size));
                self.a[*r as usize] = v;
                v
            }
            Operand::Disp(d, r) => self.a[*r as usize].wrapping_add_signed(*d as i32),
            Operand::Indexed(d, r, i, hi) => self
                .a[*r as usize]
                .wrapping_add_signed(*d as i32)
                .wrapping_add(self.index_value(i, *hi)),
            Operand::PcDisp(d) => inst_addr.wrapping_add(4).wrapping_add_signed(*d as i32),
            Operand::PcIndexed(d, i, hi) => inst_addr
                .wrapping_add(4)
                .wrapping_add_signed(*d as i32)
                .wrapping_add(self.index_value(i, *hi)),
            Operand::AbsShort(a) => (*a as i16 as i32) as u32,
            Operand::AbsLong(a) => *a,
            Operand::Label(name) => name.strip_prefix('$')?.parse().ok()?,
            _ => return None,
        })
    }

    fn ea_read(&mut self, op: &Operand, size: Size, inst_addr: u32) -> u32 {
        match op {
            Operand::DataReg(r) => self.d[*r as usize] & mask(size),
            Operand::AddrReg(r) => self.a[*r as usize],
            Operand::Imm(v) => *v as u32 & mask(size),
            Operand::Sr => self.sr() as u32,
            Operand::PostInc(r) => {
                let a = self.a[*r as usize];
                let v = self.read_sized(a, size);
                self.a[*r as usize] = a.wrapping_add(size_bytes(size));
                v
            }
            _ => {
                let a = self.ea_addr(op, size, inst_addr).unwrap_or(0);
                self.read_sized(a, size)
            }
        }
    }

    fn ea_write(&mut self, op: &Operand, size: Size, value: u32, inst_addr: u32) {
        match op {
            Operand::DataReg(r) => {
                let i = *r as usize;
                self.d[i] = match size {
                    Size::Byte => self.d[i] & !0xFF | value & 0xFF,
                    Size::Word => self.d[i] & !0xFFFF | value & 0xFFFF,
                    Size::Long => value,
                };
            }
            Operand::AddrReg(r) => self.a[*r as usize] = value,
            Operand::Sr => self.set_sr(value as u16),
            Operand::PostInc(r) => {
                let a = self.a[*r as usize];
                self.write_sized(a, size, value);
                self.a[*r as usize] = a.wrapping_add(size_bytes(size));
            }
            _ => {
                if let Some(a) = self.ea_addr(op, size, inst_addr) {
                    self.write_sized(a, size, value);
                }
            }
        }
    }

    /// MOVE destinations: word moves into data registers zero-extend.
    fn move_write(&mut self, op: &Operand, size: Size, value: u32, inst_addr: u32) {
        if let Operand::DataReg(r) = op {
            let i = *r as usize;
            self.d[i] = match size {
                Size::Byte => self.d[i] & !0xFF | value & 0xFF,
                Size::Word => value & 0xFFFF,
                Size::Long => value,
            };
            return;
        }
        self.ea_write(op, size, value, inst_addr);
    }

    fn read_sized(&self, addr: u32, size: Size) -> u32 {
        match size {
            Size::Byte => self.read8(addr) as u32,
            Size::Word => self.read16(addr) as u32,
            Size::Long => self.read32(addr),
        }
    }

    fn write_sized(&mut self, addr: u32, size: Size, value: u32) {
        match size {
            Size::Byte => self.write8(addr, value as u8),
            Size::Word => self.write16(addr, value as u16),
            Size::Long => self.write32(addr, value),
        }
    }

    /// (bit number, operand size): register destinations test 32 bits,
    /// memory destinations 8.
    fn bit_target(&mut self, bit: &Operand, op: &Operand, inst_addr: u32) -> (u32, Size) {
        let n = match bit {
            Operand::Imm(n) => *n as u32 & 31,
            Operand::DataReg(r) => self.d[*r as usize] & 31,
            other => self.ea_read(other, Size::Byte, inst_addr) & 31,
        };
        let size = if matches!(op, Operand::DataReg(_)) {
            Size::Long
        } else {
            Size::Byte
        };
        (n, size)
    }

    fn movem(&mut self, size: Size, regs: &[Reg], op: &Operand, to_mem: bool, inst_addr: u32) {
        let step = size_bytes(size);
        match (to_mem, op) {
            (true, Operand::PreDec(r)) => {
                // Predecrement stores in list order, descending addresses;
                // the decoder already reversed the mask for this mode.
                let mut a = self.a[*r as usize];
                for reg in regs {
                    a = a.wrapping_sub(step);
                    let v = self.reg_get(reg);
                    self.write_sized(a, size, v);
                }
                self.a[*r as usize] = a;
            }
            (true, _) => {
                if let Some(mut a) = self.ea_addr(op, size, inst_addr) {
                    for reg in regs {
                        let v = self.reg_get(reg);
                        self.write_sized(a, size, v);
                        a = a.wrapping_add(step);
                    }
                }
            }
            (false, Operand::PostInc(r)) => {
                let mut a = self.a[*r as usize];
                for reg in regs {
                    let v = self.read_sized(a, size);
                    self.reg_set(reg, v);
                    a = a.wrapping_add(step);
                }
                self.a[*r as usize] = a;
            }
            (false, _) => {
                if let Some(mut a) = self.ea_addr(op, size, inst_addr) {
                    for reg in regs {
                        let v = self.read_sized(a, size);
                        self.reg_set(reg, v);
                        a = a.wrapping_add(step);
                    }
                }
            }
        }
    }

    fn do_div(&mut self, src: &Operand, dst: DataReg, addr: u32, signed: bool) {
        let divisor = self.ea_read(src, Size::Word, addr) & 0xFFFF;
        let dividend = self.d[dst as usize];
        if divisor == 0 {
            let v = self.vector(5);
            self.exception(v);
            return;
        }
        let (q, r, overflow) = if signed {
            let dd = dividend as i32;
            let dv = divisor as u16 as i16 as i32;
            match dd.checked_div(dv) {
                Some(q) => (q, dd - q * dv, !q.fits_in_i16()),
                None => (0, 0, true),
            }
        } else {
            let q = dividend / divisor;
            (q as i32, (dividend % divisor) as i32, q > 0xFFFF)
        };
        if overflow {
            self.v = true;
            return;
        }
        let combined = (r as u32 & 0xFFFF) << 16 | q as u32 & 0xFFFF;
        self.d[dst as usize] = combined;
        self.set_nz(q as u32 & 0xFFFF, Size::Word);
        self.v = false;
        self.c = false;
    }

    fn bcd_op(&mut self, is_add: bool, src: &Operand, dst: &Operand, addr: u32) {
        let a = self.ea_read(src, Size::Byte, addr) as u8;
        let b = self.ea_read(dst, Size::Byte, addr) as u8;
        let (r, carry) = if is_add {
            bcd_add_byte(a, b, self.x)
        } else {
            bcd_sub_byte(b, a, self.x)
        };
        if r != 0 {
            self.z = false;
        }
        self.x = carry;
        self.c = carry;
        self.ea_write(dst, Size::Byte, r as u32, addr);
    }

    fn shift(&mut self, op: ShiftOp, size: Size, count: &Operand, reg: DataReg) {
        let count = match count {
            Operand::Imm(n) => (*n as u32) & 63,
            Operand::DataReg(r) => self.d[*r as usize] & 63,
            _ => 1,
        };
        let m = mask(size);
        let bits = bits_of(size);
        let mut v = self.d[reg as usize] & m;
        for _ in 0..count {
            match op {
                ShiftOp::Lsl => {
                    self.x = v >> (bits - 1) & 1 != 0;
                    v = v << 1 & m;
                }
                ShiftOp::Lsr => {
                    self.x = v & 1 != 0;
                    v >>= 1;
                }
                ShiftOp::Asl => {
                    self.x = v >> (bits - 1) & 1 != 0;
                    let sv = sign_extend(v, size);
                    v = (sv << 1) as u32 & m;
                }
                ShiftOp::Asr => {
                    self.x = v & 1 != 0;
                    let sv = sign_extend(v, size);
                    v = (sv >> 1) as u32 & m;
                }
                ShiftOp::Rol => {
                    let top = v >> (bits - 1) & 1;
                    v = (v << 1 | top) & m;
                    self.c = top != 0;
                }
                ShiftOp::Ror => {
                    let bottom = v & 1;
                    v = v >> 1 | bottom << (bits - 1);
                    self.c = bottom != 0;
                }
                ShiftOp::Roxl => {
                    let top = v >> (bits - 1) & 1;
                    v = (v << 1 | self.x as u32) & m;
                    self.x = top != 0;
                    self.c = self.x;
                }
                ShiftOp::Roxr => {
                    let bottom = v & 1;
                    v = v >> 1 | (self.x as u32) << (bits - 1);
                    self.x = bottom != 0;
                    self.c = self.x;
                }
            }
        }
        if count > 0 {
            if !matches!(op, ShiftOp::Rol | ShiftOp::Ror | ShiftOp::Roxl | ShiftOp::Roxr) {
                self.c = self.x;
            }
            let sign_before = self.d[reg as usize] & m >> (bits - 1) & 1;
            self.set_nz(v, size);
            self.v = matches!(op, ShiftOp::Asl) && sign_before != v >> (bits - 1) & 1;
        }
        self.d[reg as usize] = match size {
            Size::Byte => self.d[reg as usize] & !0xFF | v,
            Size::Word => self.d[reg as usize] & !0xFFFF | v,
            Size::Long => v,
        };
    }

    fn shift_mem(&mut self, op: ShiftOp, ea: &Operand, addr: u32) {
        let m = 0xFFFFu32;
        let mut v = self.ea_read(ea, Size::Word, addr) & m;
        match op {
            ShiftOp::Lsl => {
                self.x = v >> 15 & 1 != 0;
                v = v << 1 & m;
            }
            ShiftOp::Lsr => {
                self.x = v & 1 != 0;
                v >>= 1;
            }
            ShiftOp::Asl => {
                self.x = v >> 15 & 1 != 0;
                v = v << 1 & m;
            }
            ShiftOp::Asr => {
                self.x = v & 1 != 0;
                let sign = v >> 15 & 1;
                v = v >> 1 | sign << 15;
            }
            ShiftOp::Rol => {
                let top = v >> 15 & 1;
                v = (v << 1 | top) & m;
                self.c = top != 0;
            }
            ShiftOp::Ror => {
                let bottom = v & 1;
                v = v >> 1 | bottom << 15;
                self.c = bottom != 0;
            }
            ShiftOp::Roxl => {
                let top = v >> 15 & 1;
                v = (v << 1 | self.x as u32) & m;
                self.x = top != 0;
                self.c = self.x;
            }
            ShiftOp::Roxr => {
                let bottom = v & 1;
                v = v >> 1 | (self.x as u32) << 15;
                self.x = bottom != 0;
                self.c = self.x;
            }
        }
        if !matches!(op, ShiftOp::Rol | ShiftOp::Ror | ShiftOp::Roxl | ShiftOp::Roxr) {
            self.c = self.x;
        }
        self.set_nz(v, Size::Word);
        self.v = false;
        self.ea_write(ea, Size::Word, v, addr);
    }

    fn cond(&self, c: Cond) -> bool {
        match c {
            Cond::True => true,
            Cond::False => false,
            Cond::Hi => !self.c && !self.z,
            Cond::Ls => self.c || self.z,
            Cond::Cc => !self.c,
            Cond::Cs => self.c,
            Cond::Ne => !self.z,
            Cond::Eq => self.z,
            Cond::Vc => !self.v,
            Cond::Vs => self.v,
            Cond::Pl => !self.n,
            Cond::Mi => self.n,
            Cond::Ge => self.n == self.v,
            Cond::Lt => self.n != self.v,
            Cond::Gt => !self.z && self.n == self.v,
            Cond::Le => self.z || self.n != self.v,
        }
    }

    fn set_nz(&mut self, value: u32, size: Size) {
        self.n = value & mask(size) & sign_bit(size) != 0;
        self.z = value & mask(size) == 0;
    }

    /// NZVC + X for add/sub-family results.
    fn set_arith_flags(&mut self, r: u32, carry_out: bool, overflow: bool, size: Size) {
        self.set_nz(r, size);
        self.c = carry_out;
        self.x = carry_out;
        self.v = overflow;
    }

    /// NZVC for compares (X untouched).
    fn set_cmp_flags(&mut self, r: u32, borrow: bool, overflow: bool, size: Size) {
        self.set_nz(r, size);
        self.c = borrow;
        self.v = overflow;
    }

    fn set_logic_flags(&mut self, r: u32, size: Size) {
        self.set_nz(r, size);
        self.v = false;
        self.c = false;
    }
}

#[derive(Clone, Copy)]
enum ShiftOp {
    Lsl,
    Lsr,
    Asl,
    Asr,
    Rol,
    Ror,
    Roxl,
    Roxr,
}

fn mask(size: Size) -> u32 {
    match size {
        Size::Byte => 0xFF,
        Size::Word => 0xFFFF,
        Size::Long => 0xFFFF_FFFF,
    }
}

fn bits_of(size: Size) -> u32 {
    match size {
        Size::Byte => 8,
        Size::Word => 16,
        Size::Long => 32,
    }
}

fn sign_bit(size: Size) -> u32 {
    match size {
        Size::Byte => 0x80,
        Size::Word => 0x8000,
        Size::Long => 0x8000_0000,
    }
}

fn size_bytes(size: Size) -> u32 {
    match size {
        Size::Byte => 1,
        Size::Word => 2,
        Size::Long => 4,
    }
}

/// Sign-extend a masked value to i32 (for arithmetic shifts).
fn sign_extend(v: u32, size: Size) -> i32 {
    match size {
        Size::Byte => (v & 0xFF) as u8 as i8 as i32,
        Size::Word => (v & 0xFFFF) as u16 as i16 as i32,
        Size::Long => v as i32,
    }
}

fn sign_extend_word(v: u32) -> u32 {
    (v & 0xFFFF) as u16 as i16 as i32 as u32
}

/// a + b: (result, carry out, overflow).
fn calc_add(a: u32, b: u32, size: Size) -> (u32, bool, bool) {
    let m = mask(size);
    let am = a & m;
    let bm = b & m;
    let full = am as u64 + bm as u64;
    let carry = full > m as u64;
    let r = (full as u32) & m;
    let sb = sign_bit(size);
    let overflow = !(am ^ bm) & (am ^ r) & sb != 0;
    (r, carry, overflow)
}

/// a - b: (result, borrow out, overflow).
fn calc_sub(a: u32, b: u32, size: Size) -> (u32, bool, bool) {
    let m = mask(size);
    let am = a & m;
    let bm = b & m;
    let full = (am as u64).wrapping_sub(bm as u64);
    let borrow = am < bm;
    let r = full as u32 & m;
    let sb = sign_bit(size);
    let overflow = (am ^ bm) & (am ^ r) & sb != 0;
    (r, borrow, overflow)
}

trait FitsInI16 {
    fn fits_in_i16(self) -> bool;
}

impl FitsInI16 for i32 {
    fn fits_in_i16(self) -> bool {
        self >= i16::MIN as i32 && self <= i16::MAX as i32
    }
}

/// BCD byte add with carry-in.
fn bcd_add_byte(a: u8, b: u8, carry_in: bool) -> (u8, bool) {
    let mut lo = (a & 0xF) + (b & 0xF) + carry_in as u8;
    let mut hi = (a >> 4) + (b >> 4);
    if lo > 9 {
        lo += 6;
        hi += 1;
    }
    let carry = hi > 9;
    if carry {
        hi += 6;
    }
    ((hi & 0xF) << 4 | lo & 0xF, carry)
}

/// BCD byte subtract with borrow-in.
fn bcd_sub_byte(a: u8, b: u8, borrow_in: bool) -> (u8, bool) {
    let mut lo = (a & 0xF) as i16 - (b & 0xF) as i16 - borrow_in as i16;
    let borrow_lo = lo < 0;
    if borrow_lo {
        lo -= 6;
    }
    let mut hi = (a >> 4) as i16 - (b >> 4) as i16 - borrow_lo as i16;
    let borrow = hi < 0;
    if borrow {
        hi -= 6;
    }
    (((hi & 0xF) as u8) << 4 | (lo & 0xF) as u8, borrow)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::m68k::{Assembler, AsmItem};

    /// Build a tiny ROM around the given items at 0x200.
    fn mini_rom(items: Vec<AsmItem>) -> Vec<u8> {
        let mut all = vec![AsmItem::Org(0x200)];
        all.extend(items);
        let mut asm = Assembler::new(0);
        let image = asm.assemble_items(&all).unwrap();
        let mut rom = vec![0xFFu8; 0x1000];
        rom[..image.len()].copy_from_slice(&image);
        rom[0..4].copy_from_slice(&0x00FF_F000u32.to_be_bytes());
        rom[4..8].copy_from_slice(&0x0000_0200u32.to_be_bytes());
        rom
    }

    fn ram_word(rom: &[u8], addr: u32, frames: u32) -> u16 {
        let trace = run_rom(
            rom,
            &RunConfig {
                frames,
                frame_instructions: 1000,
            },
        );
        assert!(trace.error.is_none(), "{:?}", trace.error);
        let off = (addr - RAM_BASE) as usize;
        (trace.ram[off] as u16) << 8 | trace.ram[off + 1] as u16
    }

    /// Spin forever, so the frame budget ends the run cleanly.
    fn spin() -> Vec<AsmItem> {
        vec![
            AsmItem::Label("end".to_string()),
            AsmItem::Inst(M68kInst::BraS("end".to_string())),
        ]
    }

    #[test]
    fn executes_arithmetic_into_ram() {
        let mut items = vec![
            AsmItem::Inst(M68kInst::Moveq(2, DataReg::D0)),
            AsmItem::Inst(M68kInst::Moveq(3, DataReg::D1)),
            AsmItem::Inst(M68kInst::Add(
                Size::Byte,
                Operand::DataReg(DataReg::D1),
                Operand::DataReg(DataReg::D0),
            )),
            AsmItem::Inst(M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsShort(0xFF00u16 as i16),
            )),
        ];
        items.extend(spin());
        let rom = mini_rom(items);
        assert_eq!(ram_word(&rom, 0xFFFF00, 1), 5);
    }

    #[test]
    fn signed_overflow_sets_v_flag() {
        // 0x7FFF + 1 overflows a word: bvs taken.
        let mut items = vec![
            AsmItem::Inst(M68kInst::Move(
                Size::Word,
                Operand::Imm(0x7FFF),
                Operand::DataReg(DataReg::D0),
            )),
            AsmItem::Inst(M68kInst::Addq(
                Size::Word,
                1,
                Operand::DataReg(DataReg::D0),
            )),
            AsmItem::Inst(M68kInst::BccS(Cond::Vs, "ovf".to_string())),
            AsmItem::Inst(M68kInst::BraS("end".to_string())),
            AsmItem::Label("ovf".to_string()),
            AsmItem::Inst(M68kInst::Moveq(1, DataReg::D2)),
            AsmItem::Inst(M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D2),
                Operand::AbsShort(0xFF00u16 as i16),
            )),
        ];
        items.extend(spin());
        let rom = mini_rom(items);
        assert_eq!(ram_word(&rom, 0xFFFF00, 1), 1);
    }

    #[test]
    fn vblank_interrupt_runs_handler() {
        // Enable interrupts, spin in main; the level-6 handler increments
        // a RAM counter and rte's. Three frames -> counter == 3.
        let mut rom = mini_rom(vec![
            AsmItem::Inst(M68kInst::Move(
                Size::Word,
                Operand::Imm(0x2000),
                Operand::Sr,
            )),
            AsmItem::Label("main".to_string()),
            AsmItem::Inst(M68kInst::BraS("main".to_string())),
            AsmItem::Org(0x300),
            AsmItem::Label("vblank".to_string()),
            AsmItem::Inst(M68kInst::Moveq(0, DataReg::D0)),
            AsmItem::Inst(M68kInst::Move(
                Size::Word,
                Operand::AbsShort(0xFF00u16 as i16),
                Operand::DataReg(DataReg::D0),
            )),
            AsmItem::Inst(M68kInst::Addq(
                Size::Word,
                1,
                Operand::DataReg(DataReg::D0),
            )),
            AsmItem::Inst(M68kInst::Move(
                Size::Word,
                Operand::DataReg(DataReg::D0),
                Operand::AbsShort(0xFF00u16 as i16),
            )),
            AsmItem::Inst(M68kInst::Rte),
        ]);
        rom[0x78..0x7C].copy_from_slice(&0x0000_0300u32.to_be_bytes());
        assert_eq!(ram_word(&rom, 0xFFFF00, 3), 3);
    }

    #[test]
    fn move_word_to_dn_zero_extends() {
        // $FFFF loaded as a word must become 0x0000FFFF, not sign-extended.
        let mut items = vec![
            AsmItem::Inst(M68kInst::Move(
                Size::Word,
                Operand::Imm(0xFFFF),
                Operand::DataReg(DataReg::D0),
            )),
            AsmItem::Inst(M68kInst::Move(
                Size::Long,
                Operand::DataReg(DataReg::D0),
                Operand::AbsShort(0xFF00u16 as i16),
            )),
        ];
        items.extend(spin());
        let rom = mini_rom(items);
        let trace = run_rom(
            &rom,
            &RunConfig {
                frames: 1,
                frame_instructions: 100,
            },
        );
        assert!(trace.error.is_none(), "{:?}", trace.error);
        let off = (0xFFFF00 - RAM_BASE) as usize;
        let v = (trace.ram[off] as u32) << 24
            | (trace.ram[off + 1] as u32) << 16
            | (trace.ram[off + 2] as u32) << 8
            | trace.ram[off + 3] as u32;
        assert_eq!(v, 0x0000FFFF);
    }
}
