//! M68k instruction definitions

/// M68k data registers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataReg {
    D0,
    D1,
    D2,
    D3,
    D4,
    D5,
    D6,
    D7,
}

impl std::fmt::Display for DataReg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataReg::D0 => write!(f, "d0"),
            DataReg::D1 => write!(f, "d1"),
            DataReg::D2 => write!(f, "d2"),
            DataReg::D3 => write!(f, "d3"),
            DataReg::D4 => write!(f, "d4"),
            DataReg::D5 => write!(f, "d5"),
            DataReg::D6 => write!(f, "d6"),
            DataReg::D7 => write!(f, "d7"),
        }
    }
}

/// M68k address registers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddrReg {
    A0,
    A1,
    A2,
    A3,
    A4,
    A5,
    A6, // Frame pointer
    A7, // Stack pointer
}

impl std::fmt::Display for AddrReg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddrReg::A0 => write!(f, "a0"),
            AddrReg::A1 => write!(f, "a1"),
            AddrReg::A2 => write!(f, "a2"),
            AddrReg::A3 => write!(f, "a3"),
            AddrReg::A4 => write!(f, "a4"),
            AddrReg::A5 => write!(f, "a5"),
            AddrReg::A6 => write!(f, "a6"),
            AddrReg::A7 => write!(f, "sp"),
        }
    }
}

/// Any M68k register
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reg {
    Data(DataReg),
    Addr(AddrReg),
}

impl std::fmt::Display for Reg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Reg::Data(d) => write!(f, "{}", d),
            Reg::Addr(a) => write!(f, "{}", a),
        }
    }
}

/// Operation size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    Byte,  // .b
    Word,  // .w
    Long,  // .l
}

impl std::fmt::Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Size::Byte => write!(f, "b"),
            Size::Word => write!(f, "w"),
            Size::Long => write!(f, "l"),
        }
    }
}

impl Size {
    pub fn from_bytes(n: usize) -> Self {
        match n {
            1 => Size::Byte,
            2 => Size::Word,
            _ => Size::Long,
        }
    }
}

/// M68k addressing mode operand
#[derive(Debug, Clone)]
pub enum Operand {
    /// Data register direct: Dn
    DataReg(DataReg),
    /// Address register direct: An
    AddrReg(AddrReg),
    /// Address register indirect: (An)
    AddrInd(AddrReg),
    /// Post-increment: (An)+
    PostInc(AddrReg),
    /// Pre-decrement: -(An)
    PreDec(AddrReg),
    /// Displacement: d(An)
    Disp(i16, AddrReg),
    /// Indexed: d(An,Dn)
    Indexed(i8, AddrReg, DataReg),
    /// Absolute short: addr.w
    AbsShort(i16),
    /// Absolute long: addr.l
    AbsLong(u32),
    /// Immediate: #imm
    Imm(i32),
    /// PC relative: d(PC)
    PcRel(String),
    /// Label reference
    Label(String),
    /// Status register
    Sr,
}

impl std::fmt::Display for Operand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operand::DataReg(r) => write!(f, "{}", r),
            Operand::AddrReg(r) => write!(f, "{}", r),
            Operand::AddrInd(r) => write!(f, "({})", r),
            Operand::PostInc(r) => write!(f, "({})+", r),
            Operand::PreDec(r) => write!(f, "-({})", r),
            Operand::Disp(d, r) => write!(f, "{}({})", d, r),
            Operand::Indexed(d, a, d2) => write!(f, "({},{},{})", d, a, d2),
            Operand::AbsShort(a) => write!(f, "${:04X}.w", a),
            Operand::AbsLong(a) => write!(f, "${:08X}", a),
            Operand::Imm(v) => {
                if *v >= 0 {
                    write!(f, "#${:X}", v)
                } else {
                    write!(f, "#-${:X}", -v)
                }
            }
            Operand::PcRel(l) => write!(f, "{}(pc)", l),
            Operand::Label(l) => write!(f, "{}", l),
            Operand::Sr => write!(f, "sr"),
        }
    }
}

/// Condition codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cond {
    True,  // T - always
    False, // F - never
    Hi,    // High (unsigned >)
    Ls,    // Low or Same (unsigned <=)
    Cc,    // Carry Clear (unsigned >=)
    Cs,    // Carry Set (unsigned <)
    Ne,    // Not Equal
    Eq,    // Equal
    Vc,    // Overflow Clear
    Vs,    // Overflow Set
    Pl,    // Plus (positive)
    Mi,    // Minus (negative)
    Ge,    // Greater or Equal (signed)
    Lt,    // Less Than (signed)
    Gt,    // Greater Than (signed)
    Le,    // Less or Equal (signed)
}

