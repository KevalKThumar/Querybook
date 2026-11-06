//! Text the grid puts on the clipboard: TSV (pastes into Excel / Sheets / the
//! grid itself) and INSERT statements ("Copy as SQL").

use crate::sql::{quote_ident, quote_str};
use crate::Cell;

/// One TSV field; quoted (with doubled quotes) when it contains a tab, newline or quote.
fn tsv_field(text: &str) -> String {
    if text.contains(['\t', '\n', '\r', '"']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

fn tsv_cell(cell: &Cell) -> String {
    match cell {
        Cell::Null => String::new(),
        other => tsv_field(&other.display()),
    }
}

/// Rows as TSV, optionally preceded by a header line. NULL becomes an empty field.
pub fn to_tsv(headers: Option<&[String]>, rows: &[Vec<Cell>]) -> String {
    let mut lines: Vec<String> = Vec::with_capacity(rows.len() + 1);
    if let Some(h) = headers {
        lines.push(h.iter().map(|x| tsv_field(x)).collect::<Vec<_>>().join("\t"));
    }
    lines.extend(rows.iter().map(|r| r.iter().map(tsv_cell).collect::<Vec<_>>().join("\t")));
    lines.join("\n")
}

/// SQL literal for a value as the UI holds it.
// ponytail: blob bytes never reach the UI (Cell::Blob is a length), so blobs copy as NULL
// with a comment; add a backend `quote()` query if exact blob copies are needed.
pub fn cell_literal(cell: &Cell) -> String {
    match cell {
        Cell::Null => "NULL".into(),
        Cell::Int(s) => s.clone(),
        Cell::Real(s) => match s.as_str() {
            "inf" => "9e999".into(),
            "-inf" => "-9e999".into(),
            _ => s.clone(),
        },
        Cell::Text(s) => quote_str(s),
        Cell::Blob(n) => format!("NULL /* blob, {n} bytes */"),
    }
}

/// One `INSERT INTO table (cols) VALUES (...);` per row.
pub fn insert_sql(table: &str, columns: &[String], rows: &[Vec<Cell>]) -> String {
    let cols: Vec<String> = columns.iter().map(|c| quote_ident(c)).collect();
    let cols = cols.join(", ");
    rows.iter()
        .map(|r| {
            let values: Vec<String> = r.iter().map(cell_literal).collect();
            format!("INSERT INTO {} ({cols}) VALUES ({});", quote_ident(table), values.join(", "))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> Vec<Cell> {
        vec![Cell::Int("1".into()), Cell::Text("it's\ta \"b\"".into()), Cell::Null, Cell::Real("inf".into())]
    }

    #[test]
    fn tsv_quotes_only_when_needed() {
        let headers = vec!["id".to_string(), "name".to_string(), "n".to_string(), "r".to_string()];
        assert_eq!(to_tsv(Some(&headers), &[row()]), "id\tname\tn\tr\n1\t\"it's\ta \"\"b\"\"\"\t\tinf");
        assert_eq!(to_tsv(None, &[vec![Cell::Text("x".into())], vec![Cell::Blob(3)]]), "x\nBLOB (3 bytes)");
    }

    #[test]
    fn insert_statements() {
        let cols = vec!["id".to_string(), "na me".to_string(), "n".to_string(), "r".to_string()];
        assert_eq!(
            insert_sql("t", &cols, &[row(), vec![Cell::Int("2".into()), Cell::Blob(4), Cell::Null, Cell::Real("1.5".into())]]),
            "INSERT INTO \"t\" (\"id\", \"na me\", \"n\", \"r\") VALUES (1, 'it''s\ta \"b\"', NULL, 9e999);\n\
             INSERT INTO \"t\" (\"id\", \"na me\", \"n\", \"r\") VALUES (2, NULL /* blob, 4 bytes */, NULL, 1.5);"
        );
    }
}
