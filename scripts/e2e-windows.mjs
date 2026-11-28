// End-to-end test of the Windows app through UI Automation, with agent-desktop
// (npm install -g agent-desktop). Drives the real UI on throwaway databases, plain then
// SQLCipher-encrypted: Import SQL (a script's BEGIN … COMMIT waits for Write Changes;
// ATTACH, re-keying and file-level PRAGMAs, VACUUM and extensions are refused), Ctrl+S,
// Revert, the SQL editor (not held to Import SQL's limits), the close prompt
// (Cancel / Don't Save), and encrypt → reopen → unlock.
//
// It takes the mouse and keyboard for about a minute (WebView text fields only take real
// input), so don't type while it runs. It borrows the clipboard and puts its text back.
// Querybook must not be running: it's single-instance, so a launch would hand the test's
// files to that window.
//
//   node scripts/e2e-windows.mjs                     # target/release/querybook.exe
//   node scripts/e2e-windows.mjs "C:\path\to\querybook.exe"
import { execFileSync, spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { DatabaseSync } from 'node:sqlite';

const APP = resolve(process.argv[2] ?? 'target/release/querybook.exe');
const AD = join(process.env.APPDATA ?? '', 'npm/node_modules/agent-desktop/bin/agent-desktop-win32-x64.exe');
const DIR = mkdtempSync(join(tmpdir(), 'querybook-e2e-'));
const DB = join(DIR, 'shop.db');
const KEY = 'e2e-pass-42';
const slash = (p) => p.replaceAll('\\', '/');
let fails = 0;

// ── helpers ──
const sleep = (ms) => Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms);
/** Run agent-desktop; its JSON envelope ({ ok, data, error }). */
function ad(...args) {
  const r = spawnSync(AD, args, { encoding: 'utf8', timeout: 60_000 });
  try { return JSON.parse(r.stdout); } catch { return { ok: false, error: { message: r.stderr || r.stdout } }; }
}
function check(what, got, want) {
  if (got === want) console.log(`✓ ${what}`);
  else { console.log(`✗ ${what}: got ${JSON.stringify(got)}, expected ${JSON.stringify(want)}`); fails++; }
}
/** What plain SQLite reads from the file on disk (rows, or "ERR <message>"). */
function q(sql, path = DB) {
  try {
    const db = new DatabaseSync(path, { readOnly: true });
    try { return db.prepare(sql).all(); } finally { db.close(); }
  } catch (e) { return `ERR ${e.message}`; }
}
const tables = () => { const r = q("SELECT group_concat(name) AS t FROM sqlite_master WHERE type = 'table'"); return typeof r === 'string' ? r : r[0].t; };
const running = () => execFileSync('tasklist', ['/FI', 'IMAGENAME eq querybook.exe', '/NH'], { encoding: 'utf8' }).includes('querybook.exe');
// The WebView nests its content deep: at the default --max-depth 10 only the toolbar and status
// bar are in reach, and a wait for anything else (toasts, the sidebar) never matches.
const deep = ['--max-depth', '40', '--timeout-ms', '8000'];
const waitText = (text, ms = 8000) => ad('snapshot', '--app', 'querybook', ...deep, '-w', `:${text}`, '--wait-timeout', String(ms)).ok;
const waitGone = (text, ms = 8000) => ad('snapshot', '--app', 'querybook', ...deep, '--wait-for-gone', `:${text}`, '--wait-timeout', String(ms)).ok;
const see = (what, text, ms) => check(what, waitText(text, ms) ? 'on screen' : 'not on screen', 'on screen');
/** First element with this role whose name contains `name`, retried while the UI renders. */
function ref(role, name, ...more) {
  for (let i = 0; i < 25; i++) {
    const r = ad('find', '--app', 'querybook', '--role', role, ...(name ? ['--name', name] : []), ...more, '--first');
    if (r.data?.match?.ref_id) return r.data.match.ref_id;
    sleep(400);
  }
  return null;
}
function click(role, name, ...more) {
  const r = ref(role, name, ...more);
  if (!r) return console.log(`  (no ${role} "${name}")`);
  ad('click', r);
}
const front = () => ad('--headed', 'focus-window', '--app', 'querybook');
// `press --app` needs focus it can verify inside the WebView, which it rarely gets, so bring
// Querybook to the front and press system-wide.
const key = (combo) => { front(); ad('--headed', 'press', combo); };
/** Centres of the elements with this role (and state), in screen pixels, top to bottom. */
function centres(role, state) {
  const out = [];
  const walk = (n) => {
    if (n.role === role && (!state || n.states?.includes(state)) && n.bounds?.width) {
      out.push([Math.round(n.bounds.x + n.bounds.width / 2), Math.round(n.bounds.y + n.bounds.height / 2)]);
    }
    n.children?.forEach(walk);
  };
  const s = ad('snapshot', '--app', 'querybook', '--include-bounds', '--max-depth', '40', '--timeout-ms', '8000');
  if (s.data?.tree) walk(s.data.tree);
  return out;
}
// Text goes in by paste: the password fields are unnamed twins (no ref can tell them apart,
// so they're clicked by position), and pasting is one edit where per-key input drops keys.
function paste(text) { ad('clipboard-set', text); key('ctrl+a'); key('ctrl+v'); }
function pasteAt([x, y], text) { front(); ad('--headed', 'mouse-click', '--xy', `${x},${y}`); paste(text); }
function launch(path, ready = 'button:Import SQL') {
  spawn(APP, [path], { detached: true, stdio: 'ignore' }).unref();
  const ok = ad('snapshot', '--app', 'querybook', '-w', ready, '--wait-timeout', '30000', '--timeout-ms', '20000').ok;
  if (!ok) { console.log(`✗ Querybook didn't open ${path}; stopping.`); process.exit(1); }
}
/** Close the window; answer the close prompt with `answer` if it comes up. */
function closeApp(answer = "Don't Save") {
  click('button', 'Close', '--exact');
  for (let i = 0; i < 10 && running(); i++) {
    const prompt = ad('list-windows', '--app', 'querybook').data?.find((w) => w.title === 'Unsaved changes');
    if (prompt) {
      const b = ad('find', '--window-id', prompt.id, '--role', 'button', '--name', answer, '--exact', '--first').data?.match?.ref_id;
      if (b) ad('click', b);
    }
    sleep(500);
  }
}
/** Toolbar › Import SQL, then pick `file` from DIR in the Open dialog. */
function importSql(file) {
  click('button', 'Import SQL');
  let dlg;
  for (let i = 0; i < 30 && !dlg; i++) {
    dlg = ad('list-windows', '--app', 'querybook').data?.find((w) => w.title === 'Open');
    if (!dlg) sleep(300);
  }
  if (!dlg) return console.log('  (the Open dialog never came up)');
  // The shell's file view makes this tree large; give the read time.
  const name = ad('find', '--window-id', dlg.id, '--role', 'textfield', '--name', 'File name:', '--exact', '--first', '--timeout-ms', '15000').data?.match?.ref_id;
  ad('set-value', name, join(DIR, file));
  ad('--headed', 'press', 'return');   // the dialog is in front
  for (let i = 0; i < 20 && ad('list-windows', '--app', 'querybook').data?.some((w) => w.title === 'Open'); i++) sleep(300);
}

