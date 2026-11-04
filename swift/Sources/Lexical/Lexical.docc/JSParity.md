# Parity with Lexical for JS

How the Swift port matches the JS code, and where it differs.

## Overview

The port conforms to the upstream Lexical version named in the repository's
`swift/UPSTREAM_VERSION` and in each release's changelog. JS is the
reference: a difference that isn't listed here is a bug in the port.

Parity has two tiers:

- **Tier 1, exact.** The model and the headless packages (`Lexical`,
  `LexicalExtension`, `LexicalSelection`, `LexicalUtils`, `LexicalHTML`,
  `LexicalRichText`, `LexicalPlainText`, `LexicalClipboard`,
  `LexicalHistory`, `LexicalHeadless`, `LexicalA11y`, `LexicalDevtoolsCore`).
  The same inputs give the same `EditorState`, the same selection and the
  same JSON bytes, in both the legacy and the compact forms.
- **Tier 2, behavioral.** The native views. Recorded scenarios compare the
  state and selection after each key press; the platform's own text
  behavior wins where the two differ, and each such case is listed below.

### How it's checked

The `packages/lexical-conformance` package in the repository runs the real
upstream JS and writes conformance vectors: serialized documents, scripted
editing sessions with the state after every step, seeded fuzz sessions,
HTML import and export, the devtools tree view, and keyboard scenarios
recorded in WebKit from upstream's Playwright specs. The Swift tests replay
every vector and require the same result.

A parity ledger lists every upstream unit test of each ported package. Each
one is ported to Swift, marked with `.upstream("<id>")`, or skipped with a
reason (`dom-only`, `react-only`, `out-of-scope-package`,
`covered-by-vector` or `deviation`). CI fails on any test that is neither.

### API differences

- **No `$` prefix.** Swift identifiers can't start with `$`: `$getRoot()` is
  `getRoot()`, `$createTextNode` is `createTextNode`, `$isTextNode` is
  `isTextNode`. Each declaration's documentation names its JS function.
- **Errors throw.** JS's `invariant` and dev-only checks throw a
  `LexicalError` with the same message, so most APIs are `throws`. The dev
  checks always run.
- **Offsets are UTF-16 code units**, as in JS and `NSString`.
- **Typed node state.** A `StateConfig` carries its value type.
- **Custom nodes** declare their properties with the `@LexicalNode` macro
  and `@Field`, which generate the same schema JS's `$config` builds. A
  node that overrides `getTextContent` must also override
  `getTextContentSize`, since Swift can't detect the override as JS does.
- **Listeners that clean up.** A listener closure that returns a clean-up
  function spells its type: `{ _ -> ListenerCleanup? in ... }`.
- **The root element is the native view**, passed to `setRootElement`.
  There is no DOM; the parts of the DOM reconciler the model depends on
  (mutation listeners, cached text, block text format) run without it.

### The Tier 1 exception: unpaired surrogates

A JS string can hold half of a surrogate pair, which an offset inside an
emoji leaves behind. A Swift `String` can't, so each lone half becomes
U+FFFD. Lengths and offsets still agree with JS, since U+FFFD is one UTF-16
unit; only the text differs. Native input never splits a pair.

### Tier 2 deviations

Listed with their category in the repository's `swift/parity/deviations.json`:

- **Input.** Every typed character goes through
  `CONTROLLED_TEXT_INSERTION_COMMAND`, and IME marked text is written
  through the model, because the views never edit their own text. A
  replacement before the caret (autocorrection, Writing Tools) keeps the
  caret where it was, as the system text views do. Forward delete
  dispatches `DELETE_CHARACTER_COMMAND`. On iOS, a hardware Tab no handler
  takes inserts a tab. A selection made by a drag, tap, click or program is
  forward.
- **Selection.** Select All from a menu dispatches `SELECT_ALL_COMMAND`. A
  caret on the boundary of two text nodes takes the format of the node
  before it. Up and Down arrows keep the horizontal position in points, so
  where they land depends on font metrics. Option-arrow with a range
  selected moves a word from the range's edge.
- **Clipboard.** RTF without HTML is converted to HTML before
  `PASTE_COMMAND`. Lexical's own clipboard data travels as the pasteboard
  type `dev.lexical.editor`. A drag moved into another editor is removed
  from the source by the source view.
