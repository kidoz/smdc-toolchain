//! Declaration AST nodes

use super::{Block, CType, Expr, Initializer, StorageClass};
use crate::common::Span;

/// Declaration node
#[derive(Debug, Clone)]
pub struct Declaration {
    pub kind: DeclKind,
    pub span: Span,
}

impl Declaration {
    pub fn new(kind: DeclKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Declaration kinds
#[derive(Debug, Clone)]
pub enum DeclKind {
    /// Variable declaration: int x = 5;
    Variable(VarDecl),

    /// Multiple variable declarations: int a, b, c;
    MultipleVariables(Vec<VarDecl>),

    /// Function declaration or definition
    Function(FuncDecl),

    /// Struct declaration: struct foo { ... };
    Struct(StructDecl),

    /// Union declaration: union bar { ... };
    Union(UnionDecl),

    /// Enum declaration: enum baz { ... };
    Enum(EnumDecl),

    /// Typedef: typedef int myint;
    Typedef(TypedefDecl),
}

/// Variable declaration
#[derive(Debug, Clone)]
pub struct VarDecl {
    pub name: String,
    pub ty: CType,
    pub storage_class: Option<StorageClass>,
    pub init: Option<Initializer>,
    pub span: Span,
}

impl VarDecl {
    pub fn new(name: String, ty: CType, span: Span) -> Self {
        Self {
            name,
            ty,
            storage_class: None,
            init: None,
            span,
        }
    }

    pub fn with_storage_class(mut self, sc: StorageClass) -> Self {
        self.storage_class = Some(sc);
        self
    }

    pub fn with_init(mut self, init: Initializer) -> Self {
        self.init = Some(init);
        self
    }
}

/// Function declaration or definition
#[derive(Debug, Clone)]
pub struct FuncDecl {
    pub name: String,
    pub return_type: CType,
    pub params: Vec<ParamDecl>,
    pub variadic: bool,
    pub storage_class: Option<StorageClass>,
    pub body: Option<Block>,
    pub span: Span,
}

impl FuncDecl {
    pub fn new(name: String, return_type: CType, params: Vec<ParamDecl>, span: Span) -> Self {
        Self {
            name,
            return_type,
            params,
            variadic: false,
            storage_class: None,
            body: None,
            span,
        }
    }

    pub fn with_body(mut self, body: Block) -> Self {
        self.body = Some(body);
        self
    }

    pub fn with_variadic(mut self, variadic: bool) -> Self {
        self.variadic = variadic;
        self
    }

    pub fn with_storage_class(mut self, sc: StorageClass) -> Self {
        self.storage_class = Some(sc);
        self
    }

    /// Check if this is just a declaration (no body)
    pub fn is_declaration(&self) -> bool {
        self.body.is_none()
    }

    /// Check if this is a definition (has body)
    pub fn is_definition(&self) -> bool {
        self.body.is_some()
    }
}

/// Function parameter declaration
#[derive(Debug, Clone)]
pub struct ParamDecl {
    pub name: Option<String>,
    pub ty: CType,
    pub span: Span,
}

impl ParamDecl {
    pub fn new(name: Option<String>, ty: CType, span: Span) -> Self {
        Self { name, ty, span }
    }
}

/// Struct declaration
#[derive(Debug, Clone)]
pub struct StructDecl {
    pub name: Option<String>,
    pub members: Option<Vec<StructMember>>,
    pub span: Span,
}

impl StructDecl {
    pub fn new(name: Option<String>, members: Option<Vec<StructMember>>, span: Span) -> Self {
        Self { name, members, span }
    }
}

/// Struct/union member
#[derive(Debug, Clone)]
pub struct StructMember {
    pub name: String,
    pub ty: CType,
    pub bit_width: Option<Expr>, // For bit fields
    pub span: Span,
}

impl StructMember {
    pub fn new(name: String, ty: CType, span: Span) -> Self {
        Self {
            name,
            ty,
            bit_width: None,
            span,
        }
    }
}

/// Union declaration
#[derive(Debug, Clone)]
pub struct UnionDecl {
    pub name: Option<String>,
    pub members: Option<Vec<StructMember>>,
    pub span: Span,
}

impl UnionDecl {
    pub fn new(name: Option<String>, members: Option<Vec<StructMember>>, span: Span) -> Self {
        Self { name, members, span }
    }
}

/// Enum declaration
#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: Option<String>,
    pub variants: Option<Vec<EnumVariant>>,
    pub span: Span,
}

impl EnumDecl {
    pub fn new(name: Option<String>, variants: Option<Vec<EnumVariant>>, span: Span) -> Self {
        Self { name, variants, span }
    }
}

/// Enum variant
#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub value: Option<Expr>,
    pub span: Span,
}

impl EnumVariant {
    pub fn new(name: String, value: Option<Expr>, span: Span) -> Self {
        Self { name, value, span }
    }
}

/// Typedef declaration
#[derive(Debug, Clone)]
pub struct TypedefDecl {
    pub name: String,
    pub ty: CType,
    pub span: Span,
}

impl TypedefDecl {
    pub fn new(name: String, ty: CType, span: Span) -> Self {
        Self { name, ty, span }
    }
}
