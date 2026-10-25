//! Textual M68k assembly parser.
//!
//! Parses the grammar emitted by `M68kInst::format()` and the full-coverage
//! disassembler back into typed [`AsmItem`]s, so a disassembled ROM can be
//! reassembled byte-for-byte. The grammar is deliberately narrow: exactly
//! what this toolchain emits, plus `dc.b/dc.w/dc.l`, `org` and `romend`.

use super::assembler::AsmItem;
use super::m68k::*;

/// Parse error with source position.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parse a complete assembly source into layout items.
pub fn parse_assembly(text: &str) -> Result<Vec<AsmItem>, ParseError> {
    let mut items = Vec::new();
    for (idx, raw) in text.lines().enumerate() {
        parse_line(raw, idx + 1, &mut items)?;
    }
    Ok(items)
}

/// Parse a single instruction line (used by tests).
pub fn parse_inst_line(text: &str) -> Result<M68kInst, ParseError> {
    let mut items = Vec::new();
    parse_line(text, 1, &mut items)?;
    match items.pop() {
        Some(AsmItem::Inst(inst)) => Ok(inst),
        _ => Err(ParseError {
            line: 1,
            message: format!("not an instruction: {text:?}"),
        }),
    }
}

fn parse_line(raw: &str, line_no: usize, items: &mut Vec<AsmItem>) -> Result<(), ParseError> {
    let err = |msg: String| ParseError {
        line: line_no,
        message: msg,
    };

    // Strip comments and whitespace.
    let line = match raw.find(';') {
        Some(p) => &raw[..p],
        None => raw,
    };
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }

    // Leading `name:` label (possibly followed by an instruction).
    let mut rest = line;
    while let Some(colon) = rest.find(':') {
        let name = &rest[..colon];
        if is_identifier(name) {
            items.push(AsmItem::Label(name.to_string()));
            rest = rest[colon + 1..].trim_start();
            if rest.is_empty() {
                return Ok(());
            }
        } else {
            break;
        }
    }

    // Split mnemonic from operands.
    let (mnemonic, operands) = match rest.find(char::is_whitespace) {
        Some(p) => (&rest[..p], rest[p..].trim()),
        None => (rest, ""),
    };
    let mnemonic = mnemonic.to_ascii_lowercase();

    // Directives.
    match mnemonic.as_str() {
        "org" => {
            let addr = parse_number(operands).ok_or_else(|| err(format!("bad org: {operands}")))?;
            items.push(AsmItem::Org(addr as u32));
            return Ok(());
        }
        "romend" => {
            let size =
                parse_number(operands).ok_or_else(|| err(format!("bad romend: {operands}")))?;
            items.push(AsmItem::RomEnd(size as u32));
            return Ok(());
        }
        "dc.b" | "dc.w" | "dc.l" => {
            let mut bytes = Vec::new();
            for part in split_operands(operands) {
                let v = parse_number(&part)
                    .ok_or_else(|| err(format!("bad data value: {part}")))?;
                match mnemonic.as_str() {
                    "dc.b" => bytes.push(v as u8),
                    "dc.w" => bytes.extend_from_slice(&(v as u16).to_be_bytes()),
                    _ => bytes.extend_from_slice(&(v as u32).to_be_bytes()),
                }
            }
            if bytes.is_empty() {
                return Err(err(format!("empty {mnemonic}")));
            }
            items.push(AsmItem::Data(bytes));
            return Ok(());
        }
        _ => {}
    }

    // Instruction.
    let inst = parse_instruction(&mnemonic, operands)
        .map_err(|msg| err(format!("{msg} in {rest:?}")))?;
    items.push(AsmItem::Inst(inst));
    Ok(())
}

/// Split an operand list at top-level commas (parentheses protect their own
/// commas: `(4,a0,d0)` is one operand).
fn split_operands(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                cur.push(c);
            }
            ',' if depth == 0 => {
                if !cur.trim().is_empty() {
                    parts.push(cur.trim().to_string());
                }
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        parts.push(cur.trim().to_string());
    }
    parts
}

fn is_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '.')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

/// Parse a number: `$hex`, `0xhex`, `%binary`, decimal, optional leading `-`.
fn parse_number(s: &str) -> Option<i64> {
    let s = s.trim();
    let (neg, body) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    let value = if let Some(hex) = body.strip_prefix('$') {
        i64::from_str_radix(hex, 16).ok()?
    } else if let Some(hex) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        i64::from_str_radix(hex, 16).ok()?
    } else if let Some(bin) = body.strip_prefix('%') {
        i64::from_str_radix(bin, 2).ok()?
    } else {
        body.parse::<i64>().ok()?
    };
    Some(if neg { -value } else { value })
}

