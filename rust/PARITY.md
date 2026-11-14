# Parity with JS Lexical

What the Rust port (`lexical-editor`) has ported from the upstream packages,
how it is checked, and where it deliberately differs. The conformance runner
(`cargo run -p lexical-conformance`) prints the live numbers; this file is
updated with each milestone.

## Ported

| Area | JS source | Rust | Vectors |
| --- | --- | --- | --- |
| JSON parse and stringify | `JSON.parse`, `JSON.stringify`, property order, `Number#toString` | `json` | every vector file is read with it; unit tests against Node's output |
| Constants | `LexicalConstants.ts` (flags and name tables) | `constants` | through the nodes |
| Export form | `LexicalSerializedExport.ts` | `serialized_export` | `serialization/core` |
| Generated JSON of the core nodes | `LexicalGeneratedJSON.ts` | `generated_json` | `serialization/core`, `property/documents` |
| Core nodes | `nodes/LexicalRootNode.ts`, `LexicalParagraphNode.ts`, `LexicalTextNode.ts`, `LexicalTabNode.ts`, `LexicalLineBreakNode.ts`, `LexicalElementNode.ts` (fields, `getTextContent`, `shouldSerializeTextStyles`) | `nodes::*` | `serialization/core`, `property/documents` |
| Node references and `getWritable` | `LexicalNode.ts` (`getLatest`, `getParent`, siblings, children) | `node`, `utils` | through the nodes |
| Appending children | `ElementNode.splice` (insertion only), `RootNode.splice`, `caret/LexicalCaretTree.ts` | `caret_tree` | through parsing |
| Editor state | `LexicalEditorState.ts` (`toJSON`, `read`, `isEmpty`) | `editor_state` | `serialization/core` |
| Parsing | `parseEditorState`, `$parseSerializedNodeImpl` (`LexicalUpdates.ts`) | `updates` | `serialization/core`, `property/documents` |
| Editor | `createEditor` (node registration), `getEditorState`, `read` | `editor` | |
| Upstream version | `meta.json` | `UPSTREAM_VERSION` | `meta` |

## Not ported yet

Everything else; [DESIGN.md](DESIGN.md) has the order. Within what M0
touches:

- **NodeState** (`"$"`) and **slots** (`"$slots"`): a document that carries
  either fails to parse with an error naming it, rather than losing them.
- `editor.update`, the commit, normalization (JS's `setEditorState` merges
  adjacent simple text nodes; the runner compares text content, which that
  doesn't change), selection, transforms, commands and listeners.
- `RootNode.__cachedText`, which only the DOM reconciler fills.

## Documented exceptions

Cases that match JS only under one of these are counted apart in the
report.

- **Unpaired surrogates** (inherited from Swift, `swift/DESIGN.md`, "The
  Tier 1 exception"). A JS string can hold half of a surrogate pair; a Rust
  `String` cannot, so `JSON.parse` of a lone `\uD83D` gives U+FFFD. Lengths
  and UTF-16 offsets still agree with JS (U+FFFD is one unit); only the text
  differs. The vectors keep offsets off pair interiors, so no case relies on
  this yet.

Swift's other Tier 1 deviation, integer properties (`model/integer-properties`),
does not apply: Rust keeps those properties as the `f64` JS stores.
