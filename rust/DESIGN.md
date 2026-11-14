# Design

How the Rust port of Lexical is built and what comes next. It follows the
Swift port (`swift/DESIGN.md`) and wevm/zero's Rust client (`rust/` in
wevm/zero). Changing a decision here means updating this file in the same
pull request.

## Goals and scope

- A pure-Rust port of Lexical's model and headless packages: **Tier 1
  only**. The same inputs give the same editor state and the same JSON bytes,
  in both the legacy and compact forms, the same HTML and the same Markdown.
- The editing UI (Swift's Tier 2: views, input, the reconciler's rendering
  half, the WebKit scenarios) is out of scope.
- **JS is the reference.** A difference from upstream is a Rust bug, except
  the documented exceptions in [PARITY.md](PARITY.md).
- Conforms to the upstream release in `lexical_editor::UPSTREAM_VERSION`,
  which must match the vectors' `meta.json` (checked by the conformance
  runner).

## Layout

| Path                  | Crate                 | Published | What                                                      |
| --------------------- | --------------------- | --------- | --------------------------------------------------------- |
| `rust/lexical-editor` | `lexical-editor`      | not yet   | The port: core, and one feature per upstream package      |
| `rust/conformance`    | `lexical-conformance` | never     | The conformance runner: replays every vector file         |
| `rust/apps/lexical-playground-core` | `lexical-playground-core` | never | The playgrounds' binding to `lexical-editor` |
| `rust/apps/lexical-playground-rune` | `lexical-playground-rune` | never | The playground on Rune (a workspace of its own) |
| `rust/apps/lexical-playground-gpui` | `lexical-playground-gpui` | never | The playground on GPUI (a workspace of its own) |

`rust/` is developed in wevm/lexical, with its own CI
(`.github/workflows/rust.yml`), so it can later be mirrored to
wevm/lexical-rust on release the way `swift/` is mirrored to
wevm/lexical-swift (see Releases).

### The name

tmm: "let's use lexical-editor as the name right now. We might change it
later before we publish." So nothing is published, and the name lives in
few places: `name` in `lexical-editor/Cargo.toml`, the `package` of the
`lexical` entry in the workspace's `[workspace.dependencies]`, the
directory, the doc example in `lib.rs`, and the `cargo tree` check in
`rust.yml`. Every other crate in the workspace depends on it as `lexical`,
so a rename doesn't touch their code.

## Packages and features

One crate, with a feature per upstream package, as `zero-client` does. The
core (`lexical`) is always built. Planned features, each enabling those it
depends on as the upstream package depends on them:

