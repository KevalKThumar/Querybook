//! SQL autocomplete from a token scan of the statement at the caret.
//!
//! Not a parser, so it tolerates half-typed SQL: it works out what kind of
//! name fits at the caret (table, column, anything), which tables the
//! statement mentions (with aliases), and filters candidates by the typed prefix.

use std::ops::Range;

use crate::lexer::{is_word_byte, statement_at, tokenize, Tok, KEYWORDS};
use crate::sql::quote_ident;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SuggestKind {
    Column,
    Table,
    Function,
    Keyword,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub label: String,
    /// Text that replaces the typed prefix.
    pub insert: String,
    pub kind: SuggestKind,
    /// e.g. the table a column belongs to.
    pub detail: String,
}

/// What may be typed at the caret.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    Table,
    Column,
    Any,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompletionContext {
    /// Byte offset (in the whole source) where the typed prefix starts.
    pub start: usize,
    pub prefix: String,
    /// `alias.` or `table.` right before the prefix.
    pub qualifier: Option<String>,
    pub expect: Expect,
    /// (table, alias) mentioned after FROM / JOIN / INTO / UPDATE in this statement.
    pub tables: Vec<(String, Option<String>)>,
}

const TABLE_KEYWORDS: [&str; 5] = ["FROM", "JOIN", "INTO", "UPDATE", "TABLE"];
const COLUMN_KEYWORDS: [&str; 20] = [
    "SELECT", "WHERE", "ON", "BY", "SET", "HAVING", "AND", "OR", "WHEN", "THEN", "ELSE", "DISTINCT", "CASE", "NOT",
    "IN", "LIKE", "IS", "BETWEEN", "GLOB", "RETURNING",
];
pub const FUNCTIONS: [&str; 34] = [
    "abs", "avg", "char", "coalesce", "count", "date", "datetime", "group_concat", "hex", "ifnull", "iif", "instr",
    "json_array", "json_extract", "json_object", "julianday", "length", "lower", "ltrim", "max", "min", "nullif",
    "printf", "quote", "random", "replace", "round", "rtrim", "strftime", "substr", "sum", "time", "trim", "upper",
];

fn unquote(s: &str) -> String {
    let inner = || s[1..s.len() - 1].to_string();
    match (s.as_bytes().first(), s.as_bytes().last()) {
        (Some(b'"'), Some(b'"')) if s.len() >= 2 => inner().replace("\"\"", "\""),
        (Some(b'`'), Some(b'`')) if s.len() >= 2 => inner().replace("``", "`"),
        (Some(b'['), Some(b']')) if s.len() >= 2 => inner(),
        _ => s.to_string(),
    }
}

fn is_keyword(word: &str) -> bool {
    KEYWORDS.iter().any(|k| k.eq_ignore_ascii_case(word))
}

/// Significant tokens (no whitespace/comments) with their byte offsets; runs of
/// punctuation are split so `,` and `(` stand alone.
fn significant(src: &str) -> Vec<(Tok, &str, usize)> {
    let mut out = Vec::new();
    let mut pos = 0;
    for (t, s) in tokenize(src) {
        match t {
            Tok::Comment => {}
            Tok::Other => {
                for (i, ch) in s.char_indices() {
                    if !ch.is_whitespace() {
                        out.push((Tok::Other, &s[i..i + ch.len_utf8()], pos + i));
                    }
                }
            }
            _ => out.push((t, s, pos)),
        }
        pos += s.len();
    }
    out
}

/// Tables (and aliases) a statement mentions; tokens inside `skip` are ignored.
fn referenced_tables(stmt: &str, skip: Range<usize>) -> Vec<(String, Option<String>)> {
    let toks: Vec<(Tok, &str, usize)> = significant(stmt).into_iter().filter(|(_, _, at)| !skip.contains(at)).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let (t, s, _) = toks[i];
        i += 1;
        if t != Tok::Keyword || !TABLE_KEYWORDS.iter().any(|k| k.eq_ignore_ascii_case(s)) {
            continue;
        }
        // table [AS] [alias] {, table [AS] [alias]}
        while let Some(&(Tok::Ident, name, _)) = toks.get(i) {
            i += 1;
            let mut alias = None;
            if toks.get(i).is_some_and(|(t, s, _)| *t == Tok::Keyword && s.eq_ignore_ascii_case("AS")) {
                i += 1;
            }
            if let Some(&(Tok::Ident, a, _)) = toks.get(i) {
                alias = Some(unquote(a));
                i += 1;
            }
            out.push((unquote(name), alias));
            if toks.get(i).is_some_and(|(_, s, _)| *s == ",") {
                i += 1;
            } else {
                break;
            }
        }
    }
    out
}

