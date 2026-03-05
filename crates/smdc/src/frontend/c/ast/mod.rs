//! Abstract Syntax Tree definitions

mod decl;
mod expr;
mod stmt;
mod types;

pub use decl::*;
pub use expr::*;
pub use stmt::*;
pub use types::*;

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
