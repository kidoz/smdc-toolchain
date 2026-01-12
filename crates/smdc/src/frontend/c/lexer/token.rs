//! Token definitions for C lexer

use crate::common::Span;
use logos::Logos;

/// Token with source location
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

/// All token kinds in C
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\n\r\f]+")]  // Skip whitespace
#[logos(skip r"//[^\n]*")]      // Skip line comments
#[logos(skip r"/\*[^*]*\*+(?:[^/*][^*]*\*+)*/")] // Skip block comments
pub enum TokenKind {
    // === Keywords ===
    #[token("auto")]
    Auto,
    #[token("break")]
    Break,
    #[token("case")]
    Case,
    #[token("char")]
    Char,
    #[token("const")]
    Const,
    #[token("continue")]
    Continue,
    #[token("default")]
    Default,
    #[token("do")]
    Do,
    #[token("double")]
    Double,
    #[token("else")]
    Else,
    #[token("enum")]
    Enum,
    #[token("extern")]
    Extern,
    #[token("float")]
    Float,
    #[token("for")]
    For,
    #[token("goto")]
    Goto,
    #[token("if")]
    If,
    #[token("inline")]
    Inline,
    #[token("int")]
    Int,
    #[token("long")]
    Long,
    #[token("register")]
    Register,
    #[token("restrict")]
    Restrict,
    #[token("return")]
    Return,
    #[token("short")]
    Short,
    #[token("signed")]
    Signed,
    #[token("sizeof")]
    Sizeof,
    #[token("static")]
    Static,
    #[token("struct")]
    Struct,
    #[token("switch")]
    Switch,
    #[token("typedef")]
    Typedef,
    #[token("union")]
    Union,
    #[token("unsigned")]
    Unsigned,
    #[token("void")]
    Void,
    #[token("volatile")]
    Volatile,
    #[token("while")]
    While,

    // C99/C11 keywords
    #[token("_Bool")]
    Bool,
    #[token("_Complex")]
    Complex,
    #[token("_Imaginary")]
    Imaginary,
    #[token("_Alignas")]
    Alignas,
    #[token("_Alignof")]
    Alignof,
    #[token("_Atomic")]
    Atomic,
    #[token("_Generic")]
    Generic,
    #[token("_Noreturn")]
    Noreturn,
    #[token("_Static_assert")]
    StaticAssert,
    #[token("_Thread_local")]
    ThreadLocal,

    // === Identifiers ===
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice().to_string())]
    Identifier(String),

    // === Literals ===
    // Integer literals (decimal, hex, octal, binary)
    #[regex(r"0[xX][0-9a-fA-F]+[uUlL]*", |lex| lex.slice().to_string())]
    HexLiteral(String),

    #[regex(r"0[bB][01]+[uUlL]*", |lex| lex.slice().to_string())]
    BinaryLiteral(String),

    #[regex(r"0[0-7]+[uUlL]*", |lex| lex.slice().to_string())]
    OctalLiteral(String),

    #[regex(r"[0-9]+[uUlL]*", |lex| lex.slice().to_string())]
    IntLiteral(String),

    // Float literals
    #[regex(r"[0-9]+\.[0-9]*([eE][+-]?[0-9]+)?[fFlL]?", priority = 3, callback = |lex| lex.slice().to_string())]
    #[regex(r"\.[0-9]+([eE][+-]?[0-9]+)?[fFlL]?", priority = 2, callback = |lex| lex.slice().to_string())]
    #[regex(r"[0-9]+[eE][+-]?[0-9]+[fFlL]?", priority = 1, callback = |lex| lex.slice().to_string())]
    FloatLiteral(String),

    // Character literal
    #[regex(r"'([^'\\]|\\.)*'", |lex| lex.slice().to_string())]
    CharLiteral(String),

    // String literal
    #[regex(r#""([^"\\]|\\.)*""#, |lex| lex.slice().to_string())]
    StringLiteral(String),

    // === Operators ===
    // Arithmetic
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

    // Increment/Decrement
    #[token("++")]
    PlusPlus,
    #[token("--")]
    MinusMinus,

    // Comparison
    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("<=")]
    LtEq,
    #[token(">=")]
    GtEq,

    // Logical
    #[token("&&")]
    AmpAmp,
    #[token("||")]
    PipePipe,
    #[token("!")]
    Bang,

    // Bitwise
    #[token("&")]
    Amp,
    #[token("|")]
    Pipe,
    #[token("^")]
    Caret,
    #[token("~")]
    Tilde,
    #[token("<<")]
    LtLt,
    #[token(">>")]
    GtGt,

    // Assignment
    #[token("=")]
    Eq,
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
    #[token("<<=")]
    LtLtEq,
    #[token(">>=")]
    GtGtEq,

    // Punctuation
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token(";")]
    Semi,
    #[token(",")]
    Comma,
    #[token(".")]
    Dot,
    #[token("->")]
    Arrow,
    #[token(":")]
    Colon,
    #[token("?")]
    Question,
    #[token("...")]
    Ellipsis,

    // Special
    Eof,
}

