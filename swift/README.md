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

Requires iOS 26 or macOS 26 and Swift 6.4 (Xcode 27). The non-UI modules also build on
Linux.

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

MIT. Lexical is copyright Meta Platforms, Inc. and affiliates.
