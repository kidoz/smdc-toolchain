//! Rust AST definitions

mod types;
mod expr;
mod stmt;
mod item;
mod pattern;

pub use types::*;
pub use expr::*;
pub use stmt::*;
pub use item::*;
pub use pattern::*;

use crate::common::Span;

/// A complete Rust source file (crate root or module)
#[derive(Debug, Clone)]
pub struct RustModule {
    pub items: Vec<Item>,
    pub span: Span,
}

impl RustModule {
    pub fn new(items: Vec<Item>, span: Span) -> Self {
        Self { items, span }
    }
}