impl TokenKind {
    /// Check if this token is a type specifier keyword
    pub fn is_type_specifier(&self) -> bool {
        matches!(
            self,
            TokenKind::Void
                | TokenKind::Char
                | TokenKind::Short
                | TokenKind::Int
                | TokenKind::Long
                | TokenKind::Float
                | TokenKind::Double
                | TokenKind::Signed
                | TokenKind::Unsigned
                | TokenKind::Bool
                | TokenKind::Struct
                | TokenKind::Union
                | TokenKind::Enum
        )
    }

    /// Check if this token is a type qualifier
    pub fn is_type_qualifier(&self) -> bool {
        matches!(
            self,
            TokenKind::Const | TokenKind::Volatile | TokenKind::Restrict | TokenKind::Atomic
        )
    }

    /// Check if this token is a storage class specifier
    pub fn is_storage_class(&self) -> bool {
        matches!(
            self,
            TokenKind::Typedef
                | TokenKind::Extern
                | TokenKind::Static
                | TokenKind::Auto
                | TokenKind::Register
                | TokenKind::ThreadLocal
        )
    }

    /// Check if this token can start a declaration
    pub fn can_start_declaration(&self) -> bool {
        self.is_type_specifier() || self.is_type_qualifier() || self.is_storage_class()
    }

    /// Check if this is an assignment operator
    pub fn is_assignment_op(&self) -> bool {
        matches!(
            self,
            TokenKind::Eq
                | TokenKind::PlusEq
                | TokenKind::MinusEq
                | TokenKind::StarEq
                | TokenKind::SlashEq
                | TokenKind::PercentEq
                | TokenKind::AmpEq
                | TokenKind::PipeEq
                | TokenKind::CaretEq
                | TokenKind::LtLtEq
                | TokenKind::GtGtEq
        )
    }

    /// Get the precedence of binary operators (higher = tighter binding)
    pub fn binary_precedence(&self) -> Option<u8> {
        match self {
            // Comma (lowest)
            TokenKind::Comma => Some(1),
            // Assignment
            _ if self.is_assignment_op() => Some(2),
            // Ternary (handled separately)
            TokenKind::Question => Some(3),
            // Logical OR
            TokenKind::PipePipe => Some(4),
            // Logical AND
            TokenKind::AmpAmp => Some(5),
            // Bitwise OR
            TokenKind::Pipe => Some(6),
            // Bitwise XOR
            TokenKind::Caret => Some(7),
            // Bitwise AND
            TokenKind::Amp => Some(8),
            // Equality
            TokenKind::EqEq | TokenKind::NotEq => Some(9),
            // Relational
            TokenKind::Lt | TokenKind::Gt | TokenKind::LtEq | TokenKind::GtEq => Some(10),
            // Shift
            TokenKind::LtLt | TokenKind::GtGt => Some(11),
            // Additive
            TokenKind::Plus | TokenKind::Minus => Some(12),
            // Multiplicative
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Some(13),
            _ => None,
        }
    }

