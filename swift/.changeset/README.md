# Changesets

Changesets version the Swift package. Each user-facing change to `swift/`
(what an app that depends on `lexical-swift` sees: API, behavior, types or
supported platforms) adds a changeset here, in the pull request that makes
the change. Tests, the playground, examples, docs outside the package and CI
need none.

A changeset is a Markdown file with any name, written by hand (the
Changesets CLI's `add` only knows the repository root's `.changeset/`, which
belongs to upstream Lexical's npm packages):

```md
---
'lexical-swift': patch
---

Fixed `LexicalList` losing a check item's state when the item was split.
```

- `patch` for fixes and additions, `minor` for a break (an app that built
  against the last release no longer compiles, or a behavior it relied on
  changes) while the package is below 1.0, and `major` only from 1.0.
- The body is the changelog entry: written for someone who uses the
  package, in the past tense, starting with what was done ("Added …",
  "Changed …", "Fixed …", "Removed …"). A break says what to change.
  Sections of `CHANGELOG.md` already released stay as they were written.

`swift/scripts/api-breaks.sh` fails CI on a public API break since the last
release unless a pending changeset bumps by a `minor` (below 1.0) or more.

On `main`, `.github/workflows/swift-release.yml` keeps a
`chore(swift): version packages` pull request open while changesets are
pending. It runs `swift/scripts/version-packages.sh`, which consumes them,
bumps `swift/VERSION` and adds the version's section to
`swift/CHANGELOG.md`, naming the upstream version in `UPSTREAM_VERSION`.
Merging that pull request is the release: the workflow publishes the mirror,
the tag and the GitHub release.
