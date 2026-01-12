//! Rust type representations

use crate::common::Span;

/// Rust type representation
#[derive(Debug, Clone, PartialEq)]
pub struct RustType {
    pub kind: RustTypeKind,
    pub span: Span,
}

impl RustType {
    pub fn new(kind: RustTypeKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub fn unit(span: Span) -> Self {
        Self::new(RustTypeKind::Unit, span)
    }

    pub fn never(span: Span) -> Self {
        Self::new(RustTypeKind::Never, span)
    }

    pub fn i32(span: Span) -> Self {
        Self::new(RustTypeKind::Primitive(PrimitiveType::I32), span)
    }

    pub fn bool(span: Span) -> Self {
        Self::new(RustTypeKind::Primitive(PrimitiveType::Bool), span)
    }

    pub fn reference(inner: RustType, mutable: bool, span: Span) -> Self {
        Self::new(
            RustTypeKind::Reference {
                mutable,
                inner: Box::new(inner),
            },
            span,
        )
    }

    pub fn pointer(inner: RustType, mutable: bool, span: Span) -> Self {
        Self::new(
            RustTypeKind::Pointer {
                mutable,
                inner: Box::new(inner),
            },
            span,
        )
    }

    /// Get the size of this type in bytes (for M68k)
    pub fn size(&self) -> usize {
        match &self.kind {
            RustTypeKind::Unit => 0,
            RustTypeKind::Never => 0,
            RustTypeKind::Primitive(p) => p.size(),
            RustTypeKind::Reference { .. } => 4, // 32-bit pointers
            RustTypeKind::Pointer { .. } => 4,
            RustTypeKind::Array { element, size } => element.size() * size,
            RustTypeKind::Slice { .. } => 8, // Fat pointer (ptr + len)
            RustTypeKind::Tuple(types) => types.iter().map(|t| t.size()).sum(),
            RustTypeKind::Named(_) => 4, // Resolved during sema
            RustTypeKind::Infer => 4, // Resolved during type inference
        }
    }

    /// Get the alignment of this type in bytes
    pub fn alignment(&self) -> usize {
        match &self.kind {
            RustTypeKind::Unit => 1,
            RustTypeKind::Never => 1,
            RustTypeKind::Primitive(p) => p.alignment(),
            RustTypeKind::Reference { .. } => 2,
            RustTypeKind::Pointer { .. } => 2,
            RustTypeKind::Array { element, .. } => element.alignment(),
            RustTypeKind::Slice { .. } => 2,
            RustTypeKind::Tuple(types) => types.iter().map(|t| t.alignment()).max().unwrap_or(1),
            RustTypeKind::Named(_) => 2,
            RustTypeKind::Infer => 2,
        }
    }

    /// Check if this type is Copy
    pub fn is_copy(&self) -> bool {
        match &self.kind {
            RustTypeKind::Unit => true,
            RustTypeKind::Never => true,
            RustTypeKind::Primitive(_) => true,
            RustTypeKind::Reference { mutable, .. } => !mutable, // &T is Copy, &mut T is not
            RustTypeKind::Pointer { .. } => true,
            RustTypeKind::Array { element, .. } => element.is_copy(),
            RustTypeKind::Slice { .. } => false,
            RustTypeKind::Tuple(types) => types.iter().all(|t| t.is_copy()),
            RustTypeKind::Named(_) => false, // Conservative
            RustTypeKind::Infer => false,
        }
    }

    /// Check if this is a numeric type
    pub fn is_numeric(&self) -> bool {
        matches!(
            &self.kind,
            RustTypeKind::Primitive(
                PrimitiveType::I8 | PrimitiveType::I16 | PrimitiveType::I32 | PrimitiveType::I64 |
                PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64 |
                PrimitiveType::Isize | PrimitiveType::Usize |
                PrimitiveType::F32 | PrimitiveType::F64
            )
        )
    }

    /// Check if this is an integer type
    pub fn is_integer(&self) -> bool {
        matches!(
            &self.kind,
            RustTypeKind::Primitive(
                PrimitiveType::I8 | PrimitiveType::I16 | PrimitiveType::I32 | PrimitiveType::I64 |
                PrimitiveType::U8 | PrimitiveType::U16 | PrimitiveType::U32 | PrimitiveType::U64 |
                PrimitiveType::Isize | PrimitiveType::Usize
            )
        )
    }
}

/// The kind of a Rust type
#[derive(Debug, Clone, PartialEq)]
pub enum RustTypeKind {
    /// Unit type ()
    Unit,
    /// Never type !
    Never,
    /// Primitive types
    Primitive(PrimitiveType),
    /// Reference &T or &mut T
    Reference {
        mutable: bool,
        inner: Box<RustType>,
    },
    /// Raw pointer *const T or *mut T
    Pointer {
        mutable: bool,
        inner: Box<RustType>,
    },
    /// Array [T; N]
    Array {
        element: Box<RustType>,
        size: usize,
    },
    /// Slice &[T]
    Slice {
        element: Box<RustType>,
    },
    /// Tuple (T, U, ...)
    Tuple(Vec<RustType>),
    /// Named type (struct, enum, type alias)
    Named(TypePath),
    /// Type to be inferred (for let x = ...)
    Infer,
}

/// Primitive types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Isize,
    Usize,
    F32,
    F64,
    Bool,
    Char,
}