// ── preflight ──
if (!existsSync(AD)) { console.log(`No agent-desktop at ${AD}. Install it: npm install -g agent-desktop`); process.exit(1); }
if (!existsSync(APP)) { console.log(`No app at ${APP}. Build it first: cargo tauri build --no-bundle`); process.exit(1); }
if (running()) { console.log('Close Querybook first (it is single-instance); the test closes its own window when done.'); process.exit(1); }
const clipboard = ad('clipboard-get').data?.text;
process.on('exit', () => { if (clipboard) ad('clipboard-set', clipboard); else ad('clipboard-clear'); });

// ── fixtures ──
const db = new DatabaseSync(DB);
db.exec(`CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT UNIQUE, notes TEXT);
INSERT INTO customers VALUES (1,'Ada','ada@example.com',NULL),(2,'Grace','grace@example.com','vip'),(3,'Linus','linus@example.com',NULL);`);
db.close();
const scripts = {
  // An exported script: its own BEGIN … COMMIT joins the pending changes.
  'good.sql': "BEGIN TRANSACTION;\nPRAGMA foreign_keys = ON;\nCREATE TABLE imported (id INTEGER PRIMARY KEY, v TEXT);\nINSERT INTO imported VALUES (1, 'one'), (2, 'two');\nCOMMIT;\n",
  'attach.sql': `INSERT INTO customers (name) VALUES ('Mallory');\nATTACH DATABASE '${slash(join(DIR, 'evil.db'))}' AS evil;\n`,
  'rekey.sql': "PRAGMA rekey = 'stolen';\n",
  'journal.sql': 'PRAGMA journal_mode = WAL;\n',
  'vacuum.sql': `VACUUM INTO '${slash(join(DIR, 'copy.db'))}';\n`,
  'extension.sql': "SELECT load_extension('evil');\n",
  'pending.sql': "INSERT INTO customers (name) VALUES ('Zed');\n",
  'good2.sql': 'CREATE TABLE secret (id INTEGER PRIMARY KEY);\nINSERT INTO secret VALUES (42);\n',
};
for (const [f, s] of Object.entries(scripts)) writeFileSync(join(DIR, f), s);
console.log(`Test files in ${DIR}\n`);

