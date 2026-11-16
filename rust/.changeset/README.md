# Changesets

Changesets version the `lexical-editor` crate. Each user-facing change to
it (what an app that depends on the crate sees: API, behavior, types,
features or the minimum Rust version) adds a changeset here, in the pull
request that makes the change. Tests, the conformance runner, the
playgrounds, docs outside the crate and CI need none.

A changeset is a Markdown file with any name, written by hand (the
Changesets CLI's `add` only knows the repository root's `.changeset/`, which
belongs to upstream Lexical's npm packages):

```md
---
'lexical-editor': patch
---

Fixed `split_text` losing a text node's format when the split was at its end.
```

- `patch` for fixes and additions, `minor` for a break (an app that built
  against the last release no longer compiles, or a behavior it relied on
  changes) while the crate is below 1.0, and `major` only from 1.0.
- The body is the changelog entry: written for someone who uses the crate,
  in the past tense, starting with what was done ("Added …", "Changed …",
  "Fixed …", "Removed …"). A break says what to change. Sections of
  `CHANGELOG.md` already released stay as they were written.

On `main`, `.github/workflows/rust-release.yml` keeps a
`chore(rust): version packages` pull request open while changesets are
pending. It runs `rust/scripts/version-packages.sh`, which consumes them,
bumps the version in `rust/lexical-editor/Cargo.toml` (and in the
lockfiles) and adds the version's section to `rust/CHANGELOG.md`, naming
the upstream version in `lexical_editor::UPSTREAM_VERSION`. Merging that
pull request is the release: the workflow publishes the mirror, the tag,
the GitHub release and the crate on crates.io.
