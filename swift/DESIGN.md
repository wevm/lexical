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
  `native-only`, or `webkit-bug` (Q45), or the Tier 1 exception below.

### The Tier 1 exception: unpaired surrogates

Settled by tmm after M1. A JS string can hold half of a surrogate pair,
which an offset inside an emoji leaves behind (`splitText`, `removeText`, a
selection there). A Swift `String` cannot: it stores Unicode scalars, so each
lone half becomes U+FFFD. U+FFFD is also one UTF-16 unit, so lengths and
offsets still agree with JS; only the text differs, and two halves merged
back stay two U+FFFD instead of re-forming the character. This is the one
allowed Tier 1 difference. Closing it would mean storing text as UTF-16
arrays under the String API, a cost on every text API for a state no real
editing produces (native input never splits a pair). Upstream tests or
vectors that depend on a lone surrogate are skipped as `deviation`, with a
note pointing here; fuzz vectors keep offsets off pair interiors.

## Conformance tooling

- **Vectors** (Q25): `packages/lexical-conformance` runs the real upstream
  sources in Node and writes `vectors/*.json` (`{formatVersion, kind,
  description, cases}`, one case per line). CI fails when they're stale.
  Fuzz vectors use fixed seeds in CI; a local macOS-only JavaScriptCore mode
  runs long random-seed fuzzing. The Swift suites skip on the mirror, which
  doesn't carry the vectors.
- **Script vectors** (decided during M1): `vectors/scripts/*.json` are editor
  sessions written in a small JSON action language (`src/ops.ts`; node and
  selection methods by name, serialized nodes, named transforms and command
  handlers). JS records the state in both forms, selection, text and a log
  of transforms, handlers and listeners after every step; the Swift runner
  (`Tests/ConformanceTests/Ops.swift`) interprets the same scripts and must
  record the same. Fuzz vectors are scripts generated from fixed seeds. A
  headless JS editor has no window for `Selection.modify`, so scripts only
  delete within text where the model resolves it.
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

## Port conventions (decided during M1, for tmm to review)

These are reversible choices made while porting, under Q7 and Q27.

- **No `$` prefix.** Swift identifiers cannot start with `$`, so
  `$getRoot()` is `getRoot()`, `$createTextNode` is `createTextNode`,
  `$isTextNode` is `isTextNode`, `$create` is `create`. Doc comments name the
  JS function in backticks so a search for the JS name finds the port.
- **Errors throw.** JS `invariant` and `devInvariant` become a thrown
  `LexicalError` carrying the same message, so almost every API `throws`. Dev
  checks always run, because the vectors and tests come from the JS dev build.
- **Module state is per thread.** The JS module-scope variables (active
  editor and state, read-only mode, key and UID counters, compact export,
  export memo) live in a `ThreadContext` in thread-local storage, saved and
  restored around updates as JS does.
- **Microtasks** are a per-thread queue, drained by `drainMicrotasks()`;
  `editor.read()` force-commits as in JS.
- **Ordered collections.** `JSMap` and `JSSet` are insertion-ordered
  reference types whose iteration sees changes made during it, like JS `Map`
  and `Set`. `JSONObject` keeps key order the way a JS object does, array-index
  keys first, and holds `undefined` (`nil`), which `stringify` skips.
- **Schemas** (`SchemaField`, `NodeConfig`) are the runtime form of
  `$config`; `@LexicalNode` generates them from `@Field` properties
  (`withField`), in declaration order, with `getX`/`setX` accessors. Fields
  with custom accessors (`withAccessors`) and the built-in nodes, whose
  properties all have them, still write `SchemaField`s by hand. A schema's
  values are `JSONValue?`.
  A field's accessors are closures on its `SchemaField`, not names resolved
  by reflection, so the JS tests of accessor naming are skipped as
  deviations. `transformValue` maps JSON to JSON. Values are immutable, so
  where JS compares two containers by reference the port compares them by
  content, and freezing has nothing to do.
- **Declarations throw where they are made.** A combinator JS refuses with
  an invariant (`numberValue` with an empty domain, `enumValue([])`, a
  nested `arraySetValue`) throws `LexicalError`, so `config()` is
  `throws`. A class whose `config()` throws gets type `""`, and registering
  it throws that error.
- **NodeState** values are typed (`StateConfig<V>`). `unparse` is required
  unless the value is `JSONValueConvertible`.
- **Carets** are one `PointCaret` value whose `kind` (`sibling`, `child`,
  `text(offset:)`) is the JS caret class; `SiblingCaret`, `ChildCaret` and
  `TextPointCaret` are aliases of it.
- **Slots** keep their map in a value type (`SlotMap`), so a writable host
  never shares the frozen version's map.
- **Other packages' nodes in core tests.** Where a core test uses a node from
  another package only for its structure, the core test nodes stand in:
  `TestInlineElementNode` (inline, `canBeEmpty` false) for `LinkNode` and
  `MarkNode`, `TestElementNode` for list containers. A test that depends on
  that package's own behavior (list normalization, table shadow roots,
  `HeadingNode.insertNewAfter`) is skipped as `out-of-scope-package` until the
  package is ported.
