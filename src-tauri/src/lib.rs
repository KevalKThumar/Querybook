//! Tauri shell: one SQLite/SQLCipher session per window, a thin SQL gateway,
//! and the file dialogs. All SQL text is built by the UI (`shared::sql`).

mod db;
mod io;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use db::{Registry, Session, invalid, io_err};
use shared::{AppError, Cell, CsvPreview, ExecOutcome, KeyInput, OpenOutcome, ResultSet, ScriptOutcome, SessionInfo};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::{DialogExt, FilePath, MessageDialogButtons, MessageDialogKind};

type Res<T> = Result<T, AppError>;

static NEXT_WINDOW: AtomicUsize = AtomicUsize::new(1);

/// Run blocking SQLite work off the async runtime threads.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Res<T> + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| invalid(e.to_string()))?
}

fn picked_path(p: Option<FilePath>) -> Res<Option<PathBuf>> {
    p.map(|p| p.into_path().map_err(io_err)).transpose()
}

/// Bind `path` to the calling window if it's empty, else focus the window that
/// has it open, else open a new window for it.
fn open_in(app: &AppHandle, caller: &WebviewWindow, path: PathBuf) -> Res<OpenOutcome> {
    let path = fs::canonicalize(&path).unwrap_or(path);
    let reg = app.state::<Registry>();
    if let Some(label) = reg.find_path(&path) {
        if let Some(w) = app.get_webview_window(&label) {
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
        return Ok(if label == caller.label() { OpenOutcome::ThisWindow } else { OpenOutcome::OtherWindow });
    }
    let title = path.file_name().map_or("Querybook".into(), |n| n.to_string_lossy().into_owned());
    if reg.get(caller.label()).is_none_or(|s| s.path.is_none()) {
        reg.insert(caller.label(), Session::new(Some(path)));
        let _ = caller.set_title(&title);
        return Ok(OpenOutcome::ThisWindow);
    }
    let label = format!("db-{}", NEXT_WINDOW.fetch_add(1, Ordering::Relaxed));
    reg.insert(&label, Session::new(Some(path)));
    WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html".into()))
        .title(title)
        .inner_size(1280.0, 820.0)
        .min_inner_size(720.0, 480.0)
        .build()
        .map_err(|e| invalid(e.to_string()))?;
    Ok(OpenOutcome::OtherWindow)
}

#[tauri::command]
async fn session_info(window: WebviewWindow, reg: State<'_, Registry>) -> Res<SessionInfo> {
    Ok(reg.get(window.label()).unwrap_or_else(|| Session::new(None).into()).info())
}

/// Native Open (or New = save dialog) picker, then [`open_in`].
#[tauri::command]
async fn open_dialog(app: AppHandle, window: WebviewWindow, create: bool) -> Res<OpenOutcome> {
    let dialog = app
        .dialog()
        .file()
        .set_parent(&window)
        .add_filter("SQLite database", &["db", "sqlite", "sqlite3", "db3", "sqlcipher"])
        .add_filter("All files", &["*"]);
    let picked = if create { dialog.set_file_name("database.db").blocking_save_file() } else { dialog.blocking_pick_file() };
    let Some(path) = picked_path(picked)? else { return Ok(OpenOutcome::Cancelled) };
    if create && path.exists() {
        // The save dialog already asked the user to confirm replacing it.
        if app.state::<Registry>().find_path(&fs::canonicalize(&path).map_err(io_err)?).is_some() {
            return Err(invalid("That database is open in another window. Close it first."));
        }
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }
    open_in(&app, &window, path)
}

/// Open a recent file by path.
#[tauri::command]
async fn open_path(app: AppHandle, window: WebviewWindow, path: String) -> Res<OpenOutcome> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(invalid("That file no longer exists."));
    }
    open_in(&app, &window, path)
}

#[tauri::command]
async fn connect(window: WebviewWindow, reg: State<'_, Registry>, key: Option<KeyInput>) -> Res<SessionInfo> {
    let s = reg.session(window.label())?;
    blocking(move || s.connect(key)).await
}

#[tauri::command]
async fn query(window: WebviewWindow, reg: State<'_, Registry>, sql: String, params: Vec<Cell>) -> Res<ResultSet> {
    let s = reg.session(window.label())?;
    blocking(move || s.with_conn(|c| db::query(c, &sql, &params))).await
}

#[tauri::command]
async fn execute(window: WebviewWindow, reg: State<'_, Registry>, sql: String, params: Vec<Cell>) -> Res<ExecOutcome> {
    let s = reg.session(window.label())?;
    blocking(move || s.with_conn(|c| db::execute(c, &sql, &params))).await
}

#[tauri::command]
async fn exec_script(window: WebviewWindow, reg: State<'_, Registry>, sql: String) -> Res<ScriptOutcome> {
    let s = reg.session(window.label())?;
    blocking(move || s.with_conn(|c| Ok(db::exec_script(c, &sql)))).await
}

