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
    .library(name: "LexicalExtension", targets: ["LexicalExtension"]),
    .library(name: "LexicalSelection", targets: ["LexicalSelection"]),
    .library(name: "LexicalUtils", targets: ["LexicalUtils"]),
    .library(name: "LexicalHTML", targets: ["LexicalHTML"]),
    .library(name: "LexicalRichText", targets: ["LexicalRichText"]),
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
      dependencies: ["Lexical", "LexicalExtension", "LexicalTesting"]),
    // @lexical/selection: style patching, block conversion and wrapping.
    .target(name: "LexicalSelection", dependencies: ["Lexical"]),
    .testTarget(
      name: "LexicalSelectionTests",
      dependencies: ["Lexical", "LexicalExtension", "LexicalSelection", "LexicalTesting"]),
    // @lexical/utils: traversal, insertion and editing helpers.
    .target(name: "LexicalUtils", dependencies: ["Lexical", "LexicalSelection"]),
    .testTarget(
      name: "LexicalUtilsTests",
      dependencies: [
        "Lexical", "LexicalExtension", "LexicalSelection", "LexicalUtils", "LexicalTesting",
      ]),
    // @lexical/html: HTML export and import over the DOM in core.
    .target(
      name: "LexicalHTML", dependencies: ["Lexical", "LexicalExtension", "LexicalSelection"]),
    .testTarget(
      name: "LexicalHTMLTests",
      dependencies: ["Lexical", "LexicalExtension", "LexicalHTML", "LexicalTesting"]),
    // @lexical/rich-text: headings, quotes and the rich text commands.
    .target(
      name: "LexicalRichText",
      dependencies: ["Lexical", "LexicalExtension", "LexicalSelection", "LexicalUtils"]),
    .testTarget(
      name: "LexicalRichTextTests",
      dependencies: [
        "Lexical", "LexicalExtension", "LexicalHTML", "LexicalRichText", "LexicalTesting",
      ]),
    // The TextKit 2 text views and the platform-neutral document they show.
    .target(name: "LexicalTextKit", dependencies: ["Lexical"]),
    .testTarget(name: "LexicalTextKitTests", dependencies: ["Lexical", "LexicalTextKit"]),
    // Test support: the `.upstream(...)` trait the parity ledger reads.
    .target(name: "LexicalTesting", dependencies: ["Lexical"]),
    // Replays packages/lexical-conformance/vectors, which only the monorepo
    // carries; skipped elsewhere.
    .testTarget(
      name: "ConformanceTests", dependencies: ["Lexical", "LexicalHTML", "LexicalTesting"]),
    // Ports of the upstream unit tests, each marked with `.upstream(...)`.
    .testTarget(name: "LexicalTests", dependencies: ["Lexical", "LexicalTesting"]),
  ],
  swiftLanguageModes: [.v6]
)
