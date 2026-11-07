//! Tauri shell: one SQLite/SQLCipher session per window, a thin SQL gateway,
//! and the file dialogs. All SQL text is built by the UI (`shared::sql`).

mod db;
mod history;
mod io;

use std::fs;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use db::{Registry, Session, invalid, io_err};
use shared::history::HistoryPage;
use shared::{
    AppError, Cell, CsvPreview, ExecOutcome, KeyInput, OpenOutcome, ResultSet, ScriptOutcome, SessionInfo,
    SqlExportOptions,
};
use tauri::{AppHandle, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::{DialogExt, FilePath, MessageDialogButtons, MessageDialogKind, MessageDialogResult};

type Res<T> = Result<T, AppError>;

static NEXT_WINDOW: AtomicUsize = AtomicUsize::new(1);

/// Run blocking SQLite work off the async runtime threads.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Res<T> + Send + 'static) -> Res<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| invalid(e.to_string()))?
}

/// Window title for a database file.
fn title_for(path: &Path) -> String {
    path.file_name().map_or("Querybook".into(), |n| n.to_string_lossy().into_owned())
}

/// A session for `path` that keeps its change history in the app data folder
/// (`%APPDATA%\com.kevalthumar.querybook\history` on Windows,
/// `~/Library/Application Support/com.kevalthumar.querybook/history` on macOS).
fn new_session(app: &AppHandle, path: PathBuf) -> Session {
    Session::new(Some(path)).with_history(app.path().app_data_dir().ok().map(|d| d.join("history")))
}

fn picked_path(p: Option<FilePath>) -> Res<Option<PathBuf>> {
    p.map(|p| p.into_path().map_err(io_err)).transpose()
}

/// Focus the window that has `path` open, else bind it to `caller` if that
/// window is empty, else open a new window for it.
fn open_in(app: &AppHandle, caller: Option<&WebviewWindow>, path: PathBuf) -> Res<OpenOutcome> {
    let path = dunce::canonicalize(&path).unwrap_or(path);
    let reg = app.state::<Registry>();
    if let Some(label) = reg.find_path(&path) {
        if let Some(w) = app.get_webview_window(&label) {
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
        let here = caller.is_some_and(|c| c.label() == label);
        return Ok(if here { OpenOutcome::ThisWindow } else { OpenOutcome::OtherWindow });
    }
    let title = title_for(&path);
    if let Some(caller) = caller.filter(|c| reg.get(c.label()).is_none_or(|s| s.path.is_none())) {
        reg.insert(caller.label(), new_session(app, path));
        let _ = caller.set_title(&title);
        return Ok(OpenOutcome::ThisWindow);
    }
    let label = format!("db-{}", NEXT_WINDOW.fetch_add(1, Ordering::Relaxed));
    reg.insert(&label, new_session(app, path));
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
        if app.state::<Registry>().find_path(&dunce::canonicalize(&path).map_err(io_err)?).is_some() {
            return Err(invalid("That database is open in another window. Close it first."));
        }
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }
    open_in(&app, Some(&window), path)
}

/// Open a recent file by path.
#[tauri::command]
async fn open_path(app: AppHandle, window: WebviewWindow, path: String) -> Res<OpenOutcome> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(invalid("That file no longer exists."));
    }
    open_in(&app, Some(&window), path)
}

/// Files opened from outside the app while it runs: Finder on macOS (an Apple
/// event, not argv), or on Windows a second launch (double-click in File
/// Explorer, Open With) that the single-instance plugin forwards here.
/// On a macOS cold launch the event arrives before setup has created any
/// window, so the first file is queued for "main".
#[cfg(any(target_os = "macos", windows))]
fn open_external(app: &AppHandle, paths: Vec<PathBuf>) {
    let reg = app.state::<Registry>();
    let main = app.get_webview_window("main");
    for path in paths.into_iter().filter(|p| p.is_file()) {
        let path = dunce::canonicalize(&path).unwrap_or(path);
        if main.is_none() && app.webview_windows().is_empty() && reg.get("main").is_none() {
            reg.insert("main", new_session(app, path));
            continue;
        }
        let already_open = reg.find_path(&path).is_some();
        match open_in(app, main.as_ref(), path) {
            // Bound to the empty "main" window: reload it so it picks up the file,
            // whether it's showing the welcome screen or still loading.
            Ok(OpenOutcome::ThisWindow) if !already_open => {
                if let Some(w) = &main {
                    let _ = w.eval("window.location.replace('/')");
                    let _ = w.set_focus();
                }
            }
            Err(e) => app.dialog().message(e.message).title("Couldn't open the file").kind(MessageDialogKind::Error).show(|_| {}),
            _ => {}
        }
    }
}

/// Windows: Querybook was launched again (a file double-clicked in File
/// Explorer, or the Start menu). Open its files here instead of in a second
/// process, so one database is never open in two apps at once.
#[cfg(windows)]
fn on_second_instance(app: &AppHandle, argv: Vec<String>, cwd: String) {
    let paths: Vec<PathBuf> = argv.iter().skip(1).map(|a| Path::new(&cwd).join(a)).filter(|p| p.is_file()).collect();
    if paths.is_empty() {
        if let Some(w) = app.get_webview_window("main").or_else(|| app.webview_windows().into_values().next()) {
            let _ = w.unminimize();
            let _ = w.set_focus();
        }
        return;
    }
    // Building a window on the event-loop thread deadlocks WebView2; open them from a task.
    let app = app.clone();
    tauri::async_runtime::spawn(async move { open_external(&app, paths) });
}