impl std::fmt::Display for Cond {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Cond::True => write!(f, "t"),
            Cond::False => write!(f, "f"),
            Cond::Hi => write!(f, "hi"),
            Cond::Ls => write!(f, "ls"),
            Cond::Cc => write!(f, "cc"),
            Cond::Cs => write!(f, "cs"),
            Cond::Ne => write!(f, "ne"),
            Cond::Eq => write!(f, "eq"),
            Cond::Vc => write!(f, "vc"),
            Cond::Vs => write!(f, "vs"),
            Cond::Pl => write!(f, "pl"),
            Cond::Mi => write!(f, "mi"),
            Cond::Ge => write!(f, "ge"),
            Cond::Lt => write!(f, "lt"),
            Cond::Gt => write!(f, "gt"),
            Cond::Le => write!(f, "le"),
        }
    }
}

/// M68k instruction
#[derive(Debug, Clone)]
pub enum M68kInst {
    // Data movement
    Move(Size, Operand, Operand),
    Moveq(i8, DataReg), // Quick move (-128 to 127)
    Lea(Operand, AddrReg),
    Pea(Operand),
    Clr(Size, Operand),
    Exg(Reg, Reg),

    // Arithmetic
    Add(Size, Operand, Operand),
    Adda(Size, Operand, AddrReg),
    Addq(Size, u8, Operand), // Quick add (1-8)
    Addi(Size, i32, Operand),
    Sub(Size, Operand, Operand),
    Suba(Size, Operand, AddrReg),
    Subq(Size, u8, Operand),
    Subi(Size, i32, Operand),
    Muls(Operand, DataReg), // Signed multiply
    Mulu(Operand, DataReg), // Unsigned multiply
    Divs(Operand, DataReg), // Signed divide
    Divu(Operand, DataReg), // Unsigned divide
    Neg(Size, Operand),
    Ext(Size, DataReg), // Sign extend

    // Logical
    And(Size, Operand, Operand),
    Andi(Size, i32, Operand),
    Or(Size, Operand, Operand),
    Ori(Size, i32, Operand),
    Eor(Size, DataReg, Operand),
    Eori(Size, i32, Operand),
    Not(Size, Operand),

    // Shift and rotate
    Lsl(Size, Operand, DataReg),
    Lsr(Size, Operand, DataReg),
    Asl(Size, Operand, DataReg),
    Asr(Size, Operand, DataReg),
    Rol(Size, Operand, DataReg),
    Ror(Size, Operand, DataReg),

    // Bit manipulation
    Btst(Operand, Operand),
    Bset(Operand, Operand),
    Bclr(Operand, Operand),
    Bchg(Operand, Operand),

    // Comparison
    Cmp(Size, Operand, Operand),
    Cmpa(Size, Operand, AddrReg),
    Cmpi(Size, i32, Operand),
    Tst(Size, Operand),

    // Branch
    Bra(String),
    Bsr(String),
    Bcc(Cond, String),
    Dbf(DataReg, String), // Decrement and branch if not -1 (loop)

    // Jump
    Jmp(Operand),
    Jsr(Operand),

    // Stack
    Link(AddrReg, i16),
    Unlk(AddrReg),
    Rts,
    Rte,

    // Multiple register
    Movem(Size, Vec<Reg>, Operand, bool), // bool = to_memory

    // Set byte on condition
    Scc(Cond, Operand),

    // Misc
    Nop,
    Swap(DataReg),

    // Pseudo-instructions
    Label(String),
    Comment(String),
    Directive(String),
}

