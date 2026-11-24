#!/usr/bin/env bash
# End-to-end test of the macOS app through its accessibility tree, with agent-desktop
# (npm install -g agent-desktop). Drives the real UI on a throwaway database: browse,
# edit + Write Changes, Revert, the SQL editor, History, integrity check, column resize,
# column filters (= <> != contains), the cell inspector opening on click, the close
# prompt, opening any extension (File › Open, Finder, Open With), the Dock icon's recent
# databases, and encrypt / unlock / change / remove encryption (change and remove need the
# current password).
#
# It takes keyboard focus for about a minute (WebView text fields only take real
# keystrokes), so don't type while it runs. It quits Querybook at the end. The
# throwaway databases stay in Recent files (the app's and the Dock's), and their change
# history in the app data.
#
#   scripts/e2e-macos.sh                          # the release bundle in target/
#   scripts/e2e-macos.sh /Applications/Querybook.app
set -uo pipefail

APP=${1:-target/release/bundle/macos/Querybook.app}
[ -d "$APP" ] || { echo "No app at $APP. Build it first: cargo tauri build --bundles app,dmg"; exit 1; }
APP=$(cd "$APP" && pwd)   # open -a needs an absolute path
DIR=$(mktemp -d /tmp/querybook-e2e.XXXXXX)   # shallow: the Open panel's column view shows it unscrolled
DB=$DIR/shop.db
KEY=e2e-pass-42
KEY2=e2e-pass-43
FAILS=0

