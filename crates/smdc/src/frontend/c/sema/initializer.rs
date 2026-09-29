//! Initializer layout
//!
//! Maps a C initializer — nested braces, `.field` / `[index]` designators,
//! elided inner braces, string literals — onto the byte offsets of the
//! object it initializes. Semantic analysis uses it to size unsized arrays
//! and the IR builder to emit initialized data or stores. Subobjects that
//! no entry covers are zero-initialized.

use crate::common::{CompileError, CompileResult, Span};
use crate::frontend::c::ast::{CType, Designator, Expr, ExprKind, Initializer, TypeKind};

/// What initializes one subobject
#[derive(Debug)]
pub enum InitValue<'a> {
    /// An expression converted to the subobject's type (a whole struct or
    /// union when the subobject is one)
    Expr(&'a Expr),
    /// A string literal filling a char array; the terminating NUL is kept
    /// only if it fits
    Str(&'a str),
}

/// One initialized subobject: its offset from the start of the object
#[derive(Debug)]
pub struct InitEntry<'a> {
    pub offset: usize,
    pub ty: CType,
    pub value: InitValue<'a>,
}

/// Layout of an initializer
#[derive(Debug)]
pub struct InitLayout<'a> {
    pub entries: Vec<InitEntry<'a>>,
    /// For an array, the element count the initializer implies (highest
    /// initialized index + 1); this sizes `T a[] = {...}`
    pub array_len: usize,
}

/// Lay out `init` for an object of type `ty`. `resolve` supplies the values
/// of identifiers (enum constants) in `[index]` designators; errors are
/// reported at `span`.
pub fn layout_initializer<'a>(
    ty: &CType,
    init: &'a Initializer,
    resolve: &dyn Fn(&str) -> Option<i64>,
    span: Span,
) -> CompileResult<InitLayout<'a>> {
    let mut layout = Layout {
        entries: Vec::new(),
        resolve,
        span,
    };
    let array_len = layout.object(ty, 0, init)?;
    Ok(InitLayout {
        entries: layout.entries,
        array_len,
    })
}

struct Layout<'a, 'r> {
    entries: Vec<InitEntry<'a>>,
    resolve: &'r dyn Fn(&str) -> Option<i64>,
    span: Span,
}

fn is_aggregate(ty: &CType) -> bool {
    ty.is_array() || ty.is_record()
}

/// The string literal initializing char array `ty`, if that is what `expr` is
fn string_init<'a>(ty: &CType, expr: &'a Expr) -> Option<&'a str> {
    match (&ty.kind, &expr.kind) {
        (TypeKind::Array { element, .. }, ExprKind::StringLiteral(s))
            if matches!(element.kind, TypeKind::Char { .. }) =>
        {
            Some(s)
        }
        _ => None,
    }
}

/// Whether `expr` initializes struct/union `ty` as a whole (a struct value)
fn is_record_value(ty: &CType, expr: &Expr) -> bool {
    ty.is_record() && expr.ty.as_ref().is_some_and(CType::is_record)
}

