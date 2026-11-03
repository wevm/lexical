# Design

The source of truth for how lexical-swift is built. Decisions were settled in
an interview with the maintainer (Q1–Q48 below refer to it); changing one
means updating this file in the same commit.

## Goals

- A pure-Swift port of Lexical for iOS 27 and macOS 27, with UIKit, AppKit,
  and SwiftUI editor views, so native apps behave like a Lexical web app.
- Open source, community quality; not meant to be upstreamed (Q1).
- Conforms to the upstream code on this fork's main, whose version is
  recorded in `UPSTREAM_VERSION`. The fork started from upstream main just
  after 0.53.0 (at 2b388b92d), so the first release targets the next upstream
  release. From then on only upstream release tags are merged, as
  `Merge upstream vX.Y.Z` commits; nightlies are ignored (Q6, Q22, Q44).

## Parity

- **Tier 1, exact:** the model and headless packages. Same inputs, same
  EditorState, same JSON bytes in both the legacy and compact forms (Q2, Q8).
- **Tier 2, behavioral:** the native editing surface. Checked by scenarios
  recorded from the upstream playground, compared as EditorState JSON plus
  selection in model terms (Q2, Q16).
- **JS is the reference** (Q5). A difference is a Swift bug unless it's a
  Tier 2 case listed in `parity/deviations.json` with a category:
  `platform-input`, `platform-selection`, `platform-clipboard`,
  `native-only`, or `webkit-bug` (Q45).

## Conformance tooling

- **Vectors** (Q25): `packages/lexical-conformance` runs the real upstream
  sources in Node and writes `vectors/*.json` (`{formatVersion, kind,
  description, cases}`, one case per line). CI fails when they're stale.
  Fuzz vectors use fixed seeds in CI; a local macOS-only JavaScriptCore mode
  runs long random-seed fuzzing. The Swift suites skip on the mirror, which
  doesn't carry the vectors.
- **Parity ledger** (Q15, Q34): `parity/upstream-tests.json` lists every
  upstream unit test of the packages in `parity/packages.json`. Each is
  ported (`@Test(.upstream("<id>"))`) or skipped in `parity/skips.json` with a
  reason: `dom-only`, `react-only`, `out-of-scope-package`,
  `covered-by-vector`, `deviation`. Packages marked `enforced` fail CI on
  anything neither ported nor skipped.
- **Tier 2** (Q16, Q29, Q35): upstream Playwright specs translated into a
  neutral scenario format, keeping their names. Expected results are recorded
  in WebKit with Apple platform flags; a Swift runner replays them on the real
  text view through `UITextInput` / `NSTextInputClient`. A small XCUITest set
  covers real keyboard, IME, dictation, and Writing Tools. v0.1.0 covers every
  in-scope rich-text and plain-text spec.
- **Benchmarks** (Q32): reported, not gating, until stable. Targets: under
  8 ms p95 keystroke-to-reconcile on 5,000 paragraphs, under 300 ms load.

## Architecture

- **Core API mirrors JS** (Q7): `$getRoot()`, `getWritable()`, and the rest,
  so ported code diffs cleanly against upstream. An idiomatic facade sits on
  top for app developers. Internals are `package`, not `public`.
- **Offsets are UTF-16 code units** everywhere (Q26).
- **Threading** (Q27): the core `Editor` is non-`Sendable` and bound to the
  thread or actor that created it; `$functions` find the active editor through
  a task-local or thread-local. Only the UI modules are `@MainActor`.
- **Linux** (Q24): every non-UI module builds and tests on Linux.
- **Nodes** (Q30): a `@LexicalNode` macro generates `clone`, JSON import and
  export in both forms with schema key order, and accessors. Built-in nodes
  use it too.
- **Composition** (Q18): the upstream extension API (`defineExtension`).
- **HTML and clipboard** (Q17): a Swift DOM abstraction over a pure-Swift HTML
  parser, byte-exact HTML export, and a clipboard carrying
  `application/x-lexical-editor`, HTML, and plain text.
- **Rendering** (Q11, Q12): one TextKit 2 text view per editor. The
  reconciler targets a protocol, not the view. Thin `UITextView` and
  `NSTextView` subclasses, a SwiftUI `LexicalEditor` wrapping them, and
  DecoratorNodes rendered as SwiftUI views.
- **Input is model-first** (Q28, Q40): every native input becomes the command
  JS would dispatch (text input, NSResponder selectors, paste, drop,
  dictation, spellcheck, autocorrect, Writing Tools, text replacement). The
  text view never edits its own content; IME marked text commits through the
  `compositionend` path.
- **Theme** (Q13): keys match JS `EditorThemeClasses`.
- **Accessibility** (Q41): VoiceOver, Dynamic Type, RTL and `direction`, full
  keyboard access on macOS.
- **Collaboration** (Q14, Q20): deferred. NodeKeys, update tags, and dirty
  sets match JS, and History is replaceable, so a binding can come later.

## Scope of v0.1.0 (Q21)

`Lexical`, `LexicalExtension`, `LexicalRichText`, `LexicalPlainText`,
`LexicalHistory`, `LexicalSelection`, `LexicalUtils`, `LexicalHeadless`,
`LexicalHTML`, `LexicalClipboard`, `LexicalUIKit`, `LexicalAppKit`,
`LexicalSwiftUI`, and `apps/lexical-playground-swift`, laid out like the
upstream playground (Q31, Q39). Then `LexicalList`, `LexicalLink`,
`LexicalMarkdown`.

## Milestones (Q33)

- **M0** scaffold: package, conformance package, ledger, CI, mirror release.
- **M1** Tier 1 core, plus a thin typeable slice on both platforms.
- **M2** Tier 1 packages.
- **M3** native surface.
- **M4** Tier 2, playground, benchmarks, accessibility.
- **M5** v0.1.0, then list, link, markdown.

## Repos and releases

- Work happens in `swift/` of [wevm/lexical](https://github.com/wevm/lexical),
  a fork of meta/lexical. [wevm/lexical-swift](https://github.com/wevm/lexical-swift)
  is a read-only mirror filled by `git subtree split --prefix=swift` (Q9, Q19).
- `.github/workflows/swift-release.yml` publishes when `VERSION` names an
  unreleased version, after approval in the `swift-release` environment. It
  gets a mirror token from GitHub STS, authorized by
  `.github/sts/mirror.sts.yaml` on the mirror's main (Q36). The first mirror
  push is made by hand, because the policy must already be there.
- Independent semver from 0.1.0. Release notes come from `CHANGELOG.md` and
  name the upstream version (Q23).
- Until v0.1.0, commits go straight to main after the checks in AGENTS.md;
  afterwards, PRs and Changesets (Q38, Q42, Q43).
- MIT, keeping Meta's notice (Q47).
