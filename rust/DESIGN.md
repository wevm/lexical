# Design

How the Rust port of Lexical is built and what comes next. It follows the
Swift port (`swift/DESIGN.md`) and wevm/zero's Rust client (`rust/` in
wevm/zero). Changing a decision here means updating this file in the same
pull request.

## Goals and scope

- A pure-Rust port of Lexical's model and headless packages: **Tier 1
  only**. The same inputs give the same editor state and the same JSON bytes,
  in both the legacy and compact forms, the same HTML and the same Markdown.
- **Swift's feature set first.** tmm's target is parity with what the
  Swift port supports; Swift and Rust move toward the rest of JS later, as
  apps need it. So `@lexical/yjs`, `@lexical/y`, `@lexical/code-shiki`,
  `@lexical/mdast` and `@lexical/offset` are not planned: Swift has none of
  them either. `@lexical/table` was on that list until an app needed tables
  (wevm/lexical#244); its model is ported now, with Swift's port alongside
  it on the same vectors.
- The editing UI (Swift's Tier 2: views, input, the reconciler's rendering
  half, the WebKit scenarios) is out of scope.
- **JS is the reference.** A difference from upstream is a Rust bug, except
  the documented exceptions in [PARITY.md](PARITY.md).
- **Platform standards first, then an API close to the web's.** Where an
  API meets a platform (a UI toolkit such as the playgrounds' GPUI and
  Rune, the operating system's fonts, text or input), it does what that
  platform does and recommends, and only then is it shaped as close to
  the web's as that allows. Names and units follow the platform, the CSS a
  document carries maps onto them, and a mechanism the platform doesn't
  use is not invented to mirror the web. The Swift port follows the same
  rule (`swift/DESIGN.md`, Parity). JS stays the reference for the model's
  behavior.
- Conforms to the upstream release in `lexical_ui::UPSTREAM_VERSION`,
  which must match the vectors' `meta.json` (checked by the conformance
  runner).

## Layout

| Path                  | Crate                 | Published | What                                                      |
| --------------------- | --------------------- | --------- | --------------------------------------------------------- |
| `rust/lexical-ui`     | `lexical-ui`          | crates.io | The port: core, and one feature per upstream package      |
| `rust/conformance`    | `lexical-conformance` | never     | The conformance runner: replays every vector file         |
| `rust/apps/lexical-playground-core` | `lexical-playground-core` | never | The playgrounds' binding to `lexical-ui` |
| `rust/apps/lexical-playground-rune` | `lexical-playground-rune` | never | The playground on Rune (a workspace of its own) |
| `rust/apps/lexical-playground-gpui` | `lexical-playground-gpui` | never | The playground on GPUI (a workspace of its own) |

`rust/` is developed in wevm/lexical, with its own CI
(`.github/workflows/rust.yml`), and mirrored to wevm/lexical-rust on
release the way `swift/` is mirrored to wevm/lexical-swift (see Releases).

### The name

The crate is `lexical-ui` (`lexical_ui` in code): tmm chose it on
2026-11-16 over the provisional `lexical-editor`, `lexikal` and `lexica`. It
was free on crates.io then, and stays changeable until tmm's first publish
(see Releases). The name lives in few places: `name` in
`lexical-ui/Cargo.toml`, the `package` of the `lexical` entry in the
workspace's `[workspace.dependencies]`, the
directory, the doc example in `lib.rs`, the `cargo tree` and `cargo
publish` checks in `rust.yml`, `rust-release.yml`,
`scripts/version-packages.sh` and the changesets' frontmatter. Every other
crate in the workspace depends on it as `lexical`, so a rename doesn't touch
their code.

## Packages and features

One crate, with a feature per upstream package, as `zero-client` does. The
core (`lexical`) is always built. Planned features, each enabling those it
depends on as the upstream package depends on them:

| Feature     | Upstream package       | Needs                      |
| ----------- | ---------------------- | -------------------------- |
| (core)      | `lexical`, `@lexical/headless` | |
| `extension` | `@lexical/extension`, with `lexical`'s `extension-core` | `utils` (for `TabIndentationExtension`'s `$handleIndentAndOutdent`); with another package's feature, that package's extensions (`RichTextExtension`, `HistoryExtension`, `ClickableLinkExtension`, `BlockWithAlignableContents`...) |
| `selection` | `@lexical/selection`   |                            |
| `utils`     | `@lexical/utils`       | `selection`                |
| `rich-text` | `@lexical/rich-text`, with `@lexical/react`'s table of contents | `selection`, `utils`, `clipboard` |
| `plain-text`| `@lexical/plain-text`  | `clipboard` (upstream also lists `dragon`, `extension`, `selection` and `utils`; the model handlers use none of them) |
| `clipboard` | `@lexical/clipboard`   | `selection`, `utils` (upstream also lists `extension` and `html`; the HTML paths and the extensions come with those features) |
| `list`      | `@lexical/list`        | `utils` (upstream also lists `extension` and `html`; `ListExtension` and `CheckListExtension` come with `extension`, and the DOM parts with `html`) |
| `link`      | `@lexical/link`        | `utils` (upstream also lists `extension` and `html`; `LinkExtension`, and with `autolink` `AutoLinkExtension`, come with `extension`, and the DOM parts with `html`) |
| `autolink`  | `@lexical/link`'s `registerAutoLink` | `link`, `regex` |
| `code`      | `@lexical/code-core`   | (upstream lists `extension`, `html` and `utils`; `CodeExtension` and `CodeIndentExtension` come with `extension`, the DOM parts with `html`; `utils` is for the arrow-key handlers) |
| `code-highlight` | `@lexical/code-prism`'s highlighting loop (`CodeHighlighterPrism.ts`), over a `Tokenizer` the app supplies | `code` (upstream also lists `extension`, for `CodePrismExtension`, which comes with it, and `prismjs`, whose grammars aren't ported) |
| `mark`      | `@lexical/mark`        | `utils` (upstream also lists `extension` and `html`; with `extension`, `MarkExtension`, `MarkStateExtension` and the mark flavors, and with `html` too their HTML export and import of the ids) |
| `overflow`  | `@lexical/overflow`, with `@lexical/react`'s character limit | `text`, `utils` (for the character limit's `$rootTextContent`, `$dfsWithSlots` and `$unwrapNode`) |
| `text`      | `@lexical/text`        |                            |
| `hashtag`   | `@lexical/hashtag`     | `text` (upstream also lists `extension` and `utils`; `HashtagExtension` comes with `extension`, and `mergeRegister` is the core's) |
| `history`   | `@lexical/history`     | (upstream lists `utils`, but uses nothing from it) |
| `html`      | `@lexical/html`, with `dom` (a DOM in place of the browser's, which the nodes' DOM hooks use) | `selection` |
| `markdown`  | `@lexical/markdown`    | `rich-text`, `list`, `link`, `code` |
| `file`      | `@lexical/file`        |                            |
| `a11y`      | `@lexical/a11y`'s announcements; with `rich-text` and `link`, their `HeadingAnnounceExtension` and `AutoLinkAnnounceExtension` | `extension`, `utils` |
| `decorator-block` | `@lexical/react`'s `DecoratorBlockNode` | (upstream `@lexical/react` lists most packages, for its other modules; the node needs none) |
| `menu`      | `@lexical/react`'s typeahead and node menus without their views (`LexicalTypeaheadMenuPluginUtils.ts`, `shared/LexicalMenu.tsx`) | |
| `auto-embed`| `@lexical/react`'s auto-embed (`LexicalAutoEmbedPluginUtils.ts`, the logic of `LexicalAutoEmbedPlugin.tsx`) | `link`, `menu`, `utils` |
| `table`     | `@lexical/table`'s model: the nodes, `TableSelection`, `LexicalTableUtils.ts`, `INSERT_TABLE_COMMAND`, `registerTablePlugin`, the model parts of the table handlers (Tab and the arrows on the table map) | `utils` (upstream also lists `clipboard`, `extension` and `html`; `TableExtension` comes with `extension`, the DOM parts and `TableImportRules` with `html`) |
| `devtools`  | `@lexical/devtools-core`'s `generateContent` | (upstream lists `html`, `link`, `mark` and `table`; its HTML form is built with `html`, and its link, mark and table branches with `link`, `mark` and `table`) |

Each module is `lexical_ui::<package>` (`lexical_ui::rich_text`).
Names may change as packages land; this table is updated with them.

## Architecture

- **No third-party dependencies** in the default build: `cargo tree -e
  normal,build` of `lexical-ui` shows the crate alone, with the default
  features and with every feature but `autolink`, and CI fails otherwise. A
  dependency comes only behind an opt-in feature: `autolink` needs `regex`
  (with `regex-automata` and `regex-syntax`, which CI checks are all it
  brings), for `registerAutoLink`'s matchers, whose `\p{L}` and `\p{N}`
  need Unicode tables. `@lexical/link`'s nodes and
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
- **Schemas.** `LexicalSchema.ts` is the `schema` module, type-erased
  over the crate's `json::Value` as Swift's is over `JSONValue`
  (`Value::Undefined` is JS's `undefined`), so a schema is one
  `SerializationSchema` whatever it parses to and a node field converts
  through `SchemaValue`. A node declares its `$config` `json` schema in
  `LexicalNode::json_schema_chain`, most derived class first, and
  `export_json` and `update_from_json` default to the walk of the composed
  schema (`$exportNodeJSONOnce`, `$applyJSONSetters`). A Rust node has no
  methods to find by name, so every property names its accessors:
  `with_field` takes the field's two accessor functions, `with_accessors`
  two closures, and a class that overrides an accessor a field stands in for
  gives its base's schema with `NodeSchema::with_setter` (where JS resolves
  `SchemaFieldBase.method` along the prototype chain). A clone carries every
  field through `Clone`, so `$config`'s synthesized `afterCloneFrom` has
  nothing to do. JS checks in generated code for the core nodes
  (`LexicalGeneratedJSON.ts`), and the port keeps that code by hand in the
  `generated_export_json` and `generated_update_from_json` hooks; the
  conformance runner checks it against the walk for every node of every
  serialization and property case, both forms, as
  `LexicalGeneratedJSON.test.ts` does, and a unit test does so for hostile
  values.
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
  meanwhile. Without a root element nothing reconciles, so mutation
  listeners and selection-change notifications never fire, as in JS's
  headless editor (`create_headless_editor` and `HeadlessExtension` refuse
  a root element, and their editor never reconciles); with one
  (`set_root_element`), the reconciler's model
  half (`reconciler.rs`) reports created, updated and destroyed nodes to
  mutation listeners, collects what decorators decorate to (an app's
  `Rc<dyn Any>`, compared by identity, for `getDecorators()` and the
  decorator listeners), and sets each reconciled element's `textFormat`
  and `textStyle` from its first text, as JS's does.
  JS commits a non-discrete update in a microtask; with no event loop, Rust
  commits it at the next `read` or discrete update, which JS also does when
  one comes first. Update closures that may be queued are `'static`.

### The DOM

`createDOM`, `exportDOM`, `importDOM` and HTML export and import need a
DOM, which a browser or (in upstream's unit tests) jsdom gives JS. The
`html` feature brings one, `lexical_ui::dom`, ported from the Swift
port's (`swift/Sources/Lexical/DOM`, `swift/DESIGN.md`, "The DOM is a small
model of jsdom's"), since the vectors record what jsdom does:

- **One node type.** `DomNode` is a shared handle (`Rc`) compared by
  identity, as JS compares node objects, standing for `Node` and every
  interface Lexical reads through it (`Element`, `Text`, `Comment`,
  `DocumentFragment`, `Document`); an element's methods answer as empty
  values on other nodes. A node holds its children; its parent and
  document are weak links. `get_document()` is one `document` per thread,
  as JS has one global `document`.
- **The inline style declaration normalizes as jsdom's does** for what
  Lexical writes (colors to `rgb()`, trimmed numbers, `0px`, lower-case
  keywords, `calc()` of a product, CSS-wide keywords), and is read from the
  `style` attribute lazily and written back as `cssText`, as jsdom's is.
  The accepted properties and how each normalizes are a table generated
  from the `css/properties` vectors (`rust/scripts/generate-dom-tables.mjs`,
  which also writes the entity table from `html/entities`); the runner
  checks both the tables and the declaration's behavior against them.
  Unlike jsdom it keeps most values it can't parse, collapsing their
  whitespace, as Swift's does; it rejects a shadow of nothing but
  identifiers, as jsdom does.
- **The nodes' DOM hooks are `LexicalNode` methods** behind the feature:
  `create_dom(this, config, editor)`, `export_dom(this, editor)` and
  `import_dom()` (the class's static `importDOM`, read from a default
  value when the class registers). Like the other hooks they get the
  node's value and its handle, so an export can run on a value that isn't
  in the editor state (the text a selection cuts short). The base classes'
  behavior is the hooks' defaults (`TextNode` renders its text,
  `ElementNode` adds the indent and direction), as for
  `get_text_content`. A node package's DOM parts build when both its
  feature and `html` are on, so the package doesn't pull in the DOM. The
  theme is the JSON of `EditorThemeClasses`
  (`CreateEditorArgs.theme`), and a `DOMConversionMap` is a list of
  name and importer pairs in the order JS's object keys would be.
- **The render config has the hooks export reaches.** An editor's
  `EditorDomRenderConfig` (`createEditor`'s `dom`, JS
  `EditorDOMRenderConfig`) holds `$createDOM`, `$exportDOM`,
  `$extractWithChild`, `$shouldInclude` and `$shouldExclude` as shared
  closures, and `DOMRenderExtension` layers its overrides over them. The
  reconciler's hooks (`$updateDOM`, `$decorateDOM`, `$getDOMSlot`,
  `$getSlotTargetElement`) are left out, as there is no DOM to reconcile.
  Nodes have no class hierarchy to test with `instanceof`, so an override
  of a class matches by node type and by `LexicalNode::super_types`, the
  types of the classes a node's class extends (`text` for a node with
  `TextNode` fields, `link` for `AutoLinkNode`); JS's dispatch through
  `getRegisteredSubtypeMap` composes the same way and is only faster.