# ── helpers ──
ad() { agent-desktop "$@" 2>/dev/null; }
q() { sqlite3 "$DB" "$1" 2>&1; }
# Physical (--headed) delivery: keys posted to the process now and then go missing.
# Run an agent-desktop action, retrying while it says a retry is safe (an accessibility read that
# times out while the WebView re-renders comes back "not_delivered", retry "safe").
act() {
  local o
  for _ in 1 2 3; do
    o=$(ad "$@"); jq -e .ok >/dev/null <<<"$o" && return 0
    jq -e '.error.disposition.retry == "safe"' >/dev/null <<<"$o" || break
    sleep 0.3
  done
  return 1
}
# A key aimed at the app first reads its focused element, which can time out (APP_UNRESPONSIVE) in the
# SQL editor and the grid while the app is idle; then post it system-wide, with Querybook in front.
key() {
  act --headed press --app Querybook "$1" || { front; act --headed press "$1"; } ||
    echo "  (key $1 not delivered: is Querybook still in front?)"
}
# Ref of the first element with this role whose text contains $2 (with --exact: whose name is $2).
# Retries for about 10 s: the UI renders asynchronously, and accessibility reads fail while WebKit rebuilds.
ref() {
  local by=--text r; [ "${3:-}" = --exact ] && by=--name
  for _ in $(seq 25); do
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
type_into() { act --headed type "$1" "$2" || echo "  (couldn't type into $1)"; }
# Paste SQL into a new SQL tab and run it, once the editor holds it (a run that overtakes the text runs nothing).
# Typed SQL keeps the app's accessibility busy for 5-9 s after (WebKit rebuilds it per keystroke), and
# keys pressed meanwhile go missing; a paste is one edit.
run_sql() {
  key cmd+t; wait_text "Write SQL here"; ad focus "$(ref textfield 'Write SQL here')" >/dev/null
  type_cell "$1"; key escape; key cmd+return
}
# The focused text field (the grid's cell editor, the SQL editor): headed `type` can't target the cell
# editor (no AXFocusedUIElement), and per-key presses drop or double keystrokes, so paste the text over
# the field's, until it holds it, then put the clipboard's text back.
type_cell() {
  local saved; saved=$(pbpaste)
  for _ in 1 2 3; do
    printf %s "$1" | pbcopy; key cmd+a; key cmd+v
    focused_is "$1" && break
    echo "  (the paste didn't land; retrying)"
  done
  printf %s "$saved" | pbcopy
}
wait_text() { ad snapshot --app Querybook -w ":$1" --wait-timeout "${2:-8000}" >/dev/null; }
wait_gone() { ad snapshot --app Querybook --wait-for-gone ":$1" --wait-timeout 8000 >/dev/null; }
see() { wait_text "$2" "${3:-8000}" && check "$1" ok ok || check "$1" "not on screen" "\"$2\""; }
check() { # check "what" actual expected
  if [ "$2" == "$3" ]; then echo "✓ $1"; else echo "✗ $1: got '$2', expected '$3'"; FAILS=$((FAILS + 1)); fi
}
open_db() {
  open -a "$APP" "$DB" && wait_text "${1:-customers}" 15000 || { echo "✗ Querybook didn't open the database; stopping."; exit 1; }
}
quit() { ad close-app Querybook >/dev/null; for _ in 1 2 3 4 5 6 7 8 9 10; do pgrep -xq querybook || { sleep 1; return 0; }; sleep 0.5; done; }   # the pause avoids a relaunch race (error -600)
# Focus the grid (it precedes its first filter field in tab order).
focus_grid() { ad focus "$(ref textfield 'Text = contains')" >/dev/null; key shift+tab; }
# "x y width height" of a ref, in screen points (empty when there's no such ref).
box() { ad get "$1" --property bounds | jq -r '.data.value // empty | "\(.x|round) \(.y|round) \(.width|round) \(.height|round)"'; }
# Same for the first text named $1, read from a snapshot: grid cells aren't interactive, so they get no ref.
text_box() {
  local b
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    b=$(ad snapshot --app Querybook --include-bounds | jq -r --arg n "$1" \
      'first(.. | objects | select(.role == "statictext" and .name == $n and (.bounds.x | type) == "number") | .bounds) // empty | "\(.x|round) \(.y|round) \(.width|round) \(.height|round)"')
    [ -n "$b" ] && { echo "$b"; return; }
    sleep 0.4
  done
}
# The visible tab's filter fields, one per column (hidden tabs are display:none, so not listed).
filter_field() { ad find --app Querybook --role textfield --text 'Text = contains' | jq -r --argjson i "$1" '.data.matches[$i].ref_id // empty'; }
# Physical click in the middle of "x y width height": the grid reacts to mousedown, which an
# accessibility press doesn't send. Refuses when nothing was found (a click at 0,0 opens the Apple menu).
click_box() {
  local x y w h; read -r x y w h <<<"$1"
  [ -n "$h" ] || { echo "  (nothing to click)"; return 1; }
  front; ad --headed mouse-click --xy="$((x + w / 2)),$((y + h / 2))" "${@:2}" >/dev/null
}
# Physical clicks land on whatever window is on top there, so bring Querybook's to the front first.
front() { ad --headed focus-window --app Querybook >/dev/null; }
tap() { click_box "$(box "$1")" "${@:2}"; }
tap_text() { click_box "$(text_box "$1")" "${@:2}"; }
# Replace a filter field's text ("" clears it) and apply it with Return.
# Types over the selected text rather than deleting it first: `type` refocuses the field, and that blur
# would apply the emptied field. Waits for the text before Return, which can overtake it.
set_filter() {
  ad focus "$1" >/dev/null; key cmd+a
  if [ -n "$2" ]; then type_into "$1" "$2"; else key delete; fi
  for _ in 1 2 3 4 5 6 7 8 9 10; do [ "$(ad get "$1" --property value | jq -r '.data.value // ""')" == "$2" ] && break; sleep 0.2; done
  key return
}
# filter_sees "what" field text "count": set the filter, expect the count. A posted Return now and then
# goes missing, so press it once more; on a filter already applied it does nothing.
filter_sees() {
  set_filter "$2" "$3"
  { wait_text "$4" 5000 || { key return; wait_text "$4" 5000; }; } && check "$1" ok ok || check "$1" "not on screen" "\"$4\""
}
# The topmost grid value containing $1 (rows render top to bottom).
first_value() { ad find --app Querybook --role statictext --text "$1" | jq -r '.data.matches[0].name // empty'; }
# Wait until the focused text field holds $1 (the grid's cell editor; the inspector shows the same text, unfocused).
focused_is() { for _ in 1 2 3 4 5 6 7 8 9 10; do [ "$(ad find --app Querybook --role textfield --state focused | jq -r '.data.matches[0].value')" == "$1" ] && return 0; sleep 0.4; done; return 1; }
# What sqlcipher reads from the database with key $1 (an error mentions "not a database").
qk() { printf "PRAGMA key='%s';\nselect name from customers where id=1;\n" "$1" | sqlcipher "$DB" 2>&1 | tail -1; }
# Is the inspector on screen (its Text/JSON/Hex modes, or its empty state)? Its close button has no
# accessible name (WebKit doesn't expose `title` on it), so it's closed from the toolbar's Inspector toggle.
inspector() {
  [ "$(ad find --app Querybook --role button --name Hex --exact | jq '.data.matches | length')$(ad find --app Querybook --text 'Select a cell to inspect it' | jq '.data.matches | length')" != 00 ] && echo true || echo false
}
close_inspector() { [ "$(inspector)" == true ] && click button Inspector && sleep 0.5; }

agent-desktop permissions | jq -e '.data.accessibility.state == "granted"' >/dev/null ||
  { echo "Grant Accessibility first: agent-desktop permissions --guide"; exit 1; }
! pgrep -xq querybook || { echo "Quit Querybook first; the test quits it when done."; exit 1; }
# The test points the Open panel at $DIR through the app's defaults; put them back however it exits.
PREFS=com.kevalthumar.querybook
defaults export $PREFS "$DIR/prefs.plist" 2>/dev/null
trap 'if [ -s "$DIR/prefs.plist" ]; then defaults import $PREFS "$DIR/prefs.plist"; else defaults delete $PREFS NSOSPLastRootDirectory 2>/dev/null; fi' EXIT

sqlite3 "$DB" <<'SQL'
CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT UNIQUE, notes TEXT);
CREATE TABLE orders (id INTEGER PRIMARY KEY, customer_id INTEGER REFERENCES customers(id), total REAL);
INSERT INTO customers VALUES (1,'Ada','ada@example.com',NULL),(2,'Grace','grace@example.com','vip'),(3,'Linus','linus@example.com',NULL);
INSERT INTO orders VALUES (1,1,50.5),(2,2,250),(3,3,120);
CREATE TABLE readings (id INTEGER PRIMARY KEY, status INTEGER);
INSERT INTO readings (status) VALUES (1),(2),(3),(5),(6),(8);
SQL
# Databases named like app exports: a plain .Logger and a SQLCipher .Veroval.
LOGGER=$DIR/shop.Logger VEROVAL=$DIR/shop.Veroval
cp "$DB" "$LOGGER"
command -v sqlcipher >/dev/null &&
  sqlcipher "$DB" "ATTACH DATABASE '$VEROVAL' AS v KEY '$KEY'; SELECT sqlcipher_export('v'); DETACH DATABASE v;" >/dev/null
