//! Lexer module for tokenizing C source code

mod scanner;
mod token;

pub use scanner::Lexer;
pub use token::{Token, TokenKind};
