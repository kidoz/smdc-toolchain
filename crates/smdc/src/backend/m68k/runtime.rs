//! Compiler runtime support routines
//!
//! The 68000 only multiplies 16x16->32 and divides 32/16->16r16, so 32-bit
//! `*`, `/` and `%` call these helpers. They use a register convention
//! instead of the stack: the left operand in D0 and the right in D1. The
//! result is returned in D0 (and the remainder in D1 for the division
//! helpers), and every other register is preserved, so calls need no cache
//! flush and keep the D2-D7 register cache intact.

use super::m68k::*;

/// Signed and unsigned 32-bit multiply (low 32 bits of the product)
pub const MULSI3: &str = "__mulsi3";
/// Unsigned 32-bit divide: quotient in D0, remainder in D1
pub const UDIVSI3: &str = "__udivsi3";
/// Signed 32-bit divide truncating toward zero: quotient in D0, remainder
/// (with the dividend's sign) in D1
pub const DIVSI3: &str = "__divsi3";

/// Helpers `name` calls, which must be emitted along with it
pub fn dependencies(name: &str) -> &'static [&'static str] {
    match name {
        DIVSI3 => &[UDIVSI3],
        _ => &[],
    }
}

/// Instructions for helper `name`
pub fn generate(name: &str) -> Vec<M68kInst> {
    match name {
        MULSI3 => mulsi3(),
        UDIVSI3 => udivsi3(),
        DIVSI3 => divsi3(),
        _ => unreachable!("unknown runtime helper {name}"),
    }
}

fn d(reg: DataReg) -> Operand {
    Operand::DataReg(reg)
}

fn label(name: &str) -> M68kInst {
    M68kInst::Label(name.to_string())
}

fn save_d2_d3() -> M68kInst {
    M68kInst::Movem(
        Size::Long,
        vec![Reg::Data(DataReg::D2), Reg::Data(DataReg::D3)],
        Operand::PreDec(AddrReg::A7),
        true,
    )
}

fn restore_d2_d3() -> M68kInst {
    M68kInst::Movem(
        Size::Long,
        vec![Reg::Data(DataReg::D2), Reg::Data(DataReg::D3)],
        Operand::PostInc(AddrReg::A7),
        false,
    )
}

/// D0 = D0 * D1 (mod 2^32). With a = ah:al and b = bh:bl in 16-bit halves,
/// a*b mod 2^32 = al*bl + ((ah*bl + al*bh) << 16).
fn mulsi3() -> Vec<M68kInst> {
    use DataReg::{D0, D1, D2, D3};
    vec![
        label(MULSI3),
        save_d2_d3(),
        M68kInst::Move(Size::Long, d(D0), d(D2)),
        M68kInst::Move(Size::Long, d(D1), d(D3)),
        M68kInst::Swap(D2),
        M68kInst::Mulu(d(D1), D2), // ah * bl
        M68kInst::Swap(D3),
        M68kInst::Mulu(d(D0), D3), // bh * al
        M68kInst::Add(Size::Word, d(D3), d(D2)),
        M68kInst::Swap(D2),
        M68kInst::Clr(Size::Word, d(D2)), // cross terms << 16
        M68kInst::Mulu(d(D1), D0),        // al * bl
        M68kInst::Add(Size::Long, d(D2), d(D0)),
        restore_d2_d3(),
        M68kInst::Rts,
    ]
}

