//! Rust token definitions using logos

use crate::common::Span;
use logos::Logos;
use std::fmt;

/// A Rust token with its kind and source location
#[derive(Debug, Clone)]
pub struct RustToken {
    pub kind: RustTokenKind,
    pub span: Span,
}

impl RustToken {
    pub fn new(kind: RustTokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// Rust token kinds
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\n\r\f]+")]
#[logos(skip r"//[^\n]*")]
#[logos(skip r"/\*([^*]|\*[^/])*\*/")]
pub enum RustTokenKind {
    // Keywords - Control Flow
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("loop")]
    Loop,
    #[token("while")]
    While,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("match")]
    Match,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("return")]
    Return,

    // Keywords - Declarations
    #[token("let")]
    Let,
    #[token("mut")]
    Mut,
    #[token("fn")]
    Fn,
    #[token("struct")]
    Struct,
    #[token("enum")]
    Enum,
    #[token("impl")]
    Impl,
    #[token("trait")]
    Trait,
    #[token("type")]
    Type,
    #[token("const")]
    Const,
    #[token("static")]
    Static,
    #[token("mod")]
    Mod,
    #[token("use")]
    Use,
    #[token("pub")]
    Pub,
    #[token("crate")]
    Crate,
    #[token("super")]
    Super,

    // Keywords - Other
    #[token("as")]
    As,
    #[token("ref")]
    Ref,
    #[token("self")]
    SelfValue,
    #[token("Self")]
    SelfType,
    #[token("unsafe")]
    Unsafe,
    #[token("where")]
    Where,
    #[token("move")]
    Move,

    // Boolean literals
    #[token("true")]
    True,
    #[token("false")]
    False,

    // Primitive types
    #[token("i8")]
    I8,
    #[token("i16")]
    I16,
    #[token("i32")]
    I32,
    #[token("i64")]
    I64,
    #[token("u8")]
    U8,
    #[token("u16")]
    U16,
    #[token("u32")]
    U32,
    #[token("u64")]
    U64,
    #[token("isize")]
    Isize,
    #[token("usize")]
    Usize,
    #[token("f32")]
    F32,
    #[token("f64")]
    F64,
    #[token("bool")]
    Bool,
    #[token("char")]
    Char,
    #[token("str")]
    Str,

    // Integer literals (decimal, hex, octal, binary)
    #[regex(r"0x[0-9a-fA-F][0-9a-fA-F_]*", priority = 3, callback = |lex| lex.slice().to_string())]
    HexLiteral(String),
    #[regex(r"0o[0-7][0-7_]*", priority = 3, callback = |lex| lex.slice().to_string())]
    OctalLiteral(String),
    #[regex(r"0b[01][01_]*", priority = 3, callback = |lex| lex.slice().to_string())]
    BinaryLiteral(String),
    #[regex(r"[0-9][0-9_]*(_?[iu](8|16|32|64|size))?", priority = 2, callback = |lex| lex.slice().to_string())]
    IntLiteral(String),

    // Float literals
    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*([eE][+-]?[0-9_]+)?(_?f(32|64))?", priority = 3, callback = |lex| lex.slice().to_string())]
    FloatLiteral(String),

    // Character literal
    #[regex(r"'([^'\\]|\\.)'", callback = |lex| lex.slice().to_string())]
    CharLiteral(String),

    // String literal
    #[regex(r#""([^"\\]|\\.)*""#, callback = |lex| lex.slice().to_string())]
    StringLiteral(String),

    // Raw string literal (simplified - no arbitrary # count)
    #[regex(r#"r"[^"]*""#, callback = |lex| lex.slice().to_string())]
    RawStringLiteral(String),

    // Byte literal
    #[regex(r"b'([^'\\]|\\.)'", callback = |lex| lex.slice().to_string())]
    ByteLiteral(String),

    // Byte string literal
    #[regex(r#"b"([^"\\]|\\.)*""#, callback = |lex| lex.slice().to_string())]
    ByteStringLiteral(String),

    // Identifiers
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", priority = 1, callback = |lex| lex.slice().to_string())]
    Identifier(String),

    // Lifetime
    #[regex(r"'[a-zA-Z_][a-zA-Z0-9_]*", callback = |lex| lex.slice()[1..].to_string())]
    Lifetime(String),

    // Multi-character operators (order matters - longer first)
    #[token("..=")]
    DotDotEq,
    #[token("...")]
    DotDotDot,
    #[token("..")]
    DotDot,
    #[token("::")]
    ColonColon,
    #[token("->")]
    Arrow,
    #[token("=>")]
    FatArrow,
    #[token("<<=")]
    ShlEq,
    #[token(">>=")]
    ShrEq,
    #[token("<<")]
    Shl,
    #[token(">>")]
    Shr,
    #[token("<=")]
    LtEq,
    #[token(">=")]
    GtEq,
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("&&")]
    AmpAmp,
    #[token("||")]
    PipePipe,
    #[token("+=")]
    PlusEq,
    #[token("-=")]
    MinusEq,
    #[token("*=")]
    StarEq,
    #[token("/=")]
    SlashEq,
    #[token("%=")]
    PercentEq,
    #[token("&=")]
    AmpEq,
    #[token("|=")]
    PipeEq,
    #[token("^=")]
    CaretEq,

    // Single-character operators
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("&")]
    Amp,
    #[token("|")]
    Pipe,
    #[token("^")]
    Caret,
    #[token("!")]
    Bang,
    #[token("~")]
    Tilde,
    #[token("=")]
    Eq,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("@")]
    At,
    #[token("?")]
    Question,

    // Delimiters
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,

    // Punctuation
    #[token(";")]
    Semi,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token(".")]
    Dot,
    #[token("#")]
    Hash,
    #[token("$")]
    Dollar,

    // Special
    Eof,
}

