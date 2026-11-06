//! Text formatters: SQL (keyword case + clause line breaks), JSON (re-indent
//! without parsing, so numbers keep every digit) and hex dumps.

use crate::lexer::{tokenize, Tok};

/// Keywords that start a new line at query level.
const CLAUSES: &[&str] = &[
    "SELECT", "FROM", "WHERE", "GROUP", "ORDER", "HAVING", "LIMIT", "UNION", "INTERSECT", "EXCEPT", "VALUES", "SET",
    "RETURNING", "WINDOW", "JOIN", "LEFT", "RIGHT", "INNER", "CROSS", "NATURAL", "FULL",
];
/// Words that lead a join, so the JOIN after them stays on their line.
const JOIN_LEAD: &[&str] = &["LEFT", "RIGHT", "INNER", "CROSS", "NATURAL", "FULL", "OUTER"];
/// Clauses whose top-level commas each start a new line.
const LIST_CLAUSES: &[&str] = &["SELECT", "GROUP", "ORDER", "SET"];
/// Keywords often used as column names: keep the case they were typed in.
const NAME_LIKE: &[&str] =
    &["ACTION", "DATABASE", "FIRST", "KEY", "LAST", "NO", "PLAN", "QUERY", "RANGE", "ROW", "ROWS", "TEMP"];
const INDENT: &str = "    ";

/// One parenthesis level. Only query levels (the statement itself, or a
/// parenthesis that starts with SELECT / WITH) get clause line breaks.
struct Frame {
    query: bool,
    fresh: bool,
    clause: &'static str,
    between: bool,
}

impl Frame {
    fn new(query: bool) -> Self {
        Frame { query, fresh: true, clause: "", between: false }
    }
}

struct Out {
    text: String,
    space: bool,
}

impl Out {
    fn line_start(&self) -> bool {
        self.text.trim_end_matches(' ').ends_with('\n') || self.text.is_empty()
    }

    fn push(&mut self, s: &str) {
        if self.space && !self.line_start() {
            self.text.push(' ');
        }
        self.space = false;
        self.text.push_str(s);
    }

    fn newline(&mut self, indent: usize) {
        let kept = self.text.trim_end_matches(' ').len();
        self.text.truncate(kept);
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            self.text.push('\n');
        }
        self.text.push_str(&INDENT.repeat(indent));
        self.space = false;
    }
}