open_db
check "opens the database from Finder" "$(ad list-windows --app Querybook | jq -r '.data[0].title')" "shop.db"

# ── Browse, edit, Write Changes ──
row customers; wait_text ada@example.com
focus_grid; key right; key return            # edit row 1, column "name"
focused_is Ada   # the editor is open (the inspector, opened by the selection, holds "Ada" too)
type_cell "Ada Lovelace"; key return
see "the grid shows the edit as unsaved" "Unsaved changes"
check "an edit waits for Write Changes" "$(q 'select name from customers where id=1')" "Ada"
key cmd+s; wait_gone "Unsaved changes" || { echo "  (⌘S had no effect; pressing it again)"; key cmd+s; wait_gone "Unsaved changes"; }
check "⌘S writes the edit" "$(q 'select name from customers where id=1')" "Ada Lovelace"

# ── Revert ──
close_inspector; tap_text vip; key delete   # notes of row 2 → NULL (the open inspector could cover that cell)
wait_text "Unsaved changes"; click button Revert; sleep 1
check "Revert keeps the file" "$(q 'select notes from customers where id=2')" "vip"
see "Revert restores the grid" vip

# ── SQL editor ──
run_sql "update orders set total = 99 where id = 1"; see "SQL status reads in the singular" "1 statement · 1 row changed"
check "SQL changes wait for Write Changes" "$(q 'select total from orders where id=1')" "50.5"
click button "Write Changes"; wait_gone "Unsaved changes"
check "Write Changes writes SQL changes" "$(q 'select total from orders where id=1')" "99.0"

