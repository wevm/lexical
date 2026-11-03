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
    // The TextKit 2 text views and the platform-neutral document they show.
    .target(name: "LexicalTextKit", dependencies: ["Lexical"]),
    .testTarget(name: "LexicalTextKitTests", dependencies: ["Lexical", "LexicalTextKit"]),
    // Test support: the `.upstream(...)` trait the parity ledger reads.
    .target(name: "LexicalTesting", dependencies: ["Lexical"]),
    // Replays packages/lexical-conformance/vectors, which only the monorepo
    // carries; skipped elsewhere.
    .testTarget(name: "ConformanceTests", dependencies: ["Lexical", "LexicalTesting"]),
    // Ports of the upstream unit tests, each marked with `.upstream(...)`.
    .testTarget(name: "LexicalTests", dependencies: ["Lexical", "LexicalTesting"]),
  ],
  swiftLanguageModes: [.v6]
)
