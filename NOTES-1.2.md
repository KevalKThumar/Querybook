# Querybook 1.2 — branch checklist

> Lives only on `release/1.2` and its feature branches. **Delete this file in the last commit before the PR to `main`**
> (`git diff main...HEAD --name-only` must not list it). Move anything still open into a GitHub issue first.

## Release train
- [ ] **v1.2.0** — hardening, crash reports + Help/Settings/About, in-app updater, release pipeline (staging tested)
- [ ] v1.2.1 — tiny change to prove a real update on Windows / Linux (macOS later)
- [ ] **v1.3.0** (`release/1.3`) — Schema Diagram + Diagram → SQL, Query Visualizer
- [ ] **v1.4.0** (`release/1.4`) — command palette, restore session, preview before save, sample DB + tour, charts, JSON/MD/XLSX export
- Nothing merges to `main` until a batch is ready: every merge to `main` publishes a release.

## Phase status (1.2)
- [x] Version files → 1.2.0, CHANGELOG `## [1.2.0]`
- [x] Merge `dock-recent-database` (Import SQL authorizer, `scripts/e2e-windows.mjs`)
- [x] `SessionInfo.pending` restores the unsaved state after a reload
- [x] Crash reports: panic hook, session lock, rolling log, frontend capture, blank-window watchdog, viewer
- [x] Help button, Settings, About, Shortcuts, issue templates, privacy wording
- [x] Updater (Rust side) + consent / update banners
- [x] release.yml: staging channel, updater artifacts, `.sig` after SignPath, `latest.json`, `SHA256SUMS.txt`, draft-then-publish
- [x] Package-manager jobs (gated by `vars.*_ENABLED`): winget, Homebrew tap, AUR, Cloudsmith
- [x] E2E (Windows) for every item above, plain + encrypted DB

## Next
- [ ] Push `release/1.2`, run the workflow by hand with channel **staging** twice (1.2.901, then 1.2.902) and update one to the other on Windows + Linux (AppImage, deb)
- [ ] Merge `release/1.2` → main = **v1.2.0**; install by hand on each OS; then a tiny change → v1.2.1 to prove the real update
- [ ] E2E result 2026-11-28: Windows 49/49 passed (plain + encrypted)

## Needed from Keval
- [x] Updater key: generated, at `C:\Users\thuma\.tauri\querybook.key` (keep an offline backup of it + password);
      secrets `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` added 2026-11-28; pubkey in tauri.conf.json
- [ ] Support email address
- [ ] Website: private repo `KevalKThumar/querybook-website`, Netlify site (builds off) + `NETLIFY_AUTH_TOKEN` / `NETLIFY_SITE_ID`,
      `WEBSITE_DISPATCH_TOKEN` (fine-grained PAT, that repo only), domain (not querybook.org/.com — Pinterest's Querybook)
- [ ] winget: fork `microsoft/winget-pkgs`, classic PAT `WINGET_TOKEN` (public_repo, workflow), first `wingetcreate new` PR, CLA
- [ ] Homebrew: public repo `KevalKThumar/homebrew-tap`, write deploy key → `HOMEBREW_TAP_DEPLOY_KEY`
- [ ] AUR: account + SSH key → `AUR_SSH_PRIVATE_KEY`, `AUR_USERNAME`, `AUR_EMAIL`
- [ ] Cloudsmith OSS repo `querybook` → `CLOUDSMITH_API_KEY`
- [ ] Optional: Apple Developer ID ($99/yr) → `APPLE_*` secrets (notarization, official homebrew/cask later)

## Deferred macOS checks (do on a Mac later)
- [ ] `scripts/e2e-macos.sh` passes, including the new Diagram / Visualize / Crash / Help sections
- [ ] Updater on macOS: consent, check, Update now → restarts into the new version; privacy prompts after update
- [ ] Homebrew tap: `HOMEBREW_NO_INSTALL_FROM_API=1 brew install --cask kevalkthumar/tap/querybook`, launches after Open Anyway
- [ ] `brew audit --new --cask querybook`, `brew style --fix --cask querybook`, `brew livecheck querybook`
- [ ] In-app update of the brew install, then `brew upgrade` reports nothing to do
- [ ] `brew uninstall --zap --cask querybook` removes the data folders; check exact `brew trust` wording for taps
- [ ] Then set `HOMEBREW_ENABLED=true`

## Decisions
- Crash reports stay on the computer; the user chooses to copy / save / open an issue / email.
- Update check: ask once, then at launch + every 24 h; Update now / Later / Skip this version; deb/rpm notify only.
- Updater runs in Rust only (no webview permissions); install refused while any window has unsaved changes.
- Diagram is generated from the schema; layout edits are saved with Ctrl+S on the Diagram tab, in app data, never in the DB.
- Website story: rewrite the real story in our own words, don't name the other tool.