/// D0 = D0 / D1, D1 = D0 % D1, unsigned. A divisor below 65536 takes two
/// DIVU steps (high word, then remainder:low word, neither of which can
/// overflow); a divisor with bit 31 set gives a quotient of 0 or 1; any
/// other divisor uses a 32-step shift-and-subtract loop.
fn udivsi3() -> Vec<M68kInst> {
    use DataReg::{D0, D1, D2, D3};
    const BIG: &str = ".L__udivsi3_big";
    const BIG_DONE: &str = ".L__udivsi3_big_done";
    const LOOP_SETUP: &str = ".L__udivsi3_loop_setup";
    const LOOP: &str = ".L__udivsi3_loop";
    const NO_BIT: &str = ".L__udivsi3_nobit";
    const SKIP: &str = ".L__udivsi3_skip";
    const DONE: &str = ".L__udivsi3_done";
    vec![
        label(UDIVSI3),
        save_d2_d3(),
        // Divisor high word zero?
        M68kInst::Move(Size::Long, d(D1), d(D2)),
        M68kInst::Swap(D2),
        M68kInst::Tst(Size::Word, d(D2)),
        M68kInst::Bcc(Cond::Ne, BIG.to_string()),
        // 16-bit divisor: divide the high word, then remainder:low word
        M68kInst::Move(Size::Long, d(D0), d(D2)),
        M68kInst::Clr(Size::Word, d(D2)),
        M68kInst::Swap(D2),        // D2 = dividend >> 16
        M68kInst::Divu(d(D1), D2), // D2 = rem_hi:quot_hi
        M68kInst::Move(Size::Word, d(D2), d(D3)),
        M68kInst::Swap(D3),                       // D3 = quot_hi << 16
        M68kInst::Move(Size::Word, d(D0), d(D2)), // D2 = rem_hi:dividend_lo
        M68kInst::Divu(d(D1), D2),                // D2 = rem:quot_lo
        M68kInst::Move(Size::Word, d(D2), d(D3)), // D3 = quotient
        M68kInst::Clr(Size::Word, d(D2)),
        M68kInst::Swap(D2), // D2 = remainder
        M68kInst::Move(Size::Long, d(D2), d(D1)),
        M68kInst::Move(Size::Long, d(D3), d(D0)),
        M68kInst::Bra(DONE.to_string()),
        // Divisor >= 2^16
        label(BIG),
        M68kInst::Tst(Size::Long, d(D1)),
        M68kInst::Bcc(Cond::Pl, LOOP_SETUP.to_string()),
        // Divisor >= 2^31: quotient is 1 if dividend >= divisor, else 0
        M68kInst::Moveq(0, D2),
        M68kInst::Cmp(Size::Long, d(D1), d(D0)),
        M68kInst::Bcc(Cond::Cs, BIG_DONE.to_string()),
        M68kInst::Sub(Size::Long, d(D1), d(D0)),
        M68kInst::Moveq(1, D2),
        label(BIG_DONE),
        M68kInst::Move(Size::Long, d(D0), d(D1)),
        M68kInst::Move(Size::Long, d(D2), d(D0)),
        M68kInst::Bra(DONE.to_string()),
        // Shift-and-subtract; the remainder stays below the divisor (< 2^31),
        // so doubling it never overflows
        label(LOOP_SETUP),
        M68kInst::Move(Size::Long, d(D0), d(D2)), // dividend, becomes quotient
        M68kInst::Moveq(0, D0),                   // remainder
        M68kInst::Moveq(31, D3),
        label(LOOP),
        M68kInst::Add(Size::Long, d(D0), d(D0)),
        M68kInst::Add(Size::Long, d(D2), d(D2)), // next dividend bit -> carry
        M68kInst::Bcc(Cond::Cc, NO_BIT.to_string()),
        M68kInst::Addq(Size::Long, 1, d(D0)),
        label(NO_BIT),
        M68kInst::Cmp(Size::Long, d(D1), d(D0)),
        M68kInst::Bcc(Cond::Cs, SKIP.to_string()),
        M68kInst::Sub(Size::Long, d(D1), d(D0)),
        M68kInst::Addq(Size::Long, 1, d(D2)), // quotient bit
        label(SKIP),
        M68kInst::Dbf(D3, LOOP.to_string()),
        M68kInst::Move(Size::Long, d(D0), d(D1)),
        M68kInst::Move(Size::Long, d(D2), d(D0)),
        label(DONE),
        restore_d2_d3(),
        M68kInst::Rts,
    ]
}

/// D0 = D0 / D1, D1 = D0 % D1, signed (C semantics: the quotient truncates
/// toward zero and the remainder takes the dividend's sign). Divides the
/// magnitudes, then fixes the signs: D2 bit 0 negates the quotient, bit 1
/// the remainder.
fn divsi3() -> Vec<M68kInst> {
    use DataReg::{D0, D1, D2};
    const DIVIDEND_POS: &str = ".L__divsi3_npos";
    const DIVISOR_POS: &str = ".L__divsi3_dpos";
    const QUOT_POS: &str = ".L__divsi3_qpos";
    const REM_POS: &str = ".L__divsi3_rpos";
    vec![
        label(DIVSI3),
        M68kInst::Move(Size::Long, d(D2), Operand::PreDec(AddrReg::A7)),
        M68kInst::Moveq(0, D2),
        M68kInst::Tst(Size::Long, d(D0)),
        M68kInst::Bcc(Cond::Pl, DIVIDEND_POS.to_string()),
        M68kInst::Neg(Size::Long, d(D0)),
        M68kInst::Moveq(3, D2),
        label(DIVIDEND_POS),
        M68kInst::Tst(Size::Long, d(D1)),
        M68kInst::Bcc(Cond::Pl, DIVISOR_POS.to_string()),
        M68kInst::Neg(Size::Long, d(D1)),
        M68kInst::Eori(Size::Long, 1, d(D2)),
        label(DIVISOR_POS),
        M68kInst::Jsr(Operand::Label(UDIVSI3.to_string())),
        M68kInst::Btst(Operand::Imm(0), d(D2)),
        M68kInst::Bcc(Cond::Eq, QUOT_POS.to_string()),
        M68kInst::Neg(Size::Long, d(D0)),
        label(QUOT_POS),
        M68kInst::Btst(Operand::Imm(1), d(D2)),
        M68kInst::Bcc(Cond::Eq, REM_POS.to_string()),
        M68kInst::Neg(Size::Long, d(D1)),
        label(REM_POS),
        M68kInst::Move(Size::Long, Operand::PostInc(AddrReg::A7), d(D2)),
        M68kInst::Rts,
    ]
}