# ── History, integrity ──
click button History
see "History lists the SQL save" "total 50.5 → 99.0"
see "History lists the grid save" 'name "Ada" → "Ada Lovelace"'
click button Info; click button "Integrity check"
see "integrity check" "Integrity check: no problems found"

# ── Column resize ──
row customers; wait_text grace@example.com
close_inspector   # room for the widened column
read -r hx hy hw hh <<<"$(box "$(ref statictext email --exact)")"
y=$((hy + hh / 2))
read -r fx _ <<<"$(box "$(filter_field 3)")"                # the notes filter starts at the email/notes boundary
read -r vx _ <<<"$(text_box vip)"                           # a notes value in the body
edge=$((fx - 5))                                             # inside the 6px grab strip at email's right edge
front; ad --headed drag --from-xy="$edge,$y" --to-xy="$((edge + 120)),$y" --duration 400 >/dev/null; sleep 0.5
read -r fx2 _ <<<"$(box "$(filter_field 3)")"
read -r vx2 _ <<<"$(text_box vip)"
check "dragging a header edge widens the column (header + filter row)" "$(( (fx2 - fx - 120) ** 2 <= 16 ))" 1
check "the body cells follow the header" "$(( (vx2 - vx - 120) ** 2 <= 16 ))" 1
# The release didn't sort: one header click sorts ascending (a sort left by the drag would make it descending).
tap "$(ref statictext email --exact)"; sleep 0.8
check "after a resize, a header click sorts ascending" "$(first_value @example.com)" "ada@example.com"
tap "$(ref statictext email --exact)"; sleep 0.8
check "a second header click sorts descending" "$(first_value @example.com)" "linus@example.com"
tap "$(ref statictext email --exact)"; sleep 0.8   # back to unsorted
front; ad --headed mouse-click --xy="$((fx2 - 5)),$y" --count 2 >/dev/null; sleep 0.5
read -r fx3 _ <<<"$(box "$(filter_field 3)")"
check "double-clicking the edge fits the column to its values" "$(( fx3 < fx2 - 40 ))" 1

# ── Column filters ──
row readings; see "readings lists its 6 rows" "6 rows"
st=$(filter_field 1)   # id, status
filter_sees "filter !=8 hides the 8s" "$st" "!=8" "5 rows"
filter_sees "filter =8 shows only the 8" "$st" "=8" "1 row"
filter_sees "filter <>8 hides the 8s" "$st" "<>8" "5 rows"
filter_sees "clearing the filter shows all rows" "$st" "" "6 rows"
check "the filter tooltip lists != (and still starts with 'Text = contains')" \
  "$(ad find --app Querybook --role textfield --text 'Text = contains · prefix with = <> !=' | jq -r '.data.matches | length > 0')" true
row customers; wait_text grace@example.com
nm=$(filter_field 1)   # id, name, email, notes
filter_sees "a text filter still means contains" "$nm" "race" "1 row"
filter_sees "clearing it shows all customers" "$nm" "" "3 rows"

