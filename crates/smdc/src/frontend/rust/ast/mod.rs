//! Rust AST definitions

mod expr;
mod item;
mod pattern;
mod stmt;
mod types;

pub use expr::*;
pub use item::*;
pub use pattern::*;
pub use stmt::*;
pub use types::*;

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