- **Selection movement without layout.** JS moves a selection by a
  character, word or line through `Selection.modify`, after
  `$modifySelectionAroundDecoratorsAndBlocks`. Headless, and in JS under
  jsdom, only that first step runs, and the port does the same until the
  TextKit views supply the layout step. `TestLinkNode` in the tests copies
  LinkNode's core-facing overrides (`canInsertTextBefore/After`,
  `insertNewAfter`, `extractWithChild`).
- **Duck-typed JS checks become protocols.** `'__language' in node` (a code
  block, in `insertNodes`) is `CodeLanguageNode`, and
  `__isInlineFormattable` is `InlineFormattableNode`, which `TextNode`
  adopts.
- **Parameterized upstream tests** become one Swift test with `arguments:`,
  carrying one `.upstream(...)` trait per upstream case.
- **Unpaired surrogates** become U+FFFD; settled as the Tier 1 exception
  (see Parity).
- **GenMap is not ported** (settled by tmm after M1). JS keeps the node map
  in a generational `GenMap`, so cloning a large state is O(1) with copy on
  first write; the Swift node map is a `JSMap` that copies on clone. Behavior
  is the same, so the GenMap tests are deviations. Revisit only if the M4
  benchmarks (5,000 paragraphs) show the node-map copy is too slow.
- **Expectations that come from the DOM.** Upstream unit tests mount a
  `contenteditable` root, so some expected values only hold after the
  reconciler or a DOM selection round trip has run (`RootNode.__cachedText`
  is filled by the reconciler; `applyDOMRange` normalizes element points to
  text points). Where the rest of the test is about the model, the port
  asserts what the headless JS editor produces, checked by running upstream
  headless, and says so in a comment. Tests about values the reconciler
  computes (`__cachedText`, mutations) mount their editor on a test root, so
  the model reconciler runs (see below).
- **The root element is the native view.** `setRootElement(_:)` takes
  `AnyObject?` and does the model half of the JS method: reset the editor,
  commit the pending state again with `history-merge` (keeping queued
  updates and tags), and run root listeners. Styling, events and the
  mutation observer are the views' job. With a root set, a changed range
  selection dispatches `SELECTION_CHANGE_COMMAND`, as in JS, while the root
  is connected: a root that conforms to `LexicalRootElement` reports
  `isConnected` (a view: whether it is in a window), and any other root
  counts as connected. The TextKit views will call it when the incremental
  reconciler lands.
- **The reconciler's model half is in core** (`LexicalReconciler.swift`).
  While a root is attached (and the editor is not headless), each commit
  walks the previous and next trees as `$reconcileRoot` does, with a
  `ReconciledNode` in place of each keyed DOM element. It reports created,
  updated and destroyed nodes to mutation listeners, fills
  `RootNode.__cachedText`, derives `__textFormat`/`__textStyle` from each
  element's first text, collects decorators, and recovers from a failure
  with a full reconcile, as JS does. Rendering is the views' job. Two
  differences: the children fast paths (suffix splicing, the same-size
  walk) are not ported, since they only save DOM work, and every element
  other than the root takes its cached text from `getTextContent()`,
  because Swift cannot ask whether a subclass overrides it (JS does so only
  for elements that override it). Unit tests whose upstream editor is
  mounted use `createMountedTestEditor()`.
- **Overriding `getTextContent` alone.** JS gives a class that overrides
  `getTextContent` but not `getTextContentSize` a size delegated to
  `getTextContent()`. Swift cannot detect the override, so a Swift node that
  overrides `getTextContent` must override `getTextContentSize` too.
- **Listener clean-ups.** In JS any update, editable, decorator or text
  content listener that returns a function has it called before its next
  call and on teardown. Swift keeps the `Void` listener types and adds an
  `@_disfavoredOverload` of each `register…Listener` taking a closure that
  returns `ListenerCleanup?`. Plain closures resolve to the `Void` form; a
  closure that returns a clean-up spells its type
  (`{ _ -> ListenerCleanup? in … }`). Two unlabeled overloads that differ
  only by return type are otherwise ambiguous for multi-statement closures.

## Extension conventions (decided during M2, for tmm to review)

Reversible choices made porting `@lexical/extension` under Q18.

- **Extensions are classes.** `LexicalExtension<Config, Output, Init>` is a
  `final class`, because the builder tells extensions apart by identity, as
  JS does. It is `@unchecked Sendable` so an extension can be a global
  `let`, as JS defines them at module scope. `defineExtension` has an
  overload for each combination of `config`, `init` and `build` left out,
  because Swift generic parameters have no defaults. An extension with
  no config has `EmptyConfig`; no output or init is `Void`. The
  `InitialEditorConfig` fields (namespace, theme, editable, onError,
  `$initialEditorState`, …) are grouped in `editorConfig:`, and
  `nodes: { [...] }` is the deferred form.
