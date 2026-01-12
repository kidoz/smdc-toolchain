//! IR instruction definitions

use crate::frontend::c::ast::CType;

/// A temporary value (virtual register)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Temp(pub u32);

impl std::fmt::Display for Temp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "t{}", self.0)
    }
}

/// A label in the IR
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Label(pub String);

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An IR value (operand)
#[derive(Debug, Clone)]
pub enum Value {
    /// A temporary (virtual register)
    Temp(Temp),
    /// An integer constant
    IntConst(i64),
    /// A string constant (label to string data)
    StringConst(Label),
    /// A named variable/parameter
    Name(String),
    /// Memory location at address
    Mem(Box<Value>),
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Temp(t) => write!(f, "{}", t),
            Value::IntConst(n) => write!(f, "{}", n),
            Value::StringConst(l) => write!(f, "{}", l),
            Value::Name(n) => write!(f, "{}", n),
            Value::Mem(addr) => write!(f, "[{}]", addr),
        }
    }
}

/// Binary operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinOp::Add => write!(f, "+"),
            BinOp::Sub => write!(f, "-"),
            BinOp::Mul => write!(f, "*"),
            BinOp::Div => write!(f, "/"),
            BinOp::Mod => write!(f, "%"),
            BinOp::And => write!(f, "&"),
            BinOp::Or => write!(f, "|"),
            BinOp::Xor => write!(f, "^"),
            BinOp::Shl => write!(f, "<<"),
            BinOp::Shr => write!(f, ">>"),
            BinOp::Eq => write!(f, "=="),
            BinOp::Ne => write!(f, "!="),
            BinOp::Lt => write!(f, "<"),
            BinOp::Le => write!(f, "<="),
            BinOp::Gt => write!(f, ">"),
            BinOp::Ge => write!(f, ">="),
        }
    }
}

/// Unary operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    BitNot,
}

impl std::fmt::Display for UnOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnOp::Neg => write!(f, "-"),
            UnOp::Not => write!(f, "!"),
            UnOp::BitNot => write!(f, "~"),
        }
    }
}

/// IR instruction
#[derive(Debug, Clone)]
pub enum Inst {
    /// Label definition
    Label(Label),

    /// dst = src (copy)
    Copy {
        dst: Temp,
        src: Value,
    },

    /// dst = op src
    Unary {
        dst: Temp,
        op: UnOp,
        src: Value,
    },

    /// dst = left op right
    Binary {
        dst: Temp,
        op: BinOp,
        left: Value,
        right: Value,
    },

    /// dst = *src (load from memory)
    Load {
        dst: Temp,
        addr: Value,
        size: usize,
        /// If true, this is a volatile access (no optimization)
        volatile: bool,
    },

    /// *dst = src (store to memory)
    Store {
        addr: Value,
        src: Value,
        size: usize,
        /// If true, this is a volatile access (no optimization)
        volatile: bool,
    },

    /// Unconditional jump
    Jump(Label),

    /// Conditional jump: if cond goto label
    CondJump {
        cond: Value,
        target: Label,
    },

    /// Branch if condition is false
    CondJumpFalse {
        cond: Value,
        target: Label,
    },

    /// Function call: dst = func(args...)
    Call {
        dst: Option<Temp>,
        func: String,
        args: Vec<Value>,
    },

    /// Return from function
    Return(Option<Value>),

    /// Allocate stack space for local variable
    Alloca {
        dst: Temp,
        size: usize,
        align: usize,
    },

    /// Get address of named variable
    AddrOf {
        dst: Temp,
        name: String,
    },

    /// Load function parameter from stack
    LoadParam {
        dst: Temp,
        index: usize,
        size: usize,
    },

    /// Comment (for debugging)
    Comment(String),
}

impl std::fmt::Display for Inst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Inst::Label(l) => write!(f, "{}:", l),
            Inst::Copy { dst, src } => write!(f, "  {} = {}", dst, src),
            Inst::Unary { dst, op, src } => write!(f, "  {} = {} {}", dst, op, src),
            Inst::Binary { dst, op, left, right } => {
                write!(f, "  {} = {} {} {}", dst, left, op, right)
            }
            Inst::Load { dst, addr, size, volatile } => {
                if *volatile {
                    write!(f, "  {} = load.{}.volatile {}", dst, size, addr)
                } else {
                    write!(f, "  {} = load.{} {}", dst, size, addr)
                }
            }
            Inst::Store { addr, src, size, volatile } => {
                if *volatile {
                    write!(f, "  store.{}.volatile {}, {}", size, addr, src)
                } else {
                    write!(f, "  store.{} {}, {}", size, addr, src)
                }
            }
            Inst::Jump(l) => write!(f, "  jump {}", l),
            Inst::CondJump { cond, target } => write!(f, "  if {} goto {}", cond, target),
            Inst::CondJumpFalse { cond, target } => write!(f, "  ifnot {} goto {}", cond, target),
            Inst::Call { dst, func, args } => {
                if let Some(d) = dst {
                    write!(f, "  {} = call {}(", d, func)?;
                } else {
                    write!(f, "  call {}(", func)?;
                }
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
            Inst::Return(val) => {
                if let Some(v) = val {
                    write!(f, "  return {}", v)
                } else {
                    write!(f, "  return")
                }
            }
            Inst::Alloca { dst, size, align } => {
                write!(f, "  {} = alloca {}, align {}", dst, size, align)
            }
            Inst::AddrOf { dst, name } => write!(f, "  {} = &{}", dst, name),
            Inst::LoadParam { dst, index, size } => {
                write!(f, "  {} = loadparam.{} #{}", dst, size, index)
            }
            Inst::Comment(s) => write!(f, "  ; {}", s),
        }
    }
}

/// A function in IR form
#[derive(Debug, Clone)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<(String, CType)>,
    pub return_type: CType,
    pub body: Vec<Inst>,
    pub locals: Vec<(String, CType, usize)>, // name, type, stack offset
}

impl IrFunction {
    pub fn new(name: String, params: Vec<(String, CType)>, return_type: CType) -> Self {
        Self {
            name,
            params,
            return_type,
            body: Vec::new(),
            locals: Vec::new(),
        }
    }
}

impl std::fmt::Display for IrFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "function {}:", self.name)?;
        for inst in &self.body {
            writeln!(f, "{}", inst)?;
        }
        Ok(())
    }
}

/// Global variable in IR
#[derive(Debug, Clone)]
pub struct IrGlobal {
    pub name: String,
    pub ty: CType,
    pub init: Option<Vec<u8>>,
}

/// IR module (translation unit)
#[derive(Debug, Clone)]
pub struct IrModule {
    pub functions: Vec<IrFunction>,
    pub globals: Vec<IrGlobal>,
    pub strings: Vec<(Label, String)>,
}

impl IrModule {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            globals: Vec::new(),
            strings: Vec::new(),
        }
    }
}

impl Default for IrModule {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for IrModule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for global in &self.globals {
            writeln!(f, "global {}: {:?}", global.name, global.ty)?;
        }
        for (label, s) in &self.strings {
            writeln!(f, "{}: \"{}\"", label, s.escape_default())?;
        }
        for func in &self.functions {
            writeln!(f)?;
            write!(f, "{}", func)?;
        }
        Ok(())
    }
}
