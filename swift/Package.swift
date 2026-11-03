// swift-tools-version: 6.4
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
  ],
  targets: [
    .target(name: "Lexical"),
    // Test support: the `.upstream(...)` trait the parity ledger reads.
    .target(name: "LexicalTesting", dependencies: ["Lexical"]),
    // Replays packages/lexical-conformance/vectors, which only the monorepo
    // carries; skipped elsewhere.
    .testTarget(name: "ConformanceTests", dependencies: ["Lexical", "LexicalTesting"]),
  ],
  swiftLanguageModes: [.v6]
)
