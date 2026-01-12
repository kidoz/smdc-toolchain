//! Rust statement AST nodes

use super::{Expr, RustType, Pattern, Item};
use crate::common::Span;

/// A block of statements
#[derive(Debug, Clone)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub expr: Option<Box<Expr>>, // Trailing expression (no semicolon)
    pub span: Span,
}

impl Block {
    pub fn new(stmts: Vec<Stmt>, expr: Option<Expr>, span: Span) -> Self {
        Self {
            stmts,
            expr: expr.map(Box::new),
            span,
        }
    }

    pub fn empty(span: Span) -> Self {
        Self {
            stmts: Vec::new(),
            expr: None,
            span,
        }
    }
}

/// A statement
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
    /// Local variable binding: let x = 5;
    Let {
        pattern: Pattern,
        ty: Option<RustType>,
        init: Option<Expr>,
    },

    /// Expression statement: foo();
    Expr(Expr),

    /// Expression statement without semicolon (only valid as last in block)
    /// This is handled by Block.expr, but we keep this for clarity
    ExprNoSemi(Expr),

    /// Item declaration (fn, struct, etc. inside a block)
    Item(Item),

    /// Empty statement: ;
    Empty,
}

/// A local variable declaration
#[derive(Debug, Clone)]
pub struct Local {
    pub pattern: Pattern,
    pub ty: Option<RustType>,
    pub init: Option<Expr>,
    pub span: Span,
}

impl Local {
    pub fn new(pattern: Pattern, ty: Option<RustType>, init: Option<Expr>, span: Span) -> Self {
        Self { pattern, ty, init, span }
    }
}
