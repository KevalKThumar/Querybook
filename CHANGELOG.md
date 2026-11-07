# Changelog

All notable changes to Querybook are listed here. Versions follow [Semantic Versioning](https://semver.org).

## [Unreleased]

### Added
- Windows support: builds with MSVC, an NSIS installer (per-user, no admin rights), file associations for `.db`, `.db3`, `.sqlite`, `.sqlite3` and `.sqlcipher`, and a single running instance, so a file opened from File Explorer goes to the open app.
- Keyboard hints, tooltips and menus show Ctrl / Alt / Shift on Windows.

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

[0.1.0]: https://github.com/KevalKThumar/Querybook/releases/tag/v0.1.0
