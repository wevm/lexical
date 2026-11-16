#!/usr/bin/env bash
# The playground end to end, as a person types into it: a click in the
# editor's empty space below the text, then keys with their physical codes,
# which Rune routes as a window's keys (`physical_then`, then the focused
# field's `on_input_key`), not as `Runtime::key`. It types "zq" at the end of
# the document, then Return, `## Hi` (a heading, by its Markdown shortcut),
# Return and `- x` (a list). The document must end with each of them, read
# from the tree panel in the app's last frame.
#
# A click there used to blur the field (a field with links took clicks only
# on its links), so nothing typed after it reached Lexical.
#
# Run it from rust/apps/lexical-playground-rune. $1 is where to keep the
# script, the element tree and a snapshot (default: a temporary directory).
set -euo pipefail

out=${1:-$(mktemp -d)}
mkdir -p "$out"

# One key press, with its physical code.
events=()
t=1.5
key() {
  local shift=false
  [ "${3:-}" = shift ] && shift=true
  events+=("$(printf '{"t": %s, "type": "key", "name": "%s", "code": "%s", "shift": %s}' "$t" "$1" "$2" "$shift")")
  t=$(awk -v t="$t" 'BEGIN { printf "%.1f", t + 0.1 }')
}
key z z
key q q
key Enter enter
key '#' shift+3 shift
key '#' shift+3 shift
key ' ' space
key H h shift
key i i
key Enter enter
key - minus
key ' ' space
key x x
{
  echo '{"events": ['
  # The editor's empty space below the last line, near its right edge: the
  # caret goes to the end of the document.
  echo '  {"t": 1.0, "type": "tap", "x": 620, "y": 630},'
  (IFS=','; echo "  ${events[*]}")
  echo ']}'
} > "$out/script.json"

cargo run --locked --quiet -- --script "$out/script.json" --at "$(awk -v t="$t" 'BEGIN { print t + 1 }')" \
  --size 1100x720 --tree "$out/tree.json" --snap "$out/e2e.png" >/dev/null

# The tree panel: Lexical's document as the app shows it.
jq -r '.elements[] | select(.text != null and (.text | startswith("(root)"))) | .text' "$out/tree.json" > "$out/document.txt"
cat "$out/document.txt"
# Each block, in order, at the document's end.
awk '
  /text "Lexical is an extensible text editor framework.zq"$/ { s = 1; next }
  s == 1 && /heading h2$/ { s = 2; next }
  s == 2 && /text "Hi"$/ { s = 3; next }
  s == 3 && /list bullet$/ { s = 4; next }
  s == 4 && /listitem bullet$/ { s = 5; next }
  s == 5 && /text "x"$/ { s = 6; next }
  s >= 1 && s < 6 { exit 1 }
  END { exit s == 6 ? 0 : 1 }
' "$out/document.txt" || { echo "e2e: the typed text, heading or list is missing (above)" >&2; exit 1; }
echo "e2e: typed, a heading and a list"
