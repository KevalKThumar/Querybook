# Changelog

All notable changes to Querybook are listed here. Versions follow [Semantic Versioning](https://semver.org).

## [1.1.2] — 2026-11-14

### Added
- Linux builds: an AppImage, a `.deb` and an `.rpm` (x64). Opening a database from the file manager while Querybook runs sends it to the running app, as on Windows.
- Windows releases are downloadable from the Releases page; the installer is code-signed once SignPath signing is in place (see the README's code signing policy).

### Changed
- The Export SQL dialog's button reads **Export**.

### Fixed
- A count of one reads in the singular: "1 row", "1 table", "1 view · 1 index", "1 row returned", "Delete 1 row?", "1 statement · 1 row changed", and likewise in the cell inspector, the Database tab and the import messages.

## [1.1.1] — 2026-11-13

Not released; its changes ship in 1.1.2.

## [1.1.0] — 2026-11-08

### Added
- Change history: a **History** tab lists the last 3 saves made to the database on this device. Each shows the schema changes and the rows inserted, updated and deleted, with old and new values. Every save is kept (until **Clear history**) in Querybook's app data folder, one history per database. An encrypted database's history is encrypted with its key and follows key changes.

### Changed
- Write Changes is its own command, so every save can be recorded.

## [1.0.0] — 2026-11-07

First release for Windows, alongside macOS.

### Added
- Windows support: builds with MSVC, an NSIS installer (per-user, no admin rights), file associations for `.db`, `.db3`, `.sqlite`, `.sqlite3` and `.sqlcipher`, and a single running instance, so a file opened from File Explorer goes to the open app.
- Keyboard hints, tooltips and menus show Ctrl / Alt / Shift on Windows.
- Export SQL dialog: tick the tables and views to export, all of them, or just the table you're browsing (the new **SQL** button on the Browse tab). The script can drop each table and create it again (or keep it with `CREATE … IF NOT EXISTS`), then insert its current rows. Options: schema and data, schema only or data only (optionally deleting the existing rows first); indexes and triggers; column names in INSERT; several rows per INSERT; a wrapping transaction. Exporting from an encrypted database shows a note that the SQL file is plain text.

### Changed
- **Export SQL** opens the export dialog instead of writing a dump of the whole database straight away. Everything is ticked by default, so a full dump is one more click.
- Import SQL no longer writes on its own: a script's `BEGIN … COMMIT` joins the pending changes until you write them.
- Nothing is saved automatically. Statements run in the SQL editor or from a `.sql` script now join the pending changes too, so only Write Changes (⌘S / Ctrl+S) writes them. VACUUM and PRAGMA still run directly.
- Closing a window with unwritten changes now asks **Save**, **Don't Save** or **Cancel** (before, the only choices were Discard and Cancel).

### Fixed
- Windows paths show as `C:\...` (not `\\?\C:\...`) in the title, recent files and the info tab.

## [0.1.0] — 2026-11-06

First public release, for macOS on Apple Silicon.

### Added
- One database per window, with any number open at once. Open, create, recent files, and Finder **Open With** / double-click for `.db`, `.db3`, `.sqlite`, `.sqlite3` and `.sqlcipher`.
- SQLCipher 4: open with a passphrase or raw key; create encrypted databases; encrypt, re-key or decrypt an existing one.
- Data grid:
  - virtual scrolling
  - sort and per-column filters
  - inline editing with Write / Revert changes
  - cell, range and row selection
  - copy as TSV, TSV with headers, or SQL
  - duplicate and delete rows
  - column resize and fit
  - per-row "Adjust rows to contents"
  - foreign-key navigation
- Cell inspector: text, JSON and hex views, with editing.
- SQL editor:
  - highlighting and autocomplete
  - run the current statement, the selection or everything
  - stop a running query
  - EXPLAIN QUERY PLAN
  - Format SQL
  - snippets and per-database history
- Schema: structure view; create table; rename table; add, rename and drop columns; create and drop indexes; drop objects.
- Import CSV / TSV and SQL scripts; export CSV and SQL dumps.
- Database tab: file facts, the foreign-key enforcement switch, and integrity, quick and foreign-key checks, optimize and VACUUM.
- Light, dark and system themes.

[1.1.2]: https://github.com/KevalKThumar/Querybook/releases/tag/v1.1.2
[1.1.0]: https://github.com/KevalKThumar/Querybook/releases/tag/v1.1.0
[1.0.0]: https://github.com/KevalKThumar/Querybook/releases/tag/v1.0.0
[0.1.0]: https://github.com/KevalKThumar/Querybook/releases/tag/v0.1.0
