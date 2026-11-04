import Foundation
import Lexical
import LexicalLink
import LexicalMarkdown
import Testing

// Port of the `emphasis next to Unicode punctuation and symbols` describe
// block of packages/lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts.
// CommonMark counts every character in the Unicode P (punctuation) and S
// (symbol) general categories as punctuation when it decides whether a
// delimiter run is left- or right-flanking, not only the ASCII ones. The
// tests that also read the export back with @lexical/mdast are in
// skips.json.

private let EN_DASH = "\u{2013}"  // Pd
private let LEFT_DOUBLE_QUOTE = "\u{201C}"  // Pi
private let RIGHT_DOUBLE_QUOTE = "\u{201D}"  // Pf
private let LOW_DOUBLE_QUOTE = "\u{201E}"  // Ps
private let EURO_SIGN = "\u{20AC}"  // Sc
private let LATIN_E_ACUTE = "\u{00E9}"  // Ll, a letter
private let CYRILLIC_BE = "\u{0431}"  // Ll, a letter
private let HERE_LINK = "[here](https://example.com)"
private let GRINNING_FACE = "\u{1F600}"
private let QUOTED_Y = "\(LEFT_DOUBLE_QUOTE)y\(RIGHT_DOUBLE_QUOTE)"

/// A text node's text and format.
private struct FormatRun: Equatable, CustomStringConvertible {
  let text: String
  let format: Int
  init(_ text: String, _ format: Int) {
    self.text = text
    self.format = format
  }
  var description: String { "[\(text.debugDescription), \(format)]" }
}

private func runs(_ pairs: [(String, Int)]) -> [FormatRun] { pairs.map { FormatRun($0.0, $0.1) } }

private func importMarkdown(
  _ editor: Editor, _ md: String, _ transformers: [any Transformer] = TRANSFORMERS,
  _ preserveNewLines: Bool = false
) throws {
  try editor.update(discrete: true) {
    try convertFromMarkdownString(md, transformers, nil, preserveNewLines)
  }
}

private func exportMarkdown(
  _ editor: Editor, _ transformers: [any Transformer] = TRANSFORMERS,
  _ preserveNewLines: Bool = false
) throws -> String {
  try editor.read(.latest) { try convertToMarkdownString(transformers, nil, preserveNewLines) }
}

/// The text nodes of the document, mapped by `map`.
private func textNodes<T>(_ editor: Editor, _ map: @escaping (TextNode) throws -> T) throws -> [T] {
  try editor.read(.latest) { try getRoot().getAllTextNodes().map(map) }
}

private func formatRuns(_ editor: Editor) throws -> [FormatRun] {
  try textNodes(editor) { FormatRun(try $0.getTextContent(), try $0.getFormat()) }
}

/// An editor whose root holds one paragraph of `runs`.
private func paragraphEditor(_ runs: [(String, Int)]) throws -> Editor {
  let editor = try createMarkdownTestEditor()
  try editor.update(discrete: true) {
    try getRoot().append(
      createParagraphNode().append(
        contentsOf: runs.map { try createTextNode($0.0).setFormat($0.1) }))
  }
  return editor
}

private func formatOf(_ formats: [TextFormatType]) -> Int {
  formats.reduce(0) { $0 | (TEXT_TYPE_TO_FORMAT[$1] ?? 0) }
}

/// `importRuns`: each text node of `md` with its bold and italic formats,
/// as `"bold italic"`, `"bold"`, `"italic"` or `"plain"`.
private func importRuns(_ md: String) throws -> [[String]] {
  let editor = try createMarkdownTestEditor()
  try importMarkdown(editor, md)
  return try textNodes(editor) { node in
    let formats = try ["bold", "italic"].filter { try node.hasFormat($0) }
    return [try node.getTextContent(), formats.isEmpty ? "plain" : formats.joined(separator: " ")]
  }
}