    /// Check if this operator is right-associative
    pub fn is_right_associative(&self) -> bool {
        self.is_assignment_op() || matches!(self, TokenKind::Question)
    }
}

impl std::fmt::Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenKind::Identifier(s) => write!(f, "identifier '{}'", s),
            TokenKind::IntLiteral(s) => write!(f, "integer '{}'", s),
            TokenKind::HexLiteral(s) => write!(f, "hex '{}'", s),
            TokenKind::BinaryLiteral(s) => write!(f, "binary '{}'", s),
            TokenKind::OctalLiteral(s) => write!(f, "octal '{}'", s),
            TokenKind::FloatLiteral(s) => write!(f, "float '{}'", s),
            TokenKind::CharLiteral(s) => write!(f, "char {}", s),
            TokenKind::StringLiteral(s) => write!(f, "string {}", s),
            TokenKind::Plus => write!(f, "'+'"),
            TokenKind::Minus => write!(f, "'-'"),
            TokenKind::Star => write!(f, "'*'"),
            TokenKind::Slash => write!(f, "'/'"),
            TokenKind::Percent => write!(f, "'%'"),
            TokenKind::PlusPlus => write!(f, "'++'"),
            TokenKind::MinusMinus => write!(f, "'--'"),
            TokenKind::EqEq => write!(f, "'=='"),
            TokenKind::NotEq => write!(f, "'!='"),
            TokenKind::Lt => write!(f, "'<'"),
            TokenKind::Gt => write!(f, "'>'"),
            TokenKind::LtEq => write!(f, "'<='"),
            TokenKind::GtEq => write!(f, "'>='"),
            TokenKind::AmpAmp => write!(f, "'&&'"),
            TokenKind::PipePipe => write!(f, "'||'"),
            TokenKind::Bang => write!(f, "'!'"),
            TokenKind::Amp => write!(f, "'&'"),
            TokenKind::Pipe => write!(f, "'|'"),
            TokenKind::Caret => write!(f, "'^'"),
            TokenKind::Tilde => write!(f, "'~'"),
            TokenKind::LtLt => write!(f, "'<<'"),
            TokenKind::GtGt => write!(f, "'>>'"),
            TokenKind::Eq => write!(f, "'='"),
            TokenKind::PlusEq => write!(f, "'+='"),
            TokenKind::MinusEq => write!(f, "'-='"),
            TokenKind::StarEq => write!(f, "'*='"),
            TokenKind::SlashEq => write!(f, "'/='"),
            TokenKind::PercentEq => write!(f, "'%='"),
            TokenKind::AmpEq => write!(f, "'&='"),
            TokenKind::PipeEq => write!(f, "'|='"),
            TokenKind::CaretEq => write!(f, "'^='"),
            TokenKind::LtLtEq => write!(f, "'<<='"),
            TokenKind::GtGtEq => write!(f, "'>>='"),
            TokenKind::LParen => write!(f, "'('"),
            TokenKind::RParen => write!(f, "')'"),
            TokenKind::LBracket => write!(f, "'['"),
            TokenKind::RBracket => write!(f, "']'"),
            TokenKind::LBrace => write!(f, "'{{'"),
            TokenKind::RBrace => write!(f, "'}}'"),
            TokenKind::Semi => write!(f, "';'"),
            TokenKind::Comma => write!(f, "','"),
            TokenKind::Dot => write!(f, "'.'"),
            TokenKind::Arrow => write!(f, "'->'"),
            TokenKind::Colon => write!(f, "':'"),
            TokenKind::Question => write!(f, "'?'"),
            TokenKind::Ellipsis => write!(f, "'...'"),
            TokenKind::Eof => write!(f, "end of file"),
            _ => write!(f, "{:?}", self),
        }
    }
}