// ── Plain database ──
launch(DB);
check('opens the database', ad('list-windows', '--app', 'querybook').data?.[0]?.title, 'shop.db');

importSql('good.sql');
see("Import SQL runs an exported script", 'Ran 5 statements.');
see('…and its changes wait for Write Changes', 'Unsaved changes');
check("…so the script's COMMIT didn't write the file", tables(), 'customers');
key('ctrl+s');
waitGone('Unsaved changes');
check('Ctrl+S writes the import', JSON.stringify(q('SELECT v FROM imported ORDER BY id')), '[{"v":"one"},{"v":"two"}]');

importSql('attach.sql');
see('ATTACH is refused, naming the statement', "Statement 2: Import SQL doesn't run ATTACH.");
check('…and reached no other file', existsSync(join(DIR, 'evil.db')), false);
see('…the statement before it waits as a pending change', 'Unsaved changes');
click('button', 'Revert');
waitGone('Unsaved changes');
check('Revert drops it', JSON.stringify(q("SELECT count(*) AS n FROM customers WHERE name = 'Mallory'")), '[{"n":0}]');

importSql('rekey.sql');
see('PRAGMA rekey is refused', "Import SQL doesn't run PRAGMA rekey.");
check('…and the file is still plain SQLite', tables(), 'customers,imported');
importSql('journal.sql');
see('PRAGMA journal_mode = WAL is refused', "Import SQL doesn't run PRAGMA journal_mode.");
check('…and the file keeps its journal mode', JSON.stringify(q('PRAGMA journal_mode')), '[{"journal_mode":"delete"}]');
importSql('vacuum.sql');
see('VACUUM INTO is refused', "Import SQL doesn't run VACUUM statements.");
check('…and wrote no copy', existsSync(join(DIR, 'copy.db')), false);
importSql('extension.sql');
see('load_extension() is refused', "Import SQL doesn't run load_extension().");

// The SQL editor runs what the user types.
key('ctrl+t');
const editor = ref('textfield', 'Write SQL here');
ad('--headed', 'focus', editor);
paste('PRAGMA journal_mode');
click('button', 'Run all');
see("the SQL editor isn't held to Import SQL's limits", '1 statement · 0 rows changed · 1 row returned');

// The close prompt.
importSql('pending.sql');
waitText('Unsaved changes');
closeApp('Cancel');
check('Cancel in the close prompt keeps the window open', running(), true);
closeApp("Don't Save");
check("Don't Save closes without writing", `${running()} ${JSON.stringify(q("SELECT count(*) AS n FROM customers WHERE name = 'Zed'"))}`, 'false [{"n":0}]');

// ── Encrypted database ──
launch(DB);
click('button', 'Encryption');
waitText('Encryption settings');
const fields = centres('textfield', 'secure');
check('encrypting asks for the password twice', fields.length, 2);
for (const f of fields) pasteAt(f, KEY);
click('button', 'Encrypt', '--exact');
waitGone('Encryption settings', 15000);
see('the status bar reads SQLCipher 4', 'SQLCipher 4', 15000);
check('plain SQLite can no longer read the file', tables(), 'ERR file is not a database');

importSql('rekey.sql');
see('PRAGMA rekey is refused on an encrypted database', "Import SQL doesn't run PRAGMA rekey.");
importSql('good2.sql');
see('Import SQL runs on an encrypted database', 'Ran 2 statements.');
key('ctrl+s');
waitGone('Unsaved changes');
closeApp();
check('closes once saved', running(), false);

launch(DB, ':Unlock');
const [unlock] = centres('textfield', 'secure');
if (unlock) pasteAt(unlock, KEY);
key('return');
// The Unlock dialog itself mentions SQLCipher 4, so wait for it to close first.
check('the original key still unlocks it (the import did not re-key it)',
  waitGone('Unlock', 15000) && waitText('SQLCipher 4', 15000) ? 'unlocked' : 'still locked', 'unlocked');
see("…and the encrypted import's table was written", 'secret');
closeApp();

console.log(fails ? `\n${fails} check(s) failed.` : '\nAll checks passed.');
process.exitCode = fails ? 1 : 0;
