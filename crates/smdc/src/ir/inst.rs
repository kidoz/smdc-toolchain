//! IR instruction definitions

use crate::types::IrType;

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
            Value::Temp(t) => write!(f, "{t}"),
            Value::IntConst(n) => write!(f, "{n}"),
            Value::StringConst(l) => write!(f, "{l}"),
            Value::Name(n) => write!(f, "{n}"),
            Value::Mem(addr) => write!(f, "[{addr}]"),
        }
    }
}

/// Binary operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,  // Signed division
    Mod,  // Signed modulo
    UDiv, // Unsigned division
    UMod, // Unsigned modulo
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
            BinOp::Div => write!(f, "/s"),
            BinOp::Mod => write!(f, "%s"),
            BinOp::UDiv => write!(f, "/u"),
            BinOp::UMod => write!(f, "%u"),
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
    Copy { dst: Temp, src: Value },

    /// dst = op src
    Unary { dst: Temp, op: UnOp, src: Value },

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
        /// If true, sign-extend the value; otherwise zero-extend
        signed: bool,
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
    CondJump { cond: Value, target: Label },

    /// Branch if condition is false
    CondJumpFalse { cond: Value, target: Label },

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
    AddrOf { dst: Temp, name: String },

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
            Inst::Label(l) => write!(f, "{l}:"),
            Inst::Copy { dst, src } => write!(f, "  {dst} = {src}"),
            Inst::Unary { dst, op, src } => write!(f, "  {dst} = {op} {src}"),
            Inst::Binary {
                dst,
                op,
                left,
                right,
            } => {
                write!(f, "  {dst} = {left} {op} {right}")
            }
            Inst::Load {
                dst,
                addr,
                size,
                volatile,
                signed,
            } => {
                let sign_str = if *signed { "s" } else { "u" };
                if *volatile {
                    write!(f, "  {dst} = load.{sign_str}{size}.volatile {addr}")
                } else {
                    write!(f, "  {dst} = load.{sign_str}{size} {addr}")
                }
            }
            Inst::Store {
                addr,
                src,
                size,
                volatile,
            } => {
                if *volatile {
                    write!(f, "  store.{size}.volatile {addr}, {src}")
                } else {
                    write!(f, "  store.{size} {addr}, {src}")
                }
            }
            Inst::Jump(l) => write!(f, "  jump {l}"),
            Inst::CondJump { cond, target } => write!(f, "  if {cond} goto {target}"),
            Inst::CondJumpFalse { cond, target } => write!(f, "  ifnot {cond} goto {target}"),
            Inst::Call { dst, func, args } => {
                if let Some(d) = dst {
                    write!(f, "  {d} = call {func}(")?;
                } else {
                    write!(f, "  call {func}(")?;
                }
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{arg}")?;
                }
                write!(f, ")")
            }
            Inst::Return(val) => {
                if let Some(v) = val {
                    write!(f, "  return {v}")
                } else {
                    write!(f, "  return")
                }
            }
            Inst::Alloca { dst, size, align } => {
                write!(f, "  {dst} = alloca {size}, align {align}")
            }
            Inst::AddrOf { dst, name } => write!(f, "  {dst} = &{name}"),
            Inst::LoadParam { dst, index, size } => {
                write!(f, "  {dst} = loadparam.{size} #{index}")
            }
            Inst::Comment(s) => write!(f, "  ; {s}"),
        }
    }
}

/// An IR instruction paired with an optional source location
#[derive(Debug, Clone)]
pub struct SpannedInst {
    pub inst: Inst,
    pub span: Option<crate::common::Span>,
}

impl SpannedInst {
    pub fn new(inst: Inst, span: Option<crate::common::Span>) -> Self {
        Self { inst, span }
    }

    pub fn bare(inst: Inst) -> Self {
        Self { inst, span: None }
    }
}

impl std::fmt::Display for SpannedInst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inst)
    }
}

/// A basic block in the IR
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub label: Label,
    pub insts: Vec<SpannedInst>,
}

impl BasicBlock {
    pub fn new(label: Label) -> Self {
        Self {
            label,
            insts: Vec::new(),
        }
    }
}

impl std::fmt::Display for BasicBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}:", self.label)?;
        for sinst in &self.insts {
            writeln!(f, "{sinst}")?;
        }
        Ok(())
    }
}

