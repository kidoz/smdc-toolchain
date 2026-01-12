//! Error types and diagnostic reporting

use codespan_reporting::diagnostic::{Diagnostic, Label};
use codespan_reporting::files::SimpleFiles;
use codespan_reporting::term;
use codespan_reporting::term::termcolor::{ColorChoice, StandardStream};
use thiserror::Error;
use super::Span;

/// Compile error with source location
#[derive(Error, Debug)]
pub enum CompileError {
    #[error("Lexer error at {span:?}: {message}")]
    Lexer { message: String, span: Span },

    #[error("Parser error at {span:?}: {message}")]
    Parser { message: String, span: Span },

    #[error("Semantic error at {span:?}: {message}")]
    Semantic { message: String, span: Span },

    #[error("Type error at {span:?}: {message}")]
    Type { message: String, span: Span },

    #[error("Code generation error: {message}")]
    Codegen { message: String },

    #[error("Backend error: {message}")]
    Backend { message: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl CompileError {
    pub fn lexer(message: impl Into<String>, span: Span) -> Self {
        Self::Lexer {
            message: message.into(),
            span,
        }
    }

    pub fn parser(message: impl Into<String>, span: Span) -> Self {
        Self::Parser {
            message: message.into(),
            span,
        }
    }

    pub fn semantic(message: impl Into<String>, span: Span) -> Self {
        Self::Semantic {
            message: message.into(),
            span,
        }
    }

    pub fn type_error(message: impl Into<String>, span: Span) -> Self {
        Self::Type {
            message: message.into(),
            span,
        }
    }

    pub fn codegen(message: impl Into<String>) -> Self {
        Self::Codegen {
            message: message.into(),
        }
    }

    pub fn backend(message: impl Into<String>) -> Self {
        Self::Backend {
            message: message.into(),
        }
    }
}

pub type CompileResult<T> = Result<T, CompileError>;

/// Diagnostic reporter for pretty error output
pub struct DiagnosticReporter {
    files: SimpleFiles<String, String>,
    writer: StandardStream,
    config: term::Config,
}

impl DiagnosticReporter {
    pub fn new() -> Self {
        Self {
            files: SimpleFiles::new(),
            writer: StandardStream::stderr(ColorChoice::Auto),
            config: term::Config::default(),
        }
    }

    pub fn add_file(&mut self, name: impl Into<String>, source: impl Into<String>) -> usize {
        self.files.add(name.into(), source.into())
    }

    pub fn report_error(&self, file_id: usize, error: &CompileError) {
        let diagnostic = match error {
            CompileError::Lexer { message, span } => Diagnostic::error()
                .with_message("Lexer error")
                .with_labels(vec![
                    Label::primary(file_id, span.start..span.end).with_message(message)
                ]),

            CompileError::Parser { message, span } => Diagnostic::error()
                .with_message("Syntax error")
                .with_labels(vec![
                    Label::primary(file_id, span.start..span.end).with_message(message)
                ]),

            CompileError::Semantic { message, span } => Diagnostic::error()
                .with_message("Semantic error")
                .with_labels(vec![
                    Label::primary(file_id, span.start..span.end).with_message(message)
                ]),

            CompileError::Type { message, span } => Diagnostic::error()
                .with_message("Type error")
                .with_labels(vec![
                    Label::primary(file_id, span.start..span.end).with_message(message)
                ]),

            CompileError::Codegen { message } => {
                Diagnostic::error().with_message(format!("Code generation error: {}", message))
            }

            CompileError::Backend { message } => {
                Diagnostic::error().with_message(format!("Backend error: {}", message))
            }

            CompileError::Io(err) => {
                Diagnostic::error().with_message(format!("IO error: {}", err))
            }
        };

        let _ = term::emit(&mut self.writer.lock(), &self.config, &self.files, &diagnostic);
    }
}

impl Default for DiagnosticReporter {
    fn default() -> Self {
        Self::new()
    }
}