fn parse_data_reg(s: &str) -> Option<DataReg> {
    let mut chars = s.chars();
    let first = chars.next()?;
    if first != 'd' && first != 'D' {
        return None;
    }
    // The remainder must be a single decimal digit 0-7; `parse` already
    // rejects trailing garbage like "d0x".
    let n = chars.as_str().parse::<u8>().ok()?;
    (n < 8).then(|| match n {
        0 => DataReg::D0,
        1 => DataReg::D1,
        2 => DataReg::D2,
        3 => DataReg::D3,
        4 => DataReg::D4,
        5 => DataReg::D5,
        6 => DataReg::D6,
        _ => DataReg::D7,
    })
}

fn parse_addr_reg(s: &str) -> Option<AddrReg> {
    if s.eq_ignore_ascii_case("sp") {
        return Some(AddrReg::A7);
    }
    let mut chars = s.chars();
    let first = chars.next()?;
    if first != 'a' && first != 'A' {
        return None;
    }
    let n = chars.as_str().parse::<u8>().ok()?;
    (n < 8).then(|| match n {
        0 => AddrReg::A0,
        1 => AddrReg::A1,
        2 => AddrReg::A2,
        3 => AddrReg::A3,
        4 => AddrReg::A4,
        5 => AddrReg::A5,
        6 => AddrReg::A6,
        _ => AddrReg::A7,
    })
}

fn parse_reg(s: &str) -> Option<Reg> {
    parse_data_reg(s).map(Reg::Data).or_else(|| parse_addr_reg(s).map(Reg::Addr))
}

fn parse_cond(cc: &str) -> Option<Cond> {
    Some(match cc {
        "t" => Cond::True,
        "f" => Cond::False,
        "hi" => Cond::Hi,
        "ls" => Cond::Ls,
        "cc" => Cond::Cc,
        "cs" => Cond::Cs,
        "ne" => Cond::Ne,
        "eq" => Cond::Eq,
        "vc" => Cond::Vc,
        "vs" => Cond::Vs,
        "pl" => Cond::Pl,
        "mi" => Cond::Mi,
        "ge" => Cond::Ge,
        "lt" => Cond::Lt,
        "gt" => Cond::Gt,
        "le" => Cond::Le,
        _ => return None,
    })
}

/// Parse one operand.
fn parse_operand(s: &str) -> Result<Operand, String> {
    let s = s.trim();
    if s == "sr" {
        return Ok(Operand::Sr);
    }
    if let Some(v) = s.strip_prefix('#') {
        let v = parse_number(v).ok_or_else(|| format!("bad immediate: {s}"))?;
        return Ok(Operand::Imm(v as i32));
    }
    if let Some(inner) = s.strip_prefix('-').and_then(|r| r.strip_prefix('(')).and_then(|r| r.strip_suffix(')')) {
        let reg = parse_addr_reg(inner).ok_or_else(|| format!("bad predecrement: {s}"))?;
        return Ok(Operand::PreDec(reg));
    }
    if let Some(_inner) = s.strip_prefix('(') {
        // Parenthesized forms: `(aN)`, `(aN)+`, `(d,pc)`, `(d,pc,Xn)`, `(d,aN,Xn)`.
        let has_post_inc = s.ends_with(")+");
        let inner = s
            .strip_prefix('(')
            .and_then(|r| if has_post_inc { r.strip_suffix(")+") } else { r.strip_suffix(')') })
            .ok_or_else(|| format!("bad parenthesized operand: {s}"))?;
        let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
        if !has_post_inc && parts.len() == 1 {
            let reg = parse_addr_reg(parts[0]).ok_or_else(|| format!("bad indirect: {s}"))?;
            return Ok(Operand::AddrInd(reg));
        }
        if has_post_inc && parts.len() == 1 {
            let reg = parse_addr_reg(parts[0]).ok_or_else(|| format!("bad postincrement: {s}"))?;
            return Ok(Operand::PostInc(reg));
        }
        if parts.len() == 2 && parts[1].eq_ignore_ascii_case("pc") {
            let d = parse_number(parts[0]).ok_or_else(|| format!("bad pc displacement: {s}"))?;
            return Ok(Operand::PcDisp(d as i16));
        }
        if parts.len() == 3 {
            let d = parse_number(parts[0]).ok_or_else(|| format!("bad index displacement: {s}"))?;
            if parts[1].eq_ignore_ascii_case("pc") {
                let idx =
                    parse_reg(parts[2]).ok_or_else(|| format!("bad index register: {s}"))?;
                return Ok(Operand::PcIndexed(d as i8, idx));
            }
            let base = parse_addr_reg(parts[1]).ok_or_else(|| format!("bad base register: {s}"))?;
            let idx = parse_reg(parts[2]).ok_or_else(|| format!("bad index register: {s}"))?;
            return Ok(Operand::Indexed(d as i8, base, idx));
        }
        return Err(format!("bad parenthesized operand: {s}"));
    }
    // d(aN) displacement form (left part may be empty or signed/hex).
    if let Some(paren) = s.find('(') {
        if s.ends_with(')') {
            let disp_str = s[..paren].trim();
            let reg_str = &s[paren + 1..s.len() - 1];
            let reg = parse_addr_reg(reg_str).ok_or_else(|| format!("bad displacement: {s}"))?;
            let d = if disp_str.is_empty() {
                0
            } else {
                parse_number(disp_str).ok_or_else(|| format!("bad displacement: {s}"))?
            };
            return Ok(Operand::Disp(d as i16, reg));
        }
    }
    // Registers (but never in a branch-target position — callers handle labels).
    if let Some(d) = parse_data_reg(s) {
        return Ok(Operand::DataReg(d));
    }
    if let Some(a) = parse_addr_reg(s) {
        return Ok(Operand::AddrReg(a));
    }
    // $xxxx.w absolute short; $xxxxxxxx absolute long.
    if let Some(hex) = s.strip_prefix('$') {
        if let Some(hex) = hex.strip_suffix(".w") {
            let v = i64::from_str_radix(hex, 16).map_err(|_| format!("bad abs.w: {s}"))?;
            return Ok(Operand::AbsShort(v as i16));
        }
        let v = i64::from_str_radix(hex, 16).map_err(|_| format!("bad absolute: {s}"))?;
        return Ok(Operand::AbsLong(v as u32));
    }
    if is_identifier(s) {
        return Ok(Operand::Label(s.to_string()));
    }
    Err(format!("unrecognized operand: {s}"))
}

