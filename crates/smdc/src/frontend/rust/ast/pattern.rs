//! Rust pattern AST nodes

use super::TypePath;
use crate::common::Span;

/// A pattern for matching
#[derive(Debug, Clone)]
pub struct Pattern {
    pub kind: PatternKind,
    pub span: Span,
}

impl Pattern {
    pub fn new(kind: PatternKind, span: Span) -> Self {
        Self { kind, span }
    }

    pub fn wildcard(span: Span) -> Self {
        Self::new(PatternKind::Wildcard, span)
    }

    pub fn binding(name: String, mutable: bool, span: Span) -> Self {
        Self::new(
            PatternKind::Binding {
                name,
                mutable,
                subpattern: None,
            },
            span,
        )
    }
}

/// Pattern kinds
#[derive(Debug, Clone)]
pub enum PatternKind {
    /// Wildcard: _
    Wildcard,

    /// Binding: x, mut x, ref x, ref mut x
    Binding {
        name: String,
        mutable: bool,
        subpattern: Option<Box<Pattern>>, // For x @ pattern
    },

    /// Literal: 42, 'a', "hello", true (boxed to break recursion)
    Literal(Box<super::Expr>),

    /// Range: 1..=10, 'a'..='z'
    Range {
        start: Option<Box<super::Expr>>,
        end: Option<Box<super::Expr>>,
        inclusive: bool,
    },

    /// Reference: &pattern, &mut pattern
    Reference {
        mutable: bool,
        pattern: Box<Pattern>,
    },

    /// Tuple: (a, b, c)
    Tuple(Vec<Pattern>),

    /// Slice: [a, b, c] or [head, tail @ ..]
    Slice(Vec<Pattern>),

    /// Struct: Foo { x, y: z }
    Struct {
        path: TypePath,
        fields: Vec<FieldPattern>,
        rest: bool, // true if .. is present
    },

    /// Tuple struct: Foo(a, b)
    TupleStruct {
        path: TypePath,
        fields: Vec<Pattern>,
    },

    /// Path (unit variant or constant): None, Foo::Bar
    Path(TypePath),

    /// Or pattern: a | b | c
    Or(Vec<Pattern>),

    /// Rest pattern: ..
    Rest,

    /// Parenthesized: (pattern)
    Paren(Box<Pattern>),
}

/// Field pattern in struct pattern
#[derive(Debug, Clone)]
pub struct FieldPattern {
    pub name: String,
    pub pattern: Option<Pattern>, // None for shorthand (x instead of x: x)
    pub span: Span,
}

impl FieldPattern {
    pub fn new(name: String, pattern: Option<Pattern>, span: Span) -> Self {
        Self { name, pattern, span }
    }

    pub fn shorthand(name: String, span: Span) -> Self {
        Self { name, pattern: None, span }
    }
}