impl PrimitiveType {
    pub fn size(&self) -> usize {
        match self {
            PrimitiveType::I8 | PrimitiveType::U8 => 1,
            PrimitiveType::I16 | PrimitiveType::U16 => 2,
            PrimitiveType::I32 | PrimitiveType::U32 => 4,
            PrimitiveType::I64 | PrimitiveType::U64 => 8,
            PrimitiveType::Isize | PrimitiveType::Usize => 4, // 32-bit on M68k
            PrimitiveType::F32 => 4,
            PrimitiveType::F64 => 8,
            PrimitiveType::Bool => 1,
            PrimitiveType::Char => 4, // Unicode scalar value
        }
    }

    pub fn alignment(&self) -> usize {
        match self {
            PrimitiveType::I8 | PrimitiveType::U8 | PrimitiveType::Bool => 1,
            _ => 2, // M68k word alignment
        }
    }

    pub fn is_signed(&self) -> bool {
        matches!(
            self,
            PrimitiveType::I8 | PrimitiveType::I16 | PrimitiveType::I32 |
            PrimitiveType::I64 | PrimitiveType::Isize
        )
    }
}

impl std::fmt::Display for PrimitiveType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrimitiveType::I8 => write!(f, "i8"),
            PrimitiveType::I16 => write!(f, "i16"),
            PrimitiveType::I32 => write!(f, "i32"),
            PrimitiveType::I64 => write!(f, "i64"),
            PrimitiveType::U8 => write!(f, "u8"),
            PrimitiveType::U16 => write!(f, "u16"),
            PrimitiveType::U32 => write!(f, "u32"),
            PrimitiveType::U64 => write!(f, "u64"),
            PrimitiveType::Isize => write!(f, "isize"),
            PrimitiveType::Usize => write!(f, "usize"),
            PrimitiveType::F32 => write!(f, "f32"),
            PrimitiveType::F64 => write!(f, "f64"),
            PrimitiveType::Bool => write!(f, "bool"),
            PrimitiveType::Char => write!(f, "char"),
        }
    }
}

/// A path to a type (e.g., std::vec::Vec, MyStruct)
#[derive(Debug, Clone, PartialEq)]
pub struct TypePath {
    pub segments: Vec<String>,
}

impl TypePath {
    pub fn new(segments: Vec<String>) -> Self {
        Self { segments }
    }

    pub fn simple(name: String) -> Self {
        Self { segments: vec![name] }
    }

    pub fn name(&self) -> &str {
        self.segments.last().map(|s| s.as_str()).unwrap_or("")
    }
}

impl std::fmt::Display for TypePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.segments.join("::"))
    }
}

impl std::fmt::Display for RustType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            RustTypeKind::Unit => write!(f, "()"),
            RustTypeKind::Never => write!(f, "!"),
            RustTypeKind::Primitive(p) => write!(f, "{}", p),
            RustTypeKind::Reference { mutable, inner } => {
                if *mutable {
                    write!(f, "&mut {}", inner)
                } else {
                    write!(f, "&{}", inner)
                }
            }
            RustTypeKind::Pointer { mutable, inner } => {
                if *mutable {
                    write!(f, "*mut {}", inner)
                } else {
                    write!(f, "*const {}", inner)
                }
            }
            RustTypeKind::Array { element, size } => write!(f, "[{}; {}]", element, size),
            RustTypeKind::Slice { element } => write!(f, "[{}]", element),
            RustTypeKind::Tuple(types) => {
                write!(f, "(")?;
                for (i, t) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", t)?;
                }
                write!(f, ")")
            }
            RustTypeKind::Named(path) => write!(f, "{}", path),
            RustTypeKind::Infer => write!(f, "_"),
        }
    }
}