/// Inside a string literal, comment or quoted identifier at `local`?
fn inside_literal(stmt: &str, local: usize) -> bool {
    let mut pos = 0;
    for (t, s) in tokenize(stmt) {
        let end = pos + s.len();
        let quoted = t == Tok::Str || (t == Tok::Ident && s.starts_with(['"', '`', '[']));
        if (quoted || t == Tok::Comment) && pos < local && local <= end {
            let closed = match t {
                Tok::Comment => s.starts_with("/*") && s.ends_with("*/") && s.len() >= 4,
                _ => s.len() >= 2 && s.ends_with(['\'', '"', '`', ']']),
            };
            if local < end || !closed {
                return true;
            }
        }
        pos = end;
    }
    false
}

/// What fits at byte offset `caret` of `src`. None inside strings / comments / numbers.
pub fn context(src: &str, caret: usize) -> Option<CompletionContext> {
    if caret > src.len() || !src.is_char_boundary(caret) {
        return None;
    }
    let mut range = statement_at(src, caret);
    if range.end < caret {
        // Caret is in the blank tail after the last `;`: a new statement.
        range = range.end + 1..src.len();
    }
    let stmt = &src[range.clone()];
    let local = caret - range.start;
    if inside_literal(stmt, local) {
        return None;
    }
    let bytes = stmt.as_bytes();
    let mut p = local;
    while p > 0 && is_word_byte(bytes[p - 1]) {
        p -= 1;
    }
    let prefix = &stmt[p..local];
    if prefix.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }

    // `alias.` / `"quoted table".` before the prefix.
    let mut qualifier = None;
    let mut head_end = p;
    if p > 0 && bytes[p - 1] == b'.' {
        let dot = p - 1;
        let mut q = dot;
        if q > 0 && matches!(bytes[q - 1], b'"' | b'`' | b']') {
            let close = bytes[q - 1];
            let open = if close == b']' { b'[' } else { close };
            q -= 1;
            while q > 0 && bytes[q - 1] != open {
                q -= 1;
            }
            q = q.saturating_sub(1);
        } else {
            while q > 0 && is_word_byte(bytes[q - 1]) {
                q -= 1;
            }
        }
        if q < dot {
            qualifier = Some(unquote(&stmt[q..dot]));
            head_end = q;
        }
    }

    let expect = if qualifier.is_some() {
        Expect::Column
    } else {
        let toks = significant(&stmt[..head_end]);
        match toks.iter().rposition(|(t, _, _)| *t == Tok::Keyword) {
            None => Expect::Any,
            Some(k) => {
                let kw = toks[k].1;
                let after = &toks[k + 1..];
                if TABLE_KEYWORDS.iter().any(|t| t.eq_ignore_ascii_case(kw)) {
                    // "FROM ", "FROM a, " → a table; "FROM a " → a keyword (WHERE, JOIN…)
                    if after.is_empty() || after.last().is_some_and(|(_, s, _)| *s == ",") {
                        Expect::Table
                    } else {
                        Expect::Any
                    }
                } else if COLUMN_KEYWORDS.iter().any(|c| c.eq_ignore_ascii_case(kw)) {
                    Expect::Column
                } else {
                    Expect::Any
                }
            }
        }
    };

    Some(CompletionContext {
        start: range.start + p,
        prefix: prefix.to_string(),
        qualifier,
        expect,
        tables: referenced_tables(stmt, p..local.max(p + 1)),
    })
}

fn insert_text(name: &str) -> String {
    let simple = name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if simple && !is_keyword(name) { name.to_string() } else { quote_ident(name) }
}

