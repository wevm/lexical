# Lexical for Swift

A native Swift port of [Lexical](https://lexical.dev) for iOS and macOS, with
UIKit, AppKit, and SwiftUI editor views. It conforms to the upstream JS
packages: the same documents, the same JSON (byte for byte), and the same
editing behavior.

> **Pre-release.** The API is unstable until 1.0, and most of Lexical isn't
> ported yet. See [DESIGN.md](DESIGN.md) for the plan.

0.1.0 conforms to upstream Lexical main just after **0.53.0** (2b388b92d;
see `UPSTREAM_VERSION` and [DESIGN.md](DESIGN.md)). Each release's notes in
[CHANGELOG.md](CHANGELOG.md) name the upstream code it conforms to.

## Installation

```swift
.package(url: "https://github.com/wevm/lexical-swift", from: "0.1.0")
```

Requires iOS 26 or macOS 26 and Swift 6.4 (Xcode 27, which runs on macOS 26.6 or later). The
non-UI modules also build on Linux.

Add the products you use to your target, for example:

```swift
.target(name: "MyApp", dependencies: [
  .product(name: "Lexical", package: "lexical-swift"),
  .product(name: "LexicalExtension", package: "lexical-swift"),
  .product(name: "LexicalRichText", package: "lexical-swift"),
  .product(name: "LexicalHistory", package: "lexical-swift"),
  .product(name: "LexicalSwiftUI", package: "lexical-swift"),
])
```

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
    LexicalEditor(editor)
  }
}
```

`LexicalEditor` takes a placeholder, `sizedToContent(minHeight:maxHeight:)`,
`theme(_:)` and `editorBackground(_:)`. Anything else on the text view goes
in `configure`, which runs each time SwiftUI updates the editor, so it can
read SwiftUI state and must set the same values each time. Commands and
listeners belong to the editor, not here, and read-only is the editor's
too: `editor.setEditable(false)`, which the view follows.

```swift
LexicalEditor(editor, placeholder: "Message")
  .sizedToContent(minHeight: 36, maxHeight: 160)
  .configure { textView in  // LexicalUITextView on iOS
    textView.textContainerInset = UIEdgeInsets(top: 8, left: 12, bottom: 8, right: 12)
    textView.textContainer.lineFragmentPadding = 0
    textView.accessibilityLabel = "Message"
  }
```

The model API is Lexical's, without the `$` prefix (Swift identifiers can't
start with `$`), and the `$` functions throw:

```swift
try editor.update(discrete: true) {
  let paragraph = try createParagraphNode()
  try paragraph.append(createTextNode("Hello, Lexical"))
  try getRoot().clear().append(paragraph)
}

let text = try editor.read { try getRoot().getTextContent() }

// The same JSON as JS, byte for byte; parse it back with parseEditorState.
let json = try editor.getEditorState().toJSON().stringify()
try editor.setEditorState(editor.parseEditorState(json))
```

Offsets in selections and points are UTF-16 code units, as in JS and
`NSString`. The differences from JS are listed in the documentation's
`JSParity` article; `Examples/LexicalDemo` and `apps/lexical-playground-swift`
are complete apps.

### Concurrency

An `Editor` is bound to the thread that creates it, and only the view
modules are `@MainActor`, so a headless editor can run on any thread. Its
listeners run on that thread and aren't `@MainActor` themselves. In an app,
build the editor on the main actor and give the app's module default main
actor isolation (the default for new Xcode 26 app targets; in SwiftPM,
`swiftSettings: [.defaultIsolation(MainActor.self)]`). Listeners written
anywhere in it, including in an extension defined as a global `let`, can then
use main-actor state without `MainActor.assumeIsolated`.

### Observing the views

`LexicalUITextView` is its own `UITextViewDelegate`, and both views turn
native input into Lexical commands instead of editing their own text. Observe
the editor instead, with the commands and listeners a Lexical web app uses:

| Instead of | Use |
| --- | --- |
| `textViewDidBeginEditing` | `FOCUS_COMMAND` |
| `textViewDidEndEditing` | `BLUR_COMMAND` |
| `textViewDidChangeSelection` | `SELECTION_CHANGE_COMMAND` |
| `textViewDidChange` | `registerUpdateListener`, `registerTextContentListener` |
| `shouldChangeTextIn`, Return | `CONTROLLED_TEXT_INSERTION_COMMAND`, `KEY_ENTER_COMMAND`, `BEFORE_INPUT_COMMAND` |

## Custom nodes

A node class declares its type and JSON schema in `config()`, as a JS node
does in `$config()`, with a `SchemaField` for each property stored in a
field (JS's `withField`):

```swift
final class CalloutNode: ElementNode {
  var __label: String = ""

  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      CalloutNode.self, type: "callout", extends: ElementNode.self,
      json: [SchemaField("label", stringValue(), field: \CalloutNode.__label)])
  }
}
```

The config derives `clone`, `importJSON` and `exportJSON` in both forms, as
`$config` does. The optional `LexicalMacros` product has shorthand for it:
`@LexicalNode` writes the same `config()` from its `@Field` properties, and a
`getX()`/`setX(_:)` pair for each.

```swift
import Lexical
import LexicalMacros

@LexicalNode(type: "callout", extends: ElementNode.self)
final class CalloutNode: ElementNode {
  @Field(stringValue()) var __label: String = ""
}
```

`LexicalMacros` builds swift-syntax (unless SwiftPM finds a prebuilt one), and
Xcode asks to trust its macro the first time, or `xcodebuild` needs
`-skipMacroValidation`. The other products use no macros.

## Code highlighting

`LexicalCodeCore`'s `CodeHighlighterExtension` highlights code blocks with
the `CodeTokenizer` in its config, as upstream swaps Prism for Shiki.
`LexicalCodeTreeSitter` is a tokenizer built on
[tree-sitter](https://tree-sitter.github.io) (its C core, vendored), whose
captures are mapped to Prism's token names, so a theme's `codeHighlight`
colors apply. Each grammar is its own product, so an app links only the
languages it uses: `LexicalCodeTreeSitterJavaScript`, `…TypeScript`,
`…Swift`, `…Python`, `…JSON`, `…HTML`, `…CSS`, `…Rust`, `…Go`, `…Bash`,
`…C`, `…CPP`, `…Java`, `…Markdown`, `…ObjC`, `…PowerShell`, `…SQL` and
`…XML`. Markdown highlights its block structure (headings, lists, quotes,
fences) only, without the inline grammar.

```swift
configExtension(CodeHighlighterExtension) {
  $0.tokenizer = TreeSitterTokenizer([.javaScript, .swift])
}
```

`scripts/vendor-tree-sitter.sh` pins and refreshes the vendored sources.

## Development

This package is developed in [wevm/lexical](https://github.com/wevm/lexical),
a fork of [meta/lexical](https://github.com/meta/lexical), under `swift/`.
[wevm/lexical-swift](https://github.com/wevm/lexical-swift) is a read-only
mirror for SwiftPM; open issues and changes against wevm/lexical.

```sh
cd swift
swift build --build-tests && swift test

# Conformance vectors and the parity ledger, from the repository root:
pnpm --filter lexical-conformance run generate      # rewrite vectors/
pnpm --filter lexical-conformance run ledger        # rewrite parity/upstream-tests.json
pnpm --filter lexical-conformance run check-ledger  # ported/skipped counts per package
```

## License

MIT. Lexical is copyright Meta Platforms, Inc. and affiliates. The vendored
tree-sitter core and grammars under `Sources/CTreeSitter` and
`Sources/TreeSitter*` keep their own licenses, in each directory.
