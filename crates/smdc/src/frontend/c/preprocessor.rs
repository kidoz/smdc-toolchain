//! C Preprocessor for #include directive support
//!
//! This module provides a simple preprocessor that handles:
//! - `#include <file>` - search in system include paths
//! - `#include "file"` - search relative to source file, then system paths
//!
//! The preprocessor runs before lexing, expanding includes via text substitution.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::common::{CompileError, CompileResult, Span};

/// C Preprocessor
pub struct Preprocessor {
    /// System include paths (e.g., sdk/c/include)
    include_paths: Vec<PathBuf>,
    /// Track included files to prevent circular includes
    included_files: HashSet<PathBuf>,
    /// Current file being processed (for relative includes)
    current_dir: PathBuf,
}

impl Preprocessor {
    /// Create a new preprocessor with given include paths
    pub fn new(include_paths: Vec<PathBuf>) -> Self {
        Self {
            include_paths,
            included_files: HashSet::new(),
            current_dir: PathBuf::from("."),
        }
    }

    /// Process source code, expanding all #include directives
    pub fn process(&mut self, source: &str, source_path: &Path) -> CompileResult<String> {
        // Set current directory from source file
        self.current_dir = source_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        // Add source file to included set to prevent self-inclusion
        if let Ok(canonical) = fs::canonicalize(source_path) {
            self.included_files.insert(canonical);
        }

        self.expand_includes(source)
    }

    /// Expand #include directives in source code
    fn expand_includes(&mut self, source: &str) -> CompileResult<String> {
        let mut result = String::with_capacity(source.len());
        let mut line_start = 0;

        for line in source.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("#include") {
                // Parse the include directive
                let include_content = self.parse_include_directive(trimmed, line_start)?;
                result.push_str(&include_content);
                result.push('\n');
            } else if trimmed.starts_with('#') {
                // Skip other preprocessor directives (comments them out)
                result.push_str("/* ");
                result.push_str(line);
                result.push_str(" */\n");
            } else {
                // Regular line - pass through
                result.push_str(line);
                result.push('\n');
            }

            line_start += line.len() + 1; // +1 for newline
        }

        Ok(result)
    }

    /// Parse an #include directive and return the included file contents
    fn parse_include_directive(&mut self, line: &str, offset: usize) -> CompileResult<String> {
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
    fn include_system_file(&mut self, filename: &str, offset: usize) -> CompileResult<String> {
        // Search in include paths
        for path in &self.include_paths {
            let full_path = path.join(filename);
            if full_path.exists() {
                return self.read_and_expand(&full_path, offset);
            }
        }

        Err(CompileError::parser(
            format!("cannot find include file: <{}>", filename),
            Span::new(offset, offset + filename.len() + 10),
        ))
    }

    /// Include a local file (search relative to current file, then system paths)
    fn include_local_file(&mut self, filename: &str, offset: usize) -> CompileResult<String> {
        // First, try relative to current file
        let relative_path = self.current_dir.join(filename);
        if relative_path.exists() {
            return self.read_and_expand(&relative_path, offset);
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
        if self.included_files.contains(&canonical) {
            // Already included - return empty (like include guards)
            return Ok(format!("/* already included: {} */\n", path.display()));
        }

        // Mark as included
        self.included_files.insert(canonical.clone());

        // Read the file
        let content = fs::read_to_string(path).map_err(|e| {
            CompileError::parser(
                format!("cannot read include file {}: {}", path.display(), e),
                Span::new(offset, offset + 1),
            )
        })?;

        // Save current directory and set to included file's directory
        let saved_dir = self.current_dir.clone();
        self.current_dir = path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        // Add markers for debugging
        let mut result = String::new();
        result.push_str(&format!("/* BEGIN {} */\n", path.display()));

        // Recursively expand includes in the included file
        let expanded = self.expand_includes(&content)?;
        result.push_str(&expanded);

        result.push_str(&format!("/* END {} */\n", path.display()));

        // Restore current directory
        self.current_dir = saved_dir;

        Ok(result)
    }
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

    #[test]
    fn test_no_includes() {
        let source = "int main(void) { return 0; }";
        let mut pp = Preprocessor::new(vec![]);
        let result = pp.expand_includes(source).unwrap();
        assert!(result.contains("int main"));
    }

    #[test]
    fn test_skip_other_directives() {
        let source = "#define FOO 1\nint x = FOO;";
        let mut pp = Preprocessor::new(vec![]);
        let result = pp.expand_includes(source).unwrap();
        // #define should be commented out
        assert!(result.contains("/*"));
        assert!(result.contains("#define FOO 1"));
    }
}
