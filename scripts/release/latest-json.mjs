// Writes the updater manifest (latest.json) for one release, from the files in a folder.
//
//   node scripts/release/latest-json.mjs <dir> <version> <tag> <notes.md> > latest.json
//
// Each platform entry points at the release asset and carries the contents of its .sig
// (made by `tauri signer sign`). Tauri checks every entry before comparing versions, so a
// missing file or signature stops the release instead of shipping a manifest that breaks
// updates. deb and rpm get their own keys: without them a deb install would fall back to
// the AppImage entry (Querybook only tells deb/rpm users about an update; it doesn't install it).
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const [dir, version, tag, notesFile] = process.argv.slice(2);
if (!dir || !version || !tag) {
  console.error('usage: latest-json.mjs <dir> <version> <tag> [notes.md]');
  process.exit(2);
}
const repo = process.env.GITHUB_REPOSITORY ?? 'KevalKThumar/Querybook';
const assets = {
  'windows-x86_64': `Querybook_${version}_x64-setup.exe`,
  'windows-x86_64-nsis': `Querybook_${version}_x64-setup.exe`,
  'darwin-aarch64': `Querybook_${version}_aarch64.app.tar.gz`,
  'darwin-aarch64-app': `Querybook_${version}_aarch64.app.tar.gz`,
  'linux-x86_64': `Querybook_${version}_amd64.AppImage`,
  'linux-x86_64-appimage': `Querybook_${version}_amd64.AppImage`,
  'linux-x86_64-deb': `Querybook_${version}_amd64.deb`,
  'linux-x86_64-rpm': `Querybook-${version}-1.x86_64.rpm`,
};

const platforms = {};
const missing = [];
for (const [key, file] of Object.entries(assets)) {
  const sig = join(dir, `${file}.sig`);
  if (!existsSync(join(dir, file)) || !existsSync(sig)) {
    missing.push(file);
    continue;
  }
  platforms[key] = {
    url: `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(file)}`,
    signature: readFileSync(sig, 'utf8').trim(),
  };
}
if (missing.length) {
  console.error(`latest.json: missing file or signature for ${[...new Set(missing)].join(', ')}`);
  process.exit(1);
}

const notes = notesFile && existsSync(notesFile) ? readFileSync(notesFile, 'utf8').trim() : '';
const manifest = {
  version,
  notes: notes || `Querybook ${version}`,
  pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, 'Z'),
  platforms,
};
process.stdout.write(`${JSON.stringify(manifest, null, 2)}\n`);
