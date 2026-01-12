//! Rust type checking

use crate::frontend::rust::ast::*;
use crate::common::{CompileError, CompileResult, Span};

/// Type checker for Rust expressions
pub struct TypeChecker;

impl TypeChecker {
    pub fn new() -> Self {
        Self
    }

    /// Check if two types are compatible for assignment
    pub fn is_assignable(&self, target: &RustType, source: &RustType) -> bool {
        self.types_match(target, source)
    }

    /// Check if two types match (structural equality)
    pub fn types_match(&self, a: &RustType, b: &RustType) -> bool {
        match (&a.kind, &b.kind) {
            (RustTypeKind::Infer, _) | (_, RustTypeKind::Infer) => true,
            (RustTypeKind::Unit, RustTypeKind::Unit) => true,
            (RustTypeKind::Never, _) => true, // ! coerces to anything
            (RustTypeKind::Primitive(p1), RustTypeKind::Primitive(p2)) => p1 == p2,
            (
                RustTypeKind::Reference { mutable: m1, inner: i1 },
                RustTypeKind::Reference { mutable: m2, inner: i2 },
            ) => {
                // &mut T can coerce to &T
                (*m1 || !*m2) && self.types_match(i1, i2)
            }
            (
                RustTypeKind::Pointer { mutable: m1, inner: i1 },
                RustTypeKind::Pointer { mutable: m2, inner: i2 },
            ) => {
                (*m1 || !*m2) && self.types_match(i1, i2)
            }
            (
                RustTypeKind::Array { element: e1, size: s1 },
                RustTypeKind::Array { element: e2, size: s2 },
            ) => s1 == s2 && self.types_match(e1, e2),
            (
                RustTypeKind::Slice { element: e1 },
                RustTypeKind::Slice { element: e2 },
            ) => self.types_match(e1, e2),
            (RustTypeKind::Tuple(t1), RustTypeKind::Tuple(t2)) => {
                t1.len() == t2.len() && t1.iter().zip(t2.iter()).all(|(a, b)| self.types_match(a, b))
            }
            (RustTypeKind::Named(p1), RustTypeKind::Named(p2)) => {
                p1.segments == p2.segments
            }
            _ => false,
        }
    }

    /// Get the result type of a binary operation
    pub fn binary_result_type(
        &self,
        op: BinOp,
        left: &RustType,
        right: &RustType,
        span: Span,
    ) -> CompileResult<RustType> {
        match op {
            // Comparison operators always return bool
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                Ok(RustType::bool(span))
            }
            // Logical operators require and return bool
            BinOp::And | BinOp::Or => {
                if !self.is_bool(left) || !self.is_bool(right) {
                    return Err(CompileError::type_error(
                        format!("logical operators require bool operands"),
                        span,
                    ));
                }
                Ok(RustType::bool(span))
            }
            // Arithmetic and bitwise operators
            _ => {
                if !self.types_match(left, right) {
                    return Err(CompileError::type_error(
                        format!("mismatched types in binary operation"),
                        span,
                    ));
                }
                // Return the left type (simplified)
                Ok(left.clone())
            }
        }
    }

    /// Get the result type of a unary operation
    pub fn unary_result_type(
        &self,
        op: UnaryOp,
        operand: &RustType,
        span: Span,
    ) -> CompileResult<RustType> {
        match op {
            UnaryOp::Neg => {
                if !operand.is_numeric() {
                    return Err(CompileError::type_error(
                        "cannot negate non-numeric type",
                        span,
                    ));
                }
                Ok(operand.clone())
            }
            UnaryOp::Not => {
                if self.is_bool(operand) {
                    Ok(operand.clone())
                } else if operand.is_integer() {
                    Ok(operand.clone()) // Bitwise not for integers
                } else {
                    Err(CompileError::type_error(
                        "cannot apply ! to this type",
                        span,
                    ))
                }
            }
            UnaryOp::Deref => {
                match &operand.kind {
                    RustTypeKind::Pointer { inner, .. } |
                    RustTypeKind::Reference { inner, .. } => {
                        Ok((**inner).clone())
                    }
                    _ => Err(CompileError::type_error(
                        "cannot dereference non-pointer type",
                        span,
                    )),
                }
            }
            UnaryOp::Ref | UnaryOp::RefMut => {
                let mutable = matches!(op, UnaryOp::RefMut);
                Ok(RustType::reference(operand.clone(), mutable, span))
            }
        }
    }

    /// Check if a type is bool
    pub fn is_bool(&self, ty: &RustType) -> bool {
        matches!(ty.kind, RustTypeKind::Primitive(PrimitiveType::Bool))
    }

    /// Check if a type can be used as a loop condition
    pub fn is_condition_type(&self, ty: &RustType) -> bool {
        self.is_bool(ty)
    }

    /// Get the element type of an array or slice
    pub fn element_type(&self, ty: &RustType) -> Option<RustType> {
        match &ty.kind {
            RustTypeKind::Array { element, .. } |
            RustTypeKind::Slice { element } => Some((**element).clone()),
            _ => None,
        }
    }

    /// Try to coerce a type to another
    pub fn try_coerce(&self, from: &RustType, to: &RustType) -> Option<RustType> {
        // Same types
        if self.types_match(from, to) {
            return Some(to.clone());
        }

        // Never coerces to anything
        if matches!(from.kind, RustTypeKind::Never) {
            return Some(to.clone());
        }

        // &mut T -> &T
        if let (
            RustTypeKind::Reference { mutable: true, inner: from_inner },
            RustTypeKind::Reference { mutable: false, inner: to_inner },
        ) = (&from.kind, &to.kind) {
            if self.types_match(from_inner, to_inner) {
                return Some(to.clone());
            }
        }

        // Array to slice coercion: &[T; N] -> &[T]
        if let (
            RustTypeKind::Reference { mutable: from_mut, inner: from_inner },
            RustTypeKind::Reference { mutable: to_mut, inner: to_inner },
        ) = (&from.kind, &to.kind) {
            if *from_mut || !*to_mut {
                if let (
                    RustTypeKind::Array { element: from_elem, .. },
                    RustTypeKind::Slice { element: to_elem },
                ) = (&from_inner.kind, &to_inner.kind) {
                    if self.types_match(from_elem, to_elem) {
                        return Some(to.clone());
                    }
                }
            }
        }

        None
    }

    /// Infer integer literal type
    pub fn infer_int_literal_type(&self, _value: i64, hint: Option<&RustType>, span: Span) -> RustType {
        if let Some(hint) = hint {
            if hint.is_integer() {
                return hint.clone();
            }
        }

        // Default to i32
        RustType::i32(span)
    }

    /// Infer float literal type
    pub fn infer_float_literal_type(&self, hint: Option<&RustType>, span: Span) -> RustType {
        if let Some(hint) = hint {
            if let RustTypeKind::Primitive(PrimitiveType::F32 | PrimitiveType::F64) = hint.kind {
                return hint.clone();
            }
        }

        // Default to f64
        RustType::new(RustTypeKind::Primitive(PrimitiveType::F64), span)
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}
