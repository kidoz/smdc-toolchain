//! Language-agnostic type system for IR
//!
//! This type system is simpler than source language types
//! and focuses on what matters for code generation on M68k.

/// Language-agnostic type for IR
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrType {
    pub kind: IrTypeKind,
    /// Size in bytes
    pub size: usize,
    /// Alignment in bytes
    pub align: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrTypeKind {
    /// Void type (no value)
    Void,

    /// Integer type
    Int {
        bits: u8, // 8, 16, 32, 64
        signed: bool,
    },

    /// Floating point (for future M68881 support)
    Float { bits: u8 }, // 32, 64

    /// Pointer type
    Pointer(Box<IrType>),

    /// Array type
    Array { element: Box<IrType>, count: usize },

    /// Struct type
    Struct {
        name: Option<String>,
        /// (field_name, field_type, byte_offset)
        fields: Vec<(String, IrType, usize)>,
    },

    /// Function type
    Function {
        params: Vec<IrType>,
        return_type: Box<IrType>,
        variadic: bool,
    },
}

impl IrType {
    // ==================== M68k-specific type constructors ====================

    /// Void type (size 0)
    pub fn void() -> Self {
        Self {
            kind: IrTypeKind::Void,
            size: 0,
            align: 1,
        }
    }

    /// Signed 8-bit integer (byte)
    pub fn i8() -> Self {
        Self {
            kind: IrTypeKind::Int {
                bits: 8,
                signed: true,
            },
            size: 1,
            align: 1,
        }
    }

    /// Unsigned 8-bit integer (byte)
    pub fn u8() -> Self {
        Self {
            kind: IrTypeKind::Int {
                bits: 8,
                signed: false,
            },
            size: 1,
            align: 1,
        }
    }

    /// Signed 16-bit integer (word)
    pub fn i16() -> Self {
        Self {
            kind: IrTypeKind::Int {
                bits: 16,
                signed: true,
            },
            size: 2,
            align: 2,
        }
    }

    /// Unsigned 16-bit integer (word)
    pub fn u16() -> Self {
        Self {
            kind: IrTypeKind::Int {
                bits: 16,
                signed: false,
            },
            size: 2,
            align: 2,
        }
    }

    /// Signed 32-bit integer (long)
    pub fn i32() -> Self {
        // M68k aligns longs to 2 bytes (word boundary), not 4
        Self {
            kind: IrTypeKind::Int {
                bits: 32,
                signed: true,
            },
            size: 4,
            align: 2,
        }
    }

    /// Unsigned 32-bit integer (long)
    pub fn u32() -> Self {
        Self {
            kind: IrTypeKind::Int {
                bits: 32,
                signed: false,
            },
            size: 4,
            align: 2,
        }
    }

    /// Signed 64-bit integer (for extended calculations)
    pub fn i64() -> Self {
        Self {
            kind: IrTypeKind::Int {
                bits: 64,
                signed: true,
            },
            size: 8,
            align: 2,
        }
    }

    /// Unsigned 64-bit integer
    pub fn u64() -> Self {
        Self {
            kind: IrTypeKind::Int {
                bits: 64,
                signed: false,
            },
            size: 8,
            align: 2,
        }
    }

    /// Pointer type (32-bit on M68k)
    pub fn ptr(inner: IrType) -> Self {
        Self {
            kind: IrTypeKind::Pointer(Box::new(inner)),
            size: 4,
            align: 2,
        }
    }

    /// Void pointer
    pub fn ptr_void() -> Self {
        Self::ptr(Self::void())
    }

    /// Array type
    pub fn array(element: IrType, count: usize) -> Self {
        let elem_size = element.size;
        let elem_align = element.align;
        Self {
            kind: IrTypeKind::Array {
                element: Box::new(element),
                count,
            },
            size: elem_size * count,
            align: elem_align,
        }
    }

    /// Function type
    pub fn function(params: Vec<IrType>, return_type: IrType, variadic: bool) -> Self {
        Self {
            kind: IrTypeKind::Function {
                params,
                return_type: Box::new(return_type),
                variadic,
            },
            size: 0, // Functions have no size
            align: 1,
        }
    }

    // ==================== Type queries ====================

    /// Is this a void type?
    pub fn is_void(&self) -> bool {
        matches!(self.kind, IrTypeKind::Void)
    }

    /// Is this an integer type?
    pub fn is_integer(&self) -> bool {
        matches!(self.kind, IrTypeKind::Int { .. })
    }

    /// Is this a signed integer?
    pub fn is_signed(&self) -> bool {
        matches!(self.kind, IrTypeKind::Int { signed: true, .. })
    }

    /// Is this a pointer type?
    pub fn is_pointer(&self) -> bool {
        matches!(self.kind, IrTypeKind::Pointer(_))
    }

    /// Is this an array type?
    pub fn is_array(&self) -> bool {
        matches!(self.kind, IrTypeKind::Array { .. })
    }

    /// Is this a struct type?
    pub fn is_struct(&self) -> bool {
        matches!(self.kind, IrTypeKind::Struct { .. })
    }

    /// Is this a function type?
    pub fn is_function(&self) -> bool {
        matches!(self.kind, IrTypeKind::Function { .. })
    }

    /// Get the element type if this is a pointer or array
    pub fn element_type(&self) -> Option<&IrType> {
        match &self.kind {
            IrTypeKind::Pointer(inner) => Some(inner),
            IrTypeKind::Array { element, .. } => Some(element),
            _ => None,
        }
    }

    /// Get bit width for integer types
    pub fn bits(&self) -> Option<u8> {
        match self.kind {
            IrTypeKind::Int { bits, .. } => Some(bits),
            _ => None,
        }
    }
}

impl Default for IrType {
    fn default() -> Self {
        Self::i32() // Default to 32-bit signed int
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_ir_type_sizes() {
        assert_eq!(IrType::void().size, 0);
        assert_eq!(IrType::i8().size, 1);
        assert_eq!(IrType::u8().size, 1);
        assert_eq!(IrType::i16().size, 2);
        assert_eq!(IrType::u16().size, 2);
        assert_eq!(IrType::i32().size, 4);
        assert_eq!(IrType::u32().size, 4);
        assert_eq!(IrType::i64().size, 8);
        assert_eq!(IrType::u64().size, 8);
        assert_eq!(IrType::ptr(IrType::i32()).size, 4);
        assert_eq!(IrType::ptr_void().size, 4);
    }

    #[test]
    fn test_ir_type_alignments() {
        assert_eq!(IrType::void().align, 1);
        assert_eq!(IrType::i8().align, 1);
        assert_eq!(IrType::i16().align, 2);
        assert_eq!(IrType::i32().align, 2); // M68k alignment
        assert_eq!(IrType::i64().align, 2);
        assert_eq!(IrType::ptr(IrType::i8()).align, 2);
    }

    #[test]
    fn test_ir_type_queries() {
        let i32_ty = IrType::i32();
        assert!(i32_ty.is_integer());
        assert!(i32_ty.is_signed());
        assert!(!i32_ty.is_pointer());

        let u32_ty = IrType::u32();
        assert!(u32_ty.is_integer());
        assert!(!u32_ty.is_signed());

        let ptr_ty = IrType::ptr_void();
        assert!(ptr_ty.is_pointer());
        assert!(!ptr_ty.is_integer());

        let arr_ty = IrType::array(IrType::i32(), 10);
        assert!(arr_ty.is_array());
        assert_eq!(arr_ty.size, 40);
        assert_eq!(arr_ty.align, 2);
    }

    #[test]
    fn test_ir_type_element_type() {
        let inner = IrType::i16();
        let ptr_ty = IrType::ptr(inner.clone());
        assert_eq!(ptr_ty.element_type(), Some(&inner));

        let arr_ty = IrType::array(inner.clone(), 5);
        assert_eq!(arr_ty.element_type(), Some(&inner));

        let int_ty = IrType::i32();
        assert_eq!(int_ty.element_type(), None);
    }

    #[test]
    fn test_ir_type_bits() {
        assert_eq!(IrType::i8().bits(), Some(8));
        assert_eq!(IrType::u16().bits(), Some(16));
        assert_eq!(IrType::i32().bits(), Some(32));
        assert_eq!(IrType::u64().bits(), Some(64));
        assert_eq!(IrType::ptr_void().bits(), None);
    }
}