/// Parse a branch target: a label name or absolute address.
fn parse_target(s: &str) -> Result<String, String> {
    let s = s.trim();
    if is_identifier(s) {
        return Ok(s.to_string());
    }
    Err(format!("bad branch target: {s}"))
}

fn parse_size(suffix: &str) -> Result<Size, String> {
    match suffix {
        "b" => Ok(Size::Byte),
        "w" => Ok(Size::Word),
        "l" => Ok(Size::Long),
        _ => Err(format!("bad size suffix: .{suffix}")),
    }
}

/// Split a mnemonic into base and optional `.size` suffix.
fn split_mnemonic(mnemonic: &str) -> (&str, &str) {
    match mnemonic.find('.') {
        Some(p) => (&mnemonic[..p], &mnemonic[p + 1..]),
        None => (mnemonic, ""),
    }
}

fn parse_instruction(mnemonic: &str, operands: &str) -> Result<M68kInst, String> {
    let ops = split_operands(operands);

    // No-operand instructions.
    match mnemonic {
        "nop" => return Ok(M68kInst::Nop),
        "rts" => return Ok(M68kInst::Rts),
        "rte" => return Ok(M68kInst::Rte),
        "rtr" => return Ok(M68kInst::Rtr),
        "trapv" => return Ok(M68kInst::Trapv),
        "reset" => return Ok(M68kInst::Reset),
        "illegal" => return Ok(M68kInst::Illegal),
        _ => {}
    }

    let (base, suffix) = split_mnemonic(mnemonic);
    let need_size = || -> Result<Size, String> { parse_size(suffix) };
    let op = |i: usize| -> Result<Operand, String> {
        ops.get(i)
            .ok_or_else(|| format!("missing operand {i}"))
            .and_then(|s| parse_operand(s))
    };
    let imm = |i: usize| -> Result<i32, String> {
        match ops.get(i).and_then(|s| s.strip_prefix('#')) {
            Some(v) => parse_number(v).map(|n| n as i32).ok_or_else(|| "bad immediate".into()),
            None => Err(format!("expected immediate operand {i}")),
        }
    };
    let dreg = |i: usize| -> Result<DataReg, String> {
        ops.get(i)
            .and_then(|s| parse_data_reg(s))
            .ok_or_else(|| format!("expected data register at operand {i}"))
    };
    let areg = |i: usize| -> Result<AddrReg, String> {
        ops.get(i)
            .and_then(|s| parse_addr_reg(s))
            .ok_or_else(|| format!("expected address register at operand {i}"))
    };

    match base {
        "moveq" => Ok(M68kInst::Moveq(imm(0)? as i8, dreg(1)?)),
        "swap" => Ok(M68kInst::Swap(dreg(0)?)),
        "link" => Ok(M68kInst::Link(areg(0)?, imm(1)? as i16)),
        "unlk" => Ok(M68kInst::Unlk(areg(0)?)),
        "trap" => Ok(M68kInst::Trap(imm(0)? as u8)),
        "stop" => Ok(M68kInst::Stop(imm(0)? as u16)),
        "lea" => Ok(M68kInst::Lea(op(0)?, areg(1)?)),
        "pea" => Ok(M68kInst::Pea(op(0)?)),
        "clr" => Ok(M68kInst::Clr(need_size()?, op(0)?)),
        "exg" => {
            let r1 = ops
                .get(0)
                .and_then(|s| parse_reg(s))
                .ok_or("bad exg register")?;
            let r2 = ops
                .get(1)
                .and_then(|s| parse_reg(s))
                .ok_or("bad exg register")?;
            Ok(M68kInst::Exg(r1, r2))
        }
        "move" => {
            let src = ops.get(0).ok_or("missing source")?;
            let dst = ops.get(1).ok_or("missing destination")?;
            match (src.as_str(), dst.as_str()) {
                ("usp", _) => {
                    Ok(M68kInst::MoveUspToAddr(parse_addr_reg(dst).ok_or("bad usp move")?))
                }
                (_, "usp") => {
                    Ok(M68kInst::MoveAddrToUsp(parse_addr_reg(src).ok_or("bad usp move")?))
                }
                _ => {
                    let size = need_size()?;
                    match (src.as_str(), dst.as_str()) {
                        (_, "ccr") => Ok(M68kInst::MoveToCcr(parse_operand(src)?)),
                        ("sr", _) => Ok(M68kInst::Move(size, Operand::Sr, parse_operand(dst)?)),
                        (_, "sr") => Ok(M68kInst::Move(size, parse_operand(src)?, Operand::Sr)),
                        _ => Ok(M68kInst::Move(size, parse_operand(src)?, parse_operand(dst)?)),
                    }
                }
            }
        }
        "movep" => {
            let size = need_size()?;
            let a = ops.get(0).ok_or("missing operand")?;
            let b = ops.get(1).ok_or("missing operand")?;
            let to_mem = parse_data_reg(a).is_some();
            if to_mem {
                // movep Dx, d(An)
                let (disp, an) = disp_and_reg_of(b)?;
                Ok(M68kInst::Movep(size, true, disp, an, dreg(0)?))
            } else {
                // movep d(An), Dx
                let (disp, an) = disp_and_reg_of(a)?;
                Ok(M68kInst::Movep(size, false, disp, an, dreg(1)?))
            }
        }
        "add" => Ok(M68kInst::Add(need_size()?, op(0)?, op(1)?)),
        "adda" => Ok(M68kInst::Adda(need_size()?, op(0)?, areg(1)?)),
        "addq" => Ok(M68kInst::Addq(need_size()?, imm(0)? as u8, op(1)?)),
        "addi" => Ok(M68kInst::Addi(need_size()?, imm(0)?, op(1)?)),
        "addx" => Ok(M68kInst::Addx(need_size()?, op(0)?, op(1)?)),
        "sub" => Ok(M68kInst::Sub(need_size()?, op(0)?, op(1)?)),
        "suba" => Ok(M68kInst::Suba(need_size()?, op(0)?, areg(1)?)),
        "subq" => Ok(M68kInst::Subq(need_size()?, imm(0)? as u8, op(1)?)),
        "subi" => Ok(M68kInst::Subi(need_size()?, imm(0)?, op(1)?)),
        "subx" => Ok(M68kInst::Subx(need_size()?, op(0)?, op(1)?)),
        "abcd" => Ok(M68kInst::Abcd(op(0)?, op(1)?)),
        "sbcd" => Ok(M68kInst::Sbcd(op(0)?, op(1)?)),
        "nbcd" => Ok(M68kInst::Nbcd(op(0)?)),
        "cmpm" => {
            let post = |i: usize| -> Result<AddrReg, String> {
                ops.get(i)
                    .and_then(|s| s.strip_prefix('('))
                    .and_then(|s| s.strip_suffix(")+"))
                    .and_then(parse_addr_reg)
                    .ok_or_else(|| format!("expected (aN)+ at operand {i}"))
            };
            Ok(M68kInst::Cmpm(need_size()?, post(0)?, post(1)?))
        }
        "muls" => Ok(M68kInst::Muls(op(0)?, dreg(1)?)),
        "mulu" => Ok(M68kInst::Mulu(op(0)?, dreg(1)?)),
        "divs" => Ok(M68kInst::Divs(op(0)?, dreg(1)?)),
        "divu" => Ok(M68kInst::Divu(op(0)?, dreg(1)?)),
        "neg" => Ok(M68kInst::Neg(need_size()?, op(0)?)),
        "negx" => Ok(M68kInst::Negx(need_size()?, op(0)?)),
        "ext" => Ok(M68kInst::Ext(need_size()?, dreg(0)?)),
        "chk" => Ok(M68kInst::Chk(op(0)?, dreg(1)?)),
        "tas" => Ok(M68kInst::Tas(op(0)?)),
        "and" => Ok(M68kInst::And(need_size()?, op(0)?, op(1)?)),
        "andi" => {
            if ops.get(1).is_some_and(|s| s == "ccr") {
                Ok(M68kInst::AndiToCcr(imm(0)? as u16))
            } else if ops.get(1).is_some_and(|s| s == "sr") {
                Ok(M68kInst::AndiToSr(imm(0)? as u16))
            } else {
                Ok(M68kInst::Andi(need_size()?, imm(0)?, op(1)?))
            }
        }
        "or" => Ok(M68kInst::Or(need_size()?, op(0)?, op(1)?)),
        "ori" => {
            if ops.get(1).is_some_and(|s| s == "ccr") {
                Ok(M68kInst::OriToCcr(imm(0)? as u16))
            } else if ops.get(1).is_some_and(|s| s == "sr") {
                Ok(M68kInst::OriToSr(imm(0)? as u16))
            } else {
                Ok(M68kInst::Ori(need_size()?, imm(0)?, op(1)?))
            }
        }
        "eor" => Ok(M68kInst::Eor(need_size()?, dreg(0)?, op(1)?)),
        "eori" => {
            if ops.get(1).is_some_and(|s| s == "ccr") {
                Ok(M68kInst::EoriToCcr(imm(0)? as u16))
            } else if ops.get(1).is_some_and(|s| s == "sr") {
                Ok(M68kInst::EoriToSr(imm(0)? as u16))
            } else {
                Ok(M68kInst::Eori(need_size()?, imm(0)?, op(1)?))
            }
        }
        "not" => Ok(M68kInst::Not(need_size()?, op(0)?)),
        "tst" => Ok(M68kInst::Tst(need_size()?, op(0)?)),
        "cmp" => Ok(M68kInst::Cmp(need_size()?, op(0)?, op(1)?)),
        "cmpa" => Ok(M68kInst::Cmpa(need_size()?, op(0)?, areg(1)?)),
        "cmpi" => Ok(M68kInst::Cmpi(need_size()?, imm(0)?, op(1)?)),
        "btst" => Ok(M68kInst::Btst(op(0)?, op(1)?)),
        "bset" => Ok(M68kInst::Bset(op(0)?, op(1)?)),
        "bclr" => Ok(M68kInst::Bclr(op(0)?, op(1)?)),
        "bchg" => Ok(M68kInst::Bchg(op(0)?, op(1)?)),
        "jmp" => Ok(M68kInst::Jmp(op(0)?)),
        "jsr" => Ok(M68kInst::Jsr(op(0)?)),
        "movem" => {
            let size = need_size()?;
            let a = ops.get(0).ok_or("missing operand")?;
            let b = ops.get(1).ok_or("missing operand")?;
            let regs_of = |s: &str| -> Option<Vec<Reg>> {
                let mut regs = Vec::new();
                for part in s.split('/') {
                    regs.push(parse_reg(part)?);
                }
                (!regs.is_empty()).then_some(regs)
            };
            match (regs_of(a), regs_of(b)) {
                (Some(regs), None) => Ok(M68kInst::Movem(size, regs, parse_operand(b)?, true)),
                (None, Some(regs)) => Ok(M68kInst::Movem(size, regs, parse_operand(a)?, false)),
                _ => Err("movem needs one register-list operand".to_string()),
            }
        }
        "bra" | "bsr" => {
            let target = parse_target(ops.get(0).ok_or("missing target")?)?;
            match (base, suffix) {
                ("bra", "s") => Ok(M68kInst::BraS(target)),
                ("bra", _) => Ok(M68kInst::Bra(target)),
                ("bsr", "s") => Ok(M68kInst::BsrS(target)),
                _ => Ok(M68kInst::Bsr(target)),
            }
        }
        "lsl" | "lsr" | "asl" | "asr" | "rol" | "ror" | "roxl" | "roxr" => {
            if ops.len() == 1 {
                // Memory form: single word EA operand.
                let ea = op(0)?;
                return Ok(match base {
                    "lsl" => M68kInst::LslMem(ea),
                    "lsr" => M68kInst::LsrMem(ea),
                    "asl" => M68kInst::AslMem(ea),
                    "asr" => M68kInst::AsrMem(ea),
                    "rol" => M68kInst::RolMem(ea),
                    "ror" => M68kInst::RorMem(ea),
                    "roxl" => M68kInst::RoxlMem(ea),
                    _ => M68kInst::RoxrMem(ea),
                });
            }
            let size = need_size()?;
            let count = op(0)?;
            let reg = dreg(1)?;
            Ok(match base {
                "lsl" => M68kInst::Lsl(size, count, reg),
                "lsr" => M68kInst::Lsr(size, count, reg),
                "asl" => M68kInst::Asl(size, count, reg),
                "asr" => M68kInst::Asr(size, count, reg),
                "rol" => M68kInst::Rol(size, count, reg),
                "ror" => M68kInst::Ror(size, count, reg),
                "roxl" => M68kInst::Roxl(size, count, reg),
                _ => M68kInst::Roxr(size, count, reg),
            })
        }
        _ => {
            // Condition-coded families: Bcc[.s|.w], DBcc, Scc.
            if let Some(rest) = base.strip_prefix('b') {
                if let Some(cond) = parse_cond(rest) {
                    let target = parse_target(ops.get(0).ok_or("missing target")?)?;
                    return match suffix {
                        "s" => Ok(M68kInst::BccS(cond, target)),
                        _ => Ok(M68kInst::Bcc(cond, target)),
                    };
                }
            }
            if base == "dbra" {
                return Ok(M68kInst::Dbcc(Cond::False, dreg(0)?, parse_target(ops.get(1).ok_or("missing target")?)?));
            }
            if let Some(rest) = base.strip_prefix("db") {
                if let Some(cond) = parse_cond(rest) {
                    return Ok(M68kInst::Dbcc(cond, dreg(0)?, parse_target(ops.get(1).ok_or("missing target")?)?));
                }
            }
            if let Some(rest) = base.strip_prefix('s') {
                if let Some(cond) = parse_cond(rest) {
                    return Ok(M68kInst::Scc(cond, op(0)?));
                }
            }
            Err(format!("unknown mnemonic: {mnemonic}"))
        }
    }
}

