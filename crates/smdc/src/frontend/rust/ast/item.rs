//! Rust item AST nodes (top-level declarations)

use super::{RustType, Block, Expr, Pattern, TypePath};
use crate::common::Span;

/// A top-level item
#[derive(Debug, Clone)]
pub struct Item {
    pub kind: ItemKind,
    pub visibility: Visibility,
    pub attrs: Vec<Attribute>,
    pub span: Span,
}

impl Item {
    pub fn new(kind: ItemKind, visibility: Visibility, span: Span) -> Self {
        Self {
            kind,
            visibility,
            attrs: Vec::new(),
            span,
        }
    }

    pub fn with_attrs(mut self, attrs: Vec<Attribute>) -> Self {
        self.attrs = attrs;
        self
    }
}

/// Item kinds
#[derive(Debug, Clone)]
pub enum ItemKind {
    /// Function: fn foo() { }
    Fn(FnDecl),
    /// Struct: struct Foo { }
    Struct(StructDecl),
    /// Enum: enum Foo { }
    Enum(EnumDecl),
    /// Impl block: impl Foo { }
    Impl(ImplDecl),
    /// Type alias: type Foo = Bar;
    TypeAlias(TypeAliasDecl),
    /// Constant: const FOO: i32 = 42;
    Const(ConstDecl),
    /// Static: static FOO: i32 = 42;
    Static(StaticDecl),
    /// Module: mod foo { } or mod foo;
    Mod(ModDecl),
    /// Use declaration: use std::vec::Vec;
    Use(UseDecl),
}

/// Visibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Visibility {
    #[default]
    Private,
    Public,
    Crate,
    Super,
}

/// An attribute: #[...]
#[derive(Debug, Clone)]
pub struct Attribute {
    pub path: TypePath,
    pub args: Option<String>, // Simplified: just store the raw args
    pub span: Span,
}

/// Function declaration
#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub params: Vec<FnParam>,
    pub return_type: Option<RustType>,
    pub body: Option<Block>,
    pub is_unsafe: bool,
    pub is_const: bool,
    pub span: Span,
}

impl FnDecl {
    pub fn new(name: String, params: Vec<FnParam>, return_type: Option<RustType>, body: Option<Block>, span: Span) -> Self {
        Self {
            name,
            params,
            return_type,
            body,
            is_unsafe: false,
            is_const: false,
            span,
        }
    }
}

/// Function parameter
#[derive(Debug, Clone)]
pub struct FnParam {
    pub pattern: Pattern,
    pub ty: RustType,
    pub span: Span,
}

impl FnParam {
    pub fn new(pattern: Pattern, ty: RustType, span: Span) -> Self {
        Self { pattern, ty, span }
    }
}

/// Struct declaration
#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: String,
    pub kind: StructKind,
    pub span: Span,
}

/// Struct kind (named fields, tuple, or unit)
#[derive(Debug, Clone)]
pub enum StructKind {
    /// Named fields: struct Foo { x: i32, y: i32 }
    Named(Vec<StructField>),
    /// Tuple struct: struct Foo(i32, i32);
    Tuple(Vec<TupleField>),
    /// Unit struct: struct Foo;
    Unit,
}

/// A named struct field
#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub ty: RustType,
    pub visibility: Visibility,
    pub span: Span,
}

/// A tuple struct field
#[derive(Debug, Clone)]
pub struct TupleField {
    pub ty: RustType,
    pub visibility: Visibility,
    pub span: Span,
}

/// Enum declaration
#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub span: Span,
}

/// Enum variant
#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub kind: VariantKind,
    pub discriminant: Option<Expr>,
    pub span: Span,
}

/// Variant kind
#[derive(Debug, Clone)]
pub enum VariantKind {
    /// Unit variant: Foo
    Unit,
    /// Tuple variant: Foo(i32, i32)
    Tuple(Vec<TupleField>),
    /// Struct variant: Foo { x: i32 }
    Struct(Vec<StructField>),
}

/// Impl block
#[derive(Debug, Clone)]
pub struct ImplDecl {
    pub self_ty: RustType,
    pub items: Vec<Item>,
    pub span: Span,
}

/// Type alias
#[derive(Debug, Clone)]
pub struct TypeAliasDecl {
    pub name: String,
    pub ty: RustType,
    pub span: Span,
}

/// Constant declaration
#[derive(Debug, Clone)]
pub struct ConstDecl {
    pub name: String,
    pub ty: RustType,
    pub value: Expr,
    pub span: Span,
}

/// Static declaration
#[derive(Debug, Clone)]
pub struct StaticDecl {
    pub name: String,
    pub ty: RustType,
    pub value: Expr,
    pub mutable: bool,
    pub span: Span,
}

/// Module declaration
#[derive(Debug, Clone)]
pub struct ModDecl {
    pub name: String,
    pub items: Option<Vec<Item>>, // None for mod foo; (external file)
    pub span: Span,
}

/// Use declaration
#[derive(Debug, Clone)]
pub struct UseDecl {
    pub tree: UseTree,
    pub span: Span,
}

/// Use tree
#[derive(Debug, Clone)]
pub enum UseTree {
    /// Simple path: use foo::bar;
    Path {
        prefix: TypePath,
        tree: Option<Box<UseTree>>,
    },
    /// Rename: use foo as bar;
    Rename {
        path: TypePath,
        alias: String,
    },
    /// Glob: use foo::*;
    Glob(TypePath),
    /// Nested: use foo::{bar, baz};
    Nested {
        prefix: TypePath,
        trees: Vec<UseTree>,
    },
}
