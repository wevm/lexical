# Swift port

Guidance for `swift/` and `packages/lexical-conformance/`. The root AGENTS.md
describes upstream Lexical; where they differ, this file wins for the Swift
port. Read [DESIGN.md](DESIGN.md) before changing architecture.

## Rules

- **JS is the reference.** A difference from upstream is a Swift bug. Only UI
  behavior (Tier 2) may deviate, and only with an entry in
  `parity/deviations.json`.
- **Port, don't invent.** Internal code mirrors the upstream source file by
  file and function by function (`$getRoot()`, `getWritable()`, ...), so a
  diff against JS is reviewable. Idiomatic API lives in a thin facade.
- **Offsets are UTF-16 code units**, as in JS and NSString.
- **JSON is byte-exact** in both the legacy and compact forms.
- The non-UI modules must build on Linux. Never import UIKit, AppKit, or
  SwiftUI outside the UI modules.
- Internals are `package`, not `public`.

## Parity ledger

Every upstream unit test of a package in `parity/packages.json` is either
ported, with `@Test(.upstream("<id>"))` (ids are in
`parity/upstream-tests.json`; write each id as one string literal, since the
ledger reads only literals), or listed in `parity/skips.json` with a reason:
`dom-only`, `react-only`, `out-of-scope-package`, `covered-by-vector`, or
`deviation`. A package is marked `"enforced"` once it is ported; CI then fails
on anything neither ported nor skipped. An `out-of-scope-package` skip lists
the directory names of the packages it needs in `packages`; once all of them
are enforced, the ledger fails until the test is ported with them. Most
`dom-only` tests should be ported
in a reworked form that asserts the model state or `exportDOM` output.

## Vectors

`packages/lexical-conformance/src/*.test.ts` run the real upstream code and
write `vectors/*.json` with `UPDATE_VECTORS=1` (`pnpm run generate`); without
it they fail when the checked-in files are stale. Generate with the Node in
`.nvmrc`. Never edit a vector by hand.

## Pull requests

Changes reach main only through pull requests; nothing is pushed to main
directly. Before opening or updating one, rebase onto `origin/main` and run:

```sh
(cd swift && swift format lint --strict --recursive Sources Tests Benchmarks Package.swift \
  && swift build --build-tests && swift test --skip-build)
pnpm --filter lexical-conformance run test
pnpm --filter lexical-conformance run check-ledger
npx tsc -p ./tsconfig.build.json --noEmit   # after changing lexical-conformance
```

UI changes also run on macOS and the iOS Simulator.

- One change per pull request, titled as a conventional commit
  (`feat(swift): ...`, `fix(swift): ...`, `test(conformance): ...`), not the
  upstream PR template. The body says what changes and how it was checked.
- Subscribe to the pull request's CI and its comments. No human review is
  required: the author merges once every check is green, and fixes or
  answers comments first. Never merge red.
- **Squash-merge**, so each change is one commit on main.
- **Upstream syncs** are the exception: a branch with a real merge commit
  named `Merge upstream vX.Y.Z`, merged with a merge commit (never squashed),
  so upstream's history stays an ancestor of main.

## Changesets

A user-facing change to the package (API, behavior, types, platforms) adds
a changeset in `swift/.changeset/` in the same pull request; its README has
the format. A break needs a `minor` changeset below 1.0. Tests, the
playground, examples and CI need none. Don't edit `swift/CHANGELOG.md` or
`swift/VERSION` by hand.

## Releases

While changesets are pending, the Swift Release workflow keeps a
`chore(swift): version packages` pull request open, made by
`swift/scripts/version-packages.sh`, and starts the Swift CI on it. Merging
it (squash, once green, like any other) is the release: the workflow
publishes the mirror, the tag and the GitHub release from that version's
`CHANGELOG.md` section. The `swift-release` environment only limits
deployments to `main`; no one approves them.