| Feature     | Upstream package       | Needs                      |
| ----------- | ---------------------- | -------------------------- |
| (core)      | `lexical`, `@lexical/headless`, `@lexical/extension` | |
| `selection` | `@lexical/selection`   |                            |
| `utils`     | `@lexical/utils`       | `selection`                |
| `rich-text` | `@lexical/rich-text`   | `selection`, `utils`       |
| `plain-text`| `@lexical/plain-text`  | (upstream lists `clipboard`, `dragon`, `extension`, `selection` and `utils`; the model handlers use none of them) |
| `list`      | `@lexical/list`        | `utils` (upstream also lists `extension` and `html`, for `ListExtension` and the DOM import, which aren't ported) |
| `link`      | `@lexical/link`        | `utils` (upstream also lists `extension` and `html`, for `LinkExtension` and the DOM import, which aren't ported) |
| `autolink`  | `@lexical/link`'s `registerAutoLink` | `link`, `regex` |
| `code`      | `@lexical/code-core`   | (upstream lists `extension`, `html` and `utils`, for the extensions, the DOM import and the arrow-key handlers, which aren't ported) |
| `overflow`  | `@lexical/overflow`    |                            |
| `text`      | `@lexical/text`        |                            |
| `hashtag`   | `@lexical/hashtag`     | `text` (upstream also lists `extension` and `utils`, for `HashtagExtension` and `mergeRegister`, which aren't ported) |
| `history`   | `@lexical/history`     | (upstream lists `utils`, but uses nothing from it) |
| `html`      | `@lexical/html`        | `selection`                |
| `markdown`  | `@lexical/markdown`    | `rich-text`, `list`, `link`, `code` |
| `file`      | `@lexical/file`        |                            |
| `yjs`       | `@lexical/yjs`         | `selection`, `yrs`         |

Each module is `lexical_editor::<package>` (`lexical_editor::rich_text`).
Names may change as packages land; this table is updated with them.

## Architecture

- **No third-party dependencies** in the default build: `cargo tree -e
  normal,build` of `lexical-editor` shows the crate alone, with the default
  features and with every feature but `autolink`, and CI fails otherwise. A
  dependency comes only behind an opt-in feature: `autolink` needs `regex`
  (with `regex-automata` and `regex-syntax`, which CI checks are all it
  brings), for `registerAutoLink`'s matchers, whose `\p{L}` and `\p{N}`
  need Unicode tables; `yjs` will need `yrs`. `@lexical/link`'s nodes and
  `registerLink` stay in `link`, without it. The crate has its own JSON (`json`), because the output must
  be byte-identical to `JSON.stringify` (JS property order, `1e+21`, `-0`
  as `0`), which `serde_json` doesn't promise.
- **Port, don't invent.** Modules mirror the upstream files
  (`LexicalGeneratedJSON.ts` is `generated_json.rs`, `nodes/LexicalTextNode.ts`
  is `nodes/text.rs`), and functions keep their JS names in snake case
  without the `$`: `$getRoot()` is `get_root()`. Doc comments name the JS
  function in backticks so a search for the JS name finds the port.
- **`$` functions read a thread-local active editor state**, as JS reads
  module-scope variables (`updates.rs`), and only work inside a read or an
  update. The editor is neither `Send` nor `Sync`.
- **Nodes.** JS's class hierarchy is the `LexicalNode` trait plus the data
  each base class adds (`ElementData`, `TextData`). A node is a value behind
  an `Rc` in its editor state's node map; `getWritable` clones it unless
  nothing else shares it (`utils::get_writable`), so an earlier editor state
  never sees a change. The `Rc`'s uniqueness stands in for JS's
  `_cloneNotNeeded`. `getWritable` also marks the node dirty, as in JS, and
  helpers that take "writable" nodes expect their callers to have called it
  in JS's order, because update listeners see nodes in the order they were
  marked. Code outside a node reads it through `NodeRef`, a key
  that resolves the latest version on every call, as JS node methods call
  `getLatest()`. Base class behavior a subclass overrides with a `super`
  call is a free function in the base class's module.
- **Schemas.** JS derives `exportJSON` and `updateFromJSON` from each
  node's `$config` schema and checks in generated code for the core nodes
  (`LexicalGeneratedJSON.ts`). The core nodes port that generated code. A
  general schema facility (`nodeSchema`, `withField`, `withAccessors`, a
  derive macro in place of Swift's `@LexicalNode`) comes when the first node
  package needs it (M2); until then a node implements `export_json` and
  `update_from_json` by hand.
- **Numbers a document can set are `f64`.** JS stores whatever finite number
  a document carries in `format`, `detail`, `indent` and `textFormat`, and
  writes it back. Rust keeps the same `f64`, so unlike Swift (which keeps an
  `Int`, its `model/integer-properties` deviation) such documents need no
  exception. Bitwise reads go through `ToInt32` as in JS.
- **Offsets are UTF-16 code units**, as in JS, but a Rust `String` is UTF-8.
  The model stores text as `String` and converts offsets where they are used
  (`splitText`, `spliceText`, selection points), with ASCII as the fast path.
  If the M1 benchmarks show the conversion matters, text nodes cache their
  UTF-16 length. A lone surrogate half is the one Tier 1 exception, inherited
  from Swift (PARITY.md).
- **Errors.** A JS `invariant` a document or a caller can break returns a
  `LexicalError` with the same message; one only a bug in a node
  implementation can break panics, as `RefCell` does. A failed update
  returns its error from `Editor::update` (JS reports it to `onError`) after
  discarding the update and notifying listeners, as JS does.
- **The update cycle** (`updates.rs`) follows `$beginUpdate` and
  `$commitPendingUpdates` line by line: nested and queued updates,
  normalization, transforms (leaves first, the root last), GC, the
  selection check, discarding a failed update, and the text content and
  update listeners. The dirty sets are insertion-ordered like JS's `Map` and
  `Set` (`ordered.rs`), including iteration that sees entries added
  meanwhile. Without a DOM nothing reconciles, so mutation listeners and
  selection-change notifications never fire, as in JS's headless editor.
  JS commits a non-discrete update in a microtask; with no event loop, Rust
  commits it at the next `read` or discrete update, which JS also does when
  one comes first. Update closures that may be queued are `'static`.

## Conformance

`cargo run -p lexical-conformance` replays every file in
`packages/lexical-conformance/vectors` and prints a table: cases per file,
how many match JS exactly, how many match only under a documented
exception, how many wait on a package not ported yet, and how many fail.

- Every vector file is listed. A file no suite replays yet is reported as
  not implemented, and a case that needs an unported package is counted
  apart, never as passing.
- Each case is checked alone; a failing or panicking case is counted and
  the rest still run. CI fails on any failing case. `cargo test -p
  lexical-conformance` asserts the same, and checks that doctored vectors
  fail.
- Vectors are never edited or skipped to get to green, and never generated
  by Rust: `packages/lexical-conformance` runs the real upstream code.
  New vectors the port needs (for `@lexical/yjs`, say) are added there, in
  JS, as the Swift port adds its own.
- The mirror has `rust/` without the vectors, so there the conformance tests
  skip; `LEXICAL_CONFORMANCE_VECTORS` points the runner elsewhere.

### Parity ledger (planned for M1)

Swift's ledger (`swift/parity/`, `@Test(.upstream(...))`) is Swift's own.
The Rust counterpart: `rust/parity/packages.json` (which packages are
enforced) and `rust/parity/skips.json` (with Swift's reasons), the same
`upstream-tests.json` list (generated by `packages/lexical-conformance`'s
`ledger.mjs`, extended to read Rust), and each ported test marked with a
`// upstream: <id>` line just above its `#[test]`, which the ledger script
reads (no macro, so nothing in the crate depends on the ledger). CI fails
on a test of an enforced package that is neither ported nor skipped, as for
Swift.

## Milestones

Each milestone ends with green CI and a report to the Space.

| Milestone | Scope | Vectors that pass when done | Status |
| --- | --- | --- | --- |
| M0 | The `rust/` workspace, JSON as JS writes it, the core nodes (root, paragraph, text, tab, line break), `parseEditorState` and `toJSON` in both forms, the conformance runner, CI | `meta`, `serialization/core`, the core-only cases of `property/documents` (JSON and text) | done |
| M1 | The core model: `editor.update` and the commit (dirty nodes, normalization, transforms, listeners, commands, tags), node creation and methods (`splitText`, `mergeWithSibling`, `spliceText`, `insertAfter`, `remove`, `replace`...) with UTF-16 offsets, `RangeSelection` and `NodeSelection` editing, carets, NodeState, slots, the schema facility, `DecoratorNode`, the extension API (`buildEditorFromExtensions`), the parity ledger | `scripts/*`, `fuzz/core`, `file/*` | in progress: the update cycle, `setEditorState`, commands, transforms, listeners, the node methods, carets, selection reading and `RangeSelection`/`NodeSelection` editing are done (`scripts/*` and `fuzz/core` pass), and the platform's `Selection.modify` is an editor hook (`SelectionMovement`); NodeState is done (`createState`, `$getState`, `$setState`, `$getStateChange`, the `"$"` key and flat states in node JSON, unknown keys kept, clones, `$copyNode`, splits and merges, `scripts/node-state`), except its schema builders, which come with the schema facility; `DecoratorNode` is done (its hooks, block and inline decorators in `insertNodes`, deletion, `modify`, text content and formatting, `$setBlocksType` across one, `scripts/decorator`), except `editor._decorators` and the decorator listener, which the DOM reconciler fills; next, the extension API |
| P | Desktop playgrounds (`apps/`): `lexical-playground-rune` on Rune, then `lexical-playground-gpui` on GPUI, sharing `lexical-playground-core`; they grow with each milestone below | the vectors, replayed in the app; a session exported from the app replays in JS (`replay.test.ts`); in CI, a GPUI session typed in a real window replays in JS | both apps run on macOS with typing, Return, deletion, formatting, input methods and the Parity panel; they grow with each milestone |
| M2 | `rich-text` (headings, quotes, and the command handlers a text surface dispatches: typing, Enter, Backspace, formatting) and `history` (undo), which the playgrounds need first; then the node packages `list`, `link`, `code`, `overflow`, `hashtag`, with `selection` and `utils` | `serialization/*`, `property/documents` (JSON and text), `entities/hashtag-*`, `scripts/history` | done (every vector it names passes): `history` is done (`scripts/history` passes); `rich-text`'s nodes and `registerRichText`'s model command handlers are done (`scripts/rich-text` and `scripts/rich-text-commands` pass, with the heading and quote cases of `property/documents`); `plain-text`'s model command handlers are done (`scripts/plain-text` passes); `list`'s nodes, `registerList` and the model part of `registerCheckList` are done (`scripts/list` and `serialization/list` pass, with the list cases of `property/documents`); `link`'s nodes, `$toggleLink` and `registerLink`'s model handlers are done (`scripts/link` and `serialization/link` pass, with the link cases of `property/documents` that need no other package), `registerAutoLink` and its matchers are done behind `autolink` (`scripts/autolink` and `entities/autolink-match` pass); `selection`'s `$setBlocksType` is done (`scripts/set-blocks-type` passes); `code`'s nodes, `FlatStructureUtils.ts`, `CodeExtension`'s transform and `registerCodeIndentation`'s model handlers are done (`scripts/code` and `serialization/code` pass, with the code cases of `property/documents` that need no other package); `overflow`'s `OverflowNode` is done (`scripts/overflow` and `serialization/overflow` pass, with the overflow cases of `property/documents` that need no other package); `hashtag`'s `HashtagNode`, its matcher (generated character tables, no `regex`) and `registerLexicalHashtag`, with `text`'s `registerLexicalTextEntity`, are done (`entities/hashtag-match`, `entities/hashtag-documents` and `entities/hashtag-typing` pass, with the last waiting cases of `property/documents`), so M2's node packages are done |
| M3 | The commands `property/sequences` dispatches | `property/sequences` | |
| M4 | `html`: a DOM abstraction over an HTML parser, CSS declarations, export and import | `html/*`, `css/*`, `property/documents` (HTML) | |
| M5 | `markdown` | `markdown/import-export`, `property/documents` (Markdown), `scripts/markdown-shortcuts` | done: import, export, the transformers and `registerMarkdownShortcuts` (`markdown/import-export`, the Markdown of `property/documents` and `scripts/markdown-shortcuts` pass) |
| M6 | `yjs`: a port of `@lexical/yjs` on `yrs` (behind the feature), with new JS vectors for the binding (sevki/lexical-rs's `lexical-yjs` and jpcamara/yrby's `lexical-yjs-html` only as cross-checks) | the new collab vectors | |
| M7 | Releases: the mirror, changesets, a name tmm settles | | |

The `devtools/tree` vectors come with whichever milestone a tool needs
them. The `scenarios/*` vectors, recorded from a real browser, need a text
layout behind `Selection.modify`; the playgrounds' binding replays them once
rich-text is in.

## Playgrounds

tmm asked for desktop playgrounds, as wevm/zero has zbugs on GPUI and on
Rune, rather than a web one: the Rust port is meant for native apps. Each
app is its own Cargo workspace under `apps/` (Rune needs Rust 1.90 or
later; the crate stays on 1.88), and both depend on
`apps/lexical-playground-core`, which is in this workspace:

- Lexical owns the document and the selection; the app draws them, lays
  them out, hit-tests and moves the caret, as a browser does for
  `contenteditable` and TextKit does for the Swift port.
  - **Rune:** its `text_input` draws a mirror of the document. Keys that
    edit run as Lexical updates before the field sees them, and a change
    the field makes itself (an input method's commit, a cut) is replayed
    into Lexical as the text it replaced; then the mirror is rebuilt, as
    Lexical's reconciler rebuilds the DOM.
  - **GPUI:** an element of its own draws the document, and is the window's
    text input client (`EntityInputHandler`) through `Surface` in the core
    crate, which maps UTF-16 ranges to Lexical points and keeps an input
    method's composition outside Lexical until it commits, as a browser
    does. Keys are GPUI actions bound as macOS binds them.
  - Deleting by character, word or line extends the selection in the core
    crate (graphemes, words, the paragraph) before the step runs, so the
    step is what JS runs once the browser's `Selection.modify` has moved.
    `SelectionMovement` lets an app answer that from its own layout instead.
- Every edit is a step of the conformance script language, run by the same
  interpreter that replays `scripts/*`. Export session writes the steps
  and their results as a vector file that JS replays and compares
  (`packages/lexical-conformance/src/replay.test.ts`); Replay vectors runs
  the conformance report in the app.
- The GPUI app has an end-to-end mode for CI (`--features e2e`), as
  zbugs-gpui does: it types, composes and deletes in its own window through
  the input handler and key bindings, then exports the session, which JS
  replays in the same job.
- To try one: `cargo run --release` in its directory. On macOS a
  `bundle.sh` runner makes a `.app` that stays in the Dock, as zbugs' do.

## Releases (planned for M7)

Like the Swift port: a Changesets-versioned `rust/`, a `rust-release.yml`
that, on a merged version pull request, pushes a subtree split of `rust/` to
wevm/lexical-rust (github-sts, as `swift/` does), tags it and creates a
GitHub release. crates.io's first publish is by hand by tmm, after the name
is settled; nothing is published before then.
