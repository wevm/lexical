#!/usr/bin/env bash
# Consumes the pending changesets in rust/.changeset: bumps lexical-ui's
# version in rust/lexical-ui/Cargo.toml and in the lockfiles that list
# it, and adds the new version's section to rust/CHANGELOG.md, naming the
# upstream Lexical version in lexical_ui::UPSTREAM_VERSION.
#
# The Changesets CLI always works in the monorepo root's .changeset/, which
# is upstream Lexical's, so `changeset version` runs in a temporary root
# that holds only the crate and its changesets.
set -euo pipefail

rust="$(cd "$(dirname "$0")/.." && pwd)"
crate=lexical-ui
manifest="$rust/$crate/Cargo.toml"
shopt -s nullglob
sets=()
for f in "$rust"/.changeset/*.md; do
  [ "$(basename "$f")" = README.md ] || sets+=("$f")
done
if [ ${#sets[@]} -eq 0 ]; then
  echo "No pending changesets in rust/.changeset."
  exit 0
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/.changeset"
cp "$rust/.changeset/config.json" "${sets[@]}" "$tmp/.changeset/"
old="$(sed -nE '/^\[package\]/,/^\[/ s/^version = "(.*)"$/\1/p' "$manifest")"
if [ -z "$old" ]; then
  echo "No version in $manifest's [package]." >&2
  exit 1
fi
printf '{"name": "%s", "version": "%s", "private": true}\n' "$crate" "$old" \
  >"$tmp/package.json"
printf '# %s\n' "$crate" >"$tmp/CHANGELOG.md"
(cd "$tmp" && git init -q && pnpm dlx @changesets/cli@3.0.3 version)
new="$(node -p "require('$tmp/package.json').version")"

# The generated section, minus the title, with the upstream version first.
upstream="$(sed -nE 's/^pub const UPSTREAM_VERSION: &str = "(.*)";$/\1/p' "$rust/$crate/src/lib.rs")"
if [ -z "$upstream" ]; then
  echo "No UPSTREAM_VERSION in $crate/src/lib.rs." >&2
  exit 1
fi
section="$tmp/section.md"
{
  printf '## %s\n\n' "$new"
  printf 'Conforms to upstream Lexical **%s** (`lexical_ui::UPSTREAM_VERSION`).\n' "$upstream"
  awk 'NR > 1 && !/^## / {print}' "$tmp/CHANGELOG.md"
} | cat -s >"$section"

# Insert it above the newest release's section, or at the end before the
# first release.
awk -v section="$section" '
  function insert() {while ((getline line < section) > 0) print line; done = 1}
  !done && /^## / {insert(); print ""}
  {print}
  END {if (!done) {print ""; insert()}}
' "$rust/CHANGELOG.md" >"$tmp/CHANGELOG.out"
mv "$tmp/CHANGELOG.out" "$rust/CHANGELOG.md"

# The version in the manifest's [package] and in every lockfile's entry for
# the crate (rust/ and the playgrounds' own workspaces), so --locked builds
# still pass.
awk -v old="$old" -v new="$new" '
  /^\[package\]/ {pkg = 1} /^\[/ && !/^\[package\]/ {pkg = 0}
  pkg && $0 == "version = \"" old "\"" {$0 = "version = \"" new "\""; pkg = 0}
  {print}
' "$manifest" >"$tmp/Cargo.toml.out"
mv "$tmp/Cargo.toml.out" "$manifest"
for lock in "$rust"/Cargo.lock "$rust"/apps/*/Cargo.lock; do
  awk -v crate="$crate" -v old="$old" -v new="$new" '
    /^\[\[package\]\]/ {hit = 0}
    $0 == "name = \"" crate "\"" {hit = 1}
    hit && $0 == "version = \"" old "\"" {$0 = "version = \"" new "\""; hit = 0}
    {print}
  ' "$lock" >"$tmp/Cargo.lock.out"
  mv "$tmp/Cargo.lock.out" "$lock"
done
rm -f "${sets[@]}"
echo "Versioned $crate $old -> $new."