# ── Cell inspector opens on click ──
close_inspector
check "the Inspector toggle closes it" "$(inspector)" false
tap_text grace@example.com
see "clicking a cell opens the inspector on its value" "17 chars"
close_inspector
tap_text grace@example.com
see "clicking a cell again reopens it" "17 chars"
key up; see "arrow keys move the inspector with the selection" "15 chars"   # ada@example.com
tap_text ada@example.com --count 2
focused_is ada@example.com && check "double-click still edits the cell inline" ok ok || check "double-click still edits the cell inline" "no editor" "editor"
key escape; sleep 0.5
check "Esc leaves the value unchanged" "$(q 'select email from customers where id=1')" "ada@example.com"
close_inspector
tap_text 2 && sleep 0.8 &&   # row 2's number in the gutter (it precedes the id cell "2")
  check "selecting a whole row doesn't open it" "$(inspector) $(ad find --app Querybook --role statictext --text '1 selected' | jq -r '.data.matches | length > 0')" "false true"
key escape
run_sql "select * from orders"; wait_text 250.0
tap_text 250.0
see "clicking a SQL result cell opens the inspector" "5 chars"
check "the SQL result inspector is read-only (no Apply)" "$(ad find --app Querybook --role button --text Apply | jq -r '.data.matches | length')" 0

# ── Close with unsaved changes ──
row orders; wait_text 250.0
click button Row --state disabled=false; wait_text "Unsaved changes"
click button Close --exact; sleep 1
ad find --app Querybook --surface sheet --role button --text "Don't Save" | jq -r '.data.matches[0].ref_id' | xargs agent-desktop click >/dev/null 2>&1
sleep 1.5
check "Don't Save closes without writing" "$(pgrep -xq querybook && echo open || echo closed) $(q 'select count(*) from orders')" "closed 3"

# ── Any extension: File › Open, Finder, Open With ──
windows() { ad list-windows --app Querybook | jq -r '[.data[].title] | join(",")'; }
open_db
# The Open panel's UI runs out of process, so posted keys (⌘⇧G) don't reach it. Start it in $DIR
# instead (it opens in the folder last used: a bookmark in the app's defaults, put back on exit)
# and press the file's row through accessibility, which opens it.
printf '%s\n' 'import Foundation' \
  'print(try! URL(fileURLWithPath: CommandLine.arguments[1]).bookmarkData().map { String(format: "%02x", $0) }.joined())' >"$DIR/bookmark.swift"
defaults write $PREFS NSOSPLastRootDirectory -data "$(swift "$DIR/bookmark.swift" "$DIR" 2>/dev/null)"
key cmd+o                                      # the Open panel, unfiltered
for _ in $(seq 30); do   # the panel's service can take a while to start
  r=$(ad snapshot --app Querybook --surface sheet | jq -r 'first(.. | objects | select(.ref_id and any(.children[]?; .value == "shop.Logger")) | .ref_id) // empty')
  [ -n "$r" ] && break; sleep 0.5
done
if [ -n "$r" ]; then
  ad click "$r" >/dev/null   # can report a failure though it opened the file: the panel closes under it
else
  echo "  (shop.Logger isn't listed in the Open panel; windows: $(windows))"
  ad find --app Querybook --surface sheet --role button --name Cancel | jq -r '.data.matches[0].ref_id' | xargs agent-desktop click >/dev/null 2>&1
fi
for _ in 1 2 3 4 5 6 7 8 9 10; do [[ $(windows) == *shop.Logger* ]] && break; sleep 0.5; done
check "File › Open opens a .Logger database" "$(windows | tr ',' '\n' | grep -c '^shop.Logger$')" 1
if [ -f "$VEROVAL" ]; then
  open -a "$APP" "$VEROVAL"; wait_text "Unlock shop.Veroval" 15000
  type_into "$(ref textfield '' --state secure)" "$KEY"; key return
  # The Unlock dialog itself mentions "SQLCipher 4", so wait for it to close first.
  wait_gone "Unlock shop.Veroval" && wait_text "SQLCipher 4" &&
    check "an encrypted .Veroval from Finder unlocks with its key" ok ok ||
    check "an encrypted .Veroval from Finder unlocks with its key" "still locked" "unlocked"
else
  echo "  (no sqlcipher CLI: skipped the encrypted .Veroval)"
