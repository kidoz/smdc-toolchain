//! Rust lexer implementation using logos

use super::token::{RustToken, RustTokenKind};
use crate::common::{CompileError, CompileResult, Span};
use logos::Logos;

/// Lexer for Rust source code
pub struct RustLexer<'a> {
    inner: logos::Lexer<'a, RustTokenKind>,
    /// Buffer for peeked tokens (supports 2-token lookahead)
    peeked: Vec<RustToken>,
    at_eof: bool,
}

impl<'a> RustLexer<'a> {
    /// Create a new lexer for the given source code
    pub fn new(source: &'a str) -> Self {
        Self {
            inner: RustTokenKind::lexer(source),
            peeked: Vec::new(),
            at_eof: false,
        }
    }

    /// Get the next token
    pub fn next_token(&mut self) -> CompileResult<RustToken> {
        // Return from buffer first
        if !self.peeked.is_empty() {
            return Ok(self.peeked.remove(0));
        }

        self.scan_token()
    }

    /// Scan a new token from source
    fn scan_token(&mut self) -> CompileResult<RustToken> {
        if self.at_eof {
            return Ok(RustToken::new(RustTokenKind::Eof, Span::default()));
        }

        match self.inner.next() {
            Some(Ok(kind)) => {
                let span = self.inner.span();
                Ok(RustToken::new(kind, Span::new(span.start, span.end)))
            }
            Some(Err(())) => {
                let span = self.inner.span();
                Err(CompileError::lexer(
                    format!("unexpected character '{}'", self.inner.slice()),
                    Span::new(span.start, span.end),
                ))
            }
            None => {
                self.at_eof = true;
                let len = self.inner.source().len();
                Ok(RustToken::new(RustTokenKind::Eof, Span::new(len, len)))
            }
        }
    }

    /// Peek at the next token without consuming it
    pub fn peek(&mut self) -> CompileResult<&RustToken> {
        if self.peeked.is_empty() {
            let token = self.scan_token()?;
            self.peeked.push(token);
        }
        Ok(&self.peeked[0])
    }

    /// Peek at the token at offset (0 = next, 1 = after next, etc.)
    pub fn peek_at(&mut self, offset: usize) -> CompileResult<&RustToken> {
        // Ensure we have enough tokens in the buffer
        while self.peeked.len() <= offset {
            let token = self.scan_token()?;
            self.peeked.push(token);
        }
        Ok(&self.peeked[offset])
    }

    /// Check if the next token matches the expected kind
    pub fn check(&mut self, expected: &RustTokenKind) -> CompileResult<bool> {
        Ok(std::mem::discriminant(&self.peek()?.kind) == std::mem::discriminant(expected))
    }

    /// Check if the token AFTER the current peek matches expected kind (2-token lookahead)
    pub fn check_lookahead(&mut self, expected: &RustTokenKind) -> CompileResult<bool> {
        // Use peek_at(1) to look at the token after the current one
        let token = self.peek_at(1)?;
        Ok(std::mem::discriminant(&token.kind) == std::mem::discriminant(expected))
    }

