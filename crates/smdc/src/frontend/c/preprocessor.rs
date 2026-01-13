//! C89-compliant Preprocessor
//!
//! This module provides a preprocessor that handles:
//! - `#include <file>` - search in system include paths
//! - `#include "file"` - search relative to source file, then system paths
//! - `#define NAME value` - simple macro definitions
//! - `#define NAME(args) value` - function-like macros
//! - `#ifdef`, `#ifndef`, `#if`, `#elif`, `#else`, `#endif` - conditional compilation
//! - `#undef` - undefine macros
//! - `#error` - emit error message
//! - `#line` - set line number
//! - `#pragma` - implementation-defined behavior (ignored)
//! - `#` (stringification) and `##` (token pasting) in macro bodies
//! - Predefined macros: __FILE__, __LINE__, __DATE__, __TIME__, __STDC__

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::common::{CompileError, CompileResult, Span};

/// A macro definition
#[derive(Clone, Debug)]
struct MacroDef {
    /// Macro parameters (None for object-like macros)
    params: Option<Vec<String>>,
    /// Replacement text
    body: String,
    /// Is this a predefined macro that requires special handling?
    is_predefined: bool,
}

/// Conditional state
#[derive(Clone, Debug)]
struct ConditionalState {
    /// Is this branch active?
    active: bool,
    /// Has any branch in this #if chain been taken?
    any_branch_taken: bool,
    /// Is this an #else (can't have more #elif after)
    seen_else: bool,
}

/// C Preprocessor
pub struct Preprocessor {
    /// System include paths (e.g., sdk/c/include)
    include_paths: Vec<PathBuf>,
    /// Track the active include stack to detect cycles
    include_stack: Vec<PathBuf>,
    /// Current file being processed (for relative includes and __FILE__)
    current_file: PathBuf,
    /// Current directory for relative includes
    current_dir: PathBuf,
    /// Current line number (for __LINE__)
    current_line: usize,
    /// Macro definitions
    macros: HashMap<String, MacroDef>,
    /// Date string for __DATE__
    date_str: String,
    /// Time string for __TIME__
    time_str: String,
}

impl Preprocessor {
    /// Create a new preprocessor with given include paths
    pub fn new(include_paths: Vec<PathBuf>) -> Self {
        // Get current date/time for predefined macros
        let now = std::time::SystemTime::now();
        let datetime = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
        let secs = datetime.as_secs();

        // Simple date/time calculation (not perfect but good enough)
        let days_since_1970 = secs / 86400;
        let year = 1970 + (days_since_1970 / 365) as i32; // Approximate
        let month = ((days_since_1970 % 365) / 30) as u32 + 1;
        let day = ((days_since_1970 % 365) % 30) as u32 + 1;
        let hour = ((secs % 86400) / 3600) as u32;
        let minute = ((secs % 3600) / 60) as u32;
        let second = (secs % 60) as u32;

        let months = ["Jan", "Feb", "Mar", "Apr", "May", "Jun",
                      "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
        let month_name = months.get((month - 1) as usize).unwrap_or(&"Jan");

        let date_str = format!("\"{} {:2} {}\"", month_name, day, year);
        let time_str = format!("\"{:02}:{:02}:{:02}\"", hour, minute, second);

        let mut pp = Self {
            include_paths,
            include_stack: Vec::new(),
            current_file: PathBuf::from(""),
            current_dir: PathBuf::from("."),
            current_line: 1,
            macros: HashMap::new(),
            date_str,
            time_str,
        };

        // Register predefined macros
        pp.macros.insert("__STDC__".to_string(), MacroDef {
            params: None,
            body: "1".to_string(),
            is_predefined: false,
        });
        pp.macros.insert("__STDC_VERSION__".to_string(), MacroDef {
            params: None,
            body: "199409L".to_string(), // C89 with amendments
            is_predefined: false,
        });
        // __FILE__, __LINE__, __DATE__, __TIME__ are handled specially
        pp.macros.insert("__FILE__".to_string(), MacroDef {
            params: None,
            body: String::new(),
            is_predefined: true,
        });
        pp.macros.insert("__LINE__".to_string(), MacroDef {
            params: None,
            body: String::new(),
            is_predefined: true,
        });
        pp.macros.insert("__DATE__".to_string(), MacroDef {
            params: None,
            body: String::new(),
            is_predefined: true,
        });
        pp.macros.insert("__TIME__".to_string(), MacroDef {
            params: None,
            body: String::new(),
            is_predefined: true,
        });

        pp
    }

    /// Process source code, expanding all preprocessor directives
    pub fn process(&mut self, source: &str, source_path: &Path) -> CompileResult<String> {
        // Set current file and directory
        self.current_file = source_path.to_path_buf();
        self.current_dir = source_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        // Add source file to include stack to detect circular includes
        let mut pushed_root = false;
        if let Ok(canonical) = fs::canonicalize(source_path) {
            self.include_stack.push(canonical);
            pushed_root = true;
        }

        // Phase 1: Replace trigraphs (C89 feature)
        let trigraph_replaced = Self::replace_trigraphs(source);

        // Phase 2: Strip comments (replace with spaces, preserve newlines)
        let no_comments = Self::strip_comments(&trigraph_replaced);

        // Phase 3: expand includes and handle conditionals
        let included = match self.expand_includes(&no_comments) {
            Ok(value) => value,
            Err(err) => {
                if pushed_root {
                    self.include_stack.pop();
                }
                return Err(err);
            }
        };

        // Phase 4: expand macros in the result
        let expanded = self.expand_macros(&included);

        if pushed_root {
            self.include_stack.pop();
        }

        Ok(expanded)
    }

    /// Replace comments with spaces (preserving newlines for line counting)
    /// Both C89 block comments /* */ and C99 line comments // are handled
    fn strip_comments(source: &str) -> String {
        let mut result = String::with_capacity(source.len());
        let chars: Vec<char> = source.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            // Check for string literals - don't strip "comments" inside strings
            if chars[i] == '"' {
                result.push(chars[i]);
                i += 1;
                // Copy until end of string, handling escapes
                while i < chars.len() {
                    result.push(chars[i]);
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                        result.push(chars[i]);
                    } else if chars[i] == '"' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }

            // Check for character literals
            if chars[i] == '\'' {
                result.push(chars[i]);
                i += 1;
                while i < chars.len() {
                    result.push(chars[i]);
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                        result.push(chars[i]);
                    } else if chars[i] == '\'' {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }

            // Check for line comment //
            if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
                // Skip until end of line, replace with space
                result.push(' ');
                i += 2;
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }

            // Check for block comment /* */
            if i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '*' {
                result.push(' ');
                i += 2;
                while i + 1 < chars.len() {
                    if chars[i] == '\n' {
                        // Preserve newlines for line counting
                        result.push('\n');
                    }
                    if chars[i] == '*' && chars[i + 1] == '/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                continue;
            }

            result.push(chars[i]);
            i += 1;
        }

        result
    }

    /// Replace C89 trigraph sequences with their single-character equivalents
    /// Trigraphs:
    ///   ??= → #    ??/ → \    ??' → ^
    ///   ??( → [    ??) → ]    ??! → |
    ///   ??< → {    ??> → }    ??- → ~
    fn replace_trigraphs(source: &str) -> String {
        let mut result = String::with_capacity(source.len());
        let chars: Vec<char> = source.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if i + 2 < chars.len() && chars[i] == '?' && chars[i + 1] == '?' {
                let replacement = match chars[i + 2] {
                    '=' => Some('#'),
                    '/' => Some('\\'),
                    '\'' => Some('^'),
                    '(' => Some('['),
                    ')' => Some(']'),
                    '!' => Some('|'),
                    '<' => Some('{'),
                    '>' => Some('}'),
                    '-' => Some('~'),
                    _ => None,
                };

                if let Some(c) = replacement {
                    result.push(c);
                    i += 3;
                    continue;
                }
            }

            result.push(chars[i]);
            i += 1;
        }

        result
    }