/// A function in IR form
#[derive(Debug, Clone)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<(String, IrType)>,
    pub return_type: IrType,
    pub blocks: Vec<BasicBlock>,
    pub locals: Vec<(String, IrType, usize)>, // name, type, stack offset
}

impl IrFunction {
    pub fn new(name: String, params: Vec<(String, IrType)>, return_type: IrType) -> Self {
        Self {
            name,
            params,
            return_type,
            blocks: Vec::new(),
            locals: Vec::new(),
        }
    }
}

impl std::fmt::Display for IrFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "function {}:", self.name)?;
        for block in &self.blocks {
            write!(f, "{block}")?;
        }
        Ok(())
    }
}

/// Global variable in IR
#[derive(Debug, Clone)]
pub struct IrGlobal {
    pub name: String,
    pub ty: IrType,
    pub init: Option<Vec<u8>>,
}

/// Source-level debug information attached to an IR module
#[derive(Debug, Clone)]
pub struct DebugInfo {
    /// Source filename
    pub filename: String,
    /// Full source text (for byte-offset → line mapping)
    pub source: String,
}

/// IR module (translation unit)
#[derive(Debug, Clone)]
pub struct IrModule {
    pub functions: Vec<IrFunction>,
    pub globals: Vec<IrGlobal>,
    pub strings: Vec<(Label, String)>,
    /// Debug information from the frontend (populated when available)
    pub debug_info: Option<DebugInfo>,
}

impl IrModule {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            globals: Vec::new(),
            strings: Vec::new(),
            debug_info: None,
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
            write!(f, "{func}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_basic_block_display() {
        let mut bb = BasicBlock::new(Label("L_test".to_string()));
        bb.insts
            .push(SpannedInst::bare(Inst::Comment("test comment".to_string())));
        bb.insts.push(SpannedInst::bare(Inst::Return(None)));

        let output = format!("{bb}");
        assert!(output.contains("L_test:"));
        assert!(output.contains("  ; test comment"));
        assert!(output.contains("  return"));
    }

    #[test]
    fn test_ir_function_display() {
        let mut func = IrFunction::new(
            "my_func".to_string(),
            vec![("arg".to_string(), IrType::i32())],
            IrType::void(),
        );

        let mut bb = BasicBlock::new(Label("entry".to_string()));
        bb.insts.push(SpannedInst::bare(Inst::Return(None)));
        func.blocks.push(bb);

        let output = format!("{func}");
        assert!(output.contains("function my_func:"));
        assert!(output.contains("entry:"));
        assert!(output.contains("  return"));
    }

    #[test]
    fn test_ir_module_display() {
        let mut module = IrModule::new();

        module.globals.push(IrGlobal {
            name: "g_var".to_string(),
            ty: IrType::i32(),
            init: None,
        });

        module
            .strings
            .push((Label("str_1".to_string()), "hello world".to_string()));

        let output = format!("{module}");
        assert!(output.contains("global g_var:"));
        assert!(output.contains("str_1: \"hello world\""));
        assert!(module.debug_info.is_none());
    }

    #[test]
    fn test_spanned_inst_with_span() {
        let span = crate::common::Span::new(10, 20);
        let sinst = SpannedInst::new(
            Inst::Copy {
                dst: Temp(0),
                src: Value::IntConst(42),
            },
            Some(span),
        );
        assert_eq!(sinst.span, Some(span));
        assert_eq!(format!("{sinst}"), "  t0 = 42");
    }

    #[test]
    fn test_spanned_inst_bare() {
        let sinst = SpannedInst::bare(Inst::Return(None));
        assert!(sinst.span.is_none());
        assert_eq!(format!("{sinst}"), "  return");
    }

    #[test]
    fn test_debug_info() {
        let mut module = IrModule::new();
        module.debug_info = Some(DebugInfo {
            filename: "test.c".to_string(),
            source: "int main() { return 0; }".to_string(),
        });
        let di = module.debug_info.as_ref().unwrap();
        assert_eq!(di.filename, "test.c");
    }

    #[test]
    fn test_inst_display() {
        let inst1 = Inst::Copy {
            dst: Temp(1),
            src: Value::IntConst(42),
        };
        assert_eq!(format!("{}", inst1), "  t1 = 42");

        let inst2 = Inst::Binary {
            dst: Temp(2),
            op: BinOp::Add,
            left: Value::Temp(Temp(1)),
            right: Value::IntConst(1),
        };
        assert_eq!(format!("{}", inst2), "  t2 = t1 + 1");
    }
}
