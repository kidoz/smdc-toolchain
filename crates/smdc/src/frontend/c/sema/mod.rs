//! Semantic analysis module
//!
//! This module performs type checking and semantic validation.

mod analyzer;
mod scope;

pub use analyzer::SemanticAnalyzer;
pub use scope::{Scope, Symbol, SymbolKind};
