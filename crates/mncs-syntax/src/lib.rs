use std::collections::BTreeSet;

use serde::Serialize;

/// Tokenizer-neutral measurements for one source representation.
///
/// `lexical_units` is not intended to predict any particular model tokenizer.
/// It counts identifiers, literals, and punctuation/operator groups after
/// comments and whitespace are removed. This makes comparisons deterministic
/// across machines and independent of a vendor-specific vocabulary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceMetrics {
    pub bytes: usize,
    pub characters: usize,
    pub non_whitespace_characters: usize,
    pub lines: usize,
    pub lexical_units: usize,
    pub identifiers: usize,
    pub unique_identifiers: usize,
    pub numeric_literals: usize,
    pub string_literals: usize,
    pub comments: usize,
    pub max_nesting_depth: usize,
}

pub fn analyze(source: &str) -> SourceMetrics {
    let chars: Vec<char> = source.chars().collect();
    let mut index = 0;
    let mut lexical_units = 0;
    let mut identifiers = 0;
    let mut unique_identifiers = BTreeSet::new();
    let mut numeric_literals = 0;
    let mut string_literals = 0;
    let mut comments = 0;
    let mut nesting_depth = 0usize;
    let mut max_nesting_depth = 0usize;

    while index < chars.len() {
        let current = chars[index];

        if current.is_whitespace() {
            index += 1;
            continue;
        }

        if current == '/' && chars.get(index + 1) == Some(&'/') {
            comments += 1;
            index += 2;
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            continue;
        }

        if current == '/' && chars.get(index + 1) == Some(&'*') {
            comments += 1;
            index += 2;
            let mut depth = 1usize;
            while index < chars.len() && depth > 0 {
                if chars[index] == '/' && chars.get(index + 1) == Some(&'*') {
                    depth += 1;
                    index += 2;
                } else if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            continue;
        }

        if current == '"' || current == '\'' {
            lexical_units += 1;
            string_literals += 1;
            let delimiter = current;
            index += 1;
            while index < chars.len() {
                if chars[index] == '\\' {
                    index = (index + 2).min(chars.len());
                } else if chars[index] == delimiter {
                    index += 1;
                    break;
                } else {
                    index += 1;
                }
            }
            continue;
        }

        if is_identifier_start(current) {
            let start = index;
            index += 1;
            while index < chars.len() && is_identifier_continue(chars[index]) {
                index += 1;
            }

            let identifier: String = chars[start..index].iter().collect();
            unique_identifiers.insert(identifier);
            identifiers += 1;
            lexical_units += 1;
            continue;
        }

        if current.is_ascii_digit() {
            numeric_literals += 1;
            lexical_units += 1;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric()
                    || matches!(chars[index], '_' | '.' | 'x' | 'X'))
            {
                index += 1;
            }
            continue;
        }

        match current {
            '(' | '[' | '{' => {
                nesting_depth += 1;
                max_nesting_depth = max_nesting_depth.max(nesting_depth);
            }
            ')' | ']' | '}' => {
                nesting_depth = nesting_depth.saturating_sub(1);
            }
            _ => {}
        }

        lexical_units += 1;
        index += operator_width(&chars[index..]);
    }

    SourceMetrics {
        bytes: source.len(),
        characters: chars.len(),
        non_whitespace_characters: chars.iter().filter(|value| !value.is_whitespace()).count(),
        lines: if source.is_empty() {
            0
        } else {
            source.bytes().filter(|byte| *byte == b'\n').count() + 1
        },
        lexical_units,
        identifiers,
        unique_identifiers: unique_identifiers.len(),
        numeric_literals,
        string_literals,
        comments,
        max_nesting_depth,
    }
}

fn is_identifier_start(value: char) -> bool {
    value == '_' || value.is_alphabetic()
}

fn is_identifier_continue(value: char) -> bool {
    value == '_' || value.is_alphanumeric()
}

fn operator_width(remaining: &[char]) -> usize {
    const THREE_CHARACTER_OPERATORS: [&str; 3] = ["...", "<<=", ">>="];
    const TWO_CHARACTER_OPERATORS: [&str; 21] = [
        "->", "=>", "==", "!=", "<=", ">=", "+=", "-=", "*=", "/=", "%=", "&&", "||",
        "::", "..", "<<", ">>", "&=", "|=", "^=", "??",
    ];

    for operator in THREE_CHARACTER_OPERATORS {
        if starts_with(remaining, operator) {
            return 3;
        }
    }

    for operator in TWO_CHARACTER_OPERATORS {
        if starts_with(remaining, operator) {
            return 2;
        }
    }

    1
}

fn starts_with(remaining: &[char], expected: &str) -> bool {
    let mut expected = expected.chars();

    for actual in remaining {
        match expected.next() {
            Some(value) if *actual == value => {}
            Some(_) => return false,
            None => return true,
        }
    }

    expected.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::analyze;

    #[test]
    fn ignores_comments_when_counting_lexical_units() {
        let without_comment = analyze("let value = 1;");
        let with_comment = analyze("let value = 1; // req hidden_claim\n");

        assert_eq!(with_comment.comments, 1);
        assert_eq!(with_comment.lexical_units, without_comment.lexical_units);
        assert_eq!(with_comment.identifiers, without_comment.identifiers);
    }

    #[test]
    fn groups_common_operators() {
        let metrics = analyze("req amount >= 0 && balance != 0;");

        assert_eq!(metrics.lexical_units, 9);
        assert_eq!(metrics.numeric_literals, 2);
    }

    #[test]
    fn records_structural_depth() {
        let metrics = analyze("fn f(a: T) T { if (a.ok) { return a; } }");

        assert_eq!(metrics.max_nesting_depth, 2);
        assert!(metrics.unique_identifiers <= metrics.identifiers);
    }
}
