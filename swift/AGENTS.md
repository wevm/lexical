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
on anything neither ported nor skipped. Most `dom-only` tests should be ported
in a reworked form that asserts the model state or `exportDOM` output.

## Vectors

`packages/lexical-conformance/src/*.test.ts` run the real upstream code and
write `vectors/*.json` with `UPDATE_VECTORS=1` (`pnpm run generate`); without
it they fail when the checked-in files are stale. Generate with the Node in
`.nvmrc`. Never edit a vector by hand.

## Before pushing to main

Rebase onto `origin/main`, then run:

```sh
(cd swift && swift format lint --strict --recursive Sources Tests Package.swift \
  && swift build --build-tests && swift test --skip-build)
pnpm --filter lexical-conformance run test
pnpm --filter lexical-conformance run check-ledger
```

UI changes also run on macOS and the iOS Simulator.

## Commits

Conventional commits (`feat(swift): ...`, `test(conformance): ...`), not the
upstream PR template. Upstream syncs are merge commits named
`Merge upstream vX.Y.Z`. Add a line to `CHANGELOG.md` under `## Unreleased`
for user-facing changes.
