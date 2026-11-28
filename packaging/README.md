# Packaging

Templates the release workflow fills in after each release, and how to set each channel up.
Each channel's job in `.github/workflows/release.yml` runs only when its repository variable
is `true`, after the GitHub Release is published, and a failure there never undoes the release.

| Channel | Users install with | Job turns on with | Needs |
|---|---|---|---|
| winget | `winget install KevalKThumar.Querybook` | `WINGET_ENABLED` | fork of `microsoft/winget-pkgs`, secret `WINGET_TOKEN` (classic PAT: `public_repo`, `workflow`), and the first version added by hand |
| Homebrew (own tap) | `brew install --cask kevalkthumar/tap/querybook` | `HOMEBREW_ENABLED` | public repo `KevalKThumar/homebrew-tap`, secret `HOMEBREW_TAP_DEPLOY_KEY` (write deploy key) |
| AUR | `yay -S querybook-bin` | `AUR_ENABLED` | AUR account, secrets `AUR_SSH_PRIVATE_KEY`, `AUR_USERNAME`, `AUR_EMAIL` |
| apt / dnf (Cloudsmith) | see the website's Linux guide | `CLOUDSMITH_ENABLED` | Cloudsmith OSS repo `querybook`, secret `CLOUDSMITH_API_KEY`, variable `CLOUDSMITH_OWNER` |
| Website | — | `WEBSITE_ENABLED` | secret `WEBSITE_DISPATCH_TOKEN` (fine-grained PAT on `querybook-website`, Contents read and write) |

## winget: the first version

winget-releaser can only update a package that already exists, so the first manifest goes in by hand:

```powershell
winget install wingetcreate
wingetcreate new https://github.com/KevalKThumar/Querybook/releases/download/v1.2.0/Querybook_1.2.0_x64-setup.exe
```

Answer: PackageIdentifier `KevalKThumar.Querybook`, Publisher `Keval Thumar`, License `MIT OR Apache-2.0`,
InstallerType `nullsoft`, Scope `user`, silent switch `/S`, file extensions `db db3 sqlite sqlite3 sqlcipher`.
Let it submit the pull request, sign Microsoft's CLA when the bot asks, then wait for review.
After it merges, set `WINGET_ENABLED=true`.

## Homebrew

`homebrew/cask` needs Gatekeeper to pass (a notarized app) and some notability, so Querybook lives in
its own tap for now. Create `KevalKThumar/homebrew-tap` with `Casks/querybook.rb` made from
`homebrew/querybook.rb.in`, add a deploy key with write access, and store its private half as
`HOMEBREW_TAP_DEPLOY_KEY`. Test on a Mac before setting `HOMEBREW_ENABLED=true`:

```sh
brew tap kevalkthumar/tap
HOMEBREW_NO_INSTALL_FROM_API=1 brew install --cask kevalkthumar/tap/querybook
brew audit --new --cask querybook && brew style --cask querybook && brew livecheck querybook
brew uninstall --zap --cask querybook
```

## AUR

Create an account on aur.archlinux.org, add an SSH key, and check that `querybook-bin` is free.
The job writes `PKGBUILD` and `.SRCINFO` from `aur/PKGBUILD.in` and pushes them.

## apt and dnf (Cloudsmith)

Create a free open-source repository named `querybook` on cloudsmith.io (owner: your account),
an API key, and set `CLOUDSMITH_API_KEY` (secret) and `CLOUDSMITH_OWNER` (variable). Each release
pushes the `.deb` and `.rpm`; Cloudsmith signs the repository and shows the install commands.
