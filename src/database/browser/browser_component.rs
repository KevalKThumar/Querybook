//! Browser — the database window: toolbar, schema sidebar, tabs (browse /
//! structure / SQL), status bar, and the unlock / encryption flow.
//! Route: `/database/browser`. Owns all workspace state; components get it as props.
//!
//! Author: Keval Thumar

use leptos::ev;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_navigate;
use leptos_router::NavigateOptions;
use shared::{ErrorKind, FileKind, KeyInput, SessionInfo};

use super::browser_view::browser_view;
use crate::database::database_constant::ROUTE_WELCOME;
use crate::database::database_structure::{CipherMode, DatabaseService, FkJump, SchemaObject, Tab, TabKind};
use crate::shared::common_structure::ThemeService;
use crate::shared::services::toast_service::ToastService;

// Keval Thumar
/// One database per window. Connects on mount (asking for the SQLCipher key
/// when needed) and coordinates pending changes across all tabs.
#[component]
pub fn Browser() -> impl IntoView {
    let toast = expect_context::<ToastService>();
    let theme = expect_context::<ThemeService>();
    let navigate = use_navigate();

    let info = RwSignal::new(None::<SessionInfo>);
    let schema = RwSignal::new(Vec::<SchemaObject>::new());
    let tabs = RwSignal::new(Vec::<Tab>::new());
    let active_tab = RwSignal::new(None::<u32>);
    let next_tab_id = StoredValue::new(1u32);
    // A savepoint/transaction is open: there are unwritten changes.
    let in_txn = RwSignal::new(false);
    // Bumped after anything that may change data or schema; grids reload on it.
    let data_version = RwSignal::new(0u32);
    let saving = RwSignal::new(false);
    // "Go to referenced row" waiting for its browse tab to apply it.
    let jump = RwSignal::new(None::<FkJump>);

    let cipher_open = RwSignal::new(false);
    let cipher_mode = RwSignal::new(CipherMode::Unlock);
    let cipher_error = RwSignal::new(None::<String>);
    let cipher_busy = RwSignal::new(false);
    let create_table_open = RwSignal::new(false);
    let import_csv_open = RwSignal::new(false);
    let import_csv_path = RwSignal::new(None::<String>);

    let connected = Signal::derive(move || info.with(|i| i.as_ref().is_some_and(|i| i.connected)));
    let encrypted = Signal::derive(move || info.with(|i| i.as_ref().is_some_and(|i| i.encrypted)));
    let locked = Signal::derive(move || {
        info.with(|i| i.as_ref().is_some_and(|i| !i.connected && i.kind != FileKind::Plain))
    });
    let file_name = Signal::derive(move || info.with(|i| i.as_ref().map(|i| i.name.clone()).unwrap_or_default()));
    let file_path = Signal::derive(move || info.with(|i| i.as_ref().and_then(|i| i.path.clone()).unwrap_or_default()));

    // Navigate from an Effect, never synchronously inside the router's own update.
    let go_welcome = RwSignal::new(false);
    Effect::new(move |_| {
        if go_welcome.get() {
            navigate(ROUTE_WELCOME, NavigateOptions::default());
        }
    });

    // ── Tabs ──
    let open_tab = Callback::new(move |kind: TabKind| {
        let existing = tabs.with_untracked(|t| t.iter().find(|tab| tab.kind == kind).map(|tab| tab.id));
        let id = existing.unwrap_or_else(|| {
            let id = next_tab_id.get_value();
            next_tab_id.set_value(id + 1);
            tabs.update(|t| t.push(Tab { id, kind }));
            id
        });
        active_tab.set(Some(id));
    });
    let new_sql_tab = Callback::new(move |_: ()| {
        let n = next_tab_id.get_value();
        open_tab.run(TabKind::Sql(n));
    });
    let select_tab = Callback::new(move |id: u32| active_tab.set(Some(id)));
    let close_tab = Callback::new(move |id: u32| {
        tabs.update(|t| t.retain(|tab| tab.id != id));
        if active_tab.get_untracked() == Some(id) {
            active_tab.set(tabs.with_untracked(|t| t.last().map(|tab| tab.id)));
        }
    });

    // ── Schema + change tracking ──
    let reload_schema = Callback::new(move |_: ()| {
        spawn_local(async move {
            let objects = DatabaseService::load_schema(toast).await;
            // Close tabs whose object was dropped or renamed.
            tabs.update(|t| {
                t.retain(|tab| match &tab.kind {
                    TabKind::Browse(n) | TabKind::Structure(n) => objects.iter().any(|o| &o.name == n),
                    TabKind::Sql(_) => true,
                })
            });
            let active_gone = active_tab
                .get_untracked()
                .is_some_and(|a| tabs.with_untracked(|t| !t.iter().any(|tab| tab.id == a)));
            if active_gone {
                active_tab.set(tabs.with_untracked(|t| t.last().map(|tab| tab.id)));
            }
            schema.set(objects);
            // After the schema, so grids re-read row keys from fresh schema.
            data_version.update(|v| *v += 1);
        });
    });
    let on_changed = Callback::new(move |txn: bool| {
        in_txn.set(txn);
        reload_schema.run(());
    });

    // ── Connect / unlock / encryption ──
    let after_connect = move |i: SessionInfo| {
        if let Some(path) = &i.path {
            DatabaseService::push_recent(path);
        }
        info.set(Some(i));
        cipher_open.set(false);
        cipher_error.set(None);
        reload_schema.run(());
    };
    let connect = Callback::new(move |key: Option<KeyInput>| {
        cipher_busy.set(true);
        spawn_local(async move {
            match DatabaseService::connect(key).await {
                Ok(i) => after_connect(i),
                Err(e) if e.kind == ErrorKind::NeedsKey => {
                    cipher_mode.set(CipherMode::Unlock);
                    cipher_error.set(None);
                    cipher_open.set(true);
                }
                Err(e) if matches!(e.kind, ErrorKind::WrongKey | ErrorKind::Invalid) => {
                    cipher_error.set(Some(e.message));
                    cipher_open.set(true);
                }
                Err(e) => {
                    toast.error(&e.message);
                    cipher_error.set(Some(e.message));
                }
            }
            cipher_busy.set(false);
        });
    });
    let on_unlock = Callback::new(move |_: ()| {
        cipher_mode.set(CipherMode::Unlock);
        cipher_error.set(None);
        cipher_open.set(true);
    });
    let on_encryption = Callback::new(move |_: ()| {
        if in_txn.get_untracked() {
            toast.error("Write or revert your changes first.");
            return;
        }
        cipher_mode.set(CipherMode::Change { encrypted: encrypted.get_untracked() });
        cipher_error.set(None);
        cipher_open.set(true);
    });
    let on_cipher_submit = Callback::new(move |key: Option<KeyInput>| match cipher_mode.get_untracked() {
        CipherMode::Unlock | CipherMode::Create => connect.run(key),
        CipherMode::Change { .. } => {
            let removing = key.is_none();
            cipher_busy.set(true);
            spawn_local(async move {
                match DatabaseService::set_encryption(key).await {
                    Ok(i) => {
                        info.set(Some(i));
                        cipher_open.set(false);
                        toast.success(if removing { "Encryption removed." } else { "Database encrypted with the new key." });
                    }
                    Err(e) => cipher_error.set(Some(e.message)),
                }
                cipher_busy.set(false);
            });
        }
    });
    let on_cipher_cancel = Callback::new(move |_: ()| cipher_open.set(false));

    // ── Write / Revert ──
    let on_write = Callback::new(move |_: ()| {
        saving.set(true);
        spawn_local(async move {
            if DatabaseService::write_changes(toast).await {
                in_txn.set(false);
                toast.success("Changes written.");
            }
            saving.set(false);
        });
    });
    let on_revert = Callback::new(move |_: ()| {
        saving.set(true);
        spawn_local(async move {
            if DatabaseService::revert_changes(toast).await {
                on_changed.run(false);
            }
            saving.set(false);
        });
    });

    // ── Toolbar actions ──
    let on_open_db = Callback::new(move |create: bool| {
        spawn_local(async move {
            DatabaseService::open_dialog(create, toast).await;
        });
    });
    let on_new_table = Callback::new(move |_: ()| create_table_open.set(true));
    let on_import_csv = Callback::new(move |_: ()| {
        spawn_local(async move {
            if let Some(path) = DatabaseService::pick_import_file("csv", toast).await {
                import_csv_path.set(Some(path));
                import_csv_open.set(true);
            }
        });
    });
    let on_import_sql = Callback::new(move |_: ()| {
        spawn_local(async move {
            let Some(path) = DatabaseService::pick_import_file("sql", toast).await else { return };
            if let Some(out) = DatabaseService::import_sql(&path, toast).await {
                match &out.error {
                    Some(e) => toast.error(e),
                    None => toast.success(&format!("Ran {} statements.", out.statements)),
                }
                on_changed.run(out.in_txn);
            }
        });
    });
    let on_export_sql = Callback::new(move |_: ()| {
        spawn_local(async move {
            DatabaseService::export_sql_dump(toast).await;
        });
    });
    let on_theme = Callback::new(move |_: ()| theme.cycle());

    // ⌘O open · ⌘N new · ⌘S write changes · ⌘T new SQL tab
    let shortcuts = window_event_listener(ev::keydown, move |e| {
        if !(e.meta_key() || e.ctrl_key()) {
            return;
        }
        let handled = match e.key().as_str() {
            "o" => {
                on_open_db.run(false);
                true
            }
            "n" => {
                on_open_db.run(true);
                true
            }
            "s" if in_txn.get_untracked() => {
                on_write.run(());
                true
            }
            "t" if connected.get_untracked() => {
                new_sql_tab.run(());
                true
            }
            _ => false,
        };
        if handled {
            e.prevent_default();
        }
    });
    on_cleanup(move || shortcuts.remove());

    // ── Mount: bind to this window's database ──
    spawn_local(async move {
        let i = DatabaseService::session_info(toast).await;
        if i.path.is_none() {
            go_welcome.set(true);
            return;
        }
        if i.connected {
            after_connect(i);
            return;
        }
        let kind = i.kind;
        info.set(Some(i));
        match kind {
            FileKind::Plain => connect.run(None),
            FileKind::Encrypted => on_unlock.run(()),
            FileKind::New => {
                cipher_mode.set(CipherMode::Create);
                cipher_open.set(true);
            }
        }
    });

    browser_view(
        connected,
        encrypted,
        locked,
        file_name,
        file_path,
        schema,
        tabs,
        active_tab.into(),
        in_txn,
        data_version.into(),
        jump,
        saving.into(),
        theme.mode.into(),
        cipher_open,
        cipher_mode.into(),
        cipher_error,
        cipher_busy.into(),
        create_table_open,
        import_csv_open,
        import_csv_path.into(),
        on_open_db,
        open_tab,
        new_sql_tab,
        select_tab,
        close_tab,
        on_changed,
        on_write,
        on_revert,
        on_unlock,
        on_encryption,
        on_new_table,
        on_import_csv,
        on_import_sql,
        on_export_sql,
        on_theme,
        on_cipher_submit,
        on_cipher_cancel,
    )
}
