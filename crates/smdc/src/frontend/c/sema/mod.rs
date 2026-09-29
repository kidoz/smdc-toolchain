//! Semantic analysis module
//!
//! This module performs type checking and semantic validation.

mod analyzer;
mod initializer;
mod scope;

pub use analyzer::SemanticAnalyzer;
pub use initializer::{InitEntry, InitLayout, InitValue, layout_initializer};
pub use scope::{Scope, Symbol, SymbolKind};
