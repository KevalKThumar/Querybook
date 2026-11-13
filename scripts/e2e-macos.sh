#!/usr/bin/env bash
# End-to-end test of the macOS app through its accessibility tree, with agent-desktop
# (npm install -g agent-desktop). Drives the real UI on a throwaway database: browse,
# edit + Write Changes, Revert, the SQL editor, History, integrity check, the close
# prompt, and encrypt / unlock / remove encryption.
#
# It takes keyboard focus for about a minute (WebView text fields only take real
# keystrokes), so don't type while it runs. It quits Querybook at the end. The
# throwaway database stays in Recent files, and its change history in the app data.
#
#   scripts/e2e-macos.sh                          # the release bundle in target/
#   scripts/e2e-macos.sh /Applications/Querybook.app
set -uo pipefail

APP=${1:-target/release/bundle/macos/Querybook.app}
[ -d "$APP" ] || { echo "No app at $APP. Build it first: cargo tauri build --bundles app,dmg"; exit 1; }
APP=$(cd "$APP" && pwd)   # open -a needs an absolute path
DB=$(mktemp -d)/shop.db
KEY=e2e-pass-42
FAILS=0

# ── helpers ──
ad() { agent-desktop "$@" 2>/dev/null; }
q() { sqlite3 "$DB" "$1" 2>&1; }
key() { ad press --app Querybook "$1" >/dev/null; }
# Ref of the first element with this role whose text contains $2 (with --exact: whose name is $2).
# Retries for a few seconds: the UI renders asynchronously.
ref() {
  local by=--text r; [ "${3:-}" = --exact ] && by=--name
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    r=$(ad find --app Querybook --role "$1" ${2:+"$by" "$2"} "${@:3}" | jq -r '.data.matches[0].ref_id // empty')
    [ -n "$r" ] && { echo "$r"; return; }
    sleep 0.4
  done
}
click() { local r; r=$(ref "$@"); [ -n "$r" ] && ad click "$r" >/dev/null || echo "  (no $1 \"$2\")"; }
# Sidebar rows are unnamed groups around a text label.
row() {
  ad snapshot --app Querybook --compact | jq -r --arg n "$1" \
    '[.. | objects | select(.role=="group" and .ref_id and any(.children[]?; .role=="statictext" and .name==$n))][0].ref_id' |
    xargs agent-desktop click >/dev/null 2>&1
}
type_into() { ad --headed type "$1" "$2" >/dev/null; }
# The grid's cell editor: headed `type` can't target it, so send one key per character.
type_keys() {
  local t=$1 c k i
  for ((i = 0; i < ${#t}; i++)); do
    c=${t:i:1}
    case $c in " ") k=space ;; [A-Z]) k=shift+$(tr A-Z a-z <<<"$c") ;; *) k=$c ;; esac
    key "$k"
  done
}
wait_text() { ad snapshot --app Querybook -w ":$1" --wait-timeout "${2:-8000}" >/dev/null; }
see() { wait_text "$2" "${3:-8000}" && check "$1" ok ok || check "$1" "not on screen" "\"$2\""; }
check() { # check "what" actual expected
  if [ "$2" == "$3" ]; then echo "✓ $1"; else echo "✗ $1: got '$2', expected '$3'"; FAILS=$((FAILS + 1)); fi
}
open_db() {
  open -a "$APP" "$DB" && wait_text "${1:-customers}" 15000 || { echo "✗ Querybook didn't open the database; stopping."; exit 1; }
}
quit() { ad close-app Querybook >/dev/null; for _ in 1 2 3 4 5 6 7 8 9 10; do pgrep -xq querybook || return 0; sleep 0.5; done; }
# Focus the grid (it precedes its first filter field in tab order).
focus_grid() { ad focus "$(ref textfield 'Text = contains')" >/dev/null; key shift+tab; }

agent-desktop permissions | jq -e '.data.accessibility.state == "granted"' >/dev/null ||
  { echo "Grant Accessibility first: agent-desktop permissions --guide"; exit 1; }
! pgrep -xq querybook || { echo "Quit Querybook first; the test quits it when done."; exit 1; }

