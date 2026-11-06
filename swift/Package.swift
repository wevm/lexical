// swift-tools-version: 6.4
import CompilerPluginSupport
import PackageDescription

// One module per upstream package, added as each one is ported. The non-UI
// modules build and test on Linux; the UIKit, AppKit, and SwiftUI layers are
// Apple-only.
/// The vendored tree-sitter grammars: the target suffix, the grammar's
/// source directory, and whether it has an external scanner.
let treeSitterGrammars: [(name: String, src: String, hasScanner: Bool)] = [
  ("Bash", "src", true), ("CSS", "src", true), ("Go", "src", false), ("HTML", "src", true),
  ("JavaScript", "src", true), ("JSON", "src", false), ("Python", "src", true),
  ("Rust", "src", true), ("Swift", "src", true), ("TypeScript", "typescript/src", true),
  ("C", "src", false), ("CPP", "src", true), ("Java", "src", false),
  ("Markdown", "tree-sitter-markdown/src", true), ("ObjC", "src", false),
  ("PowerShell", "src", true), ("SQL", "src", true), ("XML", "xml/src", true),
]

let package = Package(
  name: "Lexical",
  platforms: [.iOS(.v26), .macOS(.v26)],
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
    .library(name: "LexicalA11y", targets: ["LexicalA11y"]),
    .library(name: "LexicalDevtoolsCore", targets: ["LexicalDevtoolsCore"]),
    .library(name: "LexicalList", targets: ["LexicalList"]),
    .library(name: "LexicalLink", targets: ["LexicalLink"]),
    .library(name: "LexicalCodeCore", targets: ["LexicalCodeCore"]),
    .library(name: "LexicalMarkdown", targets: ["LexicalMarkdown"]),
    .library(name: "LexicalOverflow", targets: ["LexicalOverflow"]),
    .library(name: "LexicalText", targets: ["LexicalText"]),
    .library(name: "LexicalMenu", targets: ["LexicalMenu"]),
    .library(name: "LexicalDecoratorBlockNode", targets: ["LexicalDecoratorBlockNode"]),
    .library(name: "LexicalAutoEmbed", targets: ["LexicalAutoEmbed"]),
    .library(name: "LexicalHashtag", targets: ["LexicalHashtag"]),
    .library(name: "LexicalMark", targets: ["LexicalMark"]),
    .library(name: "LexicalFile", targets: ["LexicalFile"]),
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
        "Lexical", "LexicalClipboard", "LexicalCodeCore", "LexicalExtension", "LexicalHeadless",
        "LexicalHistory", "LexicalHTML", "LexicalLink", "LexicalList", "LexicalMark",
        "LexicalMarkdown", "LexicalOverflow", "LexicalRichText", "LexicalSelection",
        "LexicalTesting", "LexicalUtils",
      ]),
    // @lexical/selection: style patching, block conversion and wrapping.
    .target(name: "LexicalSelection", dependencies: ["Lexical"]),
    .testTarget(
      name: "LexicalSelectionTests",
      dependencies: [
        "Lexical", "LexicalClipboard", "LexicalExtension", "LexicalHistory", "LexicalLink",
        "LexicalList", "LexicalMark", "LexicalMarkdown", "LexicalOverflow",
        "LexicalRichText", "LexicalSelection", "LexicalTesting", "LexicalUtils",
      ]),
    // @lexical/utils: traversal, insertion and editing helpers.
    .target(name: "LexicalUtils", dependencies: ["Lexical", "LexicalSelection"]),
    .testTarget(
      name: "LexicalUtilsTests",
      dependencies: [
        "Lexical", "LexicalClipboard", "LexicalExtension", "LexicalHTML", "LexicalLink",
        "LexicalList", "LexicalMark",
        "LexicalRichText", "LexicalSelection", "LexicalTesting", "LexicalText", "LexicalUtils",
      ]),
    // @lexical/html: HTML export and import over the DOM in core.
    .target(
      name: "LexicalHTML", dependencies: ["Lexical", "LexicalExtension", "LexicalSelection"]),
    .testTarget(
      name: "LexicalHTMLTests",
      dependencies: [
        "Lexical", "LexicalExtension", "LexicalHTML", "LexicalLink", "LexicalList",
        "LexicalMark", "LexicalRichText",
        "LexicalTesting",
      ]),
    // @lexical/rich-text: headings, quotes and the rich text commands.
    .target(
      name: "LexicalRichText",
      dependencies: [
        "Lexical", "LexicalA11y", "LexicalClipboard", "LexicalExtension", "LexicalHTML",
        "LexicalSelection", "LexicalUtils",
      ]),
    .testTarget(
      name: "LexicalRichTextTests",
      dependencies: [
        "Lexical", "LexicalA11y", "LexicalExtension", "LexicalHTML", "LexicalList",
        "LexicalPlainText", "LexicalRichText", "LexicalTesting",
      ]),
    // @lexical/list: list nodes, the list commands and checklists.
    .target(
      name: "LexicalList",
      dependencies: ["Lexical", "LexicalExtension", "LexicalHTML", "LexicalUtils"]),
    .testTarget(
      name: "LexicalListTests",
      dependencies: [
        "Lexical", "LexicalClipboard", "LexicalExtension", "LexicalHistory", "LexicalHTML",
        "LexicalLink", "LexicalList", "LexicalRichText", "LexicalTesting", "LexicalUtils",
      ]),
    // @lexical/link: link nodes, the link commands and autolinks.
    .target(
      name: "LexicalLink",
      dependencies: ["Lexical", "LexicalA11y", "LexicalExtension", "LexicalHTML", "LexicalUtils"]),
    .testTarget(
      name: "LexicalLinkTests",
      dependencies: [
        "Lexical", "LexicalA11y", "LexicalCodeCore", "LexicalExtension", "LexicalHistory",
        "LexicalHTML", "LexicalLink", "LexicalList", "LexicalMark", "LexicalRichText",
        "LexicalTesting", "LexicalUtils",
      ]),
    // @lexical/code-core: code block nodes, without a highlighter.
    .target(
      name: "LexicalCodeCore",
      dependencies: ["Lexical", "LexicalExtension", "LexicalHTML", "LexicalUtils"]),
    .testTarget(
      name: "LexicalCodeCoreTests",
      dependencies: [
        "Lexical", "LexicalClipboard", "LexicalCodeCore", "LexicalExtension", "LexicalHistory",
        "LexicalHTML", "LexicalLink", "LexicalList", "LexicalMarkdown", "LexicalRichText",
        "LexicalSelection", "LexicalTesting", "LexicalUtils",
      ]),
    // @lexical/markdown: markdown import, export and shortcuts.
    .target(
      name: "LexicalMarkdown",
      dependencies: [
        "Lexical", "LexicalCodeCore", "LexicalExtension", "LexicalLink", "LexicalList",
        "LexicalRichText", "LexicalSelection", "LexicalUtils",
      ]),
    .testTarget(
      name: "LexicalMarkdownTests",
      dependencies: [
        "Lexical", "LexicalCodeCore", "LexicalExtension", "LexicalHashtag", "LexicalHistory",
        "LexicalHTML", "LexicalLink", "LexicalList", "LexicalMarkdown", "LexicalRichText",
        "LexicalSelection", "LexicalTesting", "LexicalUtils",
      ]),
    // @lexical/overflow: the character limit's OverflowNode.
    .target(name: "LexicalOverflow", dependencies: ["Lexical", "LexicalText", "LexicalUtils"]),
    .testTarget(
      name: "LexicalOverflowTests",
      dependencies: [
        "Lexical", "LexicalExtension", "LexicalOverflow", "LexicalRichText", "LexicalTesting",
      ]),
    // @lexical/text: text entities, placeholder and text content helpers.
    .target(name: "LexicalText", dependencies: ["Lexical"]),
    .testTarget(
      name: "LexicalTextTests",
      dependencies: [
        "Lexical", "LexicalExtension", "LexicalHashtag", "LexicalHistory", "LexicalList",
        "LexicalMark", "LexicalMarkdown", "LexicalRichText", "LexicalTesting", "LexicalText",
      ]),
    // The platform-neutral half of @lexical/react's typeahead and node menus.
    .target(name: "LexicalMenu", dependencies: ["Lexical"]),
    .testTarget(
      name: "LexicalMenuTests",
      dependencies: ["Lexical", "LexicalMenu", "LexicalRichText", "LexicalTesting"]),
    // @lexical/react's DecoratorBlockNode, the base of block embeds, and
    // the commands of BlockWithAlignableContents.
    .target(
      name: "LexicalDecoratorBlockNode",
      dependencies: ["Lexical", "LexicalExtension", "LexicalUtils"]),
    .testTarget(
      name: "LexicalDecoratorBlockNodeTests",
      dependencies: [
        "Lexical", "LexicalDecoratorBlockNode", "LexicalExtension", "LexicalRichText",
      ]),
    // The platform-neutral half of @lexical/react's LexicalAutoEmbedPlugin.
    .target(
      name: "LexicalAutoEmbed",
      dependencies: ["Lexical", "LexicalLink", "LexicalMenu", "LexicalUtils"]
    ),
    .testTarget(
      name: "LexicalAutoEmbedTests",
      dependencies: [
        "Lexical", "LexicalAutoEmbed", "LexicalExtension", "LexicalLink", "LexicalMenu",
        "LexicalRichText", "LexicalTesting", "LexicalUtils",
      ]),
    // @lexical/hashtag: hashtag nodes, made from text by LexicalText.
    .target(name: "LexicalHashtag", dependencies: ["Lexical", "LexicalText"]),
    .target(name: "LexicalFile", dependencies: ["Lexical"]),
    .testTarget(
      name: "LexicalFileTests",
      dependencies: [
        "Lexical", "LexicalExtension", "LexicalFile", "LexicalHistory", "LexicalRichText",
        "LexicalTesting",
      ]),
    .testTarget(
      name: "LexicalHashtagTests",
      dependencies: ["Lexical", "LexicalExtension", "LexicalHashtag", "LexicalTesting"]),
    // @lexical/mark: mark nodes and mark ids in NodeState (comments).
    .target(
      name: "LexicalMark",
      dependencies: ["Lexical", "LexicalExtension", "LexicalHTML", "LexicalUtils"]),
    .testTarget(
      name: "LexicalMarkTests",
      dependencies: [
        "Lexical", "LexicalClipboard", "LexicalCodeCore", "LexicalExtension", "LexicalHashtag",
        "LexicalHistory", "LexicalHTML", "LexicalLink", "LexicalList", "LexicalMark",
        "LexicalRichText", "LexicalSelection", "LexicalTesting", "LexicalUtils",
      ]),
    // @lexical/a11y: the live region and the announcers.
    .target(name: "LexicalA11y", dependencies: ["Lexical", "LexicalExtension"]),
    .testTarget(
      name: "LexicalA11yTests",
      dependencies: [
        "Lexical", "LexicalA11y", "LexicalExtension", "LexicalHistory", "LexicalRichText",
        "LexicalTesting",
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
        "Lexical", "LexicalClipboard", "LexicalExtension", "LexicalHTML", "LexicalList",
        "LexicalMark", "LexicalRichText", "LexicalSelection", "LexicalTesting",
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
    .target(name: "LexicalTextKit", dependencies: ["Lexical", "LexicalCodeCore", "LexicalText"]),
    .testTarget(
      name: "LexicalTextKitTests",
      dependencies: [
        "Lexical", "LexicalExtension", "LexicalHashtag", "LexicalHistory", "LexicalList",
        "LexicalOverflow", "LexicalPlainText", "LexicalRichText", "LexicalSelection",
        "LexicalTesting", "LexicalTextKit", "LexicalCodeCore",
      ]),
    // The TextKit 2 editor views (Apple-only; empty modules elsewhere).
    .target(name: "LexicalAppKit", dependencies: ["Lexical", "LexicalA11y", "LexicalTextKit"]),
    .testTarget(
      name: "LexicalAppKitTests",
      dependencies: [
        "LexicalA11y",
        "Lexical", "LexicalAppKit", "LexicalCodeCore", "LexicalExtension", "LexicalHistory",
        "LexicalLink", "LexicalList", "LexicalRichText", "LexicalSwiftUI", "LexicalTextKit",
      ]),
    .target(name: "LexicalUIKit", dependencies: ["Lexical", "LexicalA11y", "LexicalTextKit"]),
    .testTarget(
      name: "LexicalUIKitTests",
      dependencies: [
        "LexicalA11y",
        "Lexical", "LexicalCodeCore", "LexicalExtension", "LexicalHistory", "LexicalList",
        "LexicalRichText", "LexicalSwiftUI", "LexicalTextKit", "LexicalUIKit",
      ]),
    .target(
      name: "LexicalSwiftUI",
      dependencies: [
        "Lexical", "LexicalTextKit",
        .target(name: "LexicalAppKit", condition: .when(platforms: [.macOS])),
        .target(
          name: "LexicalUIKit", condition: .when(platforms: [.iOS, .visionOS, .macCatalyst])),
      ]),
    // @lexical/devtools-core: the playground's tree view.
    .target(name: "LexicalDevtoolsCore", dependencies: ["Lexical", "LexicalHTML"]),
    // Test support: the `.upstream(...)` trait the parity ledger reads.
    .target(name: "LexicalTesting", dependencies: ["Lexical"]),
    // Gating in CI: `swift run -c release LexicalBenchmarks`.
    .executableTarget(
      name: "LexicalBenchmarks",
      dependencies: [
        "Lexical", "LexicalCodeCore", "LexicalExtension", "LexicalHistory", "LexicalRichText",
        "LexicalTextKit",
      ],
      path: "Benchmarks/LexicalBenchmarks"),
    // Replays packages/lexical-conformance/vectors, which only the monorepo
    // carries; skipped elsewhere.
    .testTarget(
      name: "ConformanceTests",
      dependencies: [
        "Lexical", "LexicalCodeCore", "LexicalDevtoolsCore", "LexicalExtension", "LexicalFile",
        "LexicalHashtag", "LexicalHistory", "LexicalHTML", "LexicalLink", "LexicalList",
        "LexicalMarkdown",
        "LexicalOverflow", "LexicalRichText", "LexicalSelection", "LexicalTesting", "LexicalText",
      ]),
    // Ports of the upstream unit tests, each marked with `.upstream(...)`.
    .testTarget(
      name: "LexicalTests",
      dependencies: [
        "Lexical", "LexicalExtension", "LexicalHistory", "LexicalHTML", "LexicalLink",
        "LexicalList",
        "LexicalRichText", "LexicalTesting",
      ]),
  ],
  swiftLanguageModes: [.v6]
)

