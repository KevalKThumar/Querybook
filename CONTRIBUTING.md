# Contributing to Querybook

Thanks for helping! Bug reports, ideas and pull requests are all welcome.

## Getting set up

Follow **Build from source** in the [README](README.md), then:

```sh
cargo tauri dev -- -- path/to/some.db
```

The UI reloads on save. Changes in `src-tauri/` restart the app.

## Project layout

```
Querybook/
├── src/                  # UI (Leptos, compiled to WebAssembly by Trunk)
│   ├── database/         # the feature module
│   │   ├── browser/      #   the database window: toolbar, tabs, status bar
│   │   ├── welcome/      #   the empty-window screen
│   │   ├── components/   #   grid, data_grid, sql_editor, structure_view, cell_inspector, database_info, …
│   │   ├── dialogs/      #   cipher, create_table, import_csv, snippets
│   │   ├── database_service.rs    # the only place that calls the backend
│   │   ├── database_constant.rs   # command names, SQL used by the UI, limits
│   │   └── database_structure.rs  # plain data types
│   └── shared/           # context menu, toasts, dialog shell, services (IPC bridge, clipboard, storage, theme, platform)
├── shared/               # pure Rust shared by UI and backend: wire types, SQL builders,
│                         # lexer, autocomplete, formatter, copy formats, shortcut labels (all unit-tested)
├── src-tauri/            # native backend: windows, file dialogs, SQLite/SQLCipher sessions, import/export
├── styles/               # global.scss (only @use lines), variables, mixins, base
└── assets/               # vendored Bootstrap + icons, the app logo
```

How it fits together:

- **One session per window.** The backend keeps a registry from window label to SQLite connection. Closing a window drops its connection and rolls back anything unwritten.
- **Thin SQL gateway.** The backend exposes `query`, `execute` and `exec_script` with bound parameters. All SQL text is built in the `shared` crate, which quotes identifiers and passes values as `?` parameters.
- **Numbers travel as strings.** JavaScript numbers lose precision above 2^53, so integers and reals cross the IPC boundary as text.
- **Pending changes.** The first edit opens `SAVEPOINT querybook`. **Write** runs `COMMIT` and **Revert** runs `ROLLBACK`.

## Conventions

- **Components use four files**: `<name>_component.rs` holds the signals and logic, `<name>_view.rs` is a pure view function, plus `<name>.scss` and `mod.rs`. Each component's stylesheet is added to `styles/global.scss` with one `@use` line.
- **Async work** goes through `spawn_local`. Services in `database_service.rs` show a toast on error and return plain data.
- **Put logic in `shared/`** whenever it doesn't need the browser or the OS, and give it a test. Small `#[cfg(test)]` modules next to the code are the norm.
- **Use the existing pattern** for the same job (a helper, a dialog, a constant) before adding a new one.
- **Write all code from scratch.** Querybook is an independent implementation. Don't copy code, UI files, icons or strings from DB Browser for SQLite or other tools.
- Commit messages follow `Area: what changed`, for example `Grid: column resize`.

## Before opening a pull request

```sh
cargo test -p shared -p querybook
cargo test -p querybook-ui --lib
cargo clippy -p shared -p querybook
cargo check --target wasm32-unknown-unknown
```

Then try your change in `cargo tauri dev`. For anything touching encryption, also
open an encrypted database and change or remove its key.

## Making a release

1. Bump `version` in `src-tauri/tauri.conf.json` and the three `Cargo.toml` files, and add an entry to [CHANGELOG.md](CHANGELOG.md).
2. Stop `cargo tauri dev`, then run `cargo tauri build --bundles app,dmg`.
3. Zip the app without breaking its signature:
   `ditto -c -k --sequesterRsrc --keepParent target/release/bundle/macos/Querybook.app Querybook.app.zip`
4. Create a GitHub release tagged `vX.Y.Z` and attach the `.dmg` and the `.app.zip`.
