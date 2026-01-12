//! Statement AST nodes

use super::{Declaration, Expr};
use crate::common::Span;

/// Statement node
#[derive(Debug, Clone)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

impl Stmt {
    pub fn new(kind: StmtKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Statement kinds
#[derive(Debug, Clone)]
pub enum StmtKind {
    /// Expression statement: expr;
    Expr(Expr),

    /// Empty statement: ;
    Empty,

    /// Compound statement (block): { ... }
    Block(Block),

    /// If statement: if (cond) then [else else]
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },

    /// While loop: while (cond) body
    While {
        condition: Expr,
        body: Box<Stmt>,
    },

    /// Do-while loop: do body while (cond)
    DoWhile {
        body: Box<Stmt>,
        condition: Expr,
    },

    /// For loop: for (init; cond; update) body
    For {
        init: Option<ForInit>,
        condition: Option<Expr>,
        update: Option<Expr>,
        body: Box<Stmt>,
    },

    /// Switch statement: switch (expr) body
    Switch {
        expr: Expr,
        body: Box<Stmt>,
    },

    /// Case label: case expr:
    Case {
        value: Expr,
        stmt: Box<Stmt>,
    },

    /// Default label: default:
    Default(Box<Stmt>),

    /// Break statement
    Break,

    /// Continue statement
    Continue,

    /// Return statement: return [expr];
    Return(Option<Expr>),

    /// Goto statement: goto label;
    Goto(String),

    /// Labeled statement: label: stmt
    Label {
        name: String,
        stmt: Box<Stmt>,
    },

    /// Declaration statement (for C99+ block-scope declarations)
    Declaration(Declaration),
}

/// Block (compound statement)
#[derive(Debug, Clone)]
pub struct Block {
    pub items: Vec<BlockItem>,
    pub span: Span,
}

impl Block {
    pub fn new(items: Vec<BlockItem>, span: Span) -> Self {
        Self { items, span }
    }
}

/// Item inside a block
#[derive(Debug, Clone)]
pub enum BlockItem {
    Statement(Stmt),
    Declaration(Declaration),
}

/// For loop initializer (can be expression or declaration)
#[derive(Debug, Clone)]
pub enum ForInit {
    Expr(Expr),
    Declaration(Declaration),
}