    /// Expand #include directives and handle conditionals
    fn expand_includes(&mut self, source: &str) -> CompileResult<String> {
        let mut result = String::with_capacity(source.len());
        let mut line_num = 0;
        let mut in_multiline_macro = false;
        let mut multiline_buffer = String::new();

        // Conditional compilation stack
        let mut cond_stack: Vec<ConditionalState> = vec![];

        for line in source.lines() {
            line_num += 1;
            self.current_line = line_num;
            let trimmed = line.trim();

            // Check if currently in an active conditional block
            let active = cond_stack.iter().all(|s| s.active);

            // Check if we're continuing a multiline macro
            if in_multiline_macro {
                multiline_buffer.push_str(line);
                if line.ends_with('\\') {
                    multiline_buffer.push('\n');
                } else {
                    // End of multiline macro - process it
                    if active {
                        self.process_directive(&multiline_buffer.replace("\\\n", " "));
                    }
                    multiline_buffer.clear();
                    in_multiline_macro = false;
                }
                result.push('\n');
            } else if trimmed.starts_with("#include") {
                if active {
                    let (include_content, include_path) = self.parse_include_directive(trimmed, 0)?;
                    result.push_str(&format!("#line 1 \"{}\"\n", include_path.display()));
                    result.push_str(&include_content);
                    if !include_content.ends_with('\n') {
                        result.push('\n');
                    }
                    result.push_str(&format!(
                        "#line {} \"{}\"\n",
                        line_num + 1,
                        self.current_file.display()
                    ));
                } else {
                    result.push('\n');
                }
            } else if trimmed.starts_with("#ifdef") {
                let name = trimmed.trim_start_matches("#ifdef").trim();
                let defined = self.macros.contains_key(name);
                cond_stack.push(ConditionalState {
                    active: active && defined,
                    any_branch_taken: defined,
                    seen_else: false,
                });
                result.push('\n');
            } else if trimmed.starts_with("#ifndef") {
                let name = trimmed.trim_start_matches("#ifndef").trim();
                let defined = self.macros.contains_key(name);
                cond_stack.push(ConditionalState {
                    active: active && !defined,
                    any_branch_taken: !defined,
                    seen_else: false,
                });
                result.push('\n');
            } else if trimmed.starts_with("#if ") || trimmed == "#if" {
                let expr = trimmed.trim_start_matches("#if").trim();
                let value = if active {
                    self.evaluate_constant_expression(expr)
                } else {
                    0
                };
                let is_true = value != 0;
                cond_stack.push(ConditionalState {
                    active: active && is_true,
                    any_branch_taken: is_true,
                    seen_else: false,
                });
                result.push('\n');
            } else if trimmed.starts_with("#elif") {
                // Check seen_else first before mutable borrow
                let seen_else = cond_stack.last().map(|s| s.seen_else).unwrap_or(false);
                if seen_else {
                    return Err(CompileError::parser(
                        "#elif after #else",
                        Span::new(0, trimmed.len()),
                    ));
                }

                let expr = trimmed.trim_start_matches("#elif").trim();
                // Calculate parent_active before mutable borrow
                let parent_active = cond_stack.len() <= 1 ||
                    cond_stack[..cond_stack.len().saturating_sub(1)].iter().all(|s| s.active);
                let any_branch_taken = cond_stack.last().map(|s| s.any_branch_taken).unwrap_or(false);

                if let Some(state) = cond_stack.last_mut() {
                    if parent_active && !any_branch_taken {
                        let value = self.evaluate_constant_expression(expr);
                        let is_true = value != 0;
                        state.active = is_true;
                        if is_true {
                            state.any_branch_taken = true;
                        }
                    } else {
                        state.active = false;
                    }
                }
                result.push('\n');
            } else if trimmed.starts_with("#else") {
                // Check seen_else first before mutable borrow
                let seen_else = cond_stack.last().map(|s| s.seen_else).unwrap_or(false);
                if seen_else {
                    return Err(CompileError::parser(
                        "duplicate #else",
                        Span::new(0, trimmed.len()),
                    ));
                }

                // Calculate parent_active before mutable borrow
                let parent_active = cond_stack.len() <= 1 ||
                    cond_stack[..cond_stack.len().saturating_sub(1)].iter().all(|s| s.active);
                let any_branch_taken = cond_stack.last().map(|s| s.any_branch_taken).unwrap_or(false);

                if let Some(state) = cond_stack.last_mut() {
                    state.seen_else = true;
                    state.active = parent_active && !any_branch_taken;
                }
                result.push('\n');
            } else if trimmed.starts_with("#endif") {
                if cond_stack.pop().is_none() {
                    return Err(CompileError::parser(
                        "unexpected #endif",
                        Span::new(0, trimmed.len()),
                    ));
                }
                result.push('\n');
            } else if trimmed.starts_with("#define") {
                if active {
                    if line.ends_with('\\') {
                        in_multiline_macro = true;
                        multiline_buffer = line.to_string();
                        multiline_buffer.push('\n');
                    } else {
                        self.process_define(trimmed);
                    }
                }
                result.push('\n');
            } else if trimmed.starts_with("#undef") {
                if active {
                    let name = trimmed.trim_start_matches("#undef").trim();
                    self.macros.remove(name);
                }
                result.push('\n');
            } else if trimmed.starts_with("#error") {
                if active {
                    let msg = trimmed.trim_start_matches("#error").trim();
                    return Err(CompileError::parser(
                        format!("#error: {}", msg),
                        Span::new(0, trimmed.len()),
                    ));
                }
                result.push('\n');
            } else if trimmed.starts_with("#line") {
                if active {
                    result.push_str(line);
                }
                result.push('\n');
            } else if trimmed.starts_with("#pragma") {
                // Pragmas are implementation-defined, we just ignore them
                result.push('\n');
            } else if trimmed.starts_with('#') && !trimmed.starts_with("##") {
                // Skip unknown preprocessor directives (or empty # line)
                result.push('\n');
            } else if active {
                // Regular line - pass through
                result.push_str(line);
                result.push('\n');
            } else {
                // Inactive conditional - skip content but maintain line count
                result.push('\n');
            }
        }

        if !cond_stack.is_empty() {
            return Err(CompileError::parser(
                "unterminated conditional directive",
                Span::new(0, 1),
            ));
        }

        Ok(result)
    }

