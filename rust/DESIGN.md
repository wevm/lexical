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
| `plain-text`| `@lexical/plain-text`  | `selection`, `utils`       |
| `list`      | `@lexical/list`        | `selection`, `utils`       |
| `link`      | `@lexical/link`        | `utils`                    |
| `code`      | `@lexical/code-core`   | `utils`                    |
| `overflow`  | `@lexical/overflow`    |                            |
| `hashtag`   | `@lexical/hashtag`     | `utils`                    |
| `history`   | `@lexical/history`     | `utils`                    |
| `html`      | `@lexical/html`        | `selection`                |
| `markdown`  | `@lexical/markdown`    | `rich-text`, `list`, `link`, `code` |
| `file`      | `@lexical/file`        |                            |
| `yjs`       | `@lexical/yjs`         | `selection`, `yrs`         |

Each module is `lexical_editor::<package>` (`lexical_editor::rich_text`).
Names may change as packages land; this table is updated with them.

## Architecture

- **No third-party dependencies** in the default build: `cargo tree -e
  normal,build` of `lexical-editor` shows the crate alone, and CI fails
  otherwise. A dependency comes only behind an opt-in feature (`yjs` will
  need `yrs`). The crate has its own JSON (`json`), because the output must
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
| M1 | The core model: `editor.update` and the commit (dirty nodes, normalization, transforms, listeners, commands, tags), node creation and methods (`splitText`, `mergeWithSibling`, `spliceText`, `insertAfter`, `remove`, `replace`...) with UTF-16 offsets, `RangeSelection` and `NodeSelection` editing, carets, NodeState, slots, the schema facility, `DecoratorNode`, the extension API (`buildEditorFromExtensions`), the parity ledger | `scripts/*`, `fuzz/core`, `file/*` | in progress: the update cycle, `setEditorState`, commands, transforms, listeners and the node methods are done (`scripts/updates`, `commands` pass; `listeners`, `transforms` pass but for the cases that type); next, `RangeSelection` editing and carets |
| M2 | Node packages: `rich-text`, `list`, `link`, `code`, `overflow`, `hashtag`, with `selection` and `utils` | `serialization/*`, `property/documents` (JSON and text), `entities/hashtag-*` | |
| M3 | `history`, and the commands `property/sequences` dispatches | `property/sequences` | |
| M4 | `html`: a DOM abstraction over an HTML parser, CSS declarations, export and import | `html/*`, `css/*`, `property/documents` (HTML) | |
| M5 | `markdown` | `markdown/import-export`, `property/documents` (Markdown) | |
| M6 | `yjs`: a port of `@lexical/yjs` on `yrs` (behind the feature), with new JS vectors for the binding (sevki/lexical-rs's `lexical-yjs` and jpcamara/yrby's `lexical-yjs-html` only as cross-checks) | the new collab vectors | |
| M7 | Releases: the mirror, changesets, a name tmm settles | | |

The `devtools/tree` vectors come with whichever milestone a tool needs
them; the `scenarios/*` vectors are Tier 2 and stay unreplayed.

## Releases (planned for M7)

Like the Swift port: a Changesets-versioned `rust/`, a `rust-release.yml`
that, on a merged version pull request, pushes a subtree split of `rust/` to
wevm/lexical-rust (github-sts, as `swift/` does), tags it and creates a
GitHub release. crates.io's first publish is by hand by tmm, after the name
is settled; nothing is published before then.