impl M68kInst {
    pub fn format(&self) -> String {
        match self {
            M68kInst::Move(s, src, dst) => format!("    move.{}  {}, {}", s, src, dst),
            M68kInst::Moveq(v, d) => format!("    moveq   #{}, {}", v, d),
            M68kInst::Lea(src, dst) => format!("    lea     {}, {}", src, dst),
            M68kInst::Pea(op) => format!("    pea     {}", op),
            M68kInst::Clr(s, op) => format!("    clr.{}   {}", s, op),
            M68kInst::Exg(r1, r2) => format!("    exg     {}, {}", r1, r2),

            M68kInst::Add(s, src, dst) => format!("    add.{}  {}, {}", s, src, dst),
            M68kInst::Adda(s, src, dst) => format!("    adda.{} {}, {}", s, src, dst),
            M68kInst::Addq(s, v, op) => format!("    addq.{} #{}, {}", s, v, op),
            M68kInst::Addi(s, v, op) => format!("    addi.{} #{}, {}", s, v, op),
            M68kInst::Sub(s, src, dst) => format!("    sub.{}  {}, {}", s, src, dst),
            M68kInst::Suba(s, src, dst) => format!("    suba.{} {}, {}", s, src, dst),
            M68kInst::Subq(s, v, op) => format!("    subq.{} #{}, {}", s, v, op),
            M68kInst::Subi(s, v, op) => format!("    subi.{} #{}, {}", s, v, op),
            M68kInst::Muls(src, dst) => format!("    muls.w  {}, {}", src, dst),
            M68kInst::Mulu(src, dst) => format!("    mulu.w  {}, {}", src, dst),
            M68kInst::Divs(src, dst) => format!("    divs.w  {}, {}", src, dst),
            M68kInst::Divu(src, dst) => format!("    divu.w  {}, {}", src, dst),
            M68kInst::Neg(s, op) => format!("    neg.{}  {}", s, op),
            M68kInst::Ext(s, d) => format!("    ext.{}  {}", s, d),

            M68kInst::And(s, src, dst) => format!("    and.{}  {}, {}", s, src, dst),
            M68kInst::Andi(s, v, op) => format!("    andi.{} #{}, {}", s, v, op),
            M68kInst::Or(s, src, dst) => format!("    or.{}   {}, {}", s, src, dst),
            M68kInst::Ori(s, v, op) => format!("    ori.{}  #{}, {}", s, v, op),
            M68kInst::Eor(s, src, dst) => format!("    eor.{}  {}, {}", s, src, dst),
            M68kInst::Eori(s, v, op) => format!("    eori.{} #{}, {}", s, v, op),
            M68kInst::Not(s, op) => format!("    not.{}  {}", s, op),

            M68kInst::Lsl(s, cnt, d) => format!("    lsl.{}  {}, {}", s, cnt, d),
            M68kInst::Lsr(s, cnt, d) => format!("    lsr.{}  {}, {}", s, cnt, d),
            M68kInst::Asl(s, cnt, d) => format!("    asl.{}  {}, {}", s, cnt, d),
            M68kInst::Asr(s, cnt, d) => format!("    asr.{}  {}, {}", s, cnt, d),
            M68kInst::Rol(s, cnt, d) => format!("    rol.{}  {}, {}", s, cnt, d),
            M68kInst::Ror(s, cnt, d) => format!("    ror.{}  {}, {}", s, cnt, d),

            M68kInst::Btst(bit, op) => format!("    btst    {}, {}", bit, op),
            M68kInst::Bset(bit, op) => format!("    bset    {}, {}", bit, op),
            M68kInst::Bclr(bit, op) => format!("    bclr    {}, {}", bit, op),
            M68kInst::Bchg(bit, op) => format!("    bchg    {}, {}", bit, op),

            M68kInst::Cmp(s, src, dst) => format!("    cmp.{}  {}, {}", s, src, dst),
            M68kInst::Cmpa(s, src, dst) => format!("    cmpa.{} {}, {}", s, src, dst),
            M68kInst::Cmpi(s, v, op) => format!("    cmpi.{} #{}, {}", s, v, op),
            M68kInst::Tst(s, op) => format!("    tst.{}  {}", s, op),

            M68kInst::Bra(l) => format!("    bra     {}", l),
            M68kInst::Bsr(l) => format!("    bsr     {}", l),
            M68kInst::Bcc(c, l) => format!("    b{}     {}", c, l),
            M68kInst::Dbf(d, l) => format!("    dbf     {}, {}", d, l),

            M68kInst::Jmp(op) => format!("    jmp     {}", op),
            M68kInst::Jsr(op) => format!("    jsr     {}", op),

            M68kInst::Link(a, d) => format!("    link    {}, #{}", a, d),
            M68kInst::Unlk(a) => format!("    unlk    {}", a),
            M68kInst::Rts => "    rts".to_string(),
            M68kInst::Rte => "    rte".to_string(),

            M68kInst::Movem(s, regs, op, to_mem) => {
                let reg_list: Vec<String> = regs.iter().map(|r| r.to_string()).collect();
                if *to_mem {
                    format!("    movem.{} {}, {}", s, reg_list.join("/"), op)
                } else {
                    format!("    movem.{} {}, {}", s, op, reg_list.join("/"))
                }
            }

            M68kInst::Scc(c, op) => format!("    s{}     {}", c, op),

            M68kInst::Nop => "    nop".to_string(),
            M68kInst::Swap(d) => format!("    swap    {}", d),

            M68kInst::Label(l) => format!("{}:", l),
            M68kInst::Comment(c) => format!("    ; {}", c),
            M68kInst::Directive(d) => format!("    {}", d),
        }
    }
}