- **`DOMParser` is a WHATWG tokenizer and tree builder**, checked against
  jsdom's by the `html/parse` and `html/entities` vectors. As in Swift it
  leaves out templates, framesets, the `<select>` and `<ruby>` insertion
  modes and the case adjustments of SVG and MathML names; their content
  parses as body content.

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
  New vectors the port needs (for `@lexical/mark`, say) are added there, in
  JS, as the Swift port adds its own.
- The mirror has `rust/` without the vectors, so there the conformance tests
  skip; `LEXICAL_CONFORMANCE_VECTORS` points the runner elsewhere.

### Parity ledger

The counterpart of Swift's (`swift/parity/`, `@Test(.upstream(...))`),
checked by the same script, `packages/lexical-conformance`'s
`scripts/ledger.mjs`:

- **The list** of upstream unit tests is Swift's
  `swift/parity/upstream-tests.json`, which `pnpm run ledger` regenerates
  for the packages either port tracks; both ledgers read it.
- **`rust/parity/packages.json`**: each tracked package, `"enforced"` or
  `"pending"`, as Swift's statuses. A package is enforced once each of its
  tests is ported or skipped; then CI fails on any new upstream test that is
  neither.
- **`rust/parity/skips.json`**: id → `reason` and `note`, with Swift's
  reasons (`dom-only`, `react-only`, `out-of-scope-package` with the
  `packages` it needs, `covered-by-vector`, `deviation`). The note says why
  in Rust's terms: a Swift skip is not a Rust skip unless its reason holds
  for Rust. So a test Swift skips as DOM-only that the `html` feature's DOM
  can run (`setDOMStyleFromCSS`, `$getDocument` without a root element) is
  not skipped, nor are Swift's deviations that Rust doesn't share (the
  generated JSON code Rust keeps, a fractional indent, which an `f64`
  holds), nor `@lexical/code`, which re-exports the `code-core` Rust ports;
  `covered-by-vector` names what the Rust runner checks of the vector.
