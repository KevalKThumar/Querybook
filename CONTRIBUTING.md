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
│   │   ├── components/   #   grid, data_grid, sql_editor, structure_view, cell_inspector, database_info, change_history, …
│   │   ├── dialogs/      #   cipher, create_table, import_csv, export_sql, snippets
│   │   ├── database_service.rs    # the only place that calls the backend
│   │   ├── database_constant.rs   # command names, SQL used by the UI, limits
│   │   └── database_structure.rs  # plain data types
│   ├── support/          # Help menu, crash reports, About, Settings, shortcuts, update card and dialog
│   └── shared/           # context menu, toasts, dialog shell, services (IPC bridge, clipboard, storage, theme, platform)
├── shared/               # pure Rust shared by UI and backend: wire types, SQL builders,
│                         # lexer, autocomplete, formatter, copy formats, shortcut labels (all unit-tested)
├── src-tauri/            # native backend: windows, file dialogs, SQLite/SQLCipher sessions, import/export,
│                         # change history (history.rs), crash reports and log (crash.rs, applog.rs),
│                         # blank-window watchdog, Help links (support.rs), updates (updater.rs)
├── packaging/            # winget / Homebrew / AUR / Cloudsmith templates the release workflow fills in
├── scripts/              # E2E tests (e2e-windows.mjs, e2e-macos.sh), release/latest-json.mjs
├── .cargo/config.toml    # SQLite compile flags (the preupdate hook the change history uses)
├── styles/               # global.scss (only @use lines), variables, mixins, base
└── assets/               # vendored Bootstrap + icons, the app logo
```

How it fits together:

- **One session per window.** The backend keeps a registry from window label to SQLite connection. Closing a window drops its connection and rolls back anything unwritten.
- **Thin SQL gateway.** The backend exposes `query`, `execute` and `exec_script` with bound parameters. All SQL text is built in the `shared` crate, which quotes identifiers and passes values as `?` parameters.
- **Numbers travel as strings.** JavaScript numbers lose precision above 2^53, so integers and reals cross the IPC boundary as text.
- **Pending changes.** The first edit opens `SAVEPOINT querybook`. **Write** runs `COMMIT` and **Revert** runs `ROLLBACK`.
- **Change history.** A SQLite preupdate hook remembers how each row looked before the pending changes first touched it. Just before a COMMIT, every remembered row is compared with how it looks now, so rolled-back or undone edits drop out. Each save is stored in a per-database history file in the app data folder, encrypted with the database's key when the database is encrypted.

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

Work happens on branches; `main` only receives finished changes. The [Release workflow](.github/workflows/release.yml) runs only when something lands on `main` (pushing a branch runs nothing, and changes that only touch Markdown files are skipped). It builds the macOS `.dmg` and zipped `.app` (Apple Silicon), the Windows `…_x64-setup.exe`, and the Linux `.AppImage`, `.deb` and `.rpm` (x64) on GitHub's machines.

Every merge into `main` (except one that only touches Markdown) is a release; there is no version to bump by hand.

1. The workflow takes the highest `vX.Y.Z` tag and adds one to the patch number (`v1.1.2` → `1.1.3`), and builds with that version.
   - For a minor or major release, raise `version` in `src-tauri/tauri.conf.json` (and the three `Cargo.toml` files) on your branch. A version there that is higher than the next patch is used as is; later merges continue from it.
   - The repo's version files are not updated by the workflow (`main` only takes merges from its owner), so they can lag behind the releases.
2. Optional: add a `## [X.Y.Z]` section for that version to [CHANGELOG.md](CHANGELOG.md). It becomes the release notes. Without one, GitHub writes the notes from the merged pull requests.
3. Once SignPath is set up, the workflow sends the Windows installer to it. Approve the signing request on [signpath.io](https://app.signpath.io) (the run waits up to 4 hours). Until then the installer is published unsigned.
4. The workflow signs the update files (the Windows installer after SignPath), checks every signature against the public key in `tauri.conf.json`, writes `latest.json` and `SHA256SUMS.txt`, and publishes the release `vX.Y.Z`, which creates the tag. The release is a draft until every file is uploaded, so installed apps never see a half-finished one. If a build or signing fails, nothing is published, and the next merge tries the same version again.
5. Installed copies that allow update checks see the new version within a day. The CHANGELOG section is what they show as "What's new", so write it for users.

To try a build without releasing, open **Actions → Release → Run workflow** with channel **build-only**. The files are attached to the run as artifacts.

### Testing an update before releasing it

Run the workflow by hand with channel **staging** and a version such as `1.2.901`. It publishes a prerelease `staging-1.2.901` whose app reads its updates from the prerelease `updater-staging`, never from real releases. Install it, run the workflow again with `1.2.902`, and the installed staging build offers 1.2.902 through the normal update dialog. Real users never see staging builds.

### The updater key

Updates are signed with a minisign key. The public half is `plugins.updater.pubkey` in `src-tauri/tauri.conf.json`; the private half and its password are the repository secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, with an offline backup kept by the owner. **If the private key or its password is lost, installed copies can't update any more** (a new key means a new version people install by hand). Without the secret the workflow still releases, but without update files.

### Setting up SignPath (once)

Windows signing is free through the [SignPath Foundation](https://signpath.org) open-source program. Until it's set up, the workflow skips signing and publishes the release with an unsigned installer.

1. Apply at [signpath.org](https://signpath.org/apply). The README's [Code signing policy](README.md#code-signing-policy) section is one of their requirements.
2. Once approved, in SignPath: add GitHub as a trusted build system for the project, and give the artifact configuration a zip root containing a PE file `*-setup.exe` (the GitHub artifact is a zip holding the installer).
3. In the GitHub repository settings, under **Secrets and variables → Actions**:
   - secret `SIGNPATH_API_TOKEN`: a SignPath CI user's API token
   - variables `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG` and `SIGNPATH_SIGNING_POLICY_SLUG` (the release-signing policy)

To build a release locally instead:

1. Stop `cargo tauri dev`, then run `cargo tauri build --bundles app,dmg` on a Mac, or `cargo tauri build` on Windows or Linux.
2. On the Mac, zip the app without breaking its signature:
   `ditto -c -k --sequesterRsrc --keepParent target/release/bundle/macos/Querybook.app Querybook.app.zip`
3. Create a GitHub release tagged `vX.Y.Z` and attach the files.