impl<'a> Layout<'a, '_> {
    fn error(&self, message: &str) -> CompileError {
        CompileError::semantic(message, self.span)
    }

    /// Initialize the object of type `ty` at `offset` from `init`. Returns the
    /// implied array length (0 for non-arrays).
    fn object(&mut self, ty: &CType, offset: usize, init: &'a Initializer) -> CompileResult<usize> {
        match init {
            Initializer::List(items) if is_aggregate(ty) => {
                let mut pos = 0;
                self.aggregate(ty, offset, items, &mut pos, true)
            }
            // A scalar may be wrapped in braces: `int x = { 5 };`
            Initializer::List(items) => match items.as_slice() {
                [] => Ok(0),
                [only] => self.object(ty, offset, only),
                _ => Err(self.error("excess elements in scalar initializer")),
            },
            Initializer::Expr(expr) => self.expr(ty, offset, expr),
            Initializer::Designated { .. } => {
                Err(self.error("designator outside an initializer list"))
            }
        }
    }

    /// Initialize `ty` at `offset` from a single expression
    fn expr(&mut self, ty: &CType, offset: usize, expr: &'a Expr) -> CompileResult<usize> {
        if let Some(s) = string_init(ty, expr) {
            self.entries.push(InitEntry {
                offset,
                ty: ty.clone(),
                value: InitValue::Str(s),
            });
            return Ok(s.len() + 1);
        }
        if ty.is_array() {
            return Err(
                self.error("array initializer must be a brace-enclosed list or string literal")
            );
        }
        if ty.is_record() && !is_record_value(ty, expr) {
            return Err(self.error("struct initializer must be a brace-enclosed list"));
        }
        self.entries.push(InitEntry {
            offset,
            ty: ty.clone(),
            value: InitValue::Expr(expr),
        });
        Ok(0)
    }

    /// Initialize aggregate `ty` at `offset` from `items[*pos..]`, advancing
    /// `*pos`. When `braced`, the items are the aggregate's own brace list;
    /// otherwise its braces were elided, so it takes only the items it needs
    /// and leaves a designator to the enclosing list. Returns the implied
    /// array length.
    fn aggregate(
        &mut self,
        ty: &CType,
        offset: usize,
        items: &'a [Initializer],
        pos: &mut usize,
        braced: bool,
    ) -> CompileResult<usize> {
        match &ty.kind {
            TypeKind::Array { element, size } => {
                let mut index = 0;
                let mut len = 0;
                while let Some(mut item) = items.get(*pos) {
                    if let Initializer::Designated { designator, value } = item {
                        if !braced {
                            break;
                        }
                        let Designator::Index(expr) = designator else {
                            return Err(self.error("field designator in array initializer"));
                        };
                        index = expr
                            .eval_const_with(self.resolve)
                            .and_then(|i| usize::try_from(i).ok())
                            .ok_or_else(|| {
                                self.error("array designator must be a non-negative constant")
                            })?;
                        item = value;
                    }
                    if size.is_some_and(|n| index >= n) {
                        if braced {
                            return Err(self.error("excess elements in array initializer"));
                        }
                        break;
                    }
                    self.member(element, offset + index * element.size(), item, items, pos)?;
                    index += 1;
                    len = len.max(index);
                }
                Ok(len)
            }
            TypeKind::Struct { .. } | TypeKind::Union { .. } => {
                let is_union = matches!(ty.kind, TypeKind::Union { .. });
                let members: Vec<(String, usize, CType)> = ty
                    .member_layout()
                    .into_iter()
                    .map(|(name, offset, ty)| (name.to_string(), offset, ty.clone()))
                    .collect();
                let mut next = 0;
                while let Some(mut item) = items.get(*pos) {
                    if let Initializer::Designated { designator, value } = item {
                        if !braced {
                            break;
                        }
                        let Designator::Field(field) = designator else {
                            return Err(self.error("array designator in struct initializer"));
                        };
                        next = members
                            .iter()
                            .position(|(name, _, _)| name == field)
                            .ok_or_else(|| {
                                self.error(&format!("no member named '{field}' in struct"))
                            })?;
                        item = value;
                    }
                    let Some((_, member_offset, member_ty)) = members.get(next) else {
                        if braced {
                            return Err(self.error("excess elements in struct initializer"));
                        }
                        break;
                    };
                    self.member(member_ty, offset + member_offset, item, items, pos)?;
                    // A union initializes a single member
                    next = if is_union { members.len() } else { next + 1 };
                }
                Ok(0)
            }
            _ => unreachable!("aggregate() called on a scalar type"),
        }
    }

    /// Initialize the subobject `ty` at `offset` from `item`, which is
    /// `items[*pos]` or the value of the designator there, and advance `*pos`
    /// past everything it uses
    fn member(
        &mut self,
        ty: &CType,
        offset: usize,
        item: &'a Initializer,
        items: &'a [Initializer],
        pos: &mut usize,
    ) -> CompileResult<()> {
        match item {
            // An expression for an aggregate that isn't a string or struct
            // value starts that aggregate's elided brace list
            Initializer::Expr(expr)
                if is_aggregate(ty)
                    && string_init(ty, expr).is_none()
                    && !is_record_value(ty, expr) =>
            {
                // Elision reads the following items from the list itself,
                // which can't start at a designator's value
                if matches!(items[*pos], Initializer::Designated { .. }) {
                    return Err(self
                        .error("designated aggregate member needs a brace-enclosed initializer"));
                }
                let start = *pos;
                self.aggregate(ty, offset, items, pos, false)?;
                if *pos == start {
                    return Err(self.error("cannot initialize a member of incomplete type"));
                }
            }
            Initializer::Designated { .. } => {
                return Err(self.error("nested designators are not supported"));
            }
            _ => {
                self.object(ty, offset, item)?;
                *pos += 1;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frontend::c::ast::{DeclKind, TranslationUnit};
    use crate::frontend::c::{Parser, SemanticAnalyzer};

    /// Parse and analyze `source`, then lay out the initializer of its last
    /// declaration as (offset, size) pairs
    fn layout(source: &str) -> CompileResult<Vec<(usize, usize)>> {
        let mut tu: TranslationUnit = Parser::new(source)?.parse()?;
        SemanticAnalyzer::new().analyze(&mut tu)?;
        let Some(DeclKind::Variable(var)) = tu.declarations.last().map(|d| &d.kind) else {
            panic!("last declaration is not a variable");
        };
        let init = var.init.as_ref().expect("no initializer");
        let layout = layout_initializer(&var.ty, init, &|_| None, var.span)?;
        Ok(layout
            .entries
            .iter()
            .map(|e| (e.offset, e.ty.size()))
            .collect())
    }

    #[test]
    fn designators_reposition_and_continue() {
        let entries = layout("int a[6] = { 1, [4] = 5, 6, [1] = 2 };").unwrap();
        assert_eq!(entries, vec![(0, 4), (16, 4), (20, 4), (4, 4)]);
    }

    #[test]
    fn elided_braces_fill_nested_aggregates() {
        let entries = layout(
            "struct In { short a; short b; };\n\
             struct Out { char c; struct In i; };\n\
             struct Out o[2] = { 1, 2, 3, 4, 5 };",
        )
        .unwrap();
        // o[0].c, o[0].i.a, o[0].i.b, o[1].c, o[1].i.a
        assert_eq!(entries, vec![(0, 1), (2, 2), (4, 2), (6, 1), (8, 2)]);
    }

    #[test]
    fn union_initializes_one_member() {
        let entries = layout("union U { short s; int i; };\nunion U u = { .i = 7 };").unwrap();
        assert_eq!(entries, vec![(0, 4)]);
        assert!(layout("union U { short s; int i; };\nunion U u = { 1, 2 };").is_err());
    }

    #[test]
    fn rejects_malformed_initializers() {
        for source in [
            "int a[2] = { 1, 2, 3 };",
            "int a[2] = { [2] = 1 };",
            "struct P { int x; };\nstruct P p = { .y = 1 };",
            "struct P { int x; };\nstruct P p = { [0] = 1 };",
            "int a[2] = { .x = 1 };",
            "int x = { 1, 2 };",
        ] {
            assert!(layout(source).is_err(), "accepted: {source}");
        }
    }

    #[test]
    fn unsized_array_takes_its_length_from_the_initializer() {
        let mut tu = Parser::new("int a[] = { 1, [5] = 2 };\nchar s[] = \"abc\";")
            .unwrap()
            .parse()
            .unwrap();
        SemanticAnalyzer::new().analyze(&mut tu).unwrap();
        let sizes: Vec<usize> = tu
            .declarations
            .iter()
            .map(|d| match &d.kind {
                DeclKind::Variable(v) => v.ty.size(),
                _ => 0,
            })
            .collect();
        assert_eq!(sizes, vec![24, 4]);
    }
}
