#!/usr/bin/env bash
# Consumes the pending changesets in swift/.changeset: bumps swift/VERSION
# and adds the new version's section to swift/CHANGELOG.md, naming the
# upstream Lexical version in swift/UPSTREAM_VERSION.
#
# The Changesets CLI always works in the monorepo root's .changeset/, which
# is upstream Lexical's, so `changeset version` runs in a temporary root
# that holds only the Swift package and its changesets.
set -euo pipefail

swift="$(cd "$(dirname "$0")/.." && pwd)"
shopt -s nullglob
sets=()
for f in "$swift"/.changeset/*.md; do
  [ "$(basename "$f")" = README.md ] || sets+=("$f")
done
if [ ${#sets[@]} -eq 0 ]; then
  echo "No pending changesets in swift/.changeset."
  exit 0
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/.changeset"
cp "$swift/.changeset/config.json" "${sets[@]}" "$tmp/.changeset/"
old="$(tr -d '[:space:]' <"$swift/VERSION")"
printf '{"name": "lexical-swift", "version": "%s", "private": true}\n' "$old" \
  >"$tmp/package.json"
printf '# lexical-swift\n' >"$tmp/CHANGELOG.md"
(cd "$tmp" && git init -q && pnpm dlx @changesets/cli@3.0.3 version)
new="$(node -p "require('$tmp/package.json').version")"

# The generated section, minus the title, with the upstream version first.
upstream="$(tr -d '[:space:]' <"$swift/UPSTREAM_VERSION")"
section="$tmp/section.md"
{
  printf '## %s\n\n' "$new"
  printf 'Conforms to upstream Lexical **%s** (`UPSTREAM_VERSION`).\n' "$upstream"
  awk 'NR > 1 && !/^## / {print}' "$tmp/CHANGELOG.md"
} | cat -s >"$section"

# Insert it above the newest release's section.
awk -v section="$section" '
  !done && /^## / {while ((getline line < section) > 0) print line; print ""; done = 1}
  {print}
' "$swift/CHANGELOG.md" >"$tmp/CHANGELOG.out"
mv "$tmp/CHANGELOG.out" "$swift/CHANGELOG.md"
printf '%s\n' "$new" >"$swift/VERSION"
rm -f "${sets[@]}"
echo "Versioned lexical-swift $old -> $new."