/// Candidates for `ctx`, best first. `columns_of(table)` returns None while a
/// table's columns aren't loaded yet.
pub fn suggest(
    ctx: &CompletionContext,
    tables: &[String],
    columns_of: &dyn Fn(&str) -> Option<Vec<String>>,
    limit: usize,
) -> Vec<Suggestion> {
    let prefix = ctx.prefix.to_lowercase();
    let wanted = |label: &str| {
        let l = label.to_lowercase();
        l.starts_with(&prefix) && l != prefix
    };
    // alias or table name → the table's real name
    let resolve = |name: &str| -> Option<String> {
        ctx.tables
            .iter()
            .find(|(_, a)| a.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(name)))
            .map(|(t, _)| t.as_str())
            .or(Some(name))
            .and_then(|n| tables.iter().find(|t| t.eq_ignore_ascii_case(n)).cloned())
    };
    let mut out: Vec<Suggestion> = Vec::new();
    let mut push = |label: &str, insert: String, kind: SuggestKind, detail: &str| {
        if wanted(label) && !out.iter().any(|s: &Suggestion| s.label == label && s.kind == kind) {
            out.push(Suggestion { label: label.to_string(), insert, kind, detail: detail.to_string() });
        }
    };

    if let Some(q) = &ctx.qualifier {
        if let Some(table) = resolve(q) {
            for c in columns_of(&table).unwrap_or_default() {
                push(&c, insert_text(&c), SuggestKind::Column, &table);
            }
        }
    } else {
        if ctx.expect != Expect::Table {
            for (t, _) in &ctx.tables {
                if let Some(table) = resolve(t) {
                    for c in columns_of(&table).unwrap_or_default() {
                        push(&c, insert_text(&c), SuggestKind::Column, &table);
                    }
                }
            }
            for f in FUNCTIONS {
                push(f, format!("{f}("), SuggestKind::Function, "function");
            }
        }
        if ctx.expect != Expect::Column || ctx.tables.is_empty() {
            for t in tables {
                push(t, insert_text(t), SuggestKind::Table, "table");
            }
        }
        if ctx.expect != Expect::Table {
            let lower = !ctx.prefix.is_empty() && ctx.prefix.chars().all(|c| !c.is_ascii_uppercase());
            for k in KEYWORDS {
                let text = if lower { k.to_lowercase() } else { k.to_string() };
                push(k, text, SuggestKind::Keyword, "keyword");
            }
        }
    }
    out.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase())));
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables() -> Vec<String> {
        vec!["customers".into(), "orders".into(), "order items".into()]
    }

    fn columns(t: &str) -> Option<Vec<String>> {
        match t {
            "customers" => Some(vec!["id".into(), "name".into(), "balance".into()]),
            "orders" => Some(vec!["id".into(), "customer_id".into(), "total".into()]),
            _ => None,
        }
    }

    fn labels(src: &str) -> Vec<String> {
        let ctx = context(src, src.len()).expect("context");
        suggest(&ctx, &tables(), &columns, 50).into_iter().map(|s| s.label).collect()
    }

    #[test]
    fn tables_after_from_and_join() {
        let ctx = context("SELECT * FROM cu", 16).unwrap();
        assert_eq!((ctx.expect, ctx.prefix.as_str(), ctx.start), (Expect::Table, "cu", 14));
        assert_eq!(labels("SELECT * FROM cu"), vec!["customers"]);
        assert_eq!(labels("select * from customers c join or"), vec!["order items", "orders"]);
    }

    #[test]
    fn columns_by_alias_and_scope() {
        // The FROM clause after the caret still counts.
        let src = "SELECT c.na FROM customers c";
        let ctx = context(src, 11).unwrap();
        assert_eq!(ctx.qualifier.as_deref(), Some("c"));
        assert_eq!(suggest(&ctx, &tables(), &columns, 10)[0].label, "name");
        assert_eq!(labels("select * from customers where ba"), vec!["balance"]);
        let both = labels("SELECT * FROM customers c JOIN orders o ON o.");
        assert_eq!(both, vec!["customer_id", "id", "total"]);
    }

    #[test]
    fn keywords_keep_case_and_names_get_quoted() {
        assert_eq!(labels("sel"), vec!["SELECT"]);
        let ctx = context("sel", 3).unwrap();
        assert_eq!(suggest(&ctx, &tables(), &columns, 5)[0].insert, "select");
        let ctx = context("SELECT * FROM ord", 17).unwrap();
        let s = suggest(&ctx, &tables(), &columns, 5);
        assert_eq!(s[0].insert, "\"order items\"");
    }

    #[test]
    fn nothing_inside_strings_comments_or_numbers() {
        assert!(context("SELECT 'fro", 11).is_none());
        assert!(context("SELECT 1 -- fro", 15).is_none());
        assert!(context("SELECT 12", 9).is_none());
        assert!(context("SELECT 'a' AS x, fr", 19).is_some());
    }

    #[test]
    fn new_statement_after_semicolon() {
        let src = "SELECT 1; SEL";
        let ctx = context(src, src.len()).unwrap();
        assert_eq!((ctx.start, ctx.prefix.as_str()), (10, "SEL"));
    }
}
