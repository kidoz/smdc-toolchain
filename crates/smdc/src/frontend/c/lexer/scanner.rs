//! Lexer implementation using logos

use super::token::{Token, TokenKind};
use crate::common::{CompileError, CompileResult, Span};
use logos::Logos;

/// Lexer for C source code
pub struct Lexer<'a> {
    inner: logos::Lexer<'a, TokenKind>,
    peeked: Option<Token>,
    at_eof: bool,
}

impl<'a> Lexer<'a> {
    /// Create a new lexer for the given source code
    pub fn new(source: &'a str) -> Self {
        Self {
            inner: TokenKind::lexer(source),
            peeked: None,
            at_eof: false,
        }
    }

    /// Get the next token
    pub fn next_token(&mut self) -> CompileResult<Token> {
        if let Some(token) = self.peeked.take() {
            return Ok(token);
        }

        if self.at_eof {
            return Ok(Token::new(TokenKind::Eof, Span::default()));
        }

        match self.inner.next() {
            Some(Ok(kind)) => {
                let span = self.inner.span();
                Ok(Token::new(kind, Span::new(span.start, span.end)))
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
                Ok(Token::new(TokenKind::Eof, Span::new(len, len)))
            }
        }
    }

    /// Peek at the next token without consuming it
    pub fn peek(&mut self) -> CompileResult<&Token> {
        if self.peeked.is_none() {
            self.peeked = Some(self.next_token()?);
        }
        Ok(self.peeked.as_ref().unwrap())
    }

    /// Check if the next token matches the expected kind
    pub fn check(&mut self, expected: &TokenKind) -> CompileResult<bool> {
        Ok(std::mem::discriminant(&self.peek()?.kind) == std::mem::discriminant(expected))
    }

    /// Consume the next token if it matches, return true if consumed
    pub fn match_token(&mut self, expected: &TokenKind) -> CompileResult<bool> {
        if self.check(expected)? {
            self.next_token()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Expect a specific token kind, error if not found
    pub fn expect(&mut self, expected: TokenKind) -> CompileResult<Token> {
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
    pub fn tokenize_all(mut self) -> CompileResult<Vec<Token>> {
        let mut tokens = Vec::new();
        loop {
            let token = self.next_token()?;
            let is_eof = matches!(token.kind, TokenKind::Eof);
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
        let source = "int void return if else while for";
        let mut lexer = Lexer::new(source);

        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::Int));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::Void));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::Return));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::If));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::Else));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::While));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::For));
    }

    #[test]
    fn test_identifiers() {
        let source = "foo bar_baz _test test123";
        let mut lexer = Lexer::new(source);

        assert!(matches!(
            lexer.next_token().unwrap().kind,
            TokenKind::Identifier(s) if s == "foo"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            TokenKind::Identifier(s) if s == "bar_baz"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            TokenKind::Identifier(s) if s == "_test"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            TokenKind::Identifier(s) if s == "test123"
        ));
    }

    #[test]
    fn test_integer_literals() {
        let source = "42 0x1F 0b1010 0777 123u 456L";
        let mut lexer = Lexer::new(source);

        assert!(matches!(
            lexer.next_token().unwrap().kind,
            TokenKind::IntLiteral(s) if s == "42"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            TokenKind::HexLiteral(s) if s == "0x1F"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            TokenKind::BinaryLiteral(s) if s == "0b1010"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            TokenKind::OctalLiteral(s) if s == "0777"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            TokenKind::IntLiteral(s) if s == "123u"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            TokenKind::IntLiteral(s) if s == "456L"
        ));
    }

    #[test]
    fn test_operators() {
        let source = "+ - * / % == != < > <= >= && || !";
        let mut lexer = Lexer::new(source);

        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::Plus));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::Minus));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::Star));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::Slash));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::Percent));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::EqEq));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::NotEq));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::Lt));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::Gt));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::LtEq));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::GtEq));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::AmpAmp));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::PipePipe));
        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::Bang));
    }

    #[test]
    fn test_string_and_char_literals() {
        let source = r#""hello world" 'a' '\n'"#;
        let mut lexer = Lexer::new(source);

        assert!(matches!(
            lexer.next_token().unwrap().kind,
            TokenKind::StringLiteral(s) if s == "\"hello world\""
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            TokenKind::CharLiteral(s) if s == "'a'"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            TokenKind::CharLiteral(s) if s == "'\\n'"
        ));
    }

    #[test]
    fn test_comments() {
        let source = "int // line comment\nx /* block */ y";
        let mut lexer = Lexer::new(source);

        assert!(matches!(lexer.next_token().unwrap().kind, TokenKind::Int));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            TokenKind::Identifier(s) if s == "x"
        ));
        assert!(matches!(
            lexer.next_token().unwrap().kind,
            TokenKind::Identifier(s) if s == "y"
        ));
    }

    #[test]
    fn test_simple_function() {
        let source = "int main() { return 0; }";
        let tokens = Lexer::new(source).tokenize_all().unwrap();

        // Check key tokens without hardcoding count
        assert!(matches!(tokens[0].kind, TokenKind::Int));
        assert!(matches!(&tokens[1].kind, TokenKind::Identifier(s) if s == "main"));
        assert!(matches!(tokens[2].kind, TokenKind::LParen));
        assert!(matches!(tokens[3].kind, TokenKind::RParen));
        assert!(matches!(tokens[4].kind, TokenKind::LBrace));
        assert!(matches!(tokens[5].kind, TokenKind::Return));
        assert!(matches!(&tokens[6].kind, TokenKind::IntLiteral(s) if s == "0"));
        assert!(matches!(tokens[7].kind, TokenKind::Semi));
        assert!(matches!(tokens[8].kind, TokenKind::RBrace));
        assert!(matches!(tokens[9].kind, TokenKind::Eof));
    }
}