    /// Evaluate a preprocessor constant expression
    fn evaluate_constant_expression(&self, expr: &str) -> i64 {
        // First, expand macros in the expression
        let expanded = self.expand_macros_in_expr(expr);

        // Parse and evaluate the expression
        self.eval_expr(&expanded)
    }

    /// Expand macros in a preprocessor expression (handles defined() operator)
    fn expand_macros_in_expr(&self, expr: &str) -> String {
        let mut result = String::new();
        let mut chars = expr.chars().peekable();

        while let Some(c) = chars.next() {
            if c.is_alphabetic() || c == '_' {
                let mut ident = String::new();
                ident.push(c);
                while let Some(&next) = chars.peek() {
                    if next.is_alphanumeric() || next == '_' {
                        ident.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                // Handle defined() operator
                if ident == "defined" {
                    // Skip whitespace
                    while chars.peek() == Some(&' ') {
                        chars.next();
                    }

                    let has_parens = chars.peek() == Some(&'(');
                    if has_parens {
                        chars.next(); // consume '('
                    }

                    // Skip whitespace
                    while chars.peek() == Some(&' ') {
                        chars.next();
                    }

                    // Get macro name
                    let mut macro_name = String::new();
                    while let Some(&next) = chars.peek() {
                        if next.is_alphanumeric() || next == '_' {
                            macro_name.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }

                    if has_parens {
                        // Skip whitespace and closing paren
                        while chars.peek() == Some(&' ') {
                            chars.next();
                        }
                        if chars.peek() == Some(&')') {
                            chars.next();
                        }
                    }

                    let is_defined = self.macros.contains_key(&macro_name);
                    result.push_str(if is_defined { "1" } else { "0" });
                } else if let Some(mac) = self.macros.get(&ident) {
                    if mac.params.is_none() && !mac.is_predefined {
                        result.push_str(&mac.body);
                    } else {
                        // For undefined identifiers in preprocessor expressions, use 0
                        result.push('0');
                    }
                } else {
                    // Undefined identifiers evaluate to 0 in preprocessor expressions
                    result.push('0');
                }
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Simple expression evaluator for preprocessor #if expressions
    fn eval_expr(&self, expr: &str) -> i64 {
        let expr = expr.trim();
        if expr.is_empty() {
            return 0;
        }

        // Handle ternary operator (lowest precedence)
        if let Some((cond, rest)) = self.split_ternary(expr) {
            if let Some((then_expr, else_expr)) = self.split_at_char(rest, ':') {
                let cond_val = self.eval_expr(cond);
                return if cond_val != 0 {
                    self.eval_expr(then_expr)
                } else {
                    self.eval_expr(else_expr)
                };
            }
        }

        // Handle logical OR (||)
        if let Some((left, right)) = self.split_binary_op(expr, "||") {
            let l = self.eval_expr(left);
            let r = self.eval_expr(right);
            return if l != 0 || r != 0 { 1 } else { 0 };
        }

        // Handle logical AND (&&)
        if let Some((left, right)) = self.split_binary_op(expr, "&&") {
            let l = self.eval_expr(left);
            let r = self.eval_expr(right);
            return if l != 0 && r != 0 { 1 } else { 0 };
        }

        // Handle bitwise OR (|)
        if let Some((left, right)) = self.split_binary_op(expr, "|") {
            if !right.starts_with('|') { // Avoid matching ||
                return self.eval_expr(left) | self.eval_expr(right);
            }
        }

        // Handle bitwise XOR (^)
        if let Some((left, right)) = self.split_binary_op(expr, "^") {
            return self.eval_expr(left) ^ self.eval_expr(right);
        }

        // Handle bitwise AND (&)
        if let Some((left, right)) = self.split_binary_op(expr, "&") {
            if !right.starts_with('&') { // Avoid matching &&
                return self.eval_expr(left) & self.eval_expr(right);
            }
        }

        // Handle equality operators (==, !=)
        if let Some((left, right)) = self.split_binary_op(expr, "==") {
            return if self.eval_expr(left) == self.eval_expr(right) { 1 } else { 0 };
        }
        if let Some((left, right)) = self.split_binary_op(expr, "!=") {
            return if self.eval_expr(left) != self.eval_expr(right) { 1 } else { 0 };
        }

        // Handle relational operators (<=, >=, <, >)
        if let Some((left, right)) = self.split_binary_op(expr, "<=") {
            return if self.eval_expr(left) <= self.eval_expr(right) { 1 } else { 0 };
        }
        if let Some((left, right)) = self.split_binary_op(expr, ">=") {
            return if self.eval_expr(left) >= self.eval_expr(right) { 1 } else { 0 };
        }
        if let Some((left, right)) = self.split_binary_op(expr, "<") {
            if !right.starts_with('<') && !right.starts_with('=') {
                return if self.eval_expr(left) < self.eval_expr(right) { 1 } else { 0 };
            }
        }
        if let Some((left, right)) = self.split_binary_op(expr, ">") {
            if !right.starts_with('>') && !right.starts_with('=') {
                return if self.eval_expr(left) > self.eval_expr(right) { 1 } else { 0 };
            }
        }

        // Handle shift operators (<<, >>)
        if let Some((left, right)) = self.split_binary_op(expr, "<<") {
            return self.eval_expr(left).wrapping_shl(self.eval_expr(right) as u32);
        }
        if let Some((left, right)) = self.split_binary_op(expr, ">>") {
            return self.eval_expr(left).wrapping_shr(self.eval_expr(right) as u32);
        }

        // Handle additive operators (+, -)
        // Be careful with unary minus
        if let Some((left, right)) = self.split_additive(expr) {
            let op = if expr[left.len()..].starts_with('+') { '+' } else { '-' };
            let l = self.eval_expr(left);
            let r = self.eval_expr(right);
            return if op == '+' { l + r } else { l - r };
        }

        // Handle multiplicative operators (*, /, %)
        if let Some((left, right)) = self.split_binary_op(expr, "*") {
            return self.eval_expr(left).wrapping_mul(self.eval_expr(right));
        }
        if let Some((left, right)) = self.split_binary_op(expr, "/") {
            let r = self.eval_expr(right);
            if r == 0 { return 0; }
            return self.eval_expr(left) / r;
        }
        if let Some((left, right)) = self.split_binary_op(expr, "%") {
            let r = self.eval_expr(right);
            if r == 0 { return 0; }
            return self.eval_expr(left) % r;
        }

        // Handle unary operators
        if expr.starts_with('!') {
            return if self.eval_expr(&expr[1..]) == 0 { 1 } else { 0 };
        }
        if expr.starts_with('~') {
            return !self.eval_expr(&expr[1..]);
        }
        if expr.starts_with('-') && expr.len() > 1 {
            return -self.eval_expr(&expr[1..]);
        }
        if expr.starts_with('+') && expr.len() > 1 {
            return self.eval_expr(&expr[1..]);
        }

        // Handle parentheses
        if expr.starts_with('(') && expr.ends_with(')') {
            let inner = &expr[1..expr.len()-1];
            // Check if the parens are balanced
            let mut depth = 0;
            let mut balanced = true;
            for c in inner.chars() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        if depth == 0 {
                            balanced = false;
                            break;
                        }
                        depth -= 1;
                    }
                    _ => {}
                }
            }
            if balanced && depth == 0 {
                return self.eval_expr(inner);
            }
        }

        // Handle character literals
        if expr.starts_with('\'') && expr.ends_with('\'') && expr.len() >= 3 {
            let inner = &expr[1..expr.len()-1];
            if inner.starts_with('\\') {
                match inner.chars().nth(1) {
                    Some('n') => return b'\n' as i64,
                    Some('t') => return b'\t' as i64,
                    Some('r') => return b'\r' as i64,
                    Some('0') => return 0,
                    Some('\\') => return b'\\' as i64,
                    Some('\'') => return b'\'' as i64,
                    Some(c) => return c as i64,
                    None => return 0,
                }
            } else {
                return inner.chars().next().unwrap_or('\0') as i64;
            }
        }

        // Handle numeric literals
        let expr = expr.trim_end_matches(['L', 'l', 'U', 'u']);
        if expr.starts_with("0x") || expr.starts_with("0X") {
            i64::from_str_radix(&expr[2..], 16).unwrap_or(0)
        } else if expr.starts_with("0b") || expr.starts_with("0B") {
            i64::from_str_radix(&expr[2..], 2).unwrap_or(0)
        } else if expr.starts_with('0') && expr.len() > 1 && expr.chars().skip(1).all(|c| c >= '0' && c <= '7') {
            i64::from_str_radix(&expr[1..], 8).unwrap_or(0)
        } else {
            expr.parse().unwrap_or(0)
        }
    }

    /// Split expression at a ternary operator
    fn split_ternary<'a>(&self, expr: &'a str) -> Option<(&'a str, &'a str)> {
        let mut depth = 0;
        for (i, c) in expr.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                '?' if depth == 0 => {
                    return Some((&expr[..i], &expr[i+1..]));
                }
                _ => {}
            }
        }
        None
    }