    /// Consume the next token if it matches, return true if consumed
    pub fn match_token(&mut self, expected: &RustTokenKind) -> CompileResult<bool> {
        if self.check(expected)? {
            self.next_token()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Expect a specific token kind, error if not found
    pub fn expect(&mut self, expected: RustTokenKind) -> CompileResult<RustToken> {
        let token = self.next_token()?;
        if std::mem::discriminant(&token.kind) == std::mem::discriminant(&expected) {
            Ok(token)
        } else {
            Err(CompileError::parser(
                format!("expected {}, found {}", expected, token.kind),
                token.span,
            ))
        }
    }

    /// Tokenize the entire source and return all tokens
    pub fn tokenize_all(mut self) -> CompileResult<Vec<RustToken>> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token()?;
            let is_eof = matches!(token.kind, RustTokenKind::Eof);
            tokens.push(token);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    /// Get the source being lexed
    pub fn source(&self) -> &'a str {
        self.inner.source()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keywords() {
        let source = "fn let mut struct enum impl if else while for";
        let mut lexer = RustLexer::new(source);

        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Fn));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Let));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Mut));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Struct));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Enum));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Impl));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::If));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Else));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::While));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::For));
    }

    #[test]
    fn test_primitive_types() {
        let source = "i8 i16 i32 u8 u16 u32 bool char";
        let mut lexer = RustLexer::new(source);

        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::I8));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::I16));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::I32));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::U8));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::U16));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::U32));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Bool));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Char));
    }

    #[test]
    fn test_identifiers() {
        let source = "foo bar_baz _test test123";
        let mut lexer = RustLexer::new(source);

        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::Identifier(s) if s == "foo"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::Identifier(s) if s == "bar_baz"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::Identifier(s) if s == "_test"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::Identifier(s) if s == "test123"
        ));
    }

    #[test]
    fn test_integer_literals() {
        let source = "42 0xFF 0o77 0b1010 123_456";
        let mut lexer = RustLexer::new(source);

        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::IntLiteral(s) if s == "42"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::HexLiteral(s) if s == "0xFF"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::OctalLiteral(s) if s == "0o77"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::BinaryLiteral(s) if s == "0b1010"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::IntLiteral(s) if s == "123_456"
        ));
    }

    #[test]
    fn test_operators() {
        let source = "+ - * / % == != < > <= >= && || -> => :: ..";
        let mut lexer = RustLexer::new(source);

        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Plus));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Minus));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Star));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Slash));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Percent));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::EqEq));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::NotEq));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Lt));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Gt));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::LtEq));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::GtEq));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::AmpAmp));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::PipePipe));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Arrow));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::FatArrow));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::ColonColon));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::DotDot));
    }

    #[test]
    fn test_string_and_char_literals() {
        let source = r#""hello world" 'a' '\n'"#;
        let mut lexer = RustLexer::new(source);

        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::StringLiteral(s) if s == "\"hello world\""
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::CharLiteral(s) if s == "'a'"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::CharLiteral(s) if s == "'\\n'"
        ));
    }

    #[test]
    fn test_comments() {
        let source = "fn // line comment\nmain /* block */ ()";
        let mut lexer = RustLexer::new(source);

        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::Fn));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::Identifier(s) if s == "main"
        ));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::LParen));
        assert!(matches!(lexer.next_token().unwrap().kind, RustTokenKind::RParen));
    }

    #[test]
    fn test_simple_function() {
        let source = "fn main() -> i32 { 0 }";
        let tokens = RustLexer::new(source).tokenize_all().unwrap();

        assert!(matches!(tokens[0].kind, RustTokenKind::Fn));
        assert!(matches!(&tokens[1].kind, RustTokenKind::Identifier(s) if s == "main"));
        assert!(matches!(tokens[2].kind, RustTokenKind::LParen));
        assert!(matches!(tokens[3].kind, RustTokenKind::RParen));
        assert!(matches!(tokens[4].kind, RustTokenKind::Arrow));
        assert!(matches!(tokens[5].kind, RustTokenKind::I32));
        assert!(matches!(tokens[6].kind, RustTokenKind::LBrace));
        assert!(matches!(&tokens[7].kind, RustTokenKind::IntLiteral(s) if s == "0"));
        assert!(matches!(tokens[8].kind, RustTokenKind::RBrace));
        assert!(matches!(tokens[9].kind, RustTokenKind::Eof));
    }

    #[test]
    fn test_lifetimes() {
        let source = "'a 'static 'lifetime";
        let mut lexer = RustLexer::new(source);

        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::Lifetime(s) if s == "a"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::Lifetime(s) if s == "static"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            RustTokenKind::Lifetime(s) if s == "lifetime"
        ));
    }

    #[test]
    fn test_match_expression() {
        let source = "match x { 1 => true, _ => false }";
        let tokens = RustLexer::new(source).tokenize_all().unwrap();

        assert!(matches!(tokens[0].kind, RustTokenKind::Match));
        assert!(matches!(&tokens[1].kind, RustTokenKind::Identifier(s) if s == "x"));
        assert!(matches!(tokens[2].kind, RustTokenKind::LBrace));
        assert!(matches!(&tokens[3].kind, RustTokenKind::IntLiteral(s) if s == "1"));
        assert!(matches!(tokens[4].kind, RustTokenKind::FatArrow));
        assert!(matches!(tokens[5].kind, RustTokenKind::True));
        assert!(matches!(tokens[6].kind, RustTokenKind::Comma));
        assert!(matches!(&tokens[7].kind, RustTokenKind::Identifier(s) if s == "_"));
        assert!(matches!(tokens[8].kind, RustTokenKind::FatArrow));
        assert!(matches!(tokens[9].kind, RustTokenKind::False));
        assert!(matches!(tokens[10].kind, RustTokenKind::RBrace));
    }
}
