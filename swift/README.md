# Lexical for Swift

A native [Lexical](https://lexical.dev) editor for iOS and macOS, with
UIKit, AppKit and SwiftUI views. Your app builds an editor from extensions,
as in JS, and the views turn native input into Lexical commands.

It's a port of the JS packages and behaves the same: the same documents
give the same editor state and the same JSON, byte for byte. Conformance
vectors generated from upstream JS check it, and keyboard scenarios
recorded in WebKit check the views.

> [!NOTE]
> Before 1.0, minor versions may change the API.

## Install

```swift
// Package.swift
.package(url: "https://github.com/wevm/lexical-swift", from: "0.2.0"),
// and in your target's dependencies, the products you use:
.product(name: "Lexical", package: "lexical-swift"),
.product(name: "LexicalExtension", package: "lexical-swift"),
.product(name: "LexicalRichText", package: "lexical-swift"),
.product(name: "LexicalHistory", package: "lexical-swift"),
.product(name: "LexicalSwiftUI", package: "lexical-swift"),
```

Swift 6.4 (Xcode 27, on macOS 26.6 or later), and iOS 26 or macOS 26. The
non-UI products also build on Linux.

## Quick start

Build an editor from extensions, as in JS, and show it with `LexicalEditor`
(SwiftUI), `LexicalUITextView` (UIKit) or `LexicalNSTextView` (AppKit):

```swift
import Lexical
import LexicalExtension
import LexicalHistory
import LexicalRichText
import LexicalSwiftUI
import SwiftUI

@MainActor
func makeEditor() throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(name: "my-app", dependencies: [RichTextExtension, HistoryExtension]))
}

struct ContentView: View {
  @State private var editor = try! makeEditor()

  var body: some View {
    LexicalEditor(editor, placeholder: "Message")
      .sizedToContent(minHeight: 36, maxHeight: 160)
      .configure { textView in  // LexicalUITextView on iOS; runs on each SwiftUI update
        textView.accessibilityLabel = "Message"
      }
  }
}

// The model API is Lexical's, without the `$`; the `$` functions throw.
let editor = try makeEditor()
try editor.update(discrete: true) {
  let paragraph = try createParagraphNode()
  try paragraph.append(createTextNode("Hello, Lexical"))
  try getRoot().clear().append(paragraph)
}
let text = try editor.read { try getRoot().getTextContent() }

// The same JSON as JS, byte for byte.
let json = try editor.getEditorState().toJSON().stringify()
try editor.setEditorState(editor.parseEditorState(json))
```

## API at a glance

The DocC catalog documents every symbol; open it in Xcode with Product ›
Build Documentation. Each declaration names its JS function.

| Concept | API | Notes |
| --- | --- | --- |
| Editor | `buildEditorFromExtensions(...)`, `defineExtension`, `configExtension` | `createEditor` without extensions |
| Views | `LexicalEditor` (SwiftUI), `LexicalUITextView`, `LexicalNSTextView` | `theme(_:)`, `editorBackground(_:)`, `configure` |
| Updates and reads | `editor.update { }`, `editor.read { }`, `getRoot()`, `getSelection()` | `$getRoot()` is `getRoot()` |
| Commands | `createCommand`, `registerCommand`, `dispatchCommand` | `FOCUS_COMMAND`, `KEY_ENTER_COMMAND`, ... |
| Listeners | `registerUpdateListener`, `registerTextContentListener`, `registerMutationListener` | Run on the editor's thread |
| JSON | `getEditorState().toJSON().stringify()`, `parseEditorState(_:)` | Legacy and compact forms |
| Custom nodes | `config()` returning a `NodeConfig` with a `SchemaField` per field | JS's `$config()`; `@LexicalNode` writes it |
| Node state | `createState`, `getState`, `setState` | Typed values |
| Read-only | `editor.setEditable(false)` | The views follow it |

A custom node declares its type and schema as a JS node does in `$config()`;
the optional `LexicalMacros` product writes that `config()`, and a
`getX()`/`setX(_:)` pair, from its `@Field` properties:

```swift
@LexicalNode(type: "callout", extends: ElementNode.self)
final class CalloutNode: ElementNode {
  @Field(stringValue()) var __label: String = ""
}
```

## Products

One product per upstream package, so an app links only what it uses.

| Product | Adds | Platforms |
| --- | --- | --- |
| `Lexical` | The core: editor, state, nodes, selection, commands, JSON | iOS, macOS, Linux |
| `LexicalExtension`, `LexicalRichText`, `LexicalPlainText`, `LexicalHistory`, `LexicalSelection`, `LexicalUtils`, `LexicalClipboard`, `LexicalHTML`, `LexicalList`, `LexicalLink`, `LexicalMarkdown`, `LexicalMark`, `LexicalHashtag`, `LexicalText`, `LexicalOverflow`, `LexicalFile`, `LexicalHeadless`, `LexicalA11y`, ... | The upstream package of the same name | iOS, macOS, Linux |
| `LexicalCodeCore` | Code blocks and `CodeHighlighterExtension`, over a `CodeTokenizer` | iOS, macOS, Linux |
| `LexicalCodeTreeSitter`, `LexicalCodeTreeSitter<Language>` | A tree-sitter tokenizer, one product per grammar (`JavaScript`, `Swift`, `Rust`, `Python`, ...) | iOS, macOS, Linux |
| `LexicalSwiftUI`, `LexicalUIKit`, `LexicalAppKit`, `LexicalTextKit` | The views, and the TextKit layer under them | iOS, macOS |
| `LexicalMacros` | `@LexicalNode` and `@Field` (builds swift-syntax) | iOS, macOS, Linux |

## Differences from JS

- **No `$` prefix**, since Swift identifiers can't start with `$`, and
  **errors throw**: JS's invariants throw a `LexicalError` with the same
  message.
- **One thread per editor.** An `Editor` is bound to the thread that creates
  it; only the views are `@MainActor`. In an app, build it on the main actor
  and give the module default main actor isolation.
- **The root element is the native view.** There is no DOM; the views turn
  input into commands, so observe the editor (`FOCUS_COMMAND`,
  `SELECTION_CHANGE_COMMAND`, `registerUpdateListener`) rather than the
  text view's delegate.
- **Code highlighting** uses tree-sitter where upstream uses Prism or Shiki;
  captures map to Prism's token names, so a theme's colors apply.
- **Offsets are UTF-16 code units**, as in JS and `NSString`.

[JSParity](Sources/Lexical/Lexical.docc/JSParity.md) lists each API
difference, and the views' documented deviations.

## Compatibility

| Lexical for Swift | Conforms to upstream Lexical |
| --- | --- |
| 0.1.0+ | 0.53.0 |

Each release's section in [CHANGELOG.md](CHANGELOG.md) names the upstream
version, as does `UPSTREAM_VERSION`.

## Documentation

- [Lexical's docs](https://lexical.dev/docs/intro)
- The DocC catalog: [Lexical](Sources/Lexical/Lexical.docc/Lexical.md) and
  [Coming from JS](Sources/Lexical/Lexical.docc/JSParity.md)
- [The design](DESIGN.md) and [the changelog](CHANGELOG.md)
- Example apps: [`Examples/LexicalDemo`](Examples/LexicalDemo) and the
  playground, [`apps/lexical-playground-swift`](apps/lexical-playground-swift)

## Contributing

This package is published from [wevm/lexical](https://github.com/wevm/lexical)
(`swift/`), a fork of [meta/lexical](https://github.com/meta/lexical); send
changes there. From `swift/` in that repository:

```sh
swift build --build-tests && swift test                # unit tests, and every replayed vector matches JS
pnpm --filter lexical-conformance run check-ledger     # upstream tests ported or skipped, per package
```

## License

MIT, like Lexical. The vendored tree-sitter core and grammars under
`Sources/CTreeSitter` and `Sources/TreeSitter*` keep their own licenses, in
each directory.