/// Pretty-print SQL: keywords upper-cased, each clause on its own line,
/// select lists and AND / OR conditions one per line. Only whitespace and
/// keyword case change, so the statement means exactly the same thing.
// ponytail: DML-shaped layout; CREATE TABLE column lists stay on one line.
pub fn sql(src: &str) -> String {
    let tokens = tokenize(src);
    let mut out = Out { text: String::with_capacity(src.len() + src.len() / 4), space: false };
    let mut frames = vec![Frame::new(true)];
    let mut statement_start = true;
    // Last keyword and whether the last significant token was a keyword.
    let mut prev_kw = String::new();
    let mut prev_was_kw = false;

    for (i, (tok, s)) in tokens.iter().enumerate() {
        let depth = frames.iter().filter(|f| f.query).count() - 1;
        match tok {
            Tok::Other => {
                for ch in s.chars() {
                    if ch.is_whitespace() {
                        out.space = true;
                        continue;
                    }
                    let frame = frames.last_mut().expect("base frame");
                    match ch {
                        '(' => {
                            out.push("(");
                            frame.fresh = false;
                            frames.push(Frame::new(false));
                        }
                        ')' => {
                            out.space = false;
                            if frames.len() > 1 {
                                frames.pop();
                            }
                            out.push(")");
                        }
                        ',' => {
                            out.space = false;
                            out.push(",");
                            if frame.query && LIST_CLAUSES.contains(&frame.clause) {
                                out.newline(depth + 1);
                            }
                        }
                        ';' => {
                            out.space = false;
                            out.push(";");
                            out.text.push_str("\n\n");
                            frames = vec![Frame::new(true)];
                            statement_start = true;
                            prev_kw.clear();
                        }
                        _ => {
                            let mut buf = [0u8; 4];
                            out.push(ch.encode_utf8(&mut buf));
                        }
                    }
                    if ch != ';' {
                        frames.last_mut().expect("base frame").fresh &= ch == '(';
                        statement_start = false;
                    }
                    prev_was_kw = false;
                }
            }
            Tok::Keyword => {
                let upper = s.to_ascii_uppercase();
                let frame = frames.last_mut().expect("base frame");
                if frame.fresh && (upper == "SELECT" || upper == "WITH") {
                    frame.query = true;
                }
                let depth = frames.iter().filter(|f| f.query).count() - 1;
                let frame = frames.last_mut().expect("base frame");
                let after_paren = frame.fresh;
                frame.fresh = false;

                if frame.query && !statement_start && !after_paren {
                    let breaks = match upper.as_str() {
                        "JOIN" => !JOIN_LEAD.contains(&prev_kw.as_str()),
                        "LEFT" | "RIGHT" | "INNER" | "CROSS" | "FULL" => prev_kw != "NATURAL",
                        "FROM" => prev_kw != "DELETE",
                        "SET" => !prev_was_kw,
                        "VALUES" => prev_kw != "DEFAULT",
                        k => CLAUSES.contains(&k),
                    };
                    if breaks {
                        out.newline(depth);
                    } else if (upper == "AND" || upper == "OR") && matches!(frame.clause, "WHERE" | "HAVING") {
                        if upper == "AND" && frame.between {
                            frame.between = false;
                        } else {
                            out.newline(depth + 1);
                        }
                    }
                }
                if let Some(clause) = CLAUSES.iter().find(|c| **c == upper).filter(|_| !(upper == "SET" && prev_was_kw)) {
                    frame.clause = clause;
                }
                if upper == "BETWEEN" {
                    frame.between = true;
                }

                let qualified = out.text.ends_with('.') || tokens.get(i + 1).is_some_and(|(_, n)| n.starts_with('.'));
                if NAME_LIKE.contains(&upper.as_str()) || qualified {
                    out.push(s);
                } else {
                    out.push(&upper);
                }
                prev_kw = upper;
                prev_was_kw = true;
                statement_start = false;
            }
            Tok::Comment => {
                out.push(s);
                if s.starts_with("--") {
                    out.newline(depth);
                } else {
                    out.space = true;
                }
            }
            _ => {
                frames.last_mut().expect("base frame").fresh = false;
                out.push(s);
                prev_was_kw = false;
                statement_start = false;
            }
        }
    }
    out.text.trim_end().to_string()
}

/// Re-indent JSON with two spaces. Nothing is parsed, so big integers keep
/// every digit. None if the text isn't a JSON object or array.
pub fn json_pretty(src: &str) -> Option<String> {
    let s = src.trim();
    let wrapped = (s.starts_with('{') && s.ends_with('}')) || (s.starts_with('[') && s.ends_with(']'));
    if !wrapped {
        return None;
    }
    let newline = |out: &mut String, depth: usize| {
        out.push('\n');
        out.push_str(&"  ".repeat(depth));
    };
    let mut out = String::with_capacity(s.len() * 2);
    let mut depth = 0usize;
    let (mut in_str, mut escaped) = (false, false);
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_str {
            out.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_str = false;
            }
            continue;
        }
        match ch {
            '"' => {
                in_str = true;
                out.push(ch);
            }
            '{' | '[' => {
                out.push(ch);
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
                let close = if ch == '{' { '}' } else { ']' };
                if chars.peek() == Some(&close) {
                    out.push(close);
                    chars.next();
                } else {
                    depth += 1;
                    newline(&mut out, depth);
                }
            }
            '}' | ']' => {
                depth = depth.checked_sub(1)?;
                newline(&mut out, depth);
                out.push(ch);
            }
            ',' => {
                out.push(',');
                newline(&mut out, depth);
            }
            ':' => out.push_str(": "),
            c if c.is_whitespace() => {}
            c => out.push(c),
        }
    }
    (depth == 0 && !in_str).then_some(out)
}

/// Bytes from SQLite's `hex()` text.
pub fn from_hex(hex: &str) -> Option<Vec<u8>> {
    (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()).collect()
}

