# Security

## Reporting a vulnerability

Please report security issues **privately**. Use **[Report a vulnerability](https://github.com/KevalKThumar/Querybook/security/advisories/new)**, on the repository's Security tab. Please don't open a public issue.

Include the Querybook version (Help › About Querybook), your operating system and version, and steps to reproduce. You'll get
a reply as soon as possible. Fixes ship in a new release and are noted in the changelog.

Only the latest release receives security fixes.

## What Querybook does with your data

- **No network access, except the update check you allow.** The app's window can't reach the network: its content security policy only allows its own resources. The one request Querybook makes is the update check, from the native side, and only after you say yes to "Check for updates automatically?" or choose Help › Check for Updates…. It downloads `latest.json` from this repository's GitHub releases (GitHub sees your IP address and Querybook's user agent, as with any download) and nothing else is sent. An update is installed only when you click Update Now, and only if its signature matches the public key built into Querybook.
- **Encryption keys** are kept in memory only while a database is open, so the app can reconnect. They are never written to disk, and Querybook can't recover a lost passphrase or key.
- **Encryption itself** is SQLCipher 4 with its default settings (page size 4096, PBKDF2-HMAC-SHA512 with 256,000 iterations, HMAC-SHA512). Changing or removing a key rewrites the database into a temporary file in the same folder. That copy is integrity-checked and then renamed over the original.
- **What is stored locally** is plain text in the app's WebView storage (on macOS under `~/Library/WebKit/com.kevalthumar.querybook`; on Windows and Linux in the WebView's profile in the app data folder):
  - **recent file paths**
  - **SQL query history** (per database), which can contain values from encrypted databases
  - **saved snippets**
  - **the theme**

  Avoid putting secrets in SQL text if that matters to you.
- **Crash reports and the log** stay on your computer, in the logs folder (Help › Open Logs Folder): `%LOCALAPPDATA%\com.kevalthumar.querybook\logs` on Windows, `~/Library/Logs/com.kevalthumar.querybook` on macOS, `~/.local/share/com.kevalthumar.querybook/logs` on Linux. They record what happened (error kinds, the code location, the version and OS), never SQL text, cell values, keys or file paths, and every line is filtered for paths, quoted text and long hex strings before it is written. Nothing is sent: you choose to copy, save or attach a report, and Help › Crash Reports shows the whole report first.