const SAVE: &str = "Save";
const DONT_SAVE: &str = "Don't Save";

/// Closing a window with unwritten changes: ask Save / Don't Save / Cancel.
/// Nothing is ever saved without the user choosing to.
fn confirm_close(window: &tauri::Window, session: std::sync::Arc<Session>) {
    let name = session.path.as_deref().map_or_else(|| "this database".to_string(), title_for);
    let w = window.clone();
    window
        .dialog()
        .message(format!("Do you want to save the changes you made to {name}?\n\nIf you don't save, your changes will be lost."))
        .title("Unsaved changes")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::YesNoCancelCustom(SAVE.into(), DONT_SAVE.into(), "Cancel".into()))
        .parent(window)
        .show_with_result(move |choice| match choice {
            MessageDialogResult::Custom(c) if c == SAVE => match session.write_pending() {
                Ok(()) => {
                    let _ = w.destroy();
                }
                Err(e) => {
                    w.dialog()
                        .message(format!("{}\n\nThe window stays open and your changes are still pending.", e.message))
                        .title("Couldn't save the changes")
                        .kind(MessageDialogKind::Error)
                        .parent(&w)
                        .show(|_| {});
                }
            },
            MessageDialogResult::Custom(c) if c == DONT_SAVE => {
                let _ = w.destroy();
            }
            _ => {}
        });
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
    blocking(move || s.execute(&sql, &params)).await
}

#[tauri::command]
async fn exec_script(window: WebviewWindow, reg: State<'_, Registry>, sql: String) -> Res<ScriptOutcome> {
    let s = reg.session(window.label())?;
    blocking(move || s.exec_script(&sql)).await
}

/// Write Changes / Ctrl+S: commit the pending changes and record the save.
#[tauri::command]
async fn write_changes(window: WebviewWindow, reg: State<'_, Registry>) -> Res<()> {
    let s = reg.session(window.label())?;
    blocking(move || s.write_pending()).await
}

/// The latest `limit` saves of this window's database, newest first.
#[tauri::command]
async fn change_history(window: WebviewWindow, reg: State<'_, Registry>, limit: u32) -> Res<HistoryPage> {
    let s = reg.session(window.label())?;
    blocking(move || s.change_history(limit)).await
}

#[tauri::command]
async fn clear_change_history(window: WebviewWindow, reg: State<'_, Registry>) -> Res<()> {
    let s = reg.session(window.label())?;
    blocking(move || s.clear_change_history()).await
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
    blocking(move || s.exec_import(&sql)).await
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

/// Export the ticked tables and views as a SQL script the user saves. Reads the
/// window's own connection, so unwritten changes are included; writes nothing
/// to the database.
#[tauri::command]
async fn export_sql(
    app: AppHandle,
    window: WebviewWindow,
    reg: State<'_, Registry>,
    tables: Vec<String>,
    options: SqlExportOptions,
) -> Res<Option<String>> {
    let s = reg.session(window.label())?;
    let stem = match tables.as_slice() {
        [one] => one.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "_"),
        _ => s.path.as_ref().and_then(|p| p.file_stem()).map_or("export".into(), |n| n.to_string_lossy().into_owned()),
    };
    let Some(out) = save_path(&app, &window, &format!("{stem}.sql"), ("SQL script", &["sql"]))? else { return Ok(None) };
    let shown = out.display().to_string();
    blocking(move || {
        let written = s.with_conn(|c| {
            let mut w = BufWriter::new(fs::File::create(&out).map_err(io_err)?);
            io::export_sql(c, &tables, &options, &mut w)
        });
        if written.is_err() {
            // Don't leave half a script behind.
            let _ = fs::remove_file(&out);
        }
        written
    })
    .await?;
    Ok(Some(shown))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    // Must be the first plugin: a second launch exits before anything else starts.
    #[cfg(windows)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(on_second_instance));
    builder
        .plugin(tauri_plugin_dialog::init())
        .manage(Registry::default())
        .setup(|app| {
            let reg = app.state::<Registry>();
            // `querybook path/to.db` opens that file in the first window.
            if let Some(path) = std::env::args_os().nth(1).map(PathBuf::from).filter(|p| p.is_file()) {
                let path = dunce::canonicalize(&path).unwrap_or(path);
                reg.insert("main", new_session(app.handle(), path));
            }
            // Name "main" after its file (from argv, or a Finder open queued before setup).
            if let (Some(w), Some(path)) = (app.get_webview_window("main"), reg.get("main").and_then(|s| s.path.clone())) {
                let _ = w.set_title(&title_for(&path));
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                if let Some(session) = window.state::<Registry>().get(window.label()).filter(|s| s.has_pending()) {
                    api.prevent_close();
                    confirm_close(window, session);
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
            write_changes,
            change_history,
            clear_change_history,
            cancel,
            set_encryption,
            pick_import_file,
            csv_preview,
            import_csv,
            import_sql,
            export_csv,
            export_sql,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = event {
                open_external(app, urls.iter().filter_map(|u| u.to_file_path().ok()).collect());
            }
            #[cfg(not(target_os = "macos"))]
            let _ = (app, event);
        });
}