/// Classic hex dump: offset, 16 bytes in hex, then the printable ASCII.
pub fn hex_dump(bytes: &[u8]) -> String {
    bytes
        .chunks(16)
        .enumerate()
        .map(|(i, row)| {
            let hex: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
            let ascii: String = row.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' }).collect();
            format!("{:08x}  {:<47}  |{ascii}|", i * 16, hex.join(" "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tokens minus whitespace, keywords upper-cased: what the SQL means.
    fn meaning(src: &str) -> Vec<String> {
        tokenize(src)
            .into_iter()
            .flat_map(|(t, s)| match t {
                Tok::Keyword => vec![s.to_ascii_uppercase()],
                Tok::Other => s.chars().filter(|c| !c.is_whitespace()).map(String::from).collect(),
                _ => vec![s.to_string()],
            })
            .collect()
    }

    #[test]
    fn formats_clauses_lists_and_conditions() {
        let src = "select id, name from customers c left join orders o on o.cust = c.id \
                   where id > 1 and name like 'a%' order by name limit 10";
        assert_eq!(
            sql(src),
            "SELECT id,\n    name\nFROM customers c\nLEFT JOIN orders o ON o.cust = c.id\n\
             WHERE id > 1\n    AND name LIKE 'a%'\nORDER BY name\nLIMIT 10"
        );
    }

    #[test]
    fn keeps_strings_comments_and_name_like_keywords() {
        assert_eq!(sql("select 'from x where' -- and y\nfrom t"), "SELECT 'from x where' -- and y\nFROM t");
        assert_eq!(sql("select key, t.plan from kv t"), "SELECT key,\n    t.plan\nFROM kv t");
    }

    #[test]
    fn statements_subqueries_between_and_windows() {
        assert_eq!(
            sql("delete from t where a between 1 and 2 and b = 3; select 1"),
            "DELETE FROM t\nWHERE a BETWEEN 1 AND 2\n    AND b = 3;\n\nSELECT 1"
        );
        assert_eq!(
            sql("select * from t where id in (select id from u where x = 1)"),
            "SELECT *\nFROM t\nWHERE id IN (SELECT id\n    FROM u\n    WHERE x = 1)"
        );
        assert_eq!(
            sql("select row_number() over (partition by a order by b) from t"),
            "SELECT row_number() OVER (PARTITION BY a ORDER BY b)\nFROM t"
        );
        assert_eq!(
            sql("update t set a = 1, b = 2 where id = 3"),
            "UPDATE t\nSET a = 1,\n    b = 2\nWHERE id = 3"
        );
        assert_eq!(
            sql("insert into t (a) values (1) on conflict (a) do update set a = 2"),
            "INSERT INTO t (a)\nVALUES (1) ON CONFLICT (a) DO UPDATE SET a = 2"
        );
        assert_eq!(sql("insert into t default values"), "INSERT INTO t DEFAULT VALUES");
    }

    #[test]
    fn meaning_is_preserved_and_formatting_is_stable() {
        for src in [
            "select a,b from t where x=-1 and y in (1,2) group by a having count(*)>1 order by a desc, b",
            "with r(n) as (select 1 union all select n+1 from r where n<5) select * from r",
            "select [a b], \"c\"\"d\", x'0F' from t natural left join u /* hi */ limit 1 offset 2;",
        ] {
            let once = sql(src);
            assert_eq!(meaning(&once), meaning(src), "{once}");
            assert_eq!(sql(&once), once);
        }
    }

    #[test]
    fn json_reindents_without_touching_numbers() {
        assert_eq!(
            json_pretty(r#" {"a":[1,2],"b":{},"big":12345678901234567890,"s":"x,\"y\":{"} "#).unwrap(),
            "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": {},\n  \"big\": 12345678901234567890,\n  \"s\": \"x,\\\"y\\\":{\"\n}"
        );
        assert_eq!(json_pretty("plain text"), None);
        assert_eq!(json_pretty("{\"a\": 1}}"), None);
    }

    #[test]
    fn hex_round_trip_and_dump() {
        let bytes = from_hex("48656C6C6F00FF").unwrap();
        assert_eq!(bytes, b"Hello\x00\xff");
        assert_eq!(hex_dump(&bytes), format!("00000000  {:<47}  |Hello..|", "48 65 6c 6c 6f 00 ff"));
        assert_eq!(from_hex("4"), None);
    }
}