@Suite struct MarkdownEmphasisPunctuationTests {
  // Each expected result matches what micromark, a CommonMark parser,
  // produces for the same input.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"See –**[here](https://example.com)** now.\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"See “**[here](https://example.com)**” now.\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"See „**[here](https://example.com)**“ now.\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"See €**[here](https://example.com)** now.\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"See **[here](https://example.com)**–now.\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"See –*[here](https://example.com)* now.\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"See –_[here](https://example.com)_ now.\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"See –__[here](https://example.com)__ now.\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"See -**[here](https://example.com)** now.\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"See **[here](https://example.com)** now.\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"a**b**\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"**a**b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"snake_case_word\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"é**[here](https://example.com)**\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"б**[here](https://example.com)**\""
    ),
    arguments: [
      (
        "See \(EN_DASH)**\(HERE_LINK)** now.",
        [["See \(EN_DASH)", "plain"], ["here", "bold"], [" now.", "plain"]]
      ),
      (
        "See \(LEFT_DOUBLE_QUOTE)**\(HERE_LINK)**\(RIGHT_DOUBLE_QUOTE) now.",
        [
          ["See \(LEFT_DOUBLE_QUOTE)", "plain"], ["here", "bold"],
          ["\(RIGHT_DOUBLE_QUOTE) now.", "plain"],
        ]
      ),
      (
        "See \(LOW_DOUBLE_QUOTE)**\(HERE_LINK)**\(LEFT_DOUBLE_QUOTE) now.",
        [
          ["See \(LOW_DOUBLE_QUOTE)", "plain"], ["here", "bold"],
          ["\(LEFT_DOUBLE_QUOTE) now.", "plain"],
        ]
      ),
      (
        "See \(EURO_SIGN)**\(HERE_LINK)** now.",
        [["See \(EURO_SIGN)", "plain"], ["here", "bold"], [" now.", "plain"]]
      ),
      // Only the closing run touches a Unicode punctuation character.
      (
        "See **\(HERE_LINK)**\(EN_DASH)now.",
        [["See ", "plain"], ["here", "bold"], ["\(EN_DASH)now.", "plain"]]
      ),
      (
        "See \(EN_DASH)*\(HERE_LINK)* now.",
        [["See \(EN_DASH)", "plain"], ["here", "italic"], [" now.", "plain"]]
      ),
      (
        "See \(EN_DASH)_\(HERE_LINK)_ now.",
        [["See \(EN_DASH)", "plain"], ["here", "italic"], [" now.", "plain"]]
      ),
      (
        "See \(EN_DASH)__\(HERE_LINK)__ now.",
        [["See \(EN_DASH)", "plain"], ["here", "bold"], [" now.", "plain"]]
      ),
      // Unchanged: ASCII punctuation and whitespace still work.
      (
        "See -**\(HERE_LINK)** now.", [["See -", "plain"], ["here", "bold"], [" now.", "plain"]]
      ),
      ("See **\(HERE_LINK)** now.", [["See ", "plain"], ["here", "bold"], [" now.", "plain"]]),
      // Unchanged: a delimiter run inside a word.
      ("a**b**", [["a", "plain"], ["b", "bold"]]),
      ("**a**b", [["a", "bold"], ["b", "plain"]]),
      ("snake_case_word", [["snake_case_word", "plain"]]),
      // Unchanged: a letter is not punctuation, whatever the script.
      (
        "\(LATIN_E_ACUTE)**\(HERE_LINK)**",
        [["\(LATIN_E_ACUTE)**", "plain"], ["here", "plain"], ["**", "plain"]]
      ),
      (
        "\(CYRILLIC_BE)**\(HERE_LINK)**",
        [["\(CYRILLIC_BE)**", "plain"], ["here", "plain"], ["**", "plain"]]
      ),
    ] as [(String, [[String]])])
  func imports(_ md: String, _ expected: [[String]]) throws {
    #expect(try importRuns(md) == expected)
  }

  // Importing the bold link as text used to leave the asterisks in the text,
  // so the export escaped them. The exporter writes the bold inside the link.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports \"See –**[here](https://example.com)** now.\" without escaped asterisks"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports \"See “**[here](https://example.com)**” now.\" without escaped asterisks"
    ),
    arguments: [
      (
        "See \(EN_DASH)**\(HERE_LINK)** now.",
        "See \(EN_DASH)[**here**](https://example.com) now."
      ),
      (
        "See \(LEFT_DOUBLE_QUOTE)**\(HERE_LINK)**\(RIGHT_DOUBLE_QUOTE) now.",
        "See \(LEFT_DOUBLE_QUOTE)[**here**](https://example.com)\(RIGHT_DOUBLE_QUOTE) now."
      ),
    ] as [(String, String)])
  func exportsWithoutEscapedAsterisks(_ md: String, _ exported: String) throws {
    let editor = try createMarkdownTestEditor()
    try importMarkdown(editor, md)
    #expect(try exportMarkdown(editor) == exported)
  }

  // Characters outside the Basic Multilingual Plane are read whole: an emoji
  // is a symbol (So), so it is punctuation too.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"a**😀**\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"**😀**a\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"a **😀**\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports \"x﻿**“a”**\""
    ),
    arguments: [
      ("a**\(GRINNING_FACE)**", [["a**\(GRINNING_FACE)**", "plain"]]),
      ("**\(GRINNING_FACE)**a", [["**\(GRINNING_FACE)**a", "plain"]]),
      ("a **\(GRINNING_FACE)**", [["a ", "plain"], [GRINNING_FACE, "bold"]]),
      // U+FEFF is a format character (Cf), not CommonMark whitespace.
      (
        "x\u{FEFF}**\(LEFT_DOUBLE_QUOTE)a\(RIGHT_DOUBLE_QUOTE)**",
        [["x\u{FEFF}**\(LEFT_DOUBLE_QUOTE)a\(RIGHT_DOUBLE_QUOTE)**", "plain"]]
      ),
    ] as [(String, [[String]])])
  func importsOutsideBMP(_ md: String, _ expected: [[String]]) throws {
    #expect(try importRuns(md) == expected)
  }

  // A tag between a word character and punctuation cannot open or close, so
  // the word character goes out as a numeric reference, whose `&` or `;` is
  // punctuation.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [[\"foo\",[]],[\"“bar”\",[\"bold\"]],[\"baz\",[]]] as \"fo&#111;**“bar”**&#98;az\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [[\"a\",[]],[\"😀\",[\"italic\"]],[\"b\",[]]] as \"&#97;*😀*&#98;\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [[\"a\",[]],[\"x\",[\"bold\",\"code\"]],[\"b\",[]]] as \"&#97;**`x`**&#98;\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [[\"x.\",[\"bold\"]],[\"b\",[]],[\"(y)\",[\"bold\"]]] as \"**x.**&#98;**(y)**\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [[\"b\",[\"bold\",\"italic\"]],[\"“y”\",[\"italic\"]]] as \"***&#98;****“y”*\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [[\"a\",[\"bold\"]],[\"!\",[\"strikethrough\"]]] as \"**a**~~!~~\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [[\"x\",[]],[\"a\",[\"bold\"]],[\"“b”\",[\"italic\"]]] as \"&#120;**&#97;***“b”*\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [[\"“a”\",[\"bold\",\"italic\"]],[\"b\",[\"italic\"]],[\"c\",[]]] as \"***“a”****&#98;*&#99;\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [[\"x\",[]],[\"a\",[\"bold\"]],[\"“b”\",[\"bold\",\"italic\"]]] as \"&#120;**&#97;*“b”***\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [[\"联系我@微博你好\",[]],[\"“重要”\",[\"bold\"]]] as \"联系我@微博你&#22909;**“重要”**\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [[\"(a@b.co)and\",[]],[\"“x”\",[\"bold\"]]] as \"(a@b.co)an&#100;**“x”**\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [[\"a\",[]],[\"b\",[\"bold\"]],[\"c\",[]]] as \"a**b**c\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [[\"a=\",[]],[\"“b”\",[\"highlight\"]],[\"=c\",[]]] as \"a&#61;==“b”==&#61;c\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [[\"a.\",[\"bold\"]],[\"x\",[]],[\"c\",[\"bold\",\"code\"]]] as \"**a.**&#120;**`c`**\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports and reimports [[\"“\",[\"bold\"]],[\".b \",[\"italic\"]],[\"c\",[\"italic\",\"code\"]]] as \"**“***.b `c`*\""
    ),
    arguments: [
      (
        [("foo", []), ("\(LEFT_DOUBLE_QUOTE)bar\(RIGHT_DOUBLE_QUOTE)", ["bold"]), ("baz", [])],
        "fo&#111;**\(LEFT_DOUBLE_QUOTE)bar\(RIGHT_DOUBLE_QUOTE)**&#98;az"
      ),
      ([("a", []), (GRINNING_FACE, ["italic"]), ("b", [])], "&#97;*\(GRINNING_FACE)*&#98;"),
      ([("a", []), ("x", ["bold", "code"]), ("b", [])], "&#97;**`x`**&#98;"),
      ([("x.", ["bold"]), ("b", []), ("(y)", ["bold"])], "**x.**&#98;**(y)**"),
      // Italic closes and reopens around the bold's end.
      ([("b", ["bold", "italic"]), (QUOTED_Y, ["italic"])], "***&#98;****“y”*"),
      // Markers of different characters are separate delimiter runs.
      ([("a", ["bold"]), ("!", ["strikethrough"])], "**a**~~!~~"),
      // A one-character reference reads as punctuation on its other side
      // too, so the run there is decided again.
      (
        [("x", []), ("a", ["bold"]), ("\(LEFT_DOUBLE_QUOTE)b\(RIGHT_DOUBLE_QUOTE)", ["italic"])],
        "&#120;**&#97;***“b”*"
      ),
      (
        [
          ("\(LEFT_DOUBLE_QUOTE)a\(RIGHT_DOUBLE_QUOTE)", ["bold", "italic"]), ("b", ["italic"]),
          ("c", []),
        ],
        "***“a”****&#98;*&#99;"
      ),
      (
        [
          ("x", []), ("a", ["bold"]),
          ("\(LEFT_DOUBLE_QUOTE)b\(RIGHT_DOUBLE_QUOTE)", ["bold", "italic"]),
        ],
        "&#120;**&#97;*“b”***"
      ),
      // An `@` that is not part of a link where the text ends does not stop
      // the reference.
      (
        [("联系我@微博你好", []), ("\(LEFT_DOUBLE_QUOTE)重要\(RIGHT_DOUBLE_QUOTE)", ["bold"])],
        "联系我@微博你&#22909;**\(LEFT_DOUBLE_QUOTE)重要\(RIGHT_DOUBLE_QUOTE)**"
      ),
      (
        [("(a@b.co)and", []), ("\(LEFT_DOUBLE_QUOTE)x\(RIGHT_DOUBLE_QUOTE)", ["bold"])],
        "(a@b.co)an&#100;**\(LEFT_DOUBLE_QUOTE)x\(RIGHT_DOUBLE_QUOTE)**"
      ),
      // A word character beside a word character needs no reference.
      ([("a", []), ("b", ["bold"]), ("c", [])], "a**b**c"),
      // A tag character that text does not escape is kept out of the marker.
      (
        [("a=", []), ("\(LEFT_DOUBLE_QUOTE)b\(RIGHT_DOUBLE_QUOTE)", ["highlight"]), ("=c", [])],
        "a&#61;==“b”==&#61;c"
      ),
      // Emphasis that ends before a code span is read before the span.
      ([("a.", ["bold"]), ("x", []), ("c", ["bold", "code"])], "**a.**&#120;**`c`**"),
      (
        [(LEFT_DOUBLE_QUOTE, ["bold"]), (".b ", ["italic"]), ("c", ["italic", "code"])],
        "**“***.b `c`*"
      ),
    ] as [([(String, [TextFormatType])], String)])
  func exportsAndReimports(_ runs: [(String, [TextFormatType])], _ exported: String) throws {
    let editor = try paragraphEditor(runs.map { ($0.0, formatOf($0.1)) })
    let markdown = try exportMarkdown(editor)
    #expect(markdown == exported)
    try importMarkdown(editor, markdown)
    let formats: [TextFormatType] = ["bold", "italic", "strikethrough", "highlight", "code"]
    let reimported = try textNodes(editor) { node in
      [try node.getTextContent()] + (try formats.filter { try node.hasFormat($0) })
    }
    #expect(reimported == runs.map { [$0.0] + $0.1 })
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > guards a word character after a raw backslash when preserving newlines"
    ))
  func guardsWordCharacterAfterRawBackslash() throws {
    let editor = try paragraphEditor([(#"x\b"#, 0), (QUOTED_Y, IS_BOLD)])
    let markdown = try exportMarkdown(editor, TRANSFORMERS, true)
    #expect(markdown == "x&#92;&#98;**\(QUOTED_Y)**")
    try importMarkdown(editor, markdown, TRANSFORMERS, true)
    #expect(try formatRuns(editor) == runs([(#"x\b"#, 0), (QUOTED_Y, IS_BOLD)]))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps a raw backslash from escaping a bold marker when preserving newlines"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps a raw backslash from escaping a italic marker when preserving newlines"
    ), arguments: [("bold", "a&#92;**b**"), ("italic", "a&#92;*b*")])
  func rawBackslashBeforeMarker(_ format: TextFormatType, _ exported: String) throws {
    let editor = try paragraphEditor([(#"a\"#, 0), ("b", formatOf([format]))])
    let markdown = try exportMarkdown(editor, TRANSFORMERS, true)
    #expect(markdown == exported)
    try importMarkdown(editor, markdown, TRANSFORMERS, true)
    let reimported = try textNodes(editor) {
      "\(try $0.getTextContent()) \(try $0.hasFormat(format))"
    }
    #expect(reimported == [#"a\ false"#, "b true"])
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps raw backslashes from escaping an escape when preserving newlines"
    ))
  func rawBackslashesBeforeEscape() throws {
    let editor = try paragraphEditor([(#"a\\_b_"#, 0), ("c", IS_BOLD)])
    let markdown = try exportMarkdown(editor, TRANSFORMERS, true)
    #expect(markdown == #"a\\\\\_b\_**c**"#)
    try importMarkdown(editor, markdown, TRANSFORMERS, true)
    #expect(try formatRuns(editor) == runs([(#"a\\_b_"#, 0), ("c", IS_BOLD)]))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes no reference that would break a run beside raw output"
    ))
  func noReferenceBreakingRunBesideRawOutput() throws {
    // Writing `a` as a reference for the italic would leave the bold's `**`
    // between the raw `x` and the reference's `&`, unable to open.
    let mention = TextMatchTransformer(
      dependencies: [],
      export: { node, _, _ in
        try (node as? TextNode)?.getTextContent() == "@x" ? "@x" : nil
      },
      regExp: JSRegExp("@x$"), replace: { _, _ in nil })
    let editor = try paragraphEditor([("@x", 0), ("a", IS_BOLD), (QUOTED_Y, IS_BOLD | IS_ITALIC)])
    let markdown = try exportMarkdown(editor, [mention] + TRANSFORMERS)
    #expect(!markdown.contains("&#97;"))
    try importMarkdown(editor, markdown)
    let hasBoldA = try textNodes(editor) {
      try $0.getTextContent().hasPrefix("a") && $0.hasFormat("bold")
    }
    #expect(hasBoldA.contains(true))
  }

  // GFM links none of these: a mention is not Markdown, an email needs a dot
  // in its domain, and a URL needs `http`, `https` or `ftp` for its scheme.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference after \"@john\", which readers do not autolink"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference after \"foo@bar\", which readers do not autolink"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference after \"a://b\", which readers do not autolink"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference after \"xhttp://b\", which readers do not autolink"
    ),
    arguments: [
      ("@john", "@joh&#110;"), ("foo@bar", "foo@ba&#114;"), ("a://b", "a://&#98;"),
      ("xhttp://b", "xhttp://&#98;"),
    ] as [(String, String)])
  func referenceAfterNonAutolink(_ text: String, _ markdown: String) throws {
    let quotedX = "\(LEFT_DOUBLE_QUOTE)x\(RIGHT_DOUBLE_QUOTE)"
    let editor = try paragraphEditor([("see \(text)", 0), (quotedX, IS_BOLD)])
    #expect(try exportMarkdown(editor) == "see \(markdown)**\(quotedX)**")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads formatting after a text match whose syntax has a letter beside it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads formatting before a text match whose syntax has a letter beside it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads formatting in italic after a text match whose syntax has a letter beside it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads formatting nested after a text match whose syntax has a letter beside it"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads formatting far after a text match whose syntax has a letter beside it"
    ),
    arguments: [
      ("user:bob**(admin)**", "(admin)", 1),
      ("**(admin)**user:bob", "(admin)", 1),
      ("user:bob*\u{201c}a\u{201d}*", "\u{201c}a\u{201d}", 2),
      ("user:bob**(text *more*)**", "more", 3),
      (
        "user:bob**(" + String(repeating: "x ", count: 40) + "y)**",
        "(" + String(repeating: "x ", count: 40) + "y)", 1
      ),
    ] as [(String, String, Int)])
  func formattingBesideLetterEdgedTextMatch(
    _ markdown: String, _ text: String, _ format: Int
  ) throws {
    // A mention-style transformer whose syntax starts and ends in a letter.
    let mention = TextMatchTransformer(
      dependencies: [LinkNode.self],
      export: { node, _, _ in
        guard let link = node as? LinkNode, try link.getURL().hasPrefix("mention:") else {
          return nil
        }
        return "user:" + String(try link.getURL().dropFirst("mention:".count))
      },
      importRegExp: JSRegExp(#"user:(\w+)"#), regExp: JSRegExp(#"user:(\w+)$"#),
      replace: { textNode, match in
        let link = try createLinkNode("mention:" + (match[1] ?? ""))
        try link.append(createTextNode(match[1] ?? ""))
        try textNode.replace(link)
        return nil
      })
    let transformers: [any Transformer] = [mention] + TRANSFORMERS
    let editor = try createMarkdownTestEditor()
    try importMarkdown(editor, markdown, transformers)
    #expect(try formatRuns(editor).contains(FormatRun(text, format)))
    #expect(try exportMarkdown(editor, transformers) == markdown)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run after a link the same whatever follows it on the line, with a short run"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run after a link the same whatever follows it on the line, with a long run"
    ), arguments: ["#", "#" + String(repeating: "x", count: 80)])
  func runAfterLinkWhateverFollows(_ inner: String) throws {
    // The run reads the link's end as the edge of a line, so it opens without
    // closing, and the rule of 3 does not keep `*` from closing it.
    let editor = try createMarkdownTestEditor()
    for tail in ["", " [b](v)", " " + String(repeating: "y", count: 80)] {
      try importMarkdown(editor, "[a](u)**\(inner)*\(tail)")
      #expect(
        Array(try formatRuns(editor).prefix(3))
          == runs([("a", 0), ("*", 0), (inner, IS_ITALIC)]))
    }
  }

  // JS reads the named group's range from `match.indices.groups.name`; the
  // port's match has no named ranges, so the name is checked through
  // `namedGroups` and its range is `ranges[1]`, the named group's index.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > gives a text match found earlier in the line its named groups where they are"
    ))
  func namedGroupsOfEarlierMatch() throws {
    var ranges: [String] = []
    let mention = TextMatchTransformer(
      dependencies: [], importRegExp: JSRegExp(#"@(?<name>\w+)"#, "d"),
      regExp: JSRegExp(#"@(?<name>\w+)$"#, "d"),
      replace: { textNode, match in
        let range = try #require(match.ranges[1])
        let name = (match.input as NSString).substring(
          with: NSRange(location: range.lowerBound, length: range.count))
        ranges.append(contentsOf: [name, (match.namedGroups["name"] ?? nil) ?? ""])
        try textNode.setTextContent(name)
        return nil
      })
    let editor = try createMarkdownTestEditor()
    try importMarkdown(editor, "**a** @bob", [mention] + TRANSFORMERS)
    #expect(ranges == ["bob", "bob"])
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes no reference for the `.` of `www.` that is a format's marker"
    ))
  func noReferenceForWwwDotMarker() throws {
    let dot = TextFormatTransformer(format: ["italic"], tag: ".")
    let editor = try paragraphEditor([("www", 0), ("x.co", IS_ITALIC), (" or www.y.co", 0)])
    #expect(try exportMarkdown(editor, [dot]) == "www.x.co. or www.y.co")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes no reference for an email's `@` after a raw backslash"
    ))
  func noReferenceForEmailAtAfterRawBackslash() throws {
    let editor = try paragraphEditor([(#"x\@b.co"#, 0), ("\u{201c}y\u{201d}", IS_BOLD)])
    #expect(try exportMarkdown(editor, TRANSFORMERS, true) == "x\\@b.c&#111;**\u{201c}y\u{201d}**")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > spoils a URL for the reference to a backslash before a marker"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > spoils a URL for the reference to a backslash before a code span"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > spoils a URL for the reference to a tag character before its marker"
    ),
    arguments: [
      ([(#"see http://x.com/a\"#, 0), ("b", IS_BOLD)], "see http&#58;//x.com/a&#92;**b**"),
      ([(#"see http://x.com/a\"#, 0), ("b", IS_CODE)], "see http&#58;//x.com/a&#92;`b`"),
      ([("see https://x.com/?a=", 0), ("b", IS_HIGHLIGHT)], "see https&#58;//x.com/?a&#61;==b=="),
    ] as [([(String, Int)], String)])
  func spoilsURLForReference(_ textRuns: [(String, Int)], _ markdown: String) throws {
    let editor = try paragraphEditor(textRuns)
    #expect(try exportMarkdown(editor, TRANSFORMERS, true) == markdown)
    try importMarkdown(editor, markdown, TRANSFORMERS, true)
    #expect(try formatRuns(editor) == runs(textRuns))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference after CJK punctuation that ends bold before a CJK character"
    ))
  func referenceAfterCJKPunctuation() throws {
    let editor = try paragraphEditor([
      ("\u{4f60}\u{597d}\u{3002}", IS_BOLD), ("\u{4e16}\u{754c}", 0),
    ])
    let markdown = try exportMarkdown(editor)
    #expect(markdown == "**\u{4f60}\u{597d}\u{3002}**&#19990;\u{754c}")
    try importMarkdown(editor, markdown)
    #expect(
      try formatRuns(editor)
        == runs([("\u{4f60}\u{597d}\u{3002}", IS_BOLD), ("\u{4e16}\u{754c}", 0)]))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > exports bold italic text followed by italic text so that it re-imports (#9443)"
    ))
  func boldItalicThenItalic() throws {
    let editor = try paragraphEditor([("a", IS_BOLD | IS_ITALIC), (": b", IS_ITALIC)])
    let markdown = try exportMarkdown(editor)
    #expect(markdown == "***&#97;****: b*")
    try importMarkdown(editor, markdown)
    #expect(try formatRuns(editor) == runs([("a", IS_BOLD | IS_ITALIC), (": b", IS_ITALIC)]))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes references around an underscore italic between letters"
    ))
  func underscoreItalicBetweenLetters() throws {
    let transformers = TRANSFORMERS.filter { $0 !== ITALIC_STAR }
    let editor = try paragraphEditor([("a", 0), ("b", IS_ITALIC), ("c", 0)])
    let markdown = try exportMarkdown(editor, transformers)
    #expect(markdown == "&#97;_b_&#99;")
    try importMarkdown(editor, markdown, transformers)
    #expect(try formatRuns(editor) == runs([("a", 0), ("b", IS_ITALIC), ("c", 0)]))
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference outside a URL inside a code span"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference outside a URL that starts later in the run"
    ),
    arguments: [
      (
        [("a", 0), ("\u{201c}b\u{201d}", IS_BOLD), ("http://y", IS_CODE)],
        "&#97;**\u{201c}b\u{201d}**`http://y`"
      ),
      (
        [("foo", 0), ("\u{201c}bar\u{201d}", IS_BOLD), ("(www.x.com)", 0)],
        "fo&#111;**\u{201c}bar\u{201d}**(www.x.com)"
      ),
    ] as [([(String, Int)], String)])
  func referenceOutside(_ textRuns: [(String, Int)], _ markdown: String) throws {
    #expect(try exportMarkdown(paragraphEditor(textRuns)) == markdown)
  }

  /// Imports `markdown` (with `transformers`) and expects its text nodes'
  /// formats to be `expected`.
  private func expectImport(
    _ markdown: String, _ expected: [(String, Int)],
    _ transformers: [any Transformer] = TRANSFORMERS
  ) throws {
    let editor = try createMarkdownTestEditor()
    try importMarkdown(editor, markdown, transformers)
    #expect(try formatRuns(editor) == runs(expected))
  }

  // Not as micromark reads them: it also lets a run open before, and close
  // after, another `*`, `_` or `~` marker.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run beside another marker as CommonMark does in \"**bold***~~gone~~*\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run beside another marker as CommonMark does in \"**a***_b_*\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run beside another marker as CommonMark does in \"**~~**a\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run beside another marker as CommonMark does in \"~~a*~~b\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run beside another marker as CommonMark does in \"[a](u)*`c`)**\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a run beside another marker as CommonMark does in \"**)*(.`*b!****_\""
    ),
    arguments: [
      ("**bold***~~gone~~*", [("bold", IS_BOLD), ("*", 0), ("gone", IS_STRIKETHROUGH), ("*", 0)]),
      ("**a***_b_*", [("a", IS_BOLD), ("*", 0), ("b", IS_ITALIC), ("*", 0)]),
      ("**~~**a", [("**~~**a", 0)]),
      ("~~a*~~b", [("~~a*~~b", 0)]),
      ("[a](u)*`c`)**", [("a", 0), ("*", 0), ("c", IS_CODE), (")**", 0)]),
      ("**)*(.`*b!****_", [(")", IS_BOLD), ("(.`b!", IS_BOLD | IS_ITALIC), ("_", 0)]),
    ] as [(String, [(String, Int)])])
  func runBesideAnotherMarker(_ markdown: String, _ expected: [(String, Int)]) throws {
    try expectImport(markdown, expected)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a highlight inside a format as a line of its own, and GFM's markers as GFM does, in \"**==`====**\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a highlight inside a format as a line of its own, and GFM's markers as GFM does, in \"**~~`~~~~**\""
    ),
    arguments: [
      ("**==`====**", [("`", IS_BOLD | IS_HIGHLIGHT), ("==", IS_BOLD)]),
      ("**~~`~~~~**", [("~~`~~~~", IS_BOLD)]),
    ] as [(String, [(String, Int)])])
  func highlightInsideFormat(_ markdown: String, _ expected: [(String, Int)]) throws {
    try expectImport(markdown, expected)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > opens a code span after an escaped backtick in \"x\\\\``bb`x\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > opens a code span after an escaped backtick in \"\\\\``a` `b`\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > opens a code span after an escaped backtick in \"`a\\\\`\""
    ),
    arguments: [
      (#"x\``bb`x"#, [("x`", 0), ("bb", IS_CODE), ("x", 0)]),
      (#"\``a` `b`"#, [("`", 0), ("a", IS_CODE), (" ", 0), ("b", IS_CODE)]),
      (#"`a\`"#, [(#"a\"#, IS_CODE)]),
    ] as [(String, [(String, Int)])])
  func codeSpanAfterEscapedBacktick(_ markdown: String, _ expected: [(String, Int)]) throws {
    try expectImport(markdown, expected)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads the rest of a run that a text match cuts in \":a**b*\" on its own"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads the rest of a run that a text match cuts in \":a***b*\" on its own"
    ),
    arguments: [
      (":a**b*", [("X", 0), ("b", IS_ITALIC)]),
      (":a***b*", [("X**b*", 0)]),
    ] as [(String, [(String, Int)])])
  func restOfRunCutByTextMatch(_ markdown: String, _ expected: [(String, Int)]) throws {
    let tag = TextMatchTransformer(
      dependencies: [], importRegExp: JSRegExp(#":[a-z]+\*"#), regExp: JSRegExp(#":[a-z]+\*$"#),
      replace: { textNode, _ in
        try textNode.setTextContent("X")
        return nil
      })
    try expectImport(markdown, expected, [tag] + TRANSFORMERS)
  }

  // A `;` before it starts no URL: `www.` needs whitespace or `*_~(` before
  // it.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > writes a reference before \"bwww.x.co\", which readers do not autolink"
    ), arguments: ["bwww.x.co"])
  func referenceBeforeNonAutolink(_ text: String) throws {
    let editor = try paragraphEditor([(".", IS_BOLD), (text, 0)])
    #expect(try exportMarkdown(editor) == "**.**&#98;\(text.dropFirst())")
  }

  // Upstream counts the characters a `RegExp` subclass's `exec` searches and
  // expects fewer than twice the line. `JSRegExp` is a final class whose
  // `exec` cannot be overridden, so the port checks the import only.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > searches a line once for a text match that it does not hold"
    ))
  func searchesLineOnceForAbsentTextMatch() throws {
    let never = TextMatchTransformer(
      dependencies: [], importRegExp: JSRegExp(#"\$\$[^$]+\$\$"#),
      regExp: JSRegExp(#"\$\$[^$]+\$\$$"#), replace: { _, _ in nil })
    let markdown = String(repeating: "**a** [b](c) ", count: 20)
    let editor = try createMarkdownTestEditor()
    try importMarkdown(editor, markdown, [never] + TRANSFORMERS)
    #expect(try editor.read(.latest) { try getRoot().getAllTextNodes().count } == 80)
  }

  // Upstream spies on `RegExp.prototype.exec` and expects fewer than five
  // searches with `LINK.importRegExp`; Swift cannot spy on `JSRegExp.exec`,
  // so the port checks the import only.
  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > searches the line for a link a few times with escaped openers"
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > searches the line for a link a few times with code spans before a link"
    ),
    arguments: [
      String(repeating: #"\["#, count: 100) + "a](u)",
      String(repeating: "[`a`", count: 100) + "](u)",
    ] as [String])
  func searchesForLinkAFewTimes(_ markdown: String) throws {
    let editor = try createMarkdownTestEditor()
    try importMarkdown(editor, markdown)
    #expect(
      try editor.read(.latest) { try getRoot().getTextContent() }
        == (markdown.hasPrefix("\\")
          ? markdown.replacingOccurrences(of: "\\", with: "")
          : String(repeating: "[a", count: 99) + "a"))
  }

  /// A mention whose replacement trims the start of the text after it.
  private func trimmingMention() -> TextMatchTransformer {
    TextMatchTransformer(
      dependencies: [], importRegExp: JSRegExp(#"@(\w+)"#), regExp: JSRegExp(#"@(\w+)$"#),
      replace: { textNode, _ in
        if let next = try textNode.getNextSibling() as? TextNode {
          let text = try next.getTextContent()
          try next.setTextContent(String(text.drop { $0.isJSWhitespace }))
        }
        try textNode.toggleFormat("code")
        return nil
      })
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > scans the text after a match again when its transformer changes it"
    ))
  func rescansTextAfterChangedMatch() throws {
    try expectImport(
      "@bob *a* *b*", [("@bob", IS_CODE), ("a", IS_ITALIC), (" ", 0), ("b", IS_ITALIC)],
      [trimmingMention()] + TRANSFORMERS)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads the text after a match on its own when its transformer changes it"
    ))
  func readsTextAfterChangedMatchOnItsOwn() throws {
    try expectImport(
      "@bob _a_ _b_", [("@bob", IS_CODE), ("a", IS_ITALIC), (" ", 0), ("b", IS_ITALIC)],
      [trimmingMention()] + TRANSFORMERS)
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > imports the text after a match before the text inside it"
    ))
  func importsTextAfterMatchFirst() throws {
    var seen: [String] = []
    let tag = TextMatchTransformer(
      dependencies: [], importRegExp: JSRegExp("@x"), regExp: JSRegExp("@x$"),
      replace: { textNode, _ in
        seen.append(try textNode.getParentOrThrow().getTextContent())
        return nil
      })
    let editor = try createMarkdownTestEditor()
    try importMarkdown(editor, "**_a_** @x", [tag] + TRANSFORMERS)
    #expect(seen == ["_a_ @x"])
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > guards a letter right after an inline link's destination"
    ))
  func guardsLetterAfterLinkDestination() throws {
    let editor = try createMarkdownTestEditor()
    try editor.update(discrete: true) {
      try getRoot().append(
        createParagraphNode().append(
          createLinkNode("https://e.com").append(createTextNode("API")), createTextNode("s"),
          createTextNode("(beta)").toggleFormat("bold"), createTextNode(" end")))
    }
    #expect(try exportMarkdown(editor) == "[API](https://e.com)&#115;**(beta)** end")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a delimiter run cut by an earlier match as it was in \"*)**a*\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a delimiter run cut by an earlier match as it was in \"“_)__a(“[)_\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > reads a delimiter run cut by an earlier match as it was in \"*a**`x`*\""
    ),
    arguments: [
      ("*)**a*", [("*)*", false), ("a", true)]),
      ("“_)__a(“[)_", [("“_)_", false), ("a(“[)", true)]),
      ("*a**`x`*", [("a", true), ("*", false), ("x", false), ("*", false)]),
    ] as [(String, [(String, Bool)])])
  func delimiterRunCutByEarlierMatch(_ markdown: String, _ expected: [(String, Bool)]) throws {
    let editor = try createMarkdownTestEditor()
    try importMarkdown(editor, markdown)
    let italic = try textNodes(editor) {
      "\(try $0.getTextContent()) \(try $0.hasFormat("italic"))"
    }
    #expect(italic == expected.map { "\($0.0) \($0.1)" })
  }

  /// Each text node as `text code link`, whether it is code and in a link.
  private func codeAndLinks(_ editor: Editor) throws -> [String] {
    try textNodes(editor) {
      "\(try $0.getTextContent()) \(try $0.hasFormat("code")) \(try $0.getParent() is LinkNode)"
    }
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps a code span after a stray `[` before a link in \"Use [this](u) or the [`--flag` from [docs](u)\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps a code span after a stray `[` before a link in \"[a](b) [`x`[c](d)\""
    ),
    arguments: [
      (
        "Use [this](u) or the [`--flag` from [docs](u)",
        [
          ("Use ", false, false), ("this", false, true), (" or the [", false, false),
          ("--flag", true, false), (" from ", false, false), ("docs", false, true),
        ]
      ),
      (
        "[a](b) [`x`[c](d)",
        [("a", false, true), (" [", false, false), ("x", true, false), ("c", false, true)]
      ),
    ] as [(String, [(String, Bool, Bool)])])
  func codeSpanAfterStrayBracket(_ markdown: String, _ expected: [(String, Bool, Bool)]) throws {
    let editor = try createMarkdownTestEditor()
    try importMarkdown(editor, markdown)
    #expect(try codeAndLinks(editor) == expected.map { "\($0.0) \($0.1) \($0.2)" })
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > applies a link before a later link that holds a code span"
    ))
  func linkBeforeLaterLinkWithCodeSpan() throws {
    let editor = try createMarkdownTestEditor()
    try importMarkdown(editor, "[a](https://u) and [`b`](https://v)")
    #expect(try codeAndLinks(editor) == ["a false true", " and  false false", "b true true"])
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps emphasis around a link after a code span in \"`c` *[a](b)*\""
    ),
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps emphasis around a link after a code span in \"`c` x *[a](b) `d`*\""
    ),
    arguments: [
      ("`c` *[a](b)*", [("c", false), (" ", false), ("a", true)]),
      (
        "`c` x *[a](b) `d`*", [("c", false), (" x ", false), ("a", true), (" ", true), ("d", true)]
      ),
    ] as [(String, [(String, Bool)])])
  func emphasisAroundLinkAfterCodeSpan(_ markdown: String, _ expected: [(String, Bool)]) throws {
    let editor = try createMarkdownTestEditor()
    try importMarkdown(editor, markdown)
    let italic = try textNodes(editor) {
      "\(try $0.getTextContent()) \(try $0.hasFormat("italic"))"
    }
    #expect(italic == expected.map { "\($0.0) \($0.1)" })
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > keeps a link from taking part of a later code span"
    ))
  func linkDoesNotTakePartOfLaterCodeSpan() throws {
    let editor = try createMarkdownTestEditor()
    try importMarkdown(editor, "**`a` [b** `c](d)`")
    let nodes = try textNodes(editor) {
      "\(try $0.getTextContent()) \(try $0.hasFormat("bold")) \(try $0.hasFormat("code"))"
    }
    #expect(nodes == ["a true true", " [b true false", "  false false", "c](d) false true"])
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > guards word characters in a selection export"
    ))
  func guardsWordCharactersInSelectionExport() throws {
    let quotedBar = "\(LEFT_DOUBLE_QUOTE)bar\(RIGHT_DOUBLE_QUOTE)"
    let editor = try createMarkdownTestEditor()
    try editor.update(discrete: true) {
      let bold = try createTextNode(quotedBar).toggleFormat("bold")
      let after = try createTextNode("baz")
      try getRoot().append(createParagraphNode().append(createTextNode("foo"), bold, after))
      try bold.select(0, 0).focus.set(after.getKey(), 2, .text)
    }
    #expect(
      try editor.read(.latest) {
        try convertSelectionToMarkdownString(TRANSFORMERS, getSelection())
      } == "**\(quotedBar)**&#98;a")
  }

  @Test(
    .upstream(
      "lexical-markdown/src/__tests__/unit/LexicalMarkdown.test.ts::emphasis next to Unicode punctuation and symbols > guards word characters across an inline element without a transformer"
    ))
  func guardsWordCharactersAcrossInlineElement() throws {
    let quotedBar = "\(LEFT_DOUBLE_QUOTE)bar\(RIGHT_DOUBLE_QUOTE)"
    let transformers = TRANSFORMERS.filter { $0 !== LINK }
    let editor = try createMarkdownTestEditor()
    try editor.update(discrete: true) {
      try getRoot().append(
        createParagraphNode().append(
          createTextNode("foo"),
          createLinkNode("https://example.com").append(
            createTextNode(quotedBar).toggleFormat("bold")),
          createTextNode("baz")))
    }
    #expect(try exportMarkdown(editor, transformers) == "fo&#111;**\(quotedBar)**&#98;az")
  }
}

extension Character {
  /// Whether JS `String.prototype.trimStart` removes this character.
  fileprivate var isJSWhitespace: Bool {
    unicodeScalars.allSatisfy {
      $0.properties.isWhitespace || $0 == "\u{FEFF}"
    }
  }
}
