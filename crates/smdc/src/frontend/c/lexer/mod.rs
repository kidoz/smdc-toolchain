//! Lexer module for tokenizing C source code

mod token;
mod scanner;

pub use token::{Token, TokenKind};
pub use scanner::Lexer;
