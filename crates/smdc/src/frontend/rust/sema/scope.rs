//! Rust scope and symbol table

use crate::frontend::rust::ast::{RustType, FnDecl, StructDecl, EnumDecl};
use std::collections::HashMap;

/// A symbol in the Rust symbol table
#[derive(Debug, Clone)]
pub struct RustSymbol {
    pub name: String,
    pub kind: RustSymbolKind,
    pub ty: RustType,
    pub mutable: bool,
    pub moved: bool, // For ownership tracking
}

impl RustSymbol {
    pub fn new(name: String, kind: RustSymbolKind, ty: RustType) -> Self {
        Self {
            name,
            kind,
            ty,
            mutable: false,
            moved: false,
        }
    }

    pub fn with_mutability(mut self, mutable: bool) -> Self {
        self.mutable = mutable;
        self
    }
}

/// Kind of symbol
#[derive(Debug, Clone)]
pub enum RustSymbolKind {
    /// Local variable
    Variable,
    /// Function parameter
    Parameter,
    /// Function
    Function(Box<FnDecl>),
    /// Struct type
    Struct(Box<StructDecl>),
    /// Enum type
    Enum(Box<EnumDecl>),
    /// Enum variant
    EnumVariant {
        enum_name: String,
        variant_index: usize,
    },
    /// Type alias
    TypeAlias,
    /// Constant
    Const,
    /// Static variable
    Static { mutable: bool },
    /// Module
    Module,
}

/// Scope for Rust symbol resolution
#[derive(Debug)]
pub struct RustScope {
    symbols: HashMap<String, RustSymbol>,
    types: HashMap<String, RustType>,
    parent: Option<Box<RustScope>>,
    loop_depth: usize,
    in_unsafe: bool,
}

impl RustScope {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            types: HashMap::new(),
            parent: None,
            loop_depth: 0,
            in_unsafe: false,
        }
    }

    /// Define a new symbol in the current scope
    pub fn define(&mut self, symbol: RustSymbol) -> Result<(), String> {
        if self.symbols.contains_key(&symbol.name) {
            return Err(format!("symbol '{}' already defined in this scope", symbol.name));
        }
        self.symbols.insert(symbol.name.clone(), symbol);
        Ok(())
    }

    /// Define a type in the current scope
    pub fn define_type(&mut self, name: String, ty: RustType) -> Result<(), String> {
        if self.types.contains_key(&name) {
            return Err(format!("type '{}' already defined in this scope", name));
        }
        self.types.insert(name, ty);
        Ok(())
    }

    /// Look up a symbol by name
    pub fn lookup(&self, name: &str) -> Option<&RustSymbol> {
        if let Some(sym) = self.symbols.get(name) {
            Some(sym)
        } else if let Some(parent) = &self.parent {
            parent.lookup(name)
        } else {
            None
        }
    }

    /// Look up a symbol by name (mutable)
    pub fn lookup_mut(&mut self, name: &str) -> Option<&mut RustSymbol> {
        if self.symbols.contains_key(name) {
            self.symbols.get_mut(name)
        } else if let Some(parent) = &mut self.parent {
            parent.lookup_mut(name)
        } else {
            None
        }
    }

    /// Look up a type by name
    pub fn lookup_type(&self, name: &str) -> Option<&RustType> {
        if let Some(ty) = self.types.get(name) {
            Some(ty)
        } else if let Some(parent) = &self.parent {
            parent.lookup_type(name)
        } else {
            None
        }
    }

    /// Push a new child scope
    pub fn push_child(&mut self) {
        let old_scope = std::mem::replace(self, RustScope::new());
        self.parent = Some(Box::new(old_scope));
        if let Some(parent) = &self.parent {
            self.loop_depth = parent.loop_depth;
            self.in_unsafe = parent.in_unsafe;
        }
    }

    /// Pop to parent scope
    pub fn pop_to_parent(&mut self) -> bool {
        if let Some(parent) = self.parent.take() {
            *self = *parent;
            true
        } else {
            false
        }
    }

    /// Enter a loop
    pub fn enter_loop(&mut self) {
        self.loop_depth += 1;
    }

    /// Exit a loop
    pub fn exit_loop(&mut self) {
        if self.loop_depth > 0 {
            self.loop_depth -= 1;
        }
    }

    /// Check if we're inside a loop
    pub fn in_loop(&self) -> bool {
        self.loop_depth > 0
    }

    /// Enter an unsafe block
    pub fn enter_unsafe(&mut self) {
        self.in_unsafe = true;
    }

    /// Exit an unsafe block
    pub fn exit_unsafe(&mut self) {
        self.in_unsafe = false;
    }

    /// Check if we're in an unsafe context
    pub fn is_unsafe(&self) -> bool {
        self.in_unsafe
    }

    /// Mark a symbol as moved
    pub fn mark_moved(&mut self, name: &str) -> Result<(), String> {
        if let Some(sym) = self.lookup_mut(name) {
            if sym.moved {
                return Err(format!("use of moved value '{}'", name));
            }
            sym.moved = true;
            Ok(())
        } else {
            Err(format!("undefined symbol '{}'", name))
        }
    }

    /// Check if a symbol has been moved
    pub fn is_moved(&self, name: &str) -> bool {
        self.lookup(name).map(|s| s.moved).unwrap_or(false)
    }
}

impl Default for RustScope {
    fn default() -> Self {
        Self::new()
    }
}