sqlite3 "$DB" <<'SQL'
CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT UNIQUE, notes TEXT);
CREATE TABLE orders (id INTEGER PRIMARY KEY, customer_id INTEGER REFERENCES customers(id), total REAL);
INSERT INTO customers VALUES (1,'Ada','ada@example.com',NULL),(2,'Grace','grace@example.com','vip'),(3,'Linus','linus@example.com',NULL);
INSERT INTO orders VALUES (1,1,50.5),(2,2,250),(3,3,120);
SQL
open_db
check "opens the database from Finder" "$(ad list-windows --app Querybook | jq -r '.data[0].title')" "shop.db"

# ── Browse, edit, Write Changes ──
row customers; wait_text ada@example.com
focus_grid; key right; key return            # edit row 1, column "name"
ref textfield Ada >/dev/null   # the editor is open
key cmd+a; type_keys "Ada Lovelace"; key return
see "the grid shows the edit as unsaved" "Unsaved changes"
check "an edit waits for Write Changes" "$(q 'select name from customers where id=1')" "Ada"
key cmd+s; sleep 1
check "⌘S writes the edit" "$(q 'select name from customers where id=1')" "Ada Lovelace"

# ── Revert ──
focus_grid; key escape; key down; key right; key right; key right; key delete   # notes of row 2 → NULL
wait_text "Unsaved changes"; click button Revert; sleep 1
check "Revert keeps the file" "$(q 'select notes from customers where id=2')" "vip"
see "Revert restores the grid" vip

# ── SQL editor ──
key cmd+t; wait_text "Write SQL here"
type_into "$(ref textfield 'Write SQL here')" "update orders set total = 99 where id = 1"
key escape; key cmd+return; see "SQL status reads in the singular" "1 statement · 1 row changed"
check "SQL changes wait for Write Changes" "$(q 'select total from orders where id=1')" "50.5"
click button "Write Changes"; sleep 1
check "Write Changes writes SQL changes" "$(q 'select total from orders where id=1')" "99.0"

# ── History, integrity ──
click button History
see "History lists the SQL save" "total 50.5 → 99.0"
see "History lists the grid save" 'name "Ada" → "Ada Lovelace"'
click button Info; click button "Integrity check"
see "integrity check" "Integrity check: no problems found"

# ── Close with unsaved changes ──
row orders; wait_text 250.0
click button Row --state disabled=false; wait_text "Unsaved changes"
click button Close --exact; sleep 1
ad find --app Querybook --surface sheet --role button --text "Don't Save" | jq -r '.data.matches[0].ref_id' | xargs agent-desktop click >/dev/null 2>&1
sleep 1.5
check "Don't Save closes without writing" "$(pgrep -xq querybook && echo open || echo closed) $(q 'select count(*) from orders')" "closed 3"

# ── Encryption ──
open_db; click button Encryption
pw=($(ad find --app Querybook --role textfield --state secure | jq -r '.data.matches[].ref_id'))
type_into "${pw[0]:-}" "$KEY"; type_into "${pw[1]:-}" "$KEY"
click button Encrypt --exact; wait_text "SQLCipher 4" 15000
check "encrypt: plain sqlite3 can't read it" "$(q 'select 1' | grep -c 'not a database')" "1"
if command -v sqlcipher >/dev/null; then
  check "encrypt: sqlcipher CLI reads it" "$(printf "PRAGMA key='%s';\nselect name from customers where id=1;\n" "$KEY" | sqlcipher "$DB" | tail -1)" "Ada Lovelace"
fi
quit; open_db Unlock
type_into "$(ref textfield '' --state secure)" "wrong"; key return
see "wrong key is refused" "Wrong password or key"
ad focus "$(ref textfield '' --state secure)" >/dev/null; key cmd+a; key delete
type_into "$(ref textfield '' --state secure)" "$KEY"; key return
wait_text "SQLCipher 4"; row customers; see "right key unlocks" "Ada Lovelace"
click button Encryption; click button "Remove encryption"; wait_text "Not encrypted" 15000
check "remove encryption" "$(head -c 15 "$DB") $(q 'pragma integrity_check')" "SQLite format 3 ok"

quit
echo; [ "$FAILS" -eq 0 ] && echo "All checks passed." || { echo "$FAILS check(s) failed."; exit 1; }
