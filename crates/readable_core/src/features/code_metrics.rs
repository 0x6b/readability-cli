//! Code-related metrics: braces, backticks, indentation, camelCase

/// Count code-related metrics
/// Returns: (braces, backticks, indent_lines, camel_case)
pub fn count_code_metrics(text: &str) -> (usize, usize, usize, usize) {
    let mut braces = 0;
    let mut backticks = 0;
    let mut indent_lines = 0;
    let mut camel_case = 0;

    for c in text.chars() {
        match c {
            '{' | '}' | '[' | ']' | '(' | ')' | ';' => braces += 1,
            '`' => backticks += 1,
            _ => {}
        }
    }

    // Count indent lines
    for line in text.lines() {
        if line.starts_with("  ") || line.starts_with('\t') {
            indent_lines += 1;
        }
    }

    // Count camelCase words (rough estimate)
    let mut prev_lower = false;
    for c in text.chars() {
        if prev_lower && c.is_uppercase() {
            camel_case += 1;
        }
        prev_lower = c.is_lowercase();
    }

    (braces, backticks, indent_lines, camel_case)
}
