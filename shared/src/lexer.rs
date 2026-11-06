//! Tiny SQL lexer for editor highlighting. Not a parser: every byte of the input
//! lands in exactly one token, so concatenating the tokens gives back the source.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tok {
    Keyword,
    Ident,
    Str,
    Num,
    Comment,
    Other,
}

pub(crate) const KEYWORDS: &[&str] = &[
    "ABORT", "ACTION", "ADD", "AFTER", "ALL", "ALTER", "ALWAYS", "ANALYZE", "AND", "AS", "ASC", "ATTACH",
    "AUTOINCREMENT", "BEFORE", "BEGIN", "BETWEEN", "BY", "CASCADE", "CASE", "CAST", "CHECK", "COLLATE", "COLUMN",
    "COMMIT", "CONFLICT", "CONSTRAINT", "CREATE", "CROSS", "CURRENT", "CURRENT_DATE", "CURRENT_TIME",
    "CURRENT_TIMESTAMP", "DATABASE", "DEFAULT", "DEFERRABLE", "DEFERRED", "DELETE", "DESC", "DETACH", "DISTINCT",
    "DO", "DROP", "EACH", "ELSE", "END", "ESCAPE", "EXCEPT", "EXCLUDE", "EXCLUSIVE", "EXISTS", "EXPLAIN", "FAIL",
    "FALSE", "FILTER", "FIRST", "FOLLOWING", "FOR", "FOREIGN", "FROM", "FULL", "GENERATED", "GLOB", "GROUP",
    "GROUPS", "HAVING", "IF", "IGNORE", "IMMEDIATE", "IN", "INDEX", "INDEXED", "INITIALLY", "INNER", "INSERT",
    "INSTEAD", "INTERSECT", "INTO", "IS", "ISNULL", "JOIN", "KEY", "LAST", "LEFT", "LIKE", "LIMIT", "MATCH",
    "MATERIALIZED", "NATURAL", "NO", "NOT", "NOTHING", "NOTNULL", "NULL", "NULLS", "OF", "OFFSET", "ON", "OR",
    "ORDER", "OTHERS", "OUTER", "OVER", "PARTITION", "PLAN", "PRAGMA", "PRECEDING", "PRIMARY", "QUERY", "RAISE",
    "RANGE", "RECURSIVE", "REFERENCES", "REGEXP", "REINDEX", "RELEASE", "RENAME", "REPLACE", "RESTRICT",
    "RETURNING", "RIGHT", "ROLLBACK", "ROW", "ROWS", "SAVEPOINT", "SELECT", "SET", "TABLE", "TEMP", "TEMPORARY",
    "THEN", "TIES", "TO", "TRANSACTION", "TRIGGER", "TRUE", "UNBOUNDED", "UNION", "UNIQUE", "UPDATE", "USING",
    "VACUUM", "VALUES", "VIEW", "VIRTUAL", "WHEN", "WHERE", "WINDOW", "WITH", "WITHOUT",
];

pub(crate) fn is_word_byte(b: u8) -> bool {
    b == b'_' || b == b'$' || b.is_ascii_alphanumeric() || b >= 0x80
}

/// End index (exclusive) of a quoted run starting at `start`; a doubled quote is an escape.
fn end_quoted(b: &[u8], start: usize, q: u8) -> usize {
    let mut j = start + 1;
    while j < b.len() {
        if b[j] == q {
            if b.get(j + 1) == Some(&q) {
                j += 2;
                continue;
            }
            return j + 1;
        }
        j += 1;
    }
    b.len()
}