#[tauri::command]
async fn cancel(window: WebviewWindow, reg: State<'_, Registry>) -> Res<()> {
    reg.session(window.label())?.cancel();
    Ok(())
}

/// `key = None` removes encryption.
#[tauri::command]
async fn set_encryption(window: WebviewWindow, reg: State<'_, Registry>, key: Option<KeyInput>) -> Res<SessionInfo> {
    let s = reg.session(window.label())?;
    blocking(move || s.set_encryption(key)).await
}

/// `kind` is "csv" or "sql".
#[tauri::command]
async fn pick_import_file(app: AppHandle, window: WebviewWindow, kind: String) -> Res<Option<String>> {
    let dialog = app.dialog().file().set_parent(&window);
    let dialog = match kind.as_str() {
        "csv" => dialog.add_filter("CSV / TSV", &["csv", "tsv", "txt"]),
        _ => dialog.add_filter("SQL script", &["sql", "txt"]),
    };
    Ok(picked_path(dialog.add_filter("All files", &["*"]).blocking_pick_file())?.map(|p| p.display().to_string()))
}

fn separator(sep: char) -> Res<u8> {
    u8::try_from(sep).ok().filter(u8::is_ascii).ok_or_else(|| invalid("Separator must be one ASCII character."))
}

#[tauri::command]
async fn csv_preview(path: String, sep: char, header: bool) -> Res<CsvPreview> {
    let sep = separator(sep)?;
    blocking(move || io::csv_preview(&PathBuf::from(path), sep, header)).await
}

#[tauri::command]
async fn import_csv(
    window: WebviewWindow,
    reg: State<'_, Registry>,
    path: String,
    table: String,
    sep: char,
    header: bool,
) -> Res<ExecOutcome> {
    let (s, sep) = (reg.session(window.label())?, separator(sep)?);
    blocking(move || s.with_conn(|c| io::import_csv(c, &PathBuf::from(path), table.trim(), sep, header))).await
}

#[tauri::command]
async fn import_sql(window: WebviewWindow, reg: State<'_, Registry>, path: String) -> Res<ScriptOutcome> {
    let s = reg.session(window.label())?;
    let sql = fs::read_to_string(path).map_err(io_err)?;
    blocking(move || s.with_conn(|c| Ok(db::exec_script(c, &sql)))).await
}

fn save_path(app: &AppHandle, window: &WebviewWindow, file_name: &str, filter: (&str, &[&str])) -> Res<Option<PathBuf>> {
    picked_path(app.dialog().file().set_parent(window).set_file_name(file_name).add_filter(filter.0, filter.1).blocking_save_file())
}

/// Export the full result of `sql` (a SELECT) to a CSV file the user picks.
#[tauri::command]
async fn export_csv(
    app: AppHandle,
    window: WebviewWindow,
    reg: State<'_, Registry>,
    sql: String,
    name: String,
) -> Res<Option<String>> {
    let s = reg.session(window.label())?;
    let Some(out) = save_path(&app, &window, &format!("{name}.csv"), ("CSV", &["csv"]))? else { return Ok(None) };
    let shown = out.display().to_string();
    blocking(move || s.with_conn(|c| io::export_csv(c, &sql, &out))).await?;
    Ok(Some(shown))
}

#[tauri::command]
async fn export_sql_dump(app: AppHandle, window: WebviewWindow, reg: State<'_, Registry>) -> Res<Option<String>> {
    let s = reg.session(window.label())?;
    let stem = s.path.as_ref().and_then(|p| p.file_stem()).map_or("dump".into(), |n| n.to_string_lossy().into_owned());
    let Some(out) = save_path(&app, &window, &format!("{stem}.sql"), ("SQL script", &["sql"]))? else { return Ok(None) };
    let shown = out.display().to_string();
    blocking(move || s.with_conn(|c| io::export_sql_dump(c, &out))).await?;
    Ok(Some(shown))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Registry::default())
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                let pending = window.state::<Registry>().get(window.label()).is_some_and(|s| s.has_pending());
                if pending {
                    api.prevent_close();
                    let w = window.clone();
                    window
                        .dialog()
                        .message("This database has changes that were not written. Close the window and discard them?")
                        .title("Unsaved changes")
                        .kind(MessageDialogKind::Warning)
                        .buttons(MessageDialogButtons::OkCancelCustom("Discard".into(), "Cancel".into()))
                        .parent(window)
                        .show(move |discard| {
                            if discard {
                                let _ = w.destroy();
                            }
                        });
                }
            }
            // Dropping the session closes the connection (rolling back anything unwritten).
            WindowEvent::Destroyed => window.state::<Registry>().remove(window.label()),
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            session_info,
            open_dialog,
            open_path,
            connect,
            query,
            execute,
            exec_script,
            cancel,
            set_encryption,
            pick_import_file,
            csv_preview,
            import_csv,
            import_sql,
            export_csv,
            export_sql_dump,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
