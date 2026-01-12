//! Abstract Syntax Tree definitions

mod types;
mod expr;
mod stmt;
mod decl;

pub use types::*;
pub use expr::*;
pub use stmt::*;
pub use decl::*;

/// A complete translation unit (source file)
#[derive(Debug, Clone)]
pub struct TranslationUnit {
    pub declarations: Vec<Declaration>,
}

impl TranslationUnit {
    pub fn new(declarations: Vec<Declaration>) -> Self {
        Self { declarations }
    }
}