- **Partial configs record which keys are set.** `Partial<Config>` holds
  key-path overrides, written `configExtension(Ext) { $0.disabled = true }`.
  A custom `mergeConfig` reads `partial.rules` as `nil` when the key is
  absent, as JS tests `partial.rules`. For an optional key,
  `$0.set(\.b, nil)` overrides with `nil` (JS `{b: null}`); assigning `nil`
  through the member unsets it. Peer configs name the peer's config type:
  `declarePeerDependency("Pair", config: PairConfig.self) { ... }`.
- **Signals are a port of `@preact/signals-core` 1.14.4** (`Signals.swift`,
  MIT), with the same graph and effect order; its expected order is checked
  against the real package. Values compare with `==` when `Equatable` and
  by identity for objects, where JS compares by identity, so setting an equal
  struct does not notify. Accessors can't throw, so an effect that throws
  while a `value` setter flushes is reported as uncaught; `try set(_:)`
  rethrows it. An effect body that returns a clean-up spells its type
  (`{ () -> (() -> Void)? in ... }`), like the listener clean-ups.
  `namedSignals(config)` reads as `output.disabled.value` through dynamic
  member lookup.
- **`dispose()` is on `Editor`.** JS returns `LexicalEditorWithDispose`;
  every Swift `Editor` has `dispose()`, which does nothing unless the
  builder made it. Tests write `defer { editor.dispose() }` for `using`.
- **`HMRExtension` is not ported.** It carries the editor state and history
  across a bundler's hot module replacement (`import.meta.hot`), which
  Swift has no equivalent of; its tests are `deviation` skips.
- **Extensions that only touch the DOM wait for the views.**
  `NodeSelectionDataSelectedExtension` (a host element attribute) and
  `NormalizeTripleClickSelectionExtension` (DOM event ordering) port with
  the native host views in M3; their tests are `dom-only` until then.
- **`NormalizeInlineElementsExtension` adds its transform to every element
  class.** JS adds it only to classes that override `isInline`, which Swift
  can't detect; the transform does nothing for a node that isn't inline.
- **Computed styles come from the root element.** `@lexical/selection`
  reads `getComputedStyle` of a node's DOM for `direction` and
  `writing-mode`. In Swift the root element answers through
  `ComputedStyleProvider.computedStyle(forKey:)`; the views will conform.
  Without a provider there is no computed style, as in JS for a node that
  isn't rendered. `createDOMRange` and `createRectsFromDOMRange` are not
  ported: the views measure through TextKit.
- **Style objects keep property order.** `getStyleObjectFromCSS` returns a
  `StyleObject`, ordered like a JS object's keys, so `patchStyleText`
  writes CSS in the same order as JS. A patch is `KeyValuePairs` for the
  same reason.
- **Iterators are lazy and throw.** `$firstToLastIterator`, `$dfsIterator`
  and the others re-read the tree at each step in JS, so a loop that moves
  nodes sees the moves. Swift returns a `StepwiseIterator` with a throwing
  `next()` (and `forEach`, `map`, `toArray`), not a `Sequence`, because
  `IteratorProtocol.next()` can't throw.
- **`@lexical/utils`' DOM helpers belong to the views.** `markSelection`,
  `positionNodeOnRange`, `selectionAlwaysOnDisplay`, `calculateZoomLevel`,
  `getScrollParent` and `mediaFileReader` are not in `LexicalUtils`; the
  rect filtering they share is (`filterSelectionRects`,
  `dedupeSelectionRects`), on `SelectionRect`.

- **The DOM is a small model of jsdom's.** `createDOM`, `exportDOM` and
  HTML import and export run on `DOMDocument`/`DOMElement` in core
  (Q17). Upstream's tests see jsdom, so serialization escapes as jsdom does
  and `CSSStyleDeclaration` normalizes values the way jsdom's declaration
  does for what Lexical writes: colors to `rgb()`, trimmed numbers, `0px`,
  lowercase keywords, `calc()` of a product. The `css/properties` vectors
  generate the accepted property table (`CSSProperties.swift`) and the
  `css/values` vectors pin the normalization. Unlike jsdom it does not
  reject values it can't parse; it keeps them with whitespace collapsed.
  `DOMParser` is a pure-Swift WHATWG parser checked against jsdom by the
  `html/parse` and `html/entities` vectors. It leaves out templates,
  framesets, the `<select>` and `<ruby>` insertion modes, and SVG/MathML
  name case adjustment, which pasted rich text does not need; their
  content parses as body content.
  `importDOM` is a class method returning a `DOMConversionMap`. JS skips a
  class whose static `importDOM` is inherited unchanged; Swift can't tell
  and registers it again, which changes nothing.
  `getDocument()` is one document per thread, as JS has one global
  `document`. `DOMSlot` is the base slot only: `ElementDOMSlot`'s managed
  line breaks and decorator anchors belong to the DOM reconciler, which the
  views replace.

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