fi
key cmd+n; sleep 1.5                           # New still suggests database.db
check "New still suggests database.db" "$(ad find --app Querybook --role textfield --text database.db | jq -r '.data.matches | length > 0')" true
# Esc can't reach the out-of-process panel either.
ad find --app Querybook --surface sheet --role button --name Cancel | jq -r '.data.matches[0].ref_id' | xargs agent-desktop click >/dev/null 2>&1
types=$(plutil -extract CFBundleDocumentTypes json -o - "$APP/Contents/Info.plist")
check "Info.plist: Querybook can open any data file, as an alternate app" \
  "$(jq -r '[.[] | select(.LSItemContentTypes == ["public.data"]) | .LSHandlerRank] | join(",")' <<<"$types")" "Alternate"
check "Info.plist: the SQLite extensions are still declared" \
  "$(jq -r '[.[] | .CFBundleTypeExtensions // empty | join(",")] | join(";")' <<<"$types")" "db,db3,sqlite,sqlite3,sqlcipher"
# What macOS registered for this bundle (open -a registered it). Not NSWorkspace's app list for a
# file: it leaves out every wildcard claim, DB Browser's "*" included, and lists only extension claims
# and the user's own Open With choices.
LSR=/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister
check "Launch Services registered Querybook for any data file" \
  "$("$LSR" -dump 2>/dev/null | awk -v p=" $APP (" 'index($0, "path:") == 1 && index($0, p) {f = 1} f && /^claimed UTIs:/ {print ($0 ~ /public\.data/); exit}')" 1

# ── Dock: recent databases ──
# Querybook's Dock item: the last one (a pinned copy that isn't running comes first).
dock_item() { ad find --app Dock --role dockitem --name Querybook | jq -r '.data.matches[-1].ref_id // empty'; }
# The names in the Dock icon's menu: the app's recent documents, then its open windows (titled by file name).
dock_menu() {
  local r; r=$(dock_item); [ -n "$r" ] || return
  ad right-click "$r" >/dev/null; sleep 0.6
  ad snapshot --app Dock | jq -r '.. | objects | select(.role == "menuitem") | .name // empty'
  ad --headed press escape >/dev/null
}
# Does the menu list $1 as a recent document, apart from an open window of that name? Asked a few
# times: the list updates asynchronously.
dock_lists() {
  for _ in 1 2 3 4 5; do
    [ $(($(dock_menu | grep -cxF "$1") - $(windows | tr ',' '\n' | grep -cxF "$1"))) -gt 0 ] && { echo true; return; }
    sleep 1
  done
  echo false
}
# Pick $1 in the Dock icon's menu, with a physical click: an accessibility press on its item goes stale.
dock_pick() {
  local r xy
  for _ in 1 2 3; do
    r=$(dock_item); [ -n "$r" ] && ad right-click "$r" >/dev/null; sleep 0.6
    xy=$(ad snapshot --app Dock --include-bounds | jq -r --arg n "$1" \
      'first(.. | objects | select(.role == "menuitem" and .name == $n and .bounds.width > 0) | "\(.bounds.x + 40 | round),\(.bounds.y + .bounds.height / 2 | round)") // empty')
    [ -n "$xy" ] && { ad --headed mouse-click --xy="$xy" >/dev/null; return 0; }
    ad --headed press escape >/dev/null; sleep 0.5
  done
  echo "  (the Dock's menu doesn't list $1)"
}
wait_window() { for _ in $(seq 20); do [[ ,$(windows), == *,$1,* ]] && return 0; sleep 0.5; done; return 1; }
# Copies named for this run: the Dock remembers names from earlier runs.
id=${DIR##*.}; DOCKDB=$DIR/dock-$id.db
cp "$LOGGER" "$DOCKDB"
open -a "$APP" "$DOCKDB"; wait_window "dock-$id.db"
check "the Dock icon's menu lists a database once it's open" "$(dock_lists "dock-$id.db")" true
if [ -f "$VEROVAL" ]; then
  DOCKENC=$DIR/dock-$id.Veroval; cp "$VEROVAL" "$DOCKENC"
  open -a "$APP" "$DOCKENC"; wait_text "Unlock dock-$id.Veroval" 15000
  check "…not an encrypted one still waiting at the Unlock prompt" "$(dock_lists "dock-$id.Veroval")" false
  type_into "$(ref textfield '' --state secure)" "$KEY"; key return
  wait_gone "Unlock dock-$id.Veroval"
  check "…and lists it once unlocked" "$(dock_lists "dock-$id.Veroval")" true
fi
# Picking it in the Dock's menu opens it again, the way Finder does (RunEvent::Opened).
ad --headed focus-window --app Querybook --title "dock-$id.db" >/dev/null; key cmd+w
for _ in $(seq 10); do [[ ,$(windows), != *,dock-$id.db,* ]] && break; sleep 0.5; done
dock_pick "dock-$id.db"
check "picking it in the Dock's menu opens it" "$(wait_window "dock-$id.db" && echo open || echo "not open")" open
quit

# ── Encryption ──
open_db; click button Encryption
pw=($(ad find --app Querybook --role textfield --state secure | jq -r '.data.matches[].ref_id'))
check "encrypting a plain database asks for no current password" "${#pw[@]}" 2
type_into "${pw[0]:-}" "$KEY"; type_into "${pw[1]:-}" "$KEY"
# The dialog itself reads "Encryption settings: SQLCipher 4 defaults", so wait for it to close first.
click button Encrypt --exact; wait_gone "Encryption settings"; wait_text "SQLCipher 4" 15000
check "encrypt: plain sqlite3 can't read it" "$(q 'select 1' | grep -c 'not a database')" "1"
if command -v sqlcipher >/dev/null; then
  check "encrypt: sqlcipher CLI reads it" "$(printf "PRAGMA key='%s';\nselect name from customers where id=1;\n" "$KEY" | sqlcipher "$DB" | tail -1)" "Ada Lovelace"
fi
quit; open_db Unlock
type_into "$(ref textfield '' --state secure)" "wrong"; key return
see "wrong key is refused" "Wrong password or key"
ad focus "$(ref textfield '' --state secure)" >/dev/null; key cmd+a; key delete
type_into "$(ref textfield '' --state secure)" "$KEY"; key return
wait_gone "Encryption settings"; wait_text "SQLCipher 4"; row customers; see "right key unlocks" "Ada Lovelace"

# Change key: the current password comes first.
click button Encryption; sleep 0.5
pw=($(ad find --app Querybook --role textfield --state secure | jq -r '.data.matches[].ref_id'))
check "changing the key asks for the current password" "${#pw[@]}" 3
type_into "${pw[0]:-}" "wrong"; type_into "${pw[1]:-}" "$KEY2"; type_into "${pw[2]:-}" "$KEY2"
click button "Change key" --exact
see "a wrong current password is refused" "The current password is wrong"
command -v sqlcipher >/dev/null && check "…and the key is unchanged" "$(qk "$KEY")" "Ada Lovelace"
ad focus "${pw[0]:-}" >/dev/null; key cmd+a; key delete; type_into "${pw[0]:-}" "$KEY"
click button "Change key" --exact
see "the right current password changes the key" "encrypted with the new key" 15000
command -v sqlcipher >/dev/null &&
  check "change key: the new key opens it, the old one doesn't" "$(qk "$KEY2") / $(qk "$KEY" | grep -c 'not a database')" "Ada Lovelace / 1"

# Remove encryption: refused without the current password.
click button Encryption; click button "Remove encryption"
see "removing without the current password is refused" "The current password is wrong"
check "…and the file stays encrypted" "$(q 'select 1' | grep -c 'not a database')" "1"
type_into "$(ref textfield '' --state secure)" "$KEY2"   # the first field is the current password
click button "Remove encryption"; wait_text "Not encrypted" 15000
check "remove encryption" "$(head -c 15 "$DB") $(q 'pragma integrity_check')" "SQLite format 3 ok"

quit
echo; [ "$FAILS" -eq 0 ] && echo "All checks passed." || { echo "$FAILS check(s) failed."; exit 1; }
