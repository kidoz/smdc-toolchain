//! Source location span

use std::ops::Range;

/// Source location span
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

impl From<Span> for Range<usize> {
    fn from(span: Span) -> Self {
        span.start..span.end
    }
}

impl From<Range<usize>> for Span {
    fn from(range: Range<usize>) -> Self {
        Span {
            start: range.start,
            end: range.end,
        }
    }
}

/// Convert a byte offset in source text to a 1-based line number.
///
/// Returns 1 for offsets within the first line, 2 for the second, etc.
/// Offsets beyond the source length return the last line number.
pub fn byte_offset_to_line(source: &str, offset: usize) -> usize {
    let clamped = offset.min(source.len());
    source[..clamped].bytes().filter(|&b| b == b'\n').count() + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_offset_to_line_single_line() {
        assert_eq!(byte_offset_to_line("hello", 0), 1);
        assert_eq!(byte_offset_to_line("hello", 3), 1);
        assert_eq!(byte_offset_to_line("hello", 5), 1);
    }

    #[test]
    fn byte_offset_to_line_multi_line() {
        let src = "line1\nline2\nline3\n";
        assert_eq!(byte_offset_to_line(src, 0), 1); // start of line 1
        assert_eq!(byte_offset_to_line(src, 5), 1); // the '\n' itself
        assert_eq!(byte_offset_to_line(src, 6), 2); // start of line 2
        assert_eq!(byte_offset_to_line(src, 11), 2); // '\n' at end of line 2
        assert_eq!(byte_offset_to_line(src, 12), 3); // start of line 3
    }

    #[test]
    fn byte_offset_to_line_empty() {
        assert_eq!(byte_offset_to_line("", 0), 1);
    }

    #[test]
    fn byte_offset_to_line_beyond_end() {
        assert_eq!(byte_offset_to_line("ab\ncd", 999), 2);
    }
}