    /// Split at a single character at depth 0
    fn split_at_char<'a>(&self, expr: &'a str, sep: char) -> Option<(&'a str, &'a str)> {
        let mut depth = 0;
        for (i, c) in expr.char_indices() {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                c if c == sep && depth == 0 => {
                    return Some((&expr[..i], &expr[i+1..]));
                }
                _ => {}
            }
        }
        None
    }

    /// Split expression at a binary operator (at paren depth 0, from right)
    fn split_binary_op<'a>(&self, expr: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
        let mut depth = 0;
        let chars: Vec<char> = expr.chars().collect();
        let op_chars: Vec<char> = op.chars().collect();

        // Scan from right to left for left-associative operators
        let mut i = chars.len();
        while i > 0 {
            i -= 1;
            match chars[i] {
                ')' => depth += 1,
                '(' => depth -= 1,
                _ if depth == 0 => {
                    // Check if operator matches at this position
                    if i + op_chars.len() <= chars.len() {
                        let matches = (0..op_chars.len()).all(|j| chars[i + j] == op_chars[j]);
                        if matches && i > 0 {
                            let left = &expr[..i];
                            let right = &expr[i + op.len()..];
                            if !left.is_empty() {
                                return Some((left.trim(), right.trim()));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Split at + or - for additive expressions (handling unary minus)
    fn split_additive<'a>(&self, expr: &'a str) -> Option<(&'a str, &'a str)> {
        let mut depth = 0;
        let chars: Vec<char> = expr.chars().collect();

        // Scan from right to left
        let mut i = chars.len();
        while i > 0 {
            i -= 1;
            match chars[i] {
                ')' => depth += 1,
                '(' => depth -= 1,
                '+' | '-' if depth == 0 && i > 0 => {
                    let left = &expr[..i];
                    let right = &expr[i + 1..];
                    // Make sure this isn't a unary operator or part of another operator
                    let prev = chars.get(i.saturating_sub(1));
                    if !left.is_empty() && !matches!(prev, Some('*') | Some('/') | Some('%') | Some('<') | Some('>') | Some('&') | Some('|') | Some('^') | Some('=') | Some('!') | Some('(')) {
                        return Some((left.trim(), right.trim()));
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Process a preprocessor directive (for multiline macros)
    fn process_directive(&mut self, line: &str) {
        let trimmed = line.trim();
        if trimmed.starts_with("#define") {
            self.process_define(trimmed);
        }
    }

    /// Process a #define directive
    fn process_define(&mut self, line: &str) {
        let rest = line.trim_start_matches("#define").trim();

        // Check for function-like macro: NAME(args)
        if let Some(paren_pos) = rest.find('(') {
            // Make sure there's no space before the paren (function-like macro requirement)
            let name = &rest[..paren_pos];
            if !name.contains(char::is_whitespace) {
                if let Some(end_paren) = rest.find(')') {
                    let params_str = &rest[paren_pos + 1..end_paren];
                    let params: Vec<String> = if params_str.trim().is_empty() {
                        vec![]
                    } else {
                        params_str.split(',').map(|s| s.trim().to_string()).collect()
                    };
                    let body = rest[end_paren + 1..].trim().to_string();

                    self.macros.insert(name.to_string(), MacroDef {
                        params: Some(params),
                        body,
                        is_predefined: false,
                    });
                    return;
                }
            }
        }

        // Object-like macro: NAME value
        let mut parts = rest.splitn(2, char::is_whitespace);
        if let Some(name) = parts.next() {
            let body = parts.next().unwrap_or("").trim().to_string();
            self.macros.insert(name.to_string(), MacroDef {
                params: None,
                body,
                is_predefined: false,
            });
        }
    }

    /// Expand all macros in the source
    fn expand_macros(&self, source: &str) -> String {
        let mut result = source.to_string();

        // Keep expanding until no more expansions possible
        let mut changed = true;
        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 100;

        while changed && iterations < MAX_ITERATIONS {
            changed = false;
            iterations += 1;

            let mut new_result = String::with_capacity(result.len());
            let mut chars = result.chars().peekable();
            let mut logical_line = 1usize;
            let mut current_file = self.current_file.clone();
            let mut at_line_start = true;

            while let Some(c) = chars.next() {
                if at_line_start && c == '#' {
                    let mut directive = String::new();
                    directive.push(c);
                    while let Some(next) = chars.peek().copied() {
                        directive.push(chars.next().unwrap());
                        if next == '\n' {
                            break;
                        }
                    }

                    if let Some((new_line, new_file)) = parse_line_directive(&directive) {
                        logical_line = new_line;
                        if let Some(file) = new_file {
                            current_file = file;
                        }
                        if directive.ends_with('\n') {
                            new_result.push('\n');
                            at_line_start = true;
                        } else {
                            at_line_start = false;
                        }
                        continue;
                    }

                    new_result.push_str(&directive);
                    if directive.ends_with('\n') {
                        logical_line += 1;
                        at_line_start = true;
                    } else {
                        at_line_start = false;
                    }
                    continue;
                }

                if c == '\n' {
                    logical_line += 1;
                    new_result.push(c);
                    at_line_start = true;
                    continue;
                }

                // Check for start of identifier
                if c.is_alphabetic() || c == '_' {
                    let mut ident = String::new();
                    ident.push(c);

                    // Collect rest of identifier
                    while let Some(&next) = chars.peek() {
                        if next.is_alphanumeric() || next == '_' {
                            ident.push(chars.next().unwrap());
                        } else {
                            break;
                        }
                    }

                    // Check if it's a macro
                    if let Some(mac) = self.macros.get(&ident) {
                        // Handle predefined macros
                        if mac.is_predefined {
                            let expansion = match ident.as_str() {
                                "__FILE__" => format!("\"{}\"", current_file.display()),
                                "__LINE__" => format!("{}", logical_line),
                                "__DATE__" => self.date_str.clone(),
                                "__TIME__" => self.time_str.clone(),
                                _ => ident.clone(),
                            };
                            new_result.push_str(&expansion);
                            changed = true;
                            at_line_start = false;
                            continue;
                        }

                        if let Some(ref params) = mac.params {
                            // Function-like macro - look for arguments
                            // Skip whitespace
                            while let Some(&next) = chars.peek() {
                                if next.is_whitespace() && next != '\n' {
                                    chars.next();
                                } else {
                                    break;
                                }
                            }

                            if chars.peek() == Some(&'(') {
                                chars.next(); // consume '('

                                // Parse arguments
                                let mut args: Vec<String> = vec![];
                                let mut current_arg = String::new();
                                let mut paren_depth = 1;

                                while let Some(c) = chars.next() {
                                    match c {
                                        '(' => {
                                            paren_depth += 1;
                                            current_arg.push(c);
                                        }
                                        ')' => {
                                            paren_depth -= 1;
                                            if paren_depth == 0 {
                                                args.push(current_arg.trim().to_string());
                                                break;
                                            }
                                            current_arg.push(c);
                                        }
                                        ',' if paren_depth == 1 => {
                                            args.push(current_arg.trim().to_string());
                                            current_arg = String::new();
                                        }
                                        _ => current_arg.push(c),
                                    }
                                }

                                // Process stringification and token pasting first
                                let mut expanded = mac.body.clone();

                                // Handle # (stringification)
                                for (i, param) in params.iter().enumerate() {
                                    if let Some(arg) = args.get(i) {
                                        let pattern = format!("#{}", param);
                                        let stringified = format!("\"{}\"", arg);
                                        expanded = expanded.replace(&pattern, &stringified);
                                    }
                                }

                                // Handle ## (token pasting)
                                for (i, param) in params.iter().enumerate() {
                                    if let Some(arg) = args.get(i) {
                                        // Pattern: identifier ## param
                                        let pattern = format!(" ## {}", param);
                                        expanded = expanded.replace(&pattern, arg);
                                        let pattern = format!("## {}", param);
                                        expanded = expanded.replace(&pattern, arg);
                                        // Pattern: param ## identifier
                                        let pattern = format!("{} ##", param);
                                        expanded = expanded.replace(&pattern, arg);
                                        let pattern = format!("{}##", param);
                                        expanded = expanded.replace(&pattern, arg);
                                    }
                                }

                                // Remove remaining ## (for cases where both sides were substituted)
                                expanded = expanded.replace(" ## ", "");
                                expanded = expanded.replace("## ", "");
                                expanded = expanded.replace(" ##", "");
                                expanded = expanded.replace("##", "");

                                // Substitute parameters
                                for (i, param) in params.iter().enumerate() {
                                    if let Some(arg) = args.get(i) {
                                        expanded = replace_identifier(&expanded, param, arg);
                                    }
                                }

                                new_result.push_str(&expanded);
                                changed = true;
                                at_line_start = false;
                            } else {
                                // No arguments - not a macro invocation
                                new_result.push_str(&ident);
                                at_line_start = false;
                            }
                        } else {
                            // Object-like macro
                            if !mac.body.is_empty() {
                                new_result.push_str(&mac.body);
                                changed = true;
                            } else {
                                // Empty body - just remove the identifier
                                changed = true;
                            }
                            at_line_start = false;
                        }
                    } else {
                        // Not a macro
                        new_result.push_str(&ident);
                        at_line_start = false;
                    }
                } else if c == '/' && chars.peek() == Some(&'*') {
                    // Skip block comments
                    new_result.push(c);
                    new_result.push(chars.next().unwrap());
                    let mut saw_newline = false;
                    while let Some(c) = chars.next() {
                        if c == '\n' {
                            logical_line += 1;
                            saw_newline = true;
                        }
                        new_result.push(c);
                        if c == '*' && chars.peek() == Some(&'/') {
                            new_result.push(chars.next().unwrap());
                            break;
                        }
                    }
                    at_line_start = saw_newline;
                } else if c == '/' && chars.peek() == Some(&'/') {
                    // Skip line comments
                    new_result.push(c);
                    let mut ended_with_newline = false;
                    while let Some(c) = chars.next() {
                        new_result.push(c);
                        if c == '\n' {
                            logical_line += 1;
                            ended_with_newline = true;
                            break;
                        }
                    }
                    at_line_start = ended_with_newline;
                } else if c == '"' || c == '\'' {
                    // Skip string/char literals
                    let quote = c;
                    new_result.push(c);
                    while let Some(c) = chars.next() {
                        new_result.push(c);
                        if c == '\\' {
                            if let Some(escaped) = chars.next() {
                                new_result.push(escaped);
                            }
                        } else if c == quote {
                            break;
                        }
                    }
                    at_line_start = false;
                } else {
                    new_result.push(c);
                    at_line_start = false;
                }
            }

            result = new_result;
        }

        result
    }

    /// Parse an #include directive and return the included file contents
    fn parse_include_directive(&mut self, line: &str, offset: usize) -> CompileResult<(String, PathBuf)> {
        // Extract the part after #include
        let rest = line.trim_start_matches("#include").trim();

        if rest.starts_with('<') {
            // System include: #include <file>
            if let Some(end) = rest.find('>') {
                let filename = &rest[1..end];
                return self.include_system_file(filename, offset);
            }
        } else if rest.starts_with('"') {
            // Local include: #include "file"
            if let Some(end) = rest[1..].find('"') {
                let filename = &rest[1..end + 1];
                return self.include_local_file(filename, offset);
            }
        }

        Err(CompileError::parser(
            format!("invalid #include directive: {}", line),
            Span::new(offset, offset + line.len()),
        ))
    }

    /// Include a system file (search in include paths)
    fn include_system_file(&mut self, filename: &str, offset: usize) -> CompileResult<(String, PathBuf)> {
        // Search in include paths
        for path in &self.include_paths {
            let full_path = path.join(filename);
            if full_path.exists() {
                let content = self.read_and_expand(&full_path, offset)?;
                return Ok((content, full_path));
            }
        }

        Err(CompileError::parser(
            format!("cannot find include file: <{}>", filename),
            Span::new(offset, offset + filename.len() + 10),
        ))
    }

    /// Include a local file (search relative to current file, then system paths)
    fn include_local_file(&mut self, filename: &str, offset: usize) -> CompileResult<(String, PathBuf)> {
        // First, try relative to current file
        let relative_path = self.current_dir.join(filename);
        if relative_path.exists() {
            let content = self.read_and_expand(&relative_path, offset)?;
            return Ok((content, relative_path));
        }

        // Fall back to system include paths
        self.include_system_file(filename, offset)
    }

    /// Read a file and recursively expand its includes
    fn read_and_expand(&mut self, path: &Path, offset: usize) -> CompileResult<String> {
        // Get canonical path for circular include detection
        let canonical = fs::canonicalize(path).map_err(|e| {
            CompileError::parser(
                format!("cannot resolve path {}: {}", path.display(), e),
                Span::new(offset, offset + 1),
            )
        })?;

        // Check for circular include
        if self.include_stack.contains(&canonical) {
            return Err(CompileError::parser(
                format!("circular include detected: {}", path.display()),
                Span::new(offset, offset + 1),
            ));
        }

        // Mark as included
        self.include_stack.push(canonical.clone());

        // Read the file
        let content = fs::read_to_string(path).map_err(|e| {
            CompileError::parser(
                format!("cannot read include file {}: {}", path.display(), e),
                Span::new(offset, offset + 1),
            )
        })?;

        // Save current state
        let saved_dir = self.current_dir.clone();
        let saved_file = self.current_file.clone();

        // Set to included file's context
        self.current_file = path.to_path_buf();
        self.current_dir = path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        // Recursively expand includes in the included file
        let expanded = self.expand_includes(&content);

        // Restore state
        self.current_dir = saved_dir;
        self.current_file = saved_file;
        self.include_stack.pop();

        expanded
    }
}

fn parse_line_directive(line: &str) -> Option<(usize, Option<PathBuf>)> {
    let trimmed = line.trim();
    if !trimmed.starts_with("#line") {
        return None;
    }
    let rest = trimmed.trim_start_matches("#line").trim();
    let mut parts = rest.split_whitespace();
    let line_num = parts.next()?.parse::<usize>().ok()?;
    let file = parts.next().map(|part| PathBuf::from(part.trim_matches('"')));
    Some((line_num, file))
}

/// Replace identifier occurrences (not inside other identifiers)
fn replace_identifier(text: &str, from: &str, to: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c.is_alphabetic() || c == '_' {
            let mut ident = String::new();
            ident.push(c);

            while let Some(&next) = chars.peek() {
                if next.is_alphanumeric() || next == '_' {
                    ident.push(chars.next().unwrap());
                } else {
                    break;
                }
            }

            if ident == from {
                result.push_str(to);
            } else {
                result.push_str(&ident);
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Convenience function to preprocess source with default SDK include path
pub fn preprocess(
    source: &str,
    source_path: &Path,
    include_paths: Vec<PathBuf>,
) -> CompileResult<String> {
    let mut preprocessor = Preprocessor::new(include_paths);
    preprocessor.process(source, source_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_no_includes() {
        let source = "int main(void) { return 0; }";
        let mut pp = Preprocessor::new(vec![]);
        let result = pp.process(source, Path::new("test.c")).unwrap();
        assert!(result.contains("int main"));
    }

    #[test]
    fn test_define_simple() {
        let source = "#define FOO 42\nint x = FOO;";
        let mut pp = Preprocessor::new(vec![]);
        let result = pp.process(source, Path::new("test.c")).unwrap();
        assert!(result.contains("int x = 42"));
    }

    #[test]
    fn test_define_function() {
        let source = "#define MAX(a, b) ((a) > (b) ? (a) : (b))\nint x = MAX(1, 2);";
        let mut pp = Preprocessor::new(vec![]);
        let result = pp.process(source, Path::new("test.c")).unwrap();
        assert!(result.contains("((1) > (2) ? (1) : (2))"));
    }

    #[test]
    fn test_ifdef() {
        let source = "#define FOO\n#ifdef FOO\nint x = 1;\n#else\nint x = 2;\n#endif";
        let mut pp = Preprocessor::new(vec![]);
        let result = pp.process(source, Path::new("test.c")).unwrap();
        assert!(result.contains("int x = 1"));
        assert!(!result.contains("int x = 2"));
    }

    #[test]
    fn test_ifndef() {
        let source = "#ifndef BAR\nint x = 1;\n#else\nint x = 2;\n#endif";
        let mut pp = Preprocessor::new(vec![]);
        let result = pp.process(source, Path::new("test.c")).unwrap();
        assert!(result.contains("int x = 1"));
        assert!(!result.contains("int x = 2"));
    }

    #[test]
    fn test_if_expression() {
        let source = "#define VERSION 2\n#if VERSION > 1\nint x = 1;\n#else\nint x = 2;\n#endif";
        let mut pp = Preprocessor::new(vec![]);
        let result = pp.process(source, Path::new("test.c")).unwrap();
        assert!(result.contains("int x = 1"));
        assert!(!result.contains("int x = 2"));
    }

    #[test]
    fn test_if_defined() {
        let source = "#define FOO\n#if defined(FOO)\nint x = 1;\n#endif";
        let mut pp = Preprocessor::new(vec![]);
        let result = pp.process(source, Path::new("test.c")).unwrap();
        assert!(result.contains("int x = 1"));
    }

    #[test]
    fn test_elif() {
        let source = "#define VERSION 2\n#if VERSION == 1\nint x = 1;\n#elif VERSION == 2\nint x = 2;\n#else\nint x = 3;\n#endif";
        let mut pp = Preprocessor::new(vec![]);
        let result = pp.process(source, Path::new("test.c")).unwrap();
        assert!(result.contains("int x = 2"));
        assert!(!result.contains("int x = 1"));
        assert!(!result.contains("int x = 3"));
    }

    #[test]
    fn test_stringification() {
        let source = "#define STR(x) #x\nchar *s = STR(hello);";
        let mut pp = Preprocessor::new(vec![]);
        let result = pp.process(source, Path::new("test.c")).unwrap();
        assert!(result.contains("\"hello\""));
    }

    #[test]
    fn test_predefined_stdc() {
        let source = "int x = __STDC__;";
        let mut pp = Preprocessor::new(vec![]);
        let result = pp.process(source, Path::new("test.c")).unwrap();
        assert!(result.contains("int x = 1"));
    }

    #[test]
    fn test_trigraphs() {
        // Test trigraph replacement
        // ??= → #, ??( → [, ??) → ], ??< → {, ??> → }
        let source = "??=define FOO 1\nint arr??(3??) = ??< 1, 2, 3 ??>;";
        let mut pp = Preprocessor::new(vec![]);
        let result = pp.process(source, Path::new("test.c")).unwrap();
        // FOO should be defined (??= became #)
        // Array brackets and braces should be replaced
        assert!(result.contains("int arr[3] = { 1, 2, 3 }"));
    }

    #[test]
    fn test_unexpected_endif() {
        let source = "#endif";
        let mut pp = Preprocessor::new(vec![]);
        assert!(pp.process(source, Path::new("test.c")).is_err());
    }

    #[test]
    fn test_reinclude_header() {
        let temp_dir = std::env::temp_dir()
            .join(format!(
                "smdc_pp_{}",
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
            ));
        fs::create_dir_all(&temp_dir).unwrap();

        let header_path = temp_dir.join("a.h");
        fs::write(&header_path, "int A;\n").unwrap();

        let main_path = temp_dir.join("main.c");
        fs::write(&main_path, "").unwrap();

        let source = "#include \"a.h\"\n#include \"a.h\"\n";
        let mut pp = Preprocessor::new(vec![]);
        let result = pp.process(source, &main_path).unwrap();

        assert_eq!(result.matches("int A;").count(), 2);
    }

    #[test]
    fn test_circular_include_error() {
        let temp_dir = std::env::temp_dir()
            .join(format!(
                "smdc_pp_{}",
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
            ));
        fs::create_dir_all(&temp_dir).unwrap();

        let a_path = temp_dir.join("a.h");
        let b_path = temp_dir.join("b.h");
        fs::write(&a_path, "#include \"b.h\"\n").unwrap();
        fs::write(&b_path, "#include \"a.h\"\n").unwrap();

        let main_path = temp_dir.join("main.c");
        fs::write(&main_path, "").unwrap();

        let source = "#include \"a.h\"\n";
        let mut pp = Preprocessor::new(vec![]);
        assert!(pp.process(source, &main_path).is_err());
    }

    #[test]
    fn test_file_and_line_in_include() {
        let temp_dir = std::env::temp_dir()
            .join(format!(
                "smdc_pp_{}",
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
            ));
        fs::create_dir_all(&temp_dir).unwrap();

        let header_path = temp_dir.join("a.h");
        fs::write(
            &header_path,
            "int x = __LINE__;\nconst char* f = __FILE__;\n",
        )
        .unwrap();

        let main_path = temp_dir.join("main.c");
        fs::write(&main_path, "").unwrap();

        let source = "#include \"a.h\"\n";
        let mut pp = Preprocessor::new(vec![]);
        let result = pp.process(source, &main_path).unwrap();

        assert!(result.contains("int x = 1;"));
        assert!(result.contains(&format!("\"{}\"", header_path.display())));
    }
}