pub fn tokenize(src: &str) -> Vec<(Tok, &str)> {
    let b = src.as_bytes();
    let mut out: Vec<(Tok, &str)> = Vec::new();
    let mut i = 0;
    // All split points are ASCII bytes or the end, so slicing is always on a char boundary.
    while i < b.len() {
        let start = i;
        let kind = match b[i] {
            b'-' if b.get(i + 1) == Some(&b'-') => {
                i = src[i..].find('\n').map_or(b.len(), |p| i + p);
                Tok::Comment
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i = src[i + 2..].find("*/").map_or(b.len(), |p| i + 2 + p + 2);
                Tok::Comment
            }
            b'\'' => {
                i = end_quoted(b, i, b'\'');
                Tok::Str
            }
            q @ (b'"' | b'`') => {
                i = end_quoted(b, i, q);
                Tok::Ident
            }
            b'[' => {
                i = src[i..].find(']').map_or(b.len(), |p| i + p + 1);
                Tok::Ident
            }
            b'0'..=b'9' => {
                while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'.') {
                    i += 1;
                }
                Tok::Num
            }
            c if is_word_byte(c) => {
                while i < b.len() && is_word_byte(b[i]) {
                    i += 1;
                }
                let word = &src[start..i];
                if word.eq_ignore_ascii_case("x") && b.get(i) == Some(&b'\'') {
                    i = end_quoted(b, i, b'\'');
                    Tok::Str
                } else if KEYWORDS.iter().any(|k| k.eq_ignore_ascii_case(word)) {
                    Tok::Keyword
                } else {
                    Tok::Ident
                }
            }
            _ => {
                i += 1;
                Tok::Other
            }
        };
        match out.last_mut() {
            // Merge runs of punctuation/whitespace into one token.
            Some((Tok::Other, prev)) if kind == Tok::Other => *prev = &src[start - prev.len()..i],
            _ => out.push((kind, &src[start..i])),
        }
    }
    out
}

/// True if any keyword token in `src` is one of `words` (case-insensitive).
/// Keywords inside strings, comments and quoted identifiers don't count.
pub fn has_keyword(src: &str, words: &[&str]) -> bool {
    tokenize(src).iter().any(|(t, s)| *t == Tok::Keyword && words.iter().any(|w| w.eq_ignore_ascii_case(s)))
}

/// Byte range of the statement containing `offset`, splitting on `;` outside
/// strings and comments. A caret just after the last `;` belongs to the
/// statement before it.
pub fn statement_at(src: &str, offset: usize) -> std::ops::Range<usize> {
    let mut start = 0;
    let mut pos = 0;
    for (t, s) in tokenize(src) {
        if t == Tok::Other {
            for (i, ch) in s.char_indices() {
                if ch == ';' {
                    let semi = pos + i;
                    if offset <= semi {
                        return start..semi;
                    }
                    start = semi + 1;
                }
            }
        }
        pos += s.len();
    }
    if start > 0 && src[start..].trim().is_empty() {
        return statement_at(src, start - 1);
    }
    start..src.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_classifies() {
        let src = "SELECT \"a\"\"b\", 'it''s', x'0F', 1.5e3 -- hi\nFROM [t] /* c */ WHERE név = $p";
        let toks = tokenize(src);
        assert_eq!(toks.iter().map(|(_, s)| *s).collect::<String>(), src);
        let find = |s: &str| toks.iter().find(|(_, t)| *t == s).map(|(k, _)| *k);
        assert_eq!(find("SELECT"), Some(Tok::Keyword));
        assert_eq!(find("\"a\"\"b\""), Some(Tok::Ident));
        assert_eq!(find("'it''s'"), Some(Tok::Str));
        assert_eq!(find("x'0F'"), Some(Tok::Str));
        assert_eq!(find("1.5e3"), Some(Tok::Num));
        assert_eq!(find("-- hi"), Some(Tok::Comment));
        assert_eq!(find("[t]"), Some(Tok::Ident));
        assert_eq!(find("/* c */"), Some(Tok::Comment));
        assert_eq!(find("név"), Some(Tok::Ident));
        assert_eq!(find("where"), None);
        assert_eq!(find("WHERE"), Some(Tok::Keyword));
    }

    #[test]
    fn keyword_check_ignores_strings_and_comments() {
        let words = ["DELETE", "DROP"];
        assert!(!has_keyword("SELECT 'drop' -- delete\nFROM t", &words));
        assert!(has_keyword("select 1; delete from t", &words));
    }

    #[test]
    fn statement_at_caret() {
        let src = "select 1; select ';' as x;\n\nselect 3";
        let at = |o: usize| src[statement_at(src, o)].trim().to_string();
        assert_eq!(at(3), "select 1");
        assert_eq!(at(12), "select ';' as x");
        assert_eq!(at(src.len()), "select 3");
        let trailing = "select 1;\nselect 2;  ";
        assert_eq!(trailing[statement_at(trailing, trailing.len())].trim(), "select 2");
    }

    #[test]
    fn unterminated_runs_to_end() {
        let toks = tokenize("select 'abc");
        assert_eq!(toks.last(), Some(&(Tok::Str, "'abc")));
        assert_eq!(tokenize("/* x").last(), Some(&(Tok::Comment, "/* x")));
    }
}
