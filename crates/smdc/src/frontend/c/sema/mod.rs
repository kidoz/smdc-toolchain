//! Semantic analysis module
//!
//! This module performs type checking and semantic validation.

mod scope;
mod analyzer;

pub use scope::{Scope, Symbol, SymbolKind};
pub use analyzer::SemanticAnalyzer;
