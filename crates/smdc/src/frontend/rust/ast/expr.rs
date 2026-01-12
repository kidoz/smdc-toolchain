//! Rust expression AST nodes

use super::{RustType, TypePath, Block, Pattern};
use crate::common::Span;

/// A Rust expression
#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
    pub ty: Option<RustType>,
}

impl Expr {
    pub fn new(kind: ExprKind, span: Span) -> Self {
        Self { kind, span, ty: None }
    }
}

/// Expression kinds
#[derive(Debug, Clone)]
pub enum ExprKind {
    /// Integer literal: 42, 0xFF, 0b1010
    IntLiteral(i64),
    /// Float literal: 3.14
    FloatLiteral(f64),
    /// Boolean literal: true, false
    BoolLiteral(bool),
    /// Character literal: 'a'
    CharLiteral(char),
    /// String literal: "hello"
    StringLiteral(String),
    /// Byte literal: b'a'
    ByteLiteral(u8),
    /// Byte string literal: b"hello"
    ByteStringLiteral(Vec<u8>),

    /// Identifier: x, foo
    Identifier(String),
    /// Path: std::vec::Vec
    Path(TypePath),

    /// Binary operation: a + b
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// Unary operation: -x, !x, *x, &x
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },

    /// Assignment: x = 5, x += 1
    Assign {
        target: Box<Expr>,
        op: Option<BinOp>, // None for =, Some for +=, -= etc.
        value: Box<Expr>,
    },

    /// Function/method call: foo(x, y)
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    /// Method call: x.foo(y)
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },

    /// Field access: x.field
    Field {
        object: Box<Expr>,
        field: String,
    },
    /// Tuple field access: x.0
    TupleField {
        object: Box<Expr>,
        index: usize,
    },
    /// Index: array[i]
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },

    /// Reference: &x, &mut x
    Reference {
        mutable: bool,
        operand: Box<Expr>,
    },
    /// Dereference: *x
    Dereference(Box<Expr>),

    /// Type cast: x as i32
    Cast {
        expr: Box<Expr>,
        ty: RustType,
    },

    /// Block expression: { ... }
    Block(Block),

    /// If expression: if cond { } else { }
    If {
        condition: Box<Expr>,
        then_block: Block,
        else_block: Option<Box<Expr>>, // Can be another If or Block
    },

    /// Loop: loop { }
    Loop {
        label: Option<String>,
        body: Block,
    },
    /// While loop: while cond { }
    While {
        label: Option<String>,
        condition: Box<Expr>,
        body: Block,
    },
    /// For loop: for x in iter { }
    For {
        label: Option<String>,
        pattern: Pattern,
        iter: Box<Expr>,
        body: Block,
    },

    /// Match expression: match x { ... }
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },

    /// Break: break, break 'label, break value
    Break {
        label: Option<String>,
        value: Option<Box<Expr>>,
    },
    /// Continue: continue, continue 'label
    Continue {
        label: Option<String>,
    },
    /// Return: return, return value
    Return(Option<Box<Expr>>),

    /// Range: a..b, a..=b, ..b, a..
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
    },

    /// Tuple: (a, b, c)
    Tuple(Vec<Expr>),
    /// Array: [a, b, c]
    Array(Vec<Expr>),
    /// Array with repeat: [0; 10]
    ArrayRepeat {
        value: Box<Expr>,
        count: Box<Expr>,
    },

    /// Struct literal: Foo { x: 1, y: 2 }
    Struct {
        path: TypePath,
        fields: Vec<FieldInit>,
        rest: Option<Box<Expr>>, // ..default
    },

    /// Closure: |x| x + 1 (simplified, no capture)
    Closure {
        params: Vec<(Pattern, Option<RustType>)>,
        return_type: Option<RustType>,
        body: Box<Expr>,
    },

    /// Unsafe block: unsafe { ... }
    Unsafe(Block),

    /// Grouped expression: (expr)
    Paren(Box<Expr>),
}

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    // Arithmetic
    Add,    // +
    Sub,    // -
    Mul,    // *
    Div,    // /
    Rem,    // %

    // Bitwise
    BitAnd, // &
    BitOr,  // |
    BitXor, // ^
    Shl,    // <<
    Shr,    // >>

    // Logical
    And,    // &&
    Or,     // ||

    // Comparison
    Eq,     // ==
    Ne,     // !=
    Lt,     // <
    Le,     // <=
    Gt,     // >
    Ge,     // >=
}

impl BinOp {
    pub fn is_comparison(&self) -> bool {
        matches!(self, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge)
    }

    pub fn is_logical(&self) -> bool {
        matches!(self, BinOp::And | BinOp::Or)
    }

    pub fn precedence(&self) -> u8 {
        match self {
            BinOp::Or => 1,
            BinOp::And => 2,
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => 3,
            BinOp::BitOr => 4,
            BinOp::BitXor => 5,
            BinOp::BitAnd => 6,
            BinOp::Shl | BinOp::Shr => 7,
            BinOp::Add | BinOp::Sub => 8,
            BinOp::Mul | BinOp::Div | BinOp::Rem => 9,
        }
    }
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinOp::Add => write!(f, "+"),
            BinOp::Sub => write!(f, "-"),
            BinOp::Mul => write!(f, "*"),
            BinOp::Div => write!(f, "/"),
            BinOp::Rem => write!(f, "%"),
            BinOp::BitAnd => write!(f, "&"),
            BinOp::BitOr => write!(f, "|"),
            BinOp::BitXor => write!(f, "^"),
            BinOp::Shl => write!(f, "<<"),
            BinOp::Shr => write!(f, ">>"),
            BinOp::And => write!(f, "&&"),
            BinOp::Or => write!(f, "||"),
            BinOp::Eq => write!(f, "=="),
            BinOp::Ne => write!(f, "!="),
            BinOp::Lt => write!(f, "<"),
            BinOp::Le => write!(f, "<="),
            BinOp::Gt => write!(f, ">"),
            BinOp::Ge => write!(f, ">="),
        }
    }
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,    // -
    Not,    // !
    Deref,  // *
    Ref,    // &
    RefMut, // &mut
}

impl std::fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnaryOp::Neg => write!(f, "-"),
            UnaryOp::Not => write!(f, "!"),
            UnaryOp::Deref => write!(f, "*"),
            UnaryOp::Ref => write!(f, "&"),
            UnaryOp::RefMut => write!(f, "&mut"),
        }
    }
}

/// A match arm: pattern => body
#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
    pub span: Span,
}

/// Field initializer in struct literal: field: value or just field
#[derive(Debug, Clone)]
pub struct FieldInit {
    pub name: String,
    pub value: Option<Expr>, // None for shorthand (field instead of field: field)
    pub span: Span,
}
