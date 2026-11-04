#!/usr/bin/env bash
# Fails when the public API of swift/ breaks compatibility with the newest
# release tag on the mirror (wevm/lexical-swift), unless the changelog's
# `## Unreleased` section has a `- **Breaking:**` entry (DESIGN.md, Repos
# and releases). Before the first release there is no tag, and the check
# passes.
#
# Usage: swift/scripts/api-breaks.sh [mirror-url]
# `package` declarations are internals (swift/AGENTS.md); a `public`
# declaration made `package` is reported as removed.
set -euo pipefail

mirror="${1:-https://github.com/wevm/lexical-swift.git}"
swift_dir="$(cd "$(dirname "$0")/.." && pwd)"

tag="$(git ls-remote --tags --refs "$mirror" |
  sed 's|.*refs/tags/||' | grep -E '^[0-9]+\.[0-9]+\.[0-9]+$' |
  sort -t. -k1,1n -k2,2n -k3,3n | tail -1 || true)"
if [ -z "$tag" ]; then
  echo "No release tag on $mirror yet; nothing to compare."
  exit 0
fi
echo "Comparing the public API with $tag."

# SwiftPM compares two revisions of one repository, so the working tree goes
# on top of the tag in a clone of the mirror, whose root is the package.
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git clone -q --depth 1 --branch "$tag" "$mirror" "$work/pkg"
find "$work/pkg" -mindepth 1 -maxdepth 1 ! -name .git -exec rm -rf {} +
tar -C "$swift_dir" --exclude ./.build --exclude ./.swiftpm -cf - . |
  tar -C "$work/pkg" -xf -
git -C "$work/pkg" add -A
git -C "$work/pkg" -c user.name=ci -c user.email=ci@localhost \
  commit -q --allow-empty -m "working tree"

# Every library product. On Linux the Apple-only views compile to empty
# modules, so only macOS checks their API.
status=0
(cd "$work/pkg" && swift package diagnose-api-breaking-changes "$tag") || status=$?

# The digester counts `package` declarations as API, so it misses a public
# declaration made `package`. Symbol graphs hold only public symbols: any
# public symbol of the tag that the working tree lacks is a break.
git clone -q --depth 1 --branch "$tag" "$mirror" "$work/base"
for side in base pkg; do
  mkdir -p "$work/graphs-$side"
  (cd "$work/$side" && swift build -q --scratch-path "$work/build-$side" \
    -Xswiftc -emit-symbol-graph -Xswiftc -emit-symbol-graph-dir \
    -Xswiftc "$work/graphs-$side" >/dev/null)
done
python3 - "$work/graphs-base" "$work/graphs-pkg" <<'EOF' || status=1
import glob, json, os, sys

def public(d):
    out = {}
    for f in glob.glob(os.path.join(d, "Lexical*.symbols.json")):
        if os.path.basename(f).startswith("LexicalMacros"):
            continue
        g = json.load(open(f))
        for s in g["symbols"]:
            if s["accessLevel"] in ("public", "open"):
                name = ".".join(s["pathComponents"])
                out[s["identifier"]["precise"]] = f'{g["module"]["name"]}: {name}'
    return out

base, new = public(sys.argv[1]), public(sys.argv[2])
gone = sorted(base[k] for k in base.keys() - new.keys())
for g in gone:
    print(f"  💔 no longer public: {g}")
sys.exit(1 if gone else 0)
EOF

if [ "$status" -eq 0 ]; then
  echo "No API breaks since $tag."
  exit 0
fi

if awk '/^## Unreleased/ {f = 1; next} /^## / {f = 0} f' \
  "$swift_dir/CHANGELOG.md" | grep -q '^- \*\*Breaking:\*\*'; then
  echo "API breaks since $tag are listed as breaking in CHANGELOG.md."
  exit 0
fi
echo "API breaks since $tag. Keep the API compatible, or add a" >&2
echo "'- **Breaking:**' entry under '## Unreleased' in swift/CHANGELOG.md." >&2
exit 1
