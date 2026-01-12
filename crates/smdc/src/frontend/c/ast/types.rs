//! Type representations in the AST

use crate::common::Span;

/// C type representation
#[derive(Debug, Clone, PartialEq)]
pub struct CType {
    pub kind: TypeKind,
    pub qualifiers: TypeQualifiers,
    pub span: Span,
}

impl CType {
    pub fn new(kind: TypeKind, span: Span) -> Self {
        Self {
            kind,
            qualifiers: TypeQualifiers::default(),
            span,
        }
    }

    pub fn with_qualifiers(mut self, qualifiers: TypeQualifiers) -> Self {
        self.qualifiers = qualifiers;
        self
    }

    pub fn void(span: Span) -> Self {
        Self::new(TypeKind::Void, span)
    }

    pub fn char(span: Span) -> Self {
        Self::new(TypeKind::Char { signed: true }, span)
    }

    pub fn int(span: Span) -> Self {
        Self::new(TypeKind::Int { signed: true }, span)
    }

    pub fn pointer_to(inner: CType, span: Span) -> Self {
        Self::new(TypeKind::Pointer(Box::new(inner)), span)
    }

    /// Get the size of this type in bytes (for M68k)
    pub fn size(&self) -> usize {
        match &self.kind {
            TypeKind::Void => 0,
            TypeKind::Char { .. } => 1,
            TypeKind::Short { .. } => 2,
            TypeKind::Int { .. } => 4, // 32-bit int on M68k
            TypeKind::Long { .. } => 4,
            TypeKind::LongLong { .. } => 8,
            TypeKind::Float => 4,
            TypeKind::Double => 8,
            TypeKind::Pointer(_) => 4, // 32-bit pointers
            TypeKind::Array { element, size } => element.size() * size.unwrap_or(0),
            TypeKind::Function { .. } => 4, // Function pointer
            TypeKind::Struct { members, .. } => {
                members.iter().map(|(_, t)| t.size()).sum()
            }
            TypeKind::Union { members, .. } => {
                members.iter().map(|(_, t)| t.size()).max().unwrap_or(0)
            }
            TypeKind::Enum { .. } => 4, // Enums are ints
            TypeKind::Typedef(_name) => 4, // Placeholder, resolved during sema
        }
    }

    /// Get the alignment of this type in bytes
    pub fn alignment(&self) -> usize {
        match &self.kind {
            TypeKind::Void => 1,
            TypeKind::Char { .. } => 1,
            TypeKind::Short { .. } => 2,
            TypeKind::Int { .. } => 2, // M68k aligns to 2 bytes
            TypeKind::Long { .. } => 2,
            TypeKind::LongLong { .. } => 2,
            TypeKind::Float => 2,
            TypeKind::Double => 2,
            TypeKind::Pointer(_) => 2,
            TypeKind::Array { element, .. } => element.alignment(),
            TypeKind::Function { .. } => 2,
            TypeKind::Struct { members, .. } => {
                members.iter().map(|(_, t)| t.alignment()).max().unwrap_or(1)
            }
            TypeKind::Union { members, .. } => {
                members.iter().map(|(_, t)| t.alignment()).max().unwrap_or(1)
            }
            TypeKind::Enum { .. } => 2,
            TypeKind::Typedef(_) => 2,
        }
    }

    /// Check if this type is an integer type
    pub fn is_integer(&self) -> bool {
        matches!(
            self.kind,
            TypeKind::Char { .. }
                | TypeKind::Short { .. }
                | TypeKind::Int { .. }
                | TypeKind::Long { .. }
                | TypeKind::LongLong { .. }
                | TypeKind::Enum { .. }
        )
    }

    /// Check if this type is a pointer type
    pub fn is_pointer(&self) -> bool {
        matches!(self.kind, TypeKind::Pointer(_))
    }

    /// Check if this type is an array type
    pub fn is_array(&self) -> bool {
        matches!(self.kind, TypeKind::Array { .. })
    }

    /// Check if this type is void
    pub fn is_void(&self) -> bool {
        matches!(self.kind, TypeKind::Void)
    }

    /// Check if this is a scalar type (integer, pointer, or enum)
    pub fn is_scalar(&self) -> bool {
        self.is_integer() || self.is_pointer()
    }
}

/// The kind of a C type
#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    Void,
    Char { signed: bool },
    Short { signed: bool },
    Int { signed: bool },
    Long { signed: bool },
    LongLong { signed: bool },
    Float,
    Double,
    Pointer(Box<CType>),
    Array {
        element: Box<CType>,
        size: Option<usize>,
    },
    Function {
        return_type: Box<CType>,
        params: Vec<(Option<String>, CType)>,
        variadic: bool,
    },
    Struct {
        name: Option<String>,
        members: Vec<(String, CType)>,
    },
    Union {
        name: Option<String>,
        members: Vec<(String, CType)>,
    },
    Enum {
        name: Option<String>,
        variants: Vec<(String, Option<i64>)>,
    },
    Typedef(String),
}

/// Type qualifiers (const, volatile, restrict)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TypeQualifiers {
    pub is_const: bool,
    pub is_volatile: bool,
    pub is_restrict: bool,
}

impl TypeQualifiers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_const(mut self) -> Self {
        self.is_const = true;
        self
    }

    pub fn with_volatile(mut self) -> Self {
        self.is_volatile = true;
        self
    }

    pub fn with_restrict(mut self) -> Self {
        self.is_restrict = true;
        self
    }
}

/// Storage class specifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageClass {
    Auto,
    Register,
    Static,
    Extern,
    Typedef,
}
