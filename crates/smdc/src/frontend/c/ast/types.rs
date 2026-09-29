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
            TypeKind::Struct { .. } | TypeKind::Union { .. } => {
                // End of the last member, padded to the aggregate's alignment
                // so array elements stay aligned
                let end = self
                    .member_layout()
                    .iter()
                    .map(|(_, offset, ty)| offset + ty.size())
                    .max()
                    .unwrap_or(0);
                end.next_multiple_of(self.alignment())
            }
            TypeKind::Enum { .. } => 4,    // Enums are ints
            TypeKind::Typedef(_name) => 4, // Placeholder, resolved during sema
        }
    }

    /// Member names, byte offsets, and types of a struct or union, in
    /// declaration order. Each struct member sits at the next offset aligned
    /// for its type; every union member is at offset 0. Other types have no
    /// members.
    pub fn member_layout(&self) -> Vec<(&str, usize, &CType)> {
        match &self.kind {
            TypeKind::Struct { members, .. } => {
                let mut offset = 0usize;
                members
                    .iter()
                    .map(|(name, ty)| {
                        offset = offset.next_multiple_of(ty.alignment());
                        let entry = (name.as_str(), offset, ty);
                        offset += ty.size();
                        entry
                    })
                    .collect()
            }
            TypeKind::Union { members, .. } => members
                .iter()
                .map(|(name, ty)| (name.as_str(), 0, ty))
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Byte offset and type of the struct or union member named `field`.
    pub fn member(&self, field: &str) -> Option<(usize, &CType)> {
        self.member_layout()
            .into_iter()
            .find(|(name, _, _)| *name == field)
            .map(|(_, offset, ty)| (offset, ty))
    }

    /// Whether this is a struct or union type
    pub fn is_record(&self) -> bool {
        matches!(self.kind, TypeKind::Struct { .. } | TypeKind::Union { .. })
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
            // Structs and unions are at least word-aligned on the 68000 (as
            // with m68k GCC), so they can be copied with word/long moves
            TypeKind::Struct { members, .. } | TypeKind::Union { members, .. } => members
                .iter()
                .map(|(_, t)| t.alignment())
                .max()
                .unwrap_or(1)
                .max(2),
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

    /// Check if this type is signed (returns false for unsigned and non-integer types)
    pub fn is_signed(&self) -> bool {
        match &self.kind {
            TypeKind::Char { signed } => *signed,
            TypeKind::Short { signed } => *signed,
            TypeKind::Int { signed } => *signed,
            TypeKind::Long { signed } => *signed,
            TypeKind::LongLong { signed } => *signed,
            _ => false, // Pointers, enums, etc. are treated as unsigned
        }
    }

    /// Convert to IR type
    pub fn to_ir_type(&self) -> crate::types::IrType {
        use crate::types::{IrType, IrTypeKind};

        let kind = match &self.kind {
            TypeKind::Void => IrTypeKind::Void,
            TypeKind::Char { signed } => IrTypeKind::Int {
                bits: 8,
                signed: *signed,
            },
            TypeKind::Short { signed } => IrTypeKind::Int {
                bits: 16,
                signed: *signed,
            },
            TypeKind::Int { signed } | TypeKind::Long { signed } => IrTypeKind::Int {
                bits: 32,
                signed: *signed,
            },
            TypeKind::LongLong { signed } => IrTypeKind::Int {
                bits: 64,
                signed: *signed,
            },
            TypeKind::Float => IrTypeKind::Float { bits: 32 },
            TypeKind::Double => IrTypeKind::Float { bits: 64 },
            TypeKind::Pointer(inner) => IrTypeKind::Pointer(Box::new(inner.to_ir_type())),
            TypeKind::Array { element, size } => IrTypeKind::Array {
                element: Box::new(element.to_ir_type()),
                count: size.unwrap_or(0),
            },
            TypeKind::Function {
                return_type,
                params,
                variadic,
            } => IrTypeKind::Function {
                return_type: Box::new(return_type.to_ir_type()),
                params: params.iter().map(|(_, ty)| ty.to_ir_type()).collect(),
                variadic: *variadic,
            },
            TypeKind::Struct { name, .. } => IrTypeKind::Struct {
                name: name.clone(),
                fields: self
                    .member_layout()
                    .into_iter()
                    .map(|(name, offset, ty)| (name.to_string(), ty.to_ir_type(), offset))
                    .collect(),
            },
            TypeKind::Union { .. } => {
                // Approximate unions as array of bytes for now, or just an opaque type
                IrTypeKind::Array {
                    element: Box::new(IrType {
                        kind: IrTypeKind::Int {
                            bits: 8,
                            signed: false,
                        },
                        size: 1,
                        align: 1,
                    }),
                    count: self.size(),
                }
            }
            TypeKind::Enum { .. } => IrTypeKind::Int {
                bits: 32,
                signed: true,
            }, // Enums are ints
            TypeKind::Typedef(_) => IrTypeKind::Void, // Should be resolved
        };

        IrType {
            kind,
            size: self.size(),
            align: self.alignment(),
        }
    }
}

/// The kind of a C type
#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    Void,
    Char {
        signed: bool,
    },
    Short {
        signed: bool,
    },
    Int {
        signed: bool,
    },
    Long {
        signed: bool,
    },
    LongLong {
        signed: bool,
    },
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