// LexicalCodeTreeSitter: syntax highlighting with tree-sitter, whose C core
// and grammars are vendored by scripts/vendor-tree-sitter.sh. Each grammar
// is a product of its own, so an app links only the languages it uses.
package.products.append(
  .library(name: "LexicalCodeTreeSitter", targets: ["LexicalCodeTreeSitter"]))
package.targets += [
  .target(
    name: "CTreeSitter", exclude: ["LICENSE", "VERSION"], sources: ["lib/src/lib.c"],
    publicHeadersPath: "lib/include",
    cSettings: [
      .headerSearchPath("lib/src"), .define("_POSIX_C_SOURCE", to: "200112L"),
      .define("_DEFAULT_SOURCE"),
    ]),
  .target(
    name: "LexicalCodeTreeSitter", dependencies: ["CTreeSitter", "Lexical", "LexicalCodeCore"]),
  .testTarget(
    name: "LexicalCodeTreeSitterTests",
    dependencies: [
      "Lexical", "LexicalCodeCore", "LexicalCodeTreeSitter", "LexicalExtension",
      "LexicalRichText", "LexicalTesting", "TreeSitterJavaScript",
    ] + treeSitterGrammars.map { .byName(name: "LexicalCodeTreeSitter\($0.name)") }),
]
// The benchmark's code block row highlights JavaScript.
package.targets.first { $0.name == "LexicalBenchmarks" }!.dependencies += [
  "LexicalCodeTreeSitter", "LexicalCodeTreeSitterJavaScript",
]
for grammar in treeSitterGrammars {
  package.products.append(
    .library(
      name: "LexicalCodeTreeSitter\(grammar.name)",
      targets: ["LexicalCodeTreeSitter\(grammar.name)"]))
  package.targets += [
    .target(
      name: "TreeSitter\(grammar.name)", exclude: ["LICENSE", "VERSION", "queries"],
      sources: ["\(grammar.src)/parser.c", "highlights.c"]
        + (grammar.hasScanner ? ["\(grammar.src)/scanner.c"] : []),
      publicHeadersPath: "include", cSettings: [.headerSearchPath(grammar.src)]),
    .target(
      name: "LexicalCodeTreeSitter\(grammar.name)",
      dependencies: ["LexicalCodeTreeSitter", .byName(name: "TreeSitter\(grammar.name)")]),
  ]
}
