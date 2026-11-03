// swift-tools-version: 6.4
import CompilerPluginSupport
import PackageDescription

// One module per upstream package, added as each one is ported. The non-UI
// modules build and test on Linux; the UIKit, AppKit, and SwiftUI layers are
// Apple-only.
let package = Package(
  name: "Lexical",
  platforms: [.iOS(.v27), .macOS(.v27)],
  products: [
    .library(name: "Lexical", targets: ["Lexical"]),
    .library(name: "LexicalTesting", targets: ["LexicalTesting"]),
    .library(name: "LexicalTextKit", targets: ["LexicalTextKit"]),
    .library(name: "LexicalAppKit", targets: ["LexicalAppKit"]),
    .library(name: "LexicalUIKit", targets: ["LexicalUIKit"]),
    .library(name: "LexicalSwiftUI", targets: ["LexicalSwiftUI"]),
    .library(name: "LexicalExtension", targets: ["LexicalExtension"]),
    .library(name: "LexicalSelection", targets: ["LexicalSelection"]),
    .library(name: "LexicalUtils", targets: ["LexicalUtils"]),
    .library(name: "LexicalHTML", targets: ["LexicalHTML"]),
    .library(name: "LexicalRichText", targets: ["LexicalRichText"]),
    .library(name: "LexicalClipboard", targets: ["LexicalClipboard"]),
    .library(name: "LexicalHistory", targets: ["LexicalHistory"]),
    .library(name: "LexicalHeadless", targets: ["LexicalHeadless"]),
    .library(name: "LexicalPlainText", targets: ["LexicalPlainText"]),
  ],
  dependencies: [
    // The `@LexicalNode` macro (Q30).
    .package(url: "https://github.com/swiftlang/swift-syntax", from: "604.0.0")
  ],
  targets: [
    .target(name: "Lexical", dependencies: ["LexicalMacros"]),
    .macro(
      name: "LexicalMacros",
      dependencies: [
        .product(name: "SwiftSyntaxMacros", package: "swift-syntax"),
        .product(name: "SwiftCompilerPlugin", package: "swift-syntax"),
      ]),
    .testTarget(
      name: "LexicalMacrosTests",
      dependencies: [
        "LexicalMacros",
        .product(name: "SwiftSyntaxMacroExpansion", package: "swift-syntax"),
        .product(name: "SwiftSyntaxMacrosGenericTestSupport", package: "swift-syntax"),
      ]),
    // @lexical/extension: the builder, signals and the core extensions.
    .target(name: "LexicalExtension", dependencies: ["Lexical"]),
    .testTarget(
      name: "LexicalExtensionTests",
      dependencies: [
        "Lexical", "LexicalClipboard", "LexicalExtension", "LexicalHeadless", "LexicalHistory",
        "LexicalHTML", "LexicalRichText", "LexicalSelection", "LexicalTesting", "LexicalUtils",
      ]),
    // @lexical/selection: style patching, block conversion and wrapping.
    .target(name: "LexicalSelection", dependencies: ["Lexical"]),
    .testTarget(
      name: "LexicalSelectionTests",
      dependencies: [
        "Lexical", "LexicalClipboard", "LexicalExtension", "LexicalHistory", "LexicalRichText",
        "LexicalSelection", "LexicalTesting", "LexicalUtils",
      ]),
    // @lexical/utils: traversal, insertion and editing helpers.
    .target(name: "LexicalUtils", dependencies: ["Lexical", "LexicalSelection"]),
    .testTarget(
      name: "LexicalUtilsTests",
      dependencies: [
        "Lexical", "LexicalClipboard", "LexicalExtension", "LexicalHTML", "LexicalRichText",
        "LexicalSelection", "LexicalTesting", "LexicalUtils",
      ]),
    // @lexical/html: HTML export and import over the DOM in core.
    .target(
      name: "LexicalHTML", dependencies: ["Lexical", "LexicalExtension", "LexicalSelection"]),
    .testTarget(
      name: "LexicalHTMLTests",
      dependencies: [
        "Lexical", "LexicalExtension", "LexicalHTML", "LexicalRichText", "LexicalTesting",
      ]),
    // @lexical/rich-text: headings, quotes and the rich text commands.
    .target(
      name: "LexicalRichText",
      dependencies: [
        "Lexical", "LexicalClipboard", "LexicalExtension", "LexicalHTML", "LexicalSelection",
        "LexicalUtils",
      ]),
    .testTarget(
      name: "LexicalRichTextTests",
      dependencies: [
        "Lexical", "LexicalExtension", "LexicalHTML", "LexicalRichText", "LexicalTesting",
      ]),
    // @lexical/clipboard: copy and paste in the Lexical, HTML and plain text
    // formats.
    .target(
      name: "LexicalClipboard",
      dependencies: [
        "Lexical", "LexicalExtension", "LexicalHTML", "LexicalSelection", "LexicalUtils",
      ]),
    .testTarget(
      name: "LexicalClipboardTests",
      dependencies: [
        "Lexical", "LexicalClipboard", "LexicalExtension", "LexicalHTML", "LexicalRichText",
        "LexicalSelection", "LexicalTesting",
      ]),
    // @lexical/history: undo and redo.
    .target(name: "LexicalHistory", dependencies: ["Lexical", "LexicalExtension", "LexicalUtils"]),
    .testTarget(
      name: "LexicalHistoryTests",
      dependencies: [
        "Lexical", "LexicalClipboard", "LexicalExtension", "LexicalHistory", "LexicalHTML",
        "LexicalPlainText", "LexicalRichText", "LexicalSelection", "LexicalTesting", "LexicalUtils",
      ]),
    // @lexical/plain-text: the plain text commands.
    .target(
      name: "LexicalPlainText",
      dependencies: [
        "Lexical", "LexicalClipboard", "LexicalExtension", "LexicalSelection", "LexicalUtils",
      ]),
    // @lexical/headless: an editor that never attaches to a view.
    .target(name: "LexicalHeadless", dependencies: ["Lexical"]),
    .testTarget(
      name: "LexicalHeadlessTests",
      dependencies: [
        "Lexical", "LexicalExtension", "LexicalHeadless", "LexicalHTML", "LexicalRichText",
        "LexicalTesting",
      ]),
    // The platform-neutral half of the text views: the document they show
    // and the input controller that turns native input into commands.
    .target(name: "LexicalTextKit", dependencies: ["Lexical"]),
    .testTarget(
      name: "LexicalTextKitTests",
      dependencies: ["Lexical", "LexicalRichText", "LexicalTesting", "LexicalTextKit"]),
    // The TextKit 2 editor views (Apple-only; empty modules elsewhere).
    .target(name: "LexicalAppKit", dependencies: ["Lexical", "LexicalTextKit"]),
    .testTarget(
      name: "LexicalAppKitTests", dependencies: ["Lexical", "LexicalAppKit", "LexicalTextKit"]),
    .target(name: "LexicalUIKit", dependencies: ["Lexical", "LexicalTextKit"]),
    .testTarget(
      name: "LexicalUIKitTests", dependencies: ["Lexical", "LexicalTextKit", "LexicalUIKit"]),
    .target(
      name: "LexicalSwiftUI",
      dependencies: [
        "Lexical",
        .target(name: "LexicalAppKit", condition: .when(platforms: [.macOS])),
        .target(
          name: "LexicalUIKit", condition: .when(platforms: [.iOS, .visionOS, .macCatalyst])),
      ]),
    // Test support: the `.upstream(...)` trait the parity ledger reads.
    .target(name: "LexicalTesting", dependencies: ["Lexical"]),
    // Replays packages/lexical-conformance/vectors, which only the monorepo
    // carries; skipped elsewhere.
    .testTarget(
      name: "ConformanceTests", dependencies: ["Lexical", "LexicalHTML", "LexicalTesting"]),
    // Ports of the upstream unit tests, each marked with `.upstream(...)`.
    .testTarget(
      name: "LexicalTests",
      dependencies: [
        "Lexical", "LexicalExtension", "LexicalHistory", "LexicalHTML", "LexicalRichText",
        "LexicalTesting",
      ]),
  ],
  swiftLanguageModes: [.v6]
)