impl fmt::Display for RustTokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Keywords
            RustTokenKind::If => write!(f, "if"),
            RustTokenKind::Else => write!(f, "else"),
            RustTokenKind::Loop => write!(f, "loop"),
            RustTokenKind::While => write!(f, "while"),
            RustTokenKind::For => write!(f, "for"),
            RustTokenKind::In => write!(f, "in"),
            RustTokenKind::Match => write!(f, "match"),
            RustTokenKind::Break => write!(f, "break"),
            RustTokenKind::Continue => write!(f, "continue"),
            RustTokenKind::Return => write!(f, "return"),
            RustTokenKind::Let => write!(f, "let"),
            RustTokenKind::Mut => write!(f, "mut"),
            RustTokenKind::Fn => write!(f, "fn"),
            RustTokenKind::Struct => write!(f, "struct"),
            RustTokenKind::Enum => write!(f, "enum"),
            RustTokenKind::Impl => write!(f, "impl"),
            RustTokenKind::Trait => write!(f, "trait"),
            RustTokenKind::Type => write!(f, "type"),
            RustTokenKind::Const => write!(f, "const"),
            RustTokenKind::Static => write!(f, "static"),
            RustTokenKind::Mod => write!(f, "mod"),
            RustTokenKind::Use => write!(f, "use"),
            RustTokenKind::Pub => write!(f, "pub"),
            RustTokenKind::Crate => write!(f, "crate"),
            RustTokenKind::Super => write!(f, "super"),
            RustTokenKind::As => write!(f, "as"),
            RustTokenKind::Ref => write!(f, "ref"),
            RustTokenKind::SelfValue => write!(f, "self"),
            RustTokenKind::SelfType => write!(f, "Self"),
            RustTokenKind::Unsafe => write!(f, "unsafe"),
            RustTokenKind::Where => write!(f, "where"),
            RustTokenKind::Move => write!(f, "move"),
            RustTokenKind::True => write!(f, "true"),
            RustTokenKind::False => write!(f, "false"),

            // Types
            RustTokenKind::I8 => write!(f, "i8"),
            RustTokenKind::I16 => write!(f, "i16"),
            RustTokenKind::I32 => write!(f, "i32"),
            RustTokenKind::I64 => write!(f, "i64"),
            RustTokenKind::U8 => write!(f, "u8"),
            RustTokenKind::U16 => write!(f, "u16"),
            RustTokenKind::U32 => write!(f, "u32"),
            RustTokenKind::U64 => write!(f, "u64"),
            RustTokenKind::Isize => write!(f, "isize"),
            RustTokenKind::Usize => write!(f, "usize"),
            RustTokenKind::F32 => write!(f, "f32"),
            RustTokenKind::F64 => write!(f, "f64"),
            RustTokenKind::Bool => write!(f, "bool"),
            RustTokenKind::Char => write!(f, "char"),
            RustTokenKind::Str => write!(f, "str"),

            // Literals
            RustTokenKind::IntLiteral(s) => write!(f, "{}", s),
            RustTokenKind::HexLiteral(s) => write!(f, "{}", s),
            RustTokenKind::OctalLiteral(s) => write!(f, "{}", s),
            RustTokenKind::BinaryLiteral(s) => write!(f, "{}", s),
            RustTokenKind::FloatLiteral(s) => write!(f, "{}", s),
            RustTokenKind::CharLiteral(s) => write!(f, "{}", s),
            RustTokenKind::StringLiteral(s) => write!(f, "{}", s),
            RustTokenKind::RawStringLiteral(s) => write!(f, "{}", s),
            RustTokenKind::ByteLiteral(s) => write!(f, "{}", s),
            RustTokenKind::ByteStringLiteral(s) => write!(f, "{}", s),
            RustTokenKind::Identifier(s) => write!(f, "{}", s),
            RustTokenKind::Lifetime(s) => write!(f, "'{}", s),

            // Operators
            RustTokenKind::DotDotEq => write!(f, "..="),
            RustTokenKind::DotDotDot => write!(f, "..."),
            RustTokenKind::DotDot => write!(f, ".."),
            RustTokenKind::ColonColon => write!(f, "::"),
            RustTokenKind::Arrow => write!(f, "->"),
            RustTokenKind::FatArrow => write!(f, "=>"),
            RustTokenKind::Shl => write!(f, "<<"),
            RustTokenKind::Shr => write!(f, ">>"),
            RustTokenKind::ShlEq => write!(f, "<<="),
            RustTokenKind::ShrEq => write!(f, ">>="),
            RustTokenKind::LtEq => write!(f, "<="),
            RustTokenKind::GtEq => write!(f, ">="),
            RustTokenKind::EqEq => write!(f, "=="),
            RustTokenKind::NotEq => write!(f, "!="),
            RustTokenKind::AmpAmp => write!(f, "&&"),
            RustTokenKind::PipePipe => write!(f, "||"),
            RustTokenKind::PlusEq => write!(f, "+="),
            RustTokenKind::MinusEq => write!(f, "-="),
            RustTokenKind::StarEq => write!(f, "*="),
            RustTokenKind::SlashEq => write!(f, "/="),
            RustTokenKind::PercentEq => write!(f, "%="),
            RustTokenKind::AmpEq => write!(f, "&="),
            RustTokenKind::PipeEq => write!(f, "|="),
            RustTokenKind::CaretEq => write!(f, "^="),
            RustTokenKind::Plus => write!(f, "+"),
            RustTokenKind::Minus => write!(f, "-"),
            RustTokenKind::Star => write!(f, "*"),
            RustTokenKind::Slash => write!(f, "/"),
            RustTokenKind::Percent => write!(f, "%"),
            RustTokenKind::Amp => write!(f, "&"),
            RustTokenKind::Pipe => write!(f, "|"),
            RustTokenKind::Caret => write!(f, "^"),
            RustTokenKind::Bang => write!(f, "!"),
            RustTokenKind::Tilde => write!(f, "~"),
            RustTokenKind::Eq => write!(f, "="),
            RustTokenKind::Lt => write!(f, "<"),
            RustTokenKind::Gt => write!(f, ">"),
            RustTokenKind::At => write!(f, "@"),
            RustTokenKind::Question => write!(f, "?"),

            // Delimiters
            RustTokenKind::LParen => write!(f, "("),
            RustTokenKind::RParen => write!(f, ")"),
            RustTokenKind::LBrace => write!(f, "{{"),
            RustTokenKind::RBrace => write!(f, "}}"),
            RustTokenKind::LBracket => write!(f, "["),
            RustTokenKind::RBracket => write!(f, "]"),

            // Punctuation
            RustTokenKind::Semi => write!(f, ";"),
            RustTokenKind::Comma => write!(f, ","),
            RustTokenKind::Colon => write!(f, ":"),
            RustTokenKind::Dot => write!(f, "."),
            RustTokenKind::Hash => write!(f, "#"),
            RustTokenKind::Dollar => write!(f, "$"),

            RustTokenKind::Eof => write!(f, "EOF"),
        }
    }
}
