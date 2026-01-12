//! Expression AST nodes

use super::CType;
use crate::common::Span;

/// Expression node
#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
    /// Type of this expression (filled in during semantic analysis)
    pub ty: Option<CType>,
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Self {
            kind,
            span,
            ty: None,
        }
    }

    pub fn with_type(mut self, ty: CType) -> Self {
        self.ty = Some(ty);
        self
    }
}

/// Expression kinds
#[derive(Debug, Clone)]
pub enum ExprKind {
    /// Integer literal: 42, 0xFF, 0b101
    IntLiteral(i64),

    /// Float literal: 3.14
    FloatLiteral(f64),

    /// Character literal: 'a', '\n'
    CharLiteral(char),

    /// String literal: "hello"
    StringLiteral(String),

    /// Identifier: foo, bar
    Identifier(String),

    /// Binary operation: a + b, x * y
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    /// Unary operation: -x, !flag, *ptr
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },

    /// Assignment: x = y, a += b
    Assign {
        op: AssignOp,
        target: Box<Expr>,
        value: Box<Expr>,
    },

    /// Ternary conditional: cond ? then : else
    Ternary {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },

    /// Function call: foo(a, b)
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },

    /// Array subscript: arr[i]
    Index {
        array: Box<Expr>,
        index: Box<Expr>,
    },

    /// Member access: obj.field
    Member {
        object: Box<Expr>,
        field: String,
    },

    /// Pointer member access: ptr->field
    PtrMember {
        pointer: Box<Expr>,
        field: String,
    },

    /// Type cast: (int)x
    Cast {
        ty: CType,
        expr: Box<Expr>,
    },

    /// sizeof expression: sizeof(x) or sizeof(int)
    Sizeof(SizeofArg),

    /// Address-of: &x
    AddrOf(Box<Expr>),

    /// Dereference: *ptr
    Deref(Box<Expr>),

    /// Pre-increment: ++x
    PreIncrement(Box<Expr>),

    /// Pre-decrement: --x
    PreDecrement(Box<Expr>),

    /// Post-increment: x++
    PostIncrement(Box<Expr>),

    /// Post-decrement: x--
    PostDecrement(Box<Expr>),

    /// Comma expression: (a, b, c)
    Comma(Vec<Expr>),

    /// Compound literal: (int[]){1, 2, 3}
    CompoundLiteral {
        ty: CType,
        initializers: Vec<Initializer>,
    },
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,

    // Bitwise
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,

    // Logical
    LogAnd,
    LogOr,

    // Comparison
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BinaryOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Mod => "%",
            BinaryOp::BitAnd => "&",
            BinaryOp::BitOr => "|",
            BinaryOp::BitXor => "^",
            BinaryOp::Shl => "<<",
            BinaryOp::Shr => ">>",
            BinaryOp::LogAnd => "&&",
            BinaryOp::LogOr => "||",
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Le => "<=",
            BinaryOp::Gt => ">",
            BinaryOp::Ge => ">=",
        }
    }

    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            BinaryOp::Eq
                | BinaryOp::Ne
                | BinaryOp::Lt
                | BinaryOp::Le
                | BinaryOp::Gt
                | BinaryOp::Ge
        )
    }

    pub fn is_logical(&self) -> bool {
        matches!(self, BinaryOp::LogAnd | BinaryOp::LogOr)
    }
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,     // -x
    Not,     // !x
    BitNot,  // ~x
}

impl UnaryOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "!",
            UnaryOp::BitNot => "~",
        }
    }
}

/// Assignment operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Assign,    // =
    AddAssign, // +=
    SubAssign, // -=
    MulAssign, // *=
    DivAssign, // /=
    ModAssign, // %=
    AndAssign, // &=
    OrAssign,  // |=
    XorAssign, // ^=
    ShlAssign, // <<=
    ShrAssign, // >>=
}

impl AssignOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            AssignOp::Assign => "=",
            AssignOp::AddAssign => "+=",
            AssignOp::SubAssign => "-=",
            AssignOp::MulAssign => "*=",
            AssignOp::DivAssign => "/=",
            AssignOp::ModAssign => "%=",
            AssignOp::AndAssign => "&=",
            AssignOp::OrAssign => "|=",
            AssignOp::XorAssign => "^=",
            AssignOp::ShlAssign => "<<=",
            AssignOp::ShrAssign => ">>=",
        }
    }

    /// Get the corresponding binary operator for compound assignment
    pub fn to_binary_op(&self) -> Option<BinaryOp> {
        match self {
            AssignOp::Assign => None,
            AssignOp::AddAssign => Some(BinaryOp::Add),
            AssignOp::SubAssign => Some(BinaryOp::Sub),
            AssignOp::MulAssign => Some(BinaryOp::Mul),
            AssignOp::DivAssign => Some(BinaryOp::Div),
            AssignOp::ModAssign => Some(BinaryOp::Mod),
            AssignOp::AndAssign => Some(BinaryOp::BitAnd),
            AssignOp::OrAssign => Some(BinaryOp::BitOr),
            AssignOp::XorAssign => Some(BinaryOp::BitXor),
            AssignOp::ShlAssign => Some(BinaryOp::Shl),
            AssignOp::ShrAssign => Some(BinaryOp::Shr),
        }
    }
}

/// Sizeof argument (expression or type)
#[derive(Debug, Clone)]
pub enum SizeofArg {
    Expr(Box<Expr>),
    Type(CType),
}

/// Initializer for compound literals and variable initialization
#[derive(Debug, Clone)]
pub enum Initializer {
    /// Single expression initializer
    Expr(Expr),
    /// Braced initializer list
    List(Vec<Initializer>),
    /// Designated initializer: .field = value or [index] = value
    Designated {
        designator: Designator,
        value: Box<Initializer>,
    },
}

/// Designator for designated initializers
#[derive(Debug, Clone)]
pub enum Designator {
    /// Field designator: .field
    Field(String),
    /// Array index designator: [index]
    Index(Box<Expr>),
}