/// Displacement and base register of a `d(An)` operand string.
fn disp_and_reg_of(s: &str) -> Result<(i16, AddrReg), String> {
    match parse_operand(s)? {
        Operand::Disp(d, r) => Ok((d, r)),
        Operand::AddrInd(r) => Ok((0, r)),
        other => Err(format!("expected d(An), got {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::m68k::Assembler;

    /// format() -> parse -> same instruction; and equal encoded bytes.
    #[test]
    fn format_text_parses_back() {
        let cases: Vec<M68kInst> = vec![
            M68kInst::Nop,
            M68kInst::Rts,
            M68kInst::Move(Size::Byte, Operand::PostInc(AddrReg::A0), Operand::PreDec(AddrReg::A1)),
            M68kInst::Move(Size::Word, Operand::Imm(0x1234), Operand::Disp(-8, AddrReg::A6)),
            M68kInst::Move(Size::Long, Operand::AbsShort(0xF624u16 as i16), Operand::DataReg(DataReg::D2)),
            M68kInst::Move(Size::Long, Operand::AbsLong(0x00FF8000), Operand::DataReg(DataReg::D0)),
            M68kInst::Move(Size::Word, Operand::DataReg(DataReg::D0), Operand::Sr),
            M68kInst::Move(Size::Word, Operand::Sr, Operand::DataReg(DataReg::D1)),
            M68kInst::MoveToCcr(Operand::DataReg(DataReg::D0)),
            M68kInst::Moveq(-5, DataReg::D3),
            M68kInst::Lea(Operand::Disp(16, AddrReg::A6), AddrReg::A0),
            M68kInst::Pea(Operand::AbsLong(0x00C00004)),
            M68kInst::Clr(Size::Long, Operand::DataReg(DataReg::D0)),
            M68kInst::Exg(Reg::Data(DataReg::D2), Reg::Addr(AddrReg::A3)),
            M68kInst::Add(Size::Long, Operand::Disp(8, AddrReg::A6), Operand::DataReg(DataReg::D0)),
            M68kInst::Add(Size::Word, Operand::DataReg(DataReg::D1), Operand::AddrInd(AddrReg::A0)),
            M68kInst::Adda(Size::Long, Operand::DataReg(DataReg::D0), AddrReg::A2),
            M68kInst::Addq(Size::Long, 8, Operand::AddrReg(AddrReg::A7)),
            M68kInst::Subq(Size::Word, 1, Operand::DataReg(DataReg::D2)),
            M68kInst::Addi(Size::Long, 100, Operand::DataReg(DataReg::D0)),
            M68kInst::Subi(Size::Word, -5, Operand::DataReg(DataReg::D1)),
            M68kInst::Muls(Operand::DataReg(DataReg::D1), DataReg::D0),
            M68kInst::Divu(Operand::Disp(8, AddrReg::A6), DataReg::D2),
            M68kInst::Neg(Size::Long, Operand::DataReg(DataReg::D0)),
            M68kInst::Negx(Size::Byte, Operand::AddrInd(AddrReg::A1)),
            M68kInst::Ext(Size::Word, DataReg::D0),
            M68kInst::Addx(Size::Long, Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D0)),
            M68kInst::Subx(Size::Byte, Operand::PreDec(AddrReg::A1), Operand::PreDec(AddrReg::A0)),
            M68kInst::Abcd(Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D0)),
            M68kInst::Nbcd(Operand::AddrInd(AddrReg::A0)),
            M68kInst::Cmpm(Size::Word, AddrReg::A1, AddrReg::A0),
            M68kInst::And(Size::Long, Operand::DataReg(DataReg::D1), Operand::DataReg(DataReg::D0)),
            M68kInst::Andi(Size::Long, 0xFF, Operand::DataReg(DataReg::D0)),
            M68kInst::AndiToCcr(0xF8),
            M68kInst::AndiToSr(0xF8FF),
            M68kInst::Ori(Size::Word, 0x8000 - 0x10000, Operand::DataReg(DataReg::D1)),
            M68kInst::OriToCcr(0x23),
            M68kInst::OriToSr(0x0700),
            M68kInst::Eor(Size::Long, DataReg::D1, Operand::DataReg(DataReg::D0)),
            M68kInst::Eori(Size::Word, 0x00FF, Operand::DataReg(DataReg::D3)),
            M68kInst::EoriToSr(0x0001),
            M68kInst::Not(Size::Word, Operand::DataReg(DataReg::D4)),
            M68kInst::Lsl(Size::Long, Operand::Imm(4), DataReg::D0),
            M68kInst::Lsr(Size::Word, Operand::DataReg(DataReg::D1), DataReg::D2),
            M68kInst::Roxl(Size::Word, Operand::Imm(4), DataReg::D1),
            M68kInst::Roxr(Size::Long, Operand::DataReg(DataReg::D2), DataReg::D0),
            M68kInst::AslMem(Operand::AddrInd(AddrReg::A0)),
            M68kInst::LsrMem(Operand::Disp(4, AddrReg::A1)),
            M68kInst::RorMem(Operand::AbsLong(0x00C00000)),
            M68kInst::Btst(Operand::Imm(3), Operand::DataReg(DataReg::D0)),
            M68kInst::Bset(Operand::DataReg(DataReg::D1), Operand::AddrInd(AddrReg::A0)),
            M68kInst::Bclr(Operand::Imm(7), Operand::Disp(2, AddrReg::A1)),
            M68kInst::Cmp(Size::Long, Operand::Disp(-12, AddrReg::A6), Operand::DataReg(DataReg::D1)),
            M68kInst::Cmpa(Size::Long, Operand::AddrReg(AddrReg::A1), AddrReg::A0),
            M68kInst::Cmpi(Size::Long, 0x123456, Operand::DataReg(DataReg::D0)),
            M68kInst::Chk(Operand::AddrInd(AddrReg::A0), DataReg::D2),
            M68kInst::Tst(Size::Byte, Operand::DataReg(DataReg::D0)),
            M68kInst::Jmp(Operand::AddrInd(AddrReg::A0)),
            M68kInst::Jsr(Operand::AbsLong(0x00000400)),
            M68kInst::Jsr(Operand::PcIndexed(6, Reg::Data(DataReg::D0))),
            M68kInst::Link(AddrReg::A6, -64),
            M68kInst::Unlk(AddrReg::A6),
            M68kInst::Movem(
                Size::Long,
                vec![Reg::Data(DataReg::D2), Reg::Addr(AddrReg::A2)],
                Operand::PreDec(AddrReg::A7),
                true,
            ),
            M68kInst::Movem(
                Size::Word,
                vec![Reg::Data(DataReg::D0), Reg::Data(DataReg::D1), Reg::Addr(AddrReg::A5)],
                Operand::PostInc(AddrReg::A7),
                false,
            ),
            M68kInst::Scc(Cond::Eq, Operand::DataReg(DataReg::D0)),
            M68kInst::Scc(Cond::Lt, Operand::DataReg(DataReg::D3)),
            M68kInst::Swap(DataReg::D2),
            M68kInst::MoveUspToAddr(AddrReg::A0),
            M68kInst::MoveAddrToUsp(AddrReg::A3),
            M68kInst::Movep(Size::Word, true, 4, AddrReg::A1, DataReg::D0),
            M68kInst::Movep(Size::Long, false, -2, AddrReg::A2, DataReg::D3),
            M68kInst::Tas(Operand::AddrInd(AddrReg::A0)),
            M68kInst::Trap(7),
            M68kInst::Trapv,
            M68kInst::Stop(0x2700),
            M68kInst::Reset,
            M68kInst::Illegal,
            M68kInst::Rtr,
            M68kInst::Move(
                Size::Byte,
                Operand::Indexed(0, AddrReg::A0, Reg::Data(DataReg::D0)),
                Operand::DataReg(DataReg::D3),
            ),
            M68kInst::Move(
                Size::Word,
                Operand::PcIndexed(2, Reg::Data(DataReg::D2)),
                Operand::DataReg(DataReg::D1),
            ),
            M68kInst::Move(Size::Byte, Operand::PcDisp(-4), Operand::DataReg(DataReg::D0)),
            M68kInst::BraS("L1".to_string()),
            M68kInst::BccS(Cond::Eq, "L2".to_string()),
            M68kInst::Dbcc(Cond::False, DataReg::D0, "L3".to_string()),
        ];

        for inst in cases {
            // Branches need a symbol table to encode; compare parse-back textually.
            let text = inst.format().trim().to_string();
            let parsed = parse_inst_line(&text)
                .unwrap_or_else(|e| panic!("parse failed for {text:?}: {e}"));
            assert_eq!(parsed.format().trim(), text, "parse mismatch for {inst:?}");

            if !matches!(
                inst,
                M68kInst::Bra(_)
                    | M68kInst::Bsr(_)
                    | M68kInst::Bcc(_, _)
                    | M68kInst::BraS(_)
                    | M68kInst::BsrS(_)
                    | M68kInst::BccS(_, _)
                    | M68kInst::Dbcc(_, _, _)
                    | M68kInst::Move(_, _, Operand::Label(_))
            ) {
                assert_eq!(parsed, inst, "struct mismatch for {inst:?}");
                let mut enc = crate::backend::m68k::InstructionEncoder::new();
                let a = enc.encode(&inst).expect("encode original");
                let mut enc2 = crate::backend::m68k::InstructionEncoder::new();
                let b = enc2.encode(&parsed).expect("encode parsed");
                assert_eq!(a, b, "bytes mismatch for {inst:?}");
            }
        }
    }

    #[test]
    fn parses_full_source_layout() {
        let src = "; header\n\
                   dc.l $00FFFE00, $00000206\n\
                   org $200\n\
                   start:\n\
                   moveq #1, d0\n\
                   bra.s start\n\
                   ; gap then data\n\
                   org $220\n\
                   table:\n\
                   dc.w $1234, $5678\n\
                   romend $40000\n";
        let items = parse_assembly(src).unwrap();
        assert_eq!(items.len(), 9);

        let mut asm = Assembler::new(0);
        let image = asm.assemble_items(&items).unwrap();
        assert_eq!(image.len(), 0x40000);
        // Header passthrough at 0.
        assert_eq!(&image[0..8], &[0x00, 0xFF, 0xFE, 0x00, 0x00, 0x00, 0x02, 0x06]);
        // moveq #1,d0 at 0x200; bra.s start: pc_ref 0x204 -> 0x200, disp -4.
        assert_eq!(&image[0x200..0x204], &[0x70, 0x01, 0x60, 0xFC]);
        // 0xFF gap between code (ends 0x204) and data (0x220).
        assert!(image[0x204..0x220].iter().all(|&b| b == 0xFF));
        // Data table at 0x220.
        assert_eq!(&image[0x220..0x224], &[0x12, 0x34, 0x56, 0x78]);
        // 0xFF tail padding.
        assert!(image[0x224..].iter().all(|&b| b == 0xFF));
    }

    #[test]
    fn branches_resolve_through_labels() {
        let src = "org $200\n\
                   loop:\n\
                   dbf d0, loop\n\
                   bne.w loop\n\
                   bsr.s sub\n\
                   bra.w end\n\
                   sub:\n\
                   rts\n\
                   end:\n\
                   nop\n";
        let items = parse_assembly(src).unwrap();
        let mut asm = Assembler::new(0);
        let image = asm.assemble_items(&items).unwrap();
        // 0x200: dbf d0, loop (0x200): disp = -2 -> 0x51C8 FFFE
        assert_eq!(&image[0x200..0x204], &[0x51, 0xC8, 0xFF, 0xFE]);
        // 0x204: bne.w loop: pc_ref 0x206 -> 0x200: disp -6 -> 0x6600 FFFA
        assert_eq!(&image[0x204..0x208], &[0x66, 0x00, 0xFF, 0xFA]);
        // 0x208: bsr.s sub (0x20E): pc_ref 0x20A -> 0x20E: +4 -> 0x6104
        assert_eq!(&image[0x208..0x20A], &[0x61, 0x04]);
        // 0x20A: bra.w end (0x210): pc_ref 0x20C -> 0x210: +4 -> 0x6000 0004
        assert_eq!(&image[0x20A..0x20E], &[0x60, 0x00, 0x00, 0x04]);
        // 0x20E: rts at sub
        assert_eq!(&image[0x20E..0x210], &[0x4E, 0x75]);
    }

    #[test]
    fn unknown_mnemonic_is_an_error() {
        assert!(parse_inst_line("frobnicate d0").is_err());
    }

    #[test]
    fn leading_label_with_instruction_on_same_line() {
        let items = parse_assembly("entry: nop").unwrap();
        assert_eq!(items.len(), 2);
    }
}