- **A ported test** has a `// upstream: <id>` line directly above its
  `#[test]` (other attributes may come between), one id per line, several
  for a test that ports several; an id a line comment can't hold as
  written (leading or trailing whitespace) is a JSON string literal. The
  script reads the comments in every `.rs` file under `rust/`; nothing in
  the crate depends on the ledger. An id is marked once.
- **`pnpm run check-ledger`** fails, for both ports, on an unknown id in a
  marker or a skip, a key listed twice, a skip of a test that is also
  ported, a skip without a note or with an unknown reason, an
  `out-of-scope-package` skip whose packages are all enforced, and an
  enforced package with a test neither ported nor skipped; it prints each
  port's counts per package. CI runs it in `swift.yml` (with `--fresh`,
  which reruns the upstream tests to check the list is current) and in
  `rust.yml`.

## Milestones

Each milestone ends with green CI and a report to the Space. Three Amp
threads share the work and rebase onto `main` as pull requests land: the
[Rust port thread](https://ampcode.com/threads/T-16d45229-bfa8-417a-a091-d3e3888d830d)
owns M1, M3, M6 and the merges, the
[HTML thread](https://ampcode.com/threads/T-506c8fa1-4309-40f1-9810-732b311eaeab)
owns M4, and the
[Markdown thread](https://ampcode.com/threads/T-01a1daf1-150e-709b-aaed-996443a64459)
owns M5. A change one of them needs in the core goes through the Rust port
thread.

| Milestone | Scope | Vectors that pass when done | Status |
| --- | --- | --- | --- |
| M0 | The `rust/` workspace, JSON as JS writes it, the core nodes (root, paragraph, text, tab, line break), `parseEditorState` and `toJSON` in both forms, the conformance runner, CI | `meta`, `serialization/core`, the core-only cases of `property/documents` (JSON and text) | done |
| M1 | The core model: `editor.update` and the commit (dirty nodes, normalization, transforms, listeners, commands, tags), node creation and methods (`splitText`, `mergeWithSibling`, `spliceText`, `insertAfter`, `remove`, `replace`...) with UTF-16 offsets, `RangeSelection` and `NodeSelection` editing, carets, NodeState, slots, the schema facility, `DecoratorNode`, the extension API (`buildEditorFromExtensions`), the parity ledger | `scripts/*`, `fuzz/core`, `file/*` | done (every vector it names passes): the update cycle, `setEditorState`, commands, transforms, listeners, the node methods, carets, selection reading and `RangeSelection`/`NodeSelection` editing are done (`scripts/*` and `fuzz/core` pass), and the platform's `Selection.modify` is an editor hook (`SelectionMovement`); NodeState is done (`createState`, `$getState`, `$setState`, `$getStateChange`, the `"$"` key and flat states in node JSON, unknown keys kept, clones, `$copyNode`, splits and merges, `scripts/node-state`), and its schema builders came with the schema facility; `DecoratorNode` is done (its hooks, block and inline decorators in `insertNodes`, deletion, `modify`, text content and formatting, `$setBlocksType` across one, `scripts/decorator`), and so are `editor._decorators` and the decorator listener, which the reconciler fills once the editor has a root element (`scripts/reconciler`); the extension API is done (`buildEditorFromExtensions`, the extension-core types, dependency and peer resolution, config merging, the four phases, disposal, the dependency lookups, a port of `@preact/signals-core` with `namedSignals` and `watchedSignal`, and `InitialStateExtension`, `ClearEditorExtension`, `EditorStateExtension` and `DecoratorTextExtension`; `scripts/extensions` passes), and so are the packages' model-level extensions (`RichTextExtension`, `PlainTextExtension`, `HistoryExtension`, `SharedHistoryExtension`, `ListExtension`, `CheckListExtension`, `LinkExtension`, `AutoLinkExtension`, `HashtagExtension`, `OverflowExtension`, `CodeExtension`, `CodeIndentExtension`, `NormalizeInlineElementsExtension`; `scripts/package-extensions` passes), less their key, DOM and HTML import handlers (rust/PARITY.md); slots are done (`LexicalSlot.ts` and its call sites in the core: node methods, both JSON forms, GC, selection clamping, carets across slots, insertion and deletion at a slot boundary; and in the packages; `scripts/slots` and the named slots case of `serialization/core` pass), except the `'deletion'` mode of `$setPointAtSlotBoundary`, which needs `Selection.modify` to cross a frame (rust/PARITY.md); the schema facility is done (`schema`, the core and package nodes' `$config` schemas, NodeState's schema builders; `serialization/schema` and `serialization/schema-node` pass), less what rust/PARITY.md lists; and the parity ledger is done (`rust/parity`, the `// upstream:` markers, checked with Swift's by `check-ledger`; see Parity ledger) |
| P | Desktop playgrounds (`apps/`): `lexical-playground-rune` on Rune, then `lexical-playground-gpui` on GPUI, sharing `lexical-playground-core`; they grow with each milestone below | the vectors, replayed in the app; a session exported from the app replays in JS (`replay.test.ts`); in CI, a GPUI session typed in a real window replays in JS | both apps run on macOS with typing, Return, deletion, formatting, input methods and the Parity panel; they grow with each milestone |
| M2 | `rich-text` (headings, quotes, and the command handlers a text surface dispatches: typing, Enter, Backspace, formatting) and `history` (undo), which the playgrounds need first; then the node packages `list`, `link`, `code`, `overflow`, `hashtag`, with `selection` and `utils` | `serialization/*`, `property/documents` (JSON and text), `entities/hashtag-*`, `scripts/history` | done (every vector it names passes): `history` is done (`scripts/history` passes); `rich-text`'s nodes and `registerRichText`'s model command handlers are done (`scripts/rich-text` and `scripts/rich-text-commands` pass, with the heading and quote cases of `property/documents`); `plain-text`'s model command handlers are done (`scripts/plain-text` passes); `list`'s nodes, `registerList` and the model part of `registerCheckList` are done (`scripts/list` and `serialization/list` pass, with the list cases of `property/documents`); `link`'s nodes, `$toggleLink` and `registerLink`'s model handlers are done (`scripts/link` and `serialization/link` pass, with the link cases of `property/documents` that need no other package), `registerAutoLink` and its matchers are done behind `autolink` (`scripts/autolink` and `entities/autolink-match` pass); `selection`'s `$setBlocksType` is done (`scripts/set-blocks-type` passes); `code`'s nodes, `FlatStructureUtils.ts`, `CodeExtension`'s transform and `registerCodeIndentation`'s model handlers are done (`scripts/code` and `serialization/code` pass, with the code cases of `property/documents` that need no other package); `overflow`'s `OverflowNode` is done (`scripts/overflow` and `serialization/overflow` pass, with the overflow cases of `property/documents` that need no other package); `hashtag`'s `HashtagNode`, its matcher (generated character tables, no `regex`) and `registerLexicalHashtag`, with `text`'s `registerLexicalTextEntity`, are done (`entities/hashtag-match`, `entities/hashtag-documents` and `entities/hashtag-typing` pass, with the last waiting cases of `property/documents`), so M2's node packages are done |
| M3 | The commands `property/sequences` dispatches | `property/sequences` | done (all 81 cases match JS): the suite builds `sequenceEditor`'s editor with `build_editor_from_extensions` and the ported package extensions, in JS's order; the commands M2 ported needed two fixes, `NormalizeInlineElementsExtension`'s transform (`register_normalize_inline_elements`, removing empty links) and `$extendSelectionForDeletion` and `modify` stopping where the window gives no DOM selection (`SelectionMovement::has_dom_selection`) |
| M4 | `html`: a DOM abstraction over an HTML parser, CSS declarations, export and import; with it `@lexical/clipboard`'s HTML path (`$getHtmlContent`, the `text/html` importer) | `html/*`, `css/*`, `property/documents` (HTML) | done ([HTML thread](https://ampcode.com/threads/T-506c8fa1-4309-40f1-9810-732b311eaeab)): the DOM (`dom`: nodes, `DOMParser`, the serializer, the inline style declaration and its tables), `getStyleObjectFromCSS`, the core nodes' and the node packages' DOM hooks, and `@lexical/html`'s export and import, are done (every `html/*` and `css/*` file and `property/documents`' HTML pass), and so are the `DOMImportExtension` pipeline with `CoreImportRules` and `$inlineStylesFromStyleSheets` (`html/import-extension` passes) the `DOMRenderExtension` pipeline (`html/export-render` passes) the packages' import rules (`html/import-packages` passes), `clipboard`'s HTML path with its extensions (`html/clipboard` passes) and `BlockIdHTMLExtension` (`html/block-id` passes) (clipboard's JSON and plain-text paths are done, see M6) |
| M5 | `markdown`, and Markdown shortcuts in both playgrounds | `markdown/import-export`, `property/documents` (Markdown), `scripts/markdown-shortcuts` | done ([Markdown thread](https://ampcode.com/threads/T-01a1daf1-150e-709b-aaed-996443a64459)): import, export, the transformers and `registerMarkdownShortcuts` (`markdown/import-export`, the Markdown of `property/documents` and `scripts/markdown-shortcuts` pass), registered in `lexical-playground-core`, whose Return dispatches `KEY_ENTER_COMMAND`, so both desktop playgrounds have the shortcuts |
| M6 | The rest of Swift's feature set: the extension API's bundled extensions (`ClearEditor`, `InitialState`, `EditorState`, `TabIndentation`, `NodeStructure`, `SelectBlock` and the others Swift has), clipboard (JSON copy and paste first, HTML once M4 lands), the rest of `selection` and `utils` (style helpers, `$wrapNodes`, `$splitNode`, `$dfs`...), `mark`, `file`, the devtools tree view, `text`'s root helpers, `a11y`'s announcements, code highlighting (Swift uses tree-sitter where JS uses Prism), the character limit, the menu, auto-embed, decorator block and table of contents logic, the remaining built-in commands, slots, node schemas and editable mode | `file/*`, `devtools/tree`, `scripts/text`, `scripts/extensions`, `scripts/mark`, `scripts/mark-extension`, `serialization/mark`, `html/mark-state`, `scripts/clipboard`, `scripts/package-extensions`, `scripts/selection-utils`, `scripts/nested-editor`, `scripts/character-limit`, `scripts/table-of-contents`, `scripts/code-highlight`, and new JS vectors for the rest | in progress (this thread's owner: the [Rust port thread](https://ampcode.com/threads/T-16d45229-bfa8-417a-a091-d3e3888d830d), which also owns M1's extension API and the merges): `file`, the devtools tree view and `text`'s root helpers are done (`file/*`, `devtools/tree` and `scripts/text` pass); `mark`'s model is done (`MarkNode` and its generated JSON, `$wrapSelectionInMarkNode`, `$unwrapMarkNode`, `$getMarkIDs`, and the `MarkStateExtension.ts` functions that need no extension; `scripts/mark` and `serialization/mark` pass), and so are `MarkExtension`, `MarkStateExtension`, `MarkBaseExtension` and the mark flavors of `defineMarkExtension` (`markableNodes`, `disabled`, the `markIDNodeKeys` output, `registerMarkIDsListener`, the inserted-text rule with composition, and the HTML export and import of the ids; `scripts/mark-extension` and `html/mark-state` pass), less the renderer's `$decorateMarkIDs` and `isMarkIDShown` (rust/PARITY.md); the model of `selection` and `utils` is done (the style helpers, `$wrapNodes`, `$splitNode`, `$dfs` and the rest Swift has but what needs the DOM, computed styles or `File`, with `$dfsWithSlots` and its forms since slots landed; `scripts/selection-utils` and `scripts/slots` pass); `clipboard`'s JSON and plain-text paths are done (`scripts/clipboard` passes, with `registerRichText`'s and `registerPlainText`'s copy, cut and paste handlers), and its HTML path comes with M4; of the bundled extensions, `ClearEditor`, `InitialState`, `EditorState`, `NormalizeInlineElements`, `TabIndentation`, `SelectBlock`, `PreventSelectAll`'s config and `BlockId` (ids, their repair, positional and derived ids, handles; `scripts/block-id` passes) are done (`scripts/extensions`, `scripts/package-extensions`); the character limit is done (`useCharacterLimit.ts` under `overflow`, with grapheme clusters from generated Unicode tables; `scripts/character-limit` passes), and so is the table of contents (`LexicalTableOfContentsPlugin.tsx` under `rich-text`, on the core's root element and mutation listeners; `scripts/table-of-contents` passes); `NodeStructure` is done (`LexicalStructure.ts` with its core and package call sites and `NodeStructureExtension`; `scripts/node-structure` passes, and `BlockId`'s peer on it resolves); `NestedEditor` is done (the core's parent editor, command propagation to it and `read(mode)`, `NestedEditorExtension`, and with them `$isEditorIsNestedEditor`, `SharedHistoryExtension`'s default parent and `SelectBlockExtension`'s `cascadeSelection`; `scripts/nested-editor` passes); `NodeSelection` and `HorizontalRule` are done (`scripts/horizontal-rule` passes), less the rule's selected class, which needs its DOM; the input extensions are done on the commands an app's view sends (`KeyboardShortcuts` with `formatKeyboardShortcut`, `ClickAfterLastBlock`, `NormalizeTripleClickSelection`, `IME`, `AutoFocus`, `@lexical/link`'s `ClickableLink` with an app's `LinkWindow`, and the builder's `namespace` and disposal; `scripts/input-extensions` passes), and so is `BlockWithAlignableContents` as an extension, as Swift has it; `a11y`'s announcements are done (the live region model with an app's `LiveRegionHost`, `AriaLiveRegionExtension`, `HistoryAnnounceExtension`, `EditorModeAnnounceExtension`, `HeadingAnnounceExtension` and `AutoLinkAnnounceExtension`, on the core's new root element, mutation listeners, the reconciler's model half, editable listeners and `$onUpdate`; `scripts/a11y` passes), and so are `DecoratorBlockNode` (`scripts/decorator-block`, `serialization/decorator-block`), the menu's text matching and state (`scripts/menu`) and auto-embed (`scripts/auto-embed`), less the a11y focus helpers (rust/PARITY.md); code highlighting's loop is done (`code-highlight`: `registerCodeHighlighting`, `registerHighlightingOnly` and `CodePrismExtension` over a `Tokenizer` trait, with Shiki's lazy languages; `scripts/code-highlight` passes), without a tokenizer: Prism's grammars and Swift's tree-sitter would be third-party dependencies, so apps supply one; the remaining built-in commands are done (the DOM event commands and their payloads, `LexicalKeyboardShortcuts.ts`, and `LexicalEvents.ts`' keydown, beforeinput and composition handlers with composition through the core; `scripts/events` and `scripts/events-apple` pass), and so are the packages' handlers of them that Swift has at the model level (rich and plain text's keys, clicks, pastes of files and drag and drop with `RichTextConfig`, the list's Backspace, code indentation's keys, `TabIndentationExtension`'s Tab, the link's paste of a URL and the menus' keys; `scripts/key-handlers` passes), less the check list's focus keys (rust/PARITY.md) |
| M7 | Releases: the mirror, changesets, a name tmm settles | | the release process is in (see Releases); waiting on tmm's one-time steps and first publish |

The `scenarios/*` vectors, recorded from a real browser, need a text
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
    edit run as Lexical updates before the field sees them (a shortcut as
    a `KEY_DOWN_COMMAND` the core's handler maps to its command), and a change
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
- The editor has a root element (the core crate's session root), as the
  JS playground's has its `contenteditable`, so the reconciler's model half
  runs and mutation listeners hear of each node: code blocks' line numbers
  come from `register_code_gutter`, which the GPUI app draws in a gutter as
  the web playground's theme draws `data-gutter`. A selection the app's
  view makes reaches the editor through `on_selection_change` (the
  `selectionChange` action), as a browser's `selectionchange` runs
  `onSelectionChange`, before the edit that follows it.
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

## Releases

As the Swift port releases (`swift/AGENTS.md`, Releases), with crates.io on
top:

- **Changesets.** A user-facing change to `lexical-ui` adds a changeset
  in `rust/.changeset/` (its README has the format; bodies are in the past
  tense). The crate's version is the one in `lexical-ui/Cargo.toml`;
  `CHANGELOG.md` has a section per release, naming `UPSTREAM_VERSION`.
  Neither is edited by hand. The other crates stay at the workspace's
  `0.0.0` and are never published.
- **The version pull request.** While changesets are pending,
  `.github/workflows/rust-release.yml` keeps a `chore(rust): version
  packages` pull request open, made by `scripts/version-packages.sh` (which
  also bumps the crate's entry in `Cargo.lock` and the playgrounds'
  lockfiles), and starts the Rust workflow on it. It merges, squashed, once
  that run is green; no one approves it.
- **The release.** On the merge, the workflow pushes a subtree split of
  `rust/` to wevm/lexical-rust (github-sts, with the policy in
  `rust/.github/sts/mirror.sts.yaml` and the `rust-release` environment),
  tags the version there, creates a GitHub release from the version's
  `CHANGELOG.md` section, then publishes the crate with `cargo publish`
  through crates.io's trusted publishing: `rust-lang/crates-io-auth-action`
  trades the job's GitHub OIDC token for a crates.io token that lasts 30
  minutes and is revoked when the job ends, so no crates.io secret is
  stored. The crate's trusted publisher on crates.io names wevm/lexical,
  `rust-release.yml` and the `rust-release` environment, so renaming the
  workflow file or the environment needs the same change there. Trusted
  publishing can't create a crate, so until the first publish the step is
  skipped with a notice. Each step is skipped when
  it has happened, so a failed run can be rerun. `rust.yml` packages the
  crate on every run (`cargo publish --dry-run --all-features`).
- **The first release** (0.1.0, from `rust/.changeset/first-release.md`)
  needs tmm first, once:
  1. Create the `rust-release` environment on wevm/lexical, with
     deployments limited to `main` and no reviewers, as `swift-release`.
  2. Create wevm/lexical-rust, public and empty (no README or license), and
     add it to the GitHub STS app's installation, as wevm/lexical-swift is.
  3. Push the first split by hand, from a `main` that has
     `rust/.github/sts/mirror.sts.yaml` (the STS policy is read from the
     mirror's `main`, so the workflow can't push the first one):
     `git clone https://github.com/wevm/lexical.git && cd lexical && git subtree split --prefix=rust -b rust-mirror && git push git@github.com:wevm/lexical-rust.git rust-mirror:main`.
  4. Then the version pull request merges, and the workflow tags 0.1.0 on
     the mirror and creates its release; the crate step is skipped with a
     notice, as the crate doesn't exist yet. Until step 2 is done, the
     workflow fails with an error naming the missing mirror.
  5. Publish 0.1.0 by hand, so the crate is registered to tmm. crates.io
     only creates a crate from an API token, so `cargo login` takes one with
     the `publish-new` scope (and crates.io needs a verified email); revoke
     it afterwards, as CI needs none:
     `git clone --branch 0.1.0 https://github.com/wevm/lexical-rust.git && cd lexical-rust && cargo login && cargo publish -p lexical-ui --locked`.
  6. On crates.io, open the crate's Settings → Trusted Publishing, Add,
     GitHub, with repository owner `wevm`, repository name `lexical`,
     workflow filename `rust-release.yml` and environment `rust-release`,
     and save. From then on, CI publishes each release. Once one release
     has published from CI, the crate settings' "Trusted Publishing only"
     setting can turn off publishing with API tokens.

  A rename comes before step 5 (see The name).
