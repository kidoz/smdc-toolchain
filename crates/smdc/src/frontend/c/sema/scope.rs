//! Symbol table and scope management

use crate::frontend::c::ast::CType;
use std::collections::HashMap;

/// A symbol in the symbol table
#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub ty: CType,
}

/// Kind of symbol
#[derive(Debug, Clone)]
pub enum SymbolKind {
    Variable,
    Function,
    Parameter,
    Typedef,
    EnumConstant(i64),
}

/// Struct type definition
#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub members: Vec<(String, CType)>,
}

/// Union type definition
#[derive(Debug, Clone)]
pub struct UnionDef {
    pub name: String,
    pub members: Vec<(String, CType)>,
}

/// A scope containing symbols
#[derive(Debug)]
pub struct Scope {
    symbols: HashMap<String, Symbol>,
    structs: HashMap<String, StructDef>,
    unions: HashMap<String, UnionDef>,
    labels: HashMap<String, bool>, // label name -> defined
    parent: Option<Box<Scope>>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            structs: HashMap::new(),
            unions: HashMap::new(),
            labels: HashMap::new(),
            parent: None,
        }
    }

    pub fn with_parent(parent: Scope) -> Self {
        Self {
            symbols: HashMap::new(),
            structs: HashMap::new(),
            unions: HashMap::new(),
            labels: HashMap::new(),
            parent: Some(Box::new(parent)),
        }
    }

    pub fn define(&mut self, symbol: Symbol) -> Result<(), String> {
        if self.symbols.contains_key(&symbol.name) {
            return Err(format!("symbol '{}' already defined in this scope", symbol.name));
        }
        self.symbols.insert(symbol.name.clone(), symbol);
        Ok(())
    }

    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        if let Some(sym) = self.symbols.get(name) {
            Some(sym)
        } else if let Some(parent) = &self.parent {
            parent.lookup(name)
        } else {
            None
        }
    }

    pub fn lookup_local(&self, name: &str) -> Option<&Symbol> {
        self.symbols.get(name)
    }

    /// Define a struct type in the current scope
    pub fn define_struct(&mut self, def: StructDef) -> Result<(), String> {
        if self.structs.contains_key(&def.name) {
            return Err(format!("struct '{}' already defined in this scope", def.name));
        }
        self.structs.insert(def.name.clone(), def);
        Ok(())
    }

    /// Look up a struct type by name
    pub fn lookup_struct(&self, name: &str) -> Option<&StructDef> {
        if let Some(def) = self.structs.get(name) {
            Some(def)
        } else if let Some(parent) = &self.parent {
            parent.lookup_struct(name)
        } else {
            None
        }
    }

    /// Define a union type in the current scope
    pub fn define_union(&mut self, def: UnionDef) -> Result<(), String> {
        if self.unions.contains_key(&def.name) {
            return Err(format!("union '{}' already defined in this scope", def.name));
        }
        self.unions.insert(def.name.clone(), def);
        Ok(())
    }

    /// Look up a union type by name
    pub fn lookup_union(&self, name: &str) -> Option<&UnionDef> {
        if let Some(def) = self.unions.get(name) {
            Some(def)
        } else if let Some(parent) = &self.parent {
            parent.lookup_union(name)
        } else {
            None
        }
    }

    /// Define a label in the current function scope
    pub fn define_label(&mut self, name: &str) -> Result<(), String> {
        if let Some(defined) = self.labels.get(name) {
            if *defined {
                return Err(format!("label '{}' already defined", name));
            }
        }
        self.labels.insert(name.to_string(), true);
        Ok(())
    }

    /// Reference a label (for goto)
    pub fn reference_label(&mut self, name: &str) {
        if !self.labels.contains_key(name) {
            self.labels.insert(name.to_string(), false);
        }
    }

    /// Check if all referenced labels are defined
    pub fn check_labels(&self) -> Result<(), Vec<String>> {
        let undefined: Vec<String> = self.labels
            .iter()
            .filter(|(_, defined)| !**defined)
            .map(|(name, _)| name.clone())
            .collect();
        if undefined.is_empty() {
            Ok(())
        } else {
            Err(undefined)
        }
    }

    /// Clear labels (for new function scope)
    pub fn clear_labels(&mut self) {
        self.labels.clear();
    }

    /// Take the parent scope, replacing self with the parent
    pub fn pop_to_parent(&mut self) -> bool {
        if let Some(parent) = self.parent.take() {
            *self = *parent;
            true
        } else {
            false
        }
    }

    /// Push a new child scope
    pub fn push_child(&mut self) {
        let old_scope = std::mem::replace(self, Scope::new());
        self.parent = Some(Box::new(old_scope));
    }
}

impl Default for Scope {
    fn default() -> Self {
        Self::new()
    }
}
