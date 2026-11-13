//! DatabaseInfo — the "Database" tab: file facts (size, free space, pages,
//! encoding, journal mode, versions…), foreign-key enforcement, and one-click
//! maintenance (integrity / quick / foreign-key check, optimize, VACUUM).
//!
//! Author: Keval Thumar

use leptos::prelude::*;
use leptos::task::spawn_local;
use shared::format::count;
use shared::Cell;

use super::database_info_view::database_info_view;
use crate::database::database_constant::{SCHEMA_GROUPS, SQL_CIPHER_VERSION, SQL_DATABASE_INFO};
use crate::database::database_structure::{DatabaseService, SchemaObject};
use crate::shared::services::toast_service::ToastService;

fn human_bytes(n: u64) -> String {
    let mut value = n as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < 3 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{n} bytes") } else { format!("{value:.1} {}", ["", "KB", "MB", "GB"][unit]) }
}

// Keval Thumar
#[component]
pub fn DatabaseInfo(
    in_txn: RwSignal<bool>,
    #[prop(into)] data_version: Signal<u32>,
    #[prop(into)] file_path: Signal<String>,
    #[prop(into)] encrypted: Signal<bool>,
    #[prop(into)] schema: Signal<Vec<SchemaObject>>,
    on_changed: Callback<bool>,
) -> impl IntoView {
    let toast = expect_context::<ToastService>();
    let facts = RwSignal::new(Vec::<(&'static str, String)>::new());
    // None until read: the build turns enforcement on by default, so always ask the database.
    let fk_on = RwSignal::new(None::<bool>);
    let running = RwSignal::new(None::<&'static str>);
    // (ok, heading, detail lines) of the last action.
    let report = RwSignal::new(None::<(bool, String, Vec<String>)>);

    let refresh = move || {
        spawn_local(async move {
            let Some(rs) = DatabaseService::query(SQL_DATABASE_INFO, &[], toast).await else { return };
            let cipher = DatabaseService::query(SQL_CIPHER_VERSION, &[], toast).await;
            let Some(row) = rs.rows.into_iter().next() else { return };
            let v = |i: usize| row.get(i).map(Cell::display).unwrap_or_default();
            let n = |i: usize| v(i).parse::<u64>().unwrap_or(0);
            let (page, pages, free) = (n(0), n(1), n(2));
            let objects = schema.with_untracked(|s| {
                SCHEMA_GROUPS
                    .iter()
                    .map(|(kind, label)| {
                        // Singular from the type ("index"), plural from the label ("Indexes").
                        let total = s.iter().filter(|o| o.kind == *kind).count();
                        format!("{total} {}", if total == 1 { kind.to_string() } else { label.to_lowercase() })
                    })
                    .collect::<Vec<_>>()
                    .join(" · ")
            });
            let cipher = cipher.and_then(|c| c.rows.first().and_then(|r| r.first()).map(Cell::display)).unwrap_or_default();
            facts.set(vec![
                ("File", file_path.get_untracked()),
                ("Size", human_bytes(page * pages)),
                ("Free space", format!("{} in {}", human_bytes(page * free), count(free, "free page"))),
                ("Page size", format!("{page} bytes · {}", count(pages, "page"))),
                ("Encoding", v(3)),
                ("Journal mode", v(4)),
                ("Auto-vacuum", ["None", "Full", "Incremental"].get(n(5) as usize).unwrap_or(&"?").to_string()),
                ("User version", v(6)),
                ("Application ID", v(7)),
                ("Objects", objects),
                ("SQLite", v(9)),
                ("Encryption", if encrypted.get_untracked() { format!("SQLCipher 4 ({cipher})") } else { "None".into() }),
            ]);
            fk_on.set(Some(v(8) == "1"));
        });
    };
    // On open and after every change (writes, revert, imports…).
    Effect::new(move |_| {
        data_version.track();
        refresh();
    });

    // (label, SQL, needs no pending changes) — checks report rows as issues.
    let on_action = Callback::new(move |(label, sql, needs_clean): (&'static str, &'static str, bool)| {
        if running.get_untracked().is_some() {
            return;
        }
        if needs_clean && in_txn.get_untracked() {
            return toast.error("Write or revert your changes first.");
        }
        running.set(Some(label));
        report.set(None);
        spawn_local(async move {
            let out = DatabaseService::exec_script(sql, toast).await;
            running.set(None);
            let Some(out) = out else { return };
            let lines: Vec<String> = out
                .result
                .map(|r| r.rows.iter().map(|row| row.iter().map(Cell::display).collect::<Vec<_>>().join(" · ")).collect())
                .unwrap_or_default();
            let check = !needs_clean;
            let ok = out.error.is_none() && (!check || lines.is_empty() || lines == ["ok"]);
            let heading = match &out.error {
                Some(e) => format!("{label} failed: {e}"),
                None if !check => format!("{label}: done in {} ms", out.millis),
                None if ok => format!("{label}: no problems found ({} ms)", out.millis),
                None => format!("{label}: {} problem{} ({} ms)", lines.len(), if lines.len() == 1 { "" } else { "s" }, out.millis),
            };
            let lines = if ok && check { vec![] } else { lines };
            report.set(Some((ok, heading, lines)));
            if sql == "VACUUM" {
                // Rowids may be renumbered: reload every grid (this also refreshes the facts).
                on_changed.run(out.in_txn);
            } else {
                in_txn.set(out.in_txn);
                refresh();
            }
        });
    });
    let on_stop = Callback::new(move |_: ()| spawn_local(DatabaseService::cancel()));
    // Per connection: resets to the default (on) when the database is reopened.
    let on_toggle_fk = Callback::new(move |_: ()| {
        if in_txn.get_untracked() {
            return toast.error("Write or revert your changes first: SQLite ignores this inside a transaction.");
        }
        let sql = if fk_on.get_untracked().unwrap_or(true) { "PRAGMA foreign_keys = OFF" } else { "PRAGMA foreign_keys = ON" };
        spawn_local(async move {
            if let Some(out) = DatabaseService::exec_script(sql, toast).await {
                if let Some(e) = out.error {
                    toast.error(&e);
                }
                refresh();
            }
        });
    });

    database_info_view(
        facts.into(),
        fk_on.into(),
        in_txn.into(),
        running.into(),
        report.into(),
        on_action,
        on_stop,
        on_toggle_fk,
    )
}

#[cfg(test)]
mod tests {
    use super::human_bytes;

    #[test]
    fn sizes() {
        assert_eq!(human_bytes(512), "512 bytes");
        assert_eq!(human_bytes(4096 * 5), "20.0 KB");
        assert_eq!(human_bytes(3 * 1024 * 1024 * 1024), "3.0 GB");
    }
}
