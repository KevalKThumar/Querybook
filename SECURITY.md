# Security

## Reporting a vulnerability

Please report security issues **privately**. Use **[Report a vulnerability](https://github.com/KevalKThumar/Querybook/security/advisories/new)**, on the repository's Security tab. Please don't open a public issue.

Include the Querybook version, your macOS version, and steps to reproduce. You'll get
a reply as soon as possible. Fixes ship in a new release and are noted in the changelog.

Only the latest release receives security fixes.

## What Querybook does with your data

- **No network access.** The app makes no network requests. Its content security policy only allows its own resources.
- **Encryption keys** are kept in memory only while a database is open, so the app can reconnect. They are never written to disk, and Querybook can't recover a lost passphrase or key.
- **Encryption itself** is SQLCipher 4 with its default settings (page size 4096, PBKDF2-HMAC-SHA512 with 256,000 iterations, HMAC-SHA512). Changing or removing a key rewrites the database into a temporary file in the same folder. That copy is integrity-checked and then renamed over the original.
- **What is stored locally** is plain text in the app's WebKit storage, under `~/Library/WebKit/com.kevalthumar.querybook`:
  - **recent file paths**
  - **SQL query history** (per database), which can contain values from encrypted databases
  - **saved snippets**
  - **the theme**

  Avoid putting secrets in SQL text if that matters to you.
