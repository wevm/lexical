import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalHashtag
import LexicalOverflow
import LexicalRichText
import LexicalTextKit
import Testing

// The attribute layer: CSS value parsing, every key of the default theme,
// and the cascade through blocks, text formats and inline styles.

private let theme = TextKitTheme.default
private let paragraph = TextKitElement(type: "paragraph")
private let root = TextKitElement(type: "root")

private func text(
  _ format: Int = 0, _ style: String = "", in ancestors: [TextKitElement] = [paragraph],
  theme: TextKitTheme = theme
) -> TextKitTextAttributes {
  theme.textAttributes(format: format, style: style, ancestors: ancestors)
}

private func near(_ a: Double, _ b: Double) -> Bool { abs(a - b) < 1e-9 }

private func near(_ a: TextKitColor?, _ b: TextKitColor) -> Bool {
  guard let a else { return false }
  return near(a.red, b.red) && near(a.green, b.green) && near(a.blue, b.blue)
    && near(a.alpha, b.alpha)
}

/// Plain text in a paragraph with the default theme.
private let plain: TextKitTextAttributes = {
  var a = TextKitTextAttributes()
  a.fontSize = 16
  a.fontWeight = 400
  a.foregroundColor = TextKitColor(0, 0, 0).withDark(TextKitColor(235, 235, 240))
  return a
}()

/// The default theme's secondary text and code background.
private let secondary = TextKitColor(101, 103, 107).withDark(TextKitColor(160, 163, 170))
private let codeBackground = TextKitColor(240, 242, 245).withDark(TextKitColor(44, 46, 51))

private let codeFamily = ["Menlo", "Consolas", "Monaco", "monospace"]

@Suite struct TextKitColorTests {
  @Test func darkVariant() {
    let color = TextKitColor(0, 0, 0).withDark(TextKitColor(235, 235, 240))
    #expect(color.darkAppearance == TextKitColor(235, 235, 240))
    #expect(color != TextKitColor(0, 0, 0))
    // A CSS color has no variant: it looks the same in both appearances.
    #expect(TextKitColor(css: "#123456")!.darkAppearance == TextKitColor(css: "#123456"))
  }

  /// The default theme's text is readable on a dark background: every
  /// foreground it sets is light in a dark appearance.
  @Test func defaultThemeHasDarkText() {
    for key in ["root", "quote", "link", "heading.h1", "heading.h2"] {
      let dark = try! #require(TextKitTheme.default[key]?.foregroundColor).darkAppearance
      #expect(0.2126 * dark.red + 0.7152 * dark.green + 0.0722 * dark.blue > 0.5, "\(key)")
    }
  }

  @Test func parsesHex() {
    #expect(TextKitColor(css: "#f00") == TextKitColor(255, 0, 0))
    #expect(TextKitColor(css: "#F0A8") == TextKitColor(255, 0, 170, alpha: 136.0 / 255))
    #expect(TextKitColor(css: " #d0021b ") == TextKitColor(208, 2, 27))
    #expect(TextKitColor(css: "#4a90e280") == TextKitColor(74, 144, 226, alpha: 128.0 / 255))
  }

  @Test func parsesRGB() {
    #expect(TextKitColor(css: "rgb(33, 111, 219)") == TextKitColor(33, 111, 219))
    #expect(TextKitColor(css: "RGBA(255,212,0,0.14)") == TextKitColor(255, 212, 0, alpha: 0.14))
    #expect(TextKitColor(css: "rgba(1, 2, 3)") == TextKitColor(1, 2, 3))
    #expect(TextKitColor(css: "rgb(10 20 30 / 0.5)") == TextKitColor(10, 20, 30, alpha: 0.5))
    #expect(
      near(TextKitColor(css: "rgb(100%, 0%, 300, 50%)"), TextKitColor(255, 0, 255, alpha: 0.5)))
  }

  @Test func parsesNamedColors() {
    #expect(TextKitColor(css: "Red") == TextKitColor(255, 0, 0))
    #expect(TextKitColor(css: "grey") == TextKitColor(128, 128, 128))
    #expect(TextKitColor(css: "orange") == TextKitColor(255, 165, 0))
    #expect(TextKitColor(css: "transparent") == TextKitColor(0, 0, 0, alpha: 0))
  }

  @Test(arguments: [
    "", "#", "#12", "#12345", "#ggg", "rgb(1, 2)", "rgb(1, 2, 3, 4, 5)", "rgb(a, b, c)",
    "hsl(0, 100%, 50%)", "rgb(1 2 / 3 / 4)", "blurple", "rgb(1, 2, nan)", "rgb(1, 2, 3",
  ])
  func rejectsOtherColors(_ css: String) {
    #expect(TextKitColor(css: css) == nil)
  }
}

@Suite struct TextKitFontSizeTests {
  @Test func parsesUnits() {
    #expect(TextKitFontSize(css: "15px") == .points(15))
    #expect(TextKitFontSize(css: " 12PT ") == .points(16))
    #expect(TextKitFontSize(css: "1.5em") == .scale(1.5))
    #expect(TextKitFontSize(css: "80%") == .scale(0.8))
  }

  @Test(arguments: ["large", "px", "-3px", "0px", "12", "12rem", "12 px", "calc(1px)"])
  func rejectsOtherSizes(_ css: String) {
    #expect(TextKitFontSize(css: css) == nil)
  }
}

@Suite struct TextKitAttributesTests {
  @Test func plainParagraph() {
    #expect(text() == plain)
    #expect(text(in: [root, paragraph]) == plain)
    #expect(theme.paragraphAttributes(blocks: [root, paragraph]).lineHeightMultiple == 1.7)
  }

  @Test func eachTextKey() {
    var expected: [Int: TextKitTextAttributes] = [:]
    func with(_ format: Int, _ change: (inout TextKitTextAttributes) -> Void) {
      var a = plain
      change(&a)
      expected[format] = a
    }
    with(IS_BOLD) { $0.fontWeight = 700 }
    with(IS_ITALIC) { $0.italic = true }
    with(IS_UNDERLINE) { $0.underline = true }
    with(IS_STRIKETHROUGH) { $0.strikethrough = true }
    with(IS_UNDERLINE | IS_STRIKETHROUGH) {
      $0.underline = true
      $0.strikethrough = true
    }
    with(IS_CODE) {
      $0.fontFamily = codeFamily
      $0.fontSize = 16 * 0.94
      $0.backgroundColor = codeBackground
    }
    with(IS_HIGHLIGHT) {
      $0.backgroundColor = TextKitColor(255, 212, 0, alpha: 0.14)
      $0.highlight = true
    }
    with(IS_SUBSCRIPT) {
      $0.fontSize = 0.8 * 16
      $0.baselineOffset = -0.2 * 16
    }
    with(IS_SUPERSCRIPT) {
      $0.fontSize = 0.8 * 16
      $0.baselineOffset = 0.33 * 16
    }
    with(IS_LOWERCASE) { $0.textTransform = .lowercase }
    with(IS_UPPERCASE) { $0.textTransform = .uppercase }
    with(IS_CAPITALIZE) { $0.textTransform = .capitalize }
    for (format, attributes) in expected {
      #expect(text(format) == attributes, "format \(format)")
    }
    #expect(text(IS_CODE).monospace)
    #expect(!text().monospace)
    #expect(text(IS_BOLD).bold && !text().bold)
  }

  @Test func eachBlockKey() {
    func heading(_ tag: String) -> TextKitTextAttributes {
      text(in: [TextKitElement(type: "heading", tag: tag)])
    }
    var h1 = plain
    h1.fontSize = 24
    h1.foregroundColor = TextKitColor(5, 5, 5).withDark(TextKitColor(245, 245, 250))
    #expect(heading("h1") == h1)
    var h2 = plain
    h2.fontWeight = 700
    h2.foregroundColor = secondary
    h2.textTransform = .uppercase
    #expect(heading("h2") == h2)
    var h3 = plain
    h3.fontSize = 12
    h3.textTransform = .uppercase
    #expect(heading("h3") == h3)
    for (tag, size) in [("h4", 16.0), ("h5", 0.83 * 16), ("h6", 0.67 * 16)] {
      var h = plain
      h.fontWeight = 700
      h.fontSize = size
      #expect(heading(tag) == h, "\(tag)")
    }

    var quote = plain
    quote.foregroundColor = secondary
    #expect(text(in: [TextKitElement(type: "quote")]) == quote)
    var quoteParagraph = TextKitParagraphAttributes()
    quoteParagraph.headIndent = 40
    quoteParagraph.spacingAfter = 10
    quoteParagraph.lineHeightMultiple = 1.7
    quoteParagraph.blockBars = [
      TextKitBlockBar(
        offset: 20, width: 4, color: TextKitColor(206, 208, 212).withDark(TextKitColor(84, 86, 92)))
    ]
    #expect(theme.paragraphAttributes(blocks: [TextKitElement(type: "quote")]) == quoteParagraph)

    // A code block's background is the paragraph's, not the text's.
    let code = TextKitElement(type: "code")
    var codeText = plain
    codeText.fontFamily = codeFamily
    codeText.fontSize = 13
    #expect(text(in: [code]) == codeText)
    var codeParagraph = TextKitParagraphAttributes()
    codeParagraph.headIndent = 52
    // The 8-point margins, outside the box, and its 8-point padding.
    codeParagraph.spacingBefore = 8 + 8
    codeParagraph.spacingAfter = 8 + 8
    codeParagraph.lineHeightMultiple = 1.53
    codeParagraph.backgroundColor = codeBackground
    codeParagraph.tabSize = 2
    codeParagraph.blockBox = TextKitBlockBox(
      offset: 0, paddingTop: 8, paddingBottom: 8, color: codeBackground,
      gutter: TextKitGutter(
        width: 25 + 2 * 8, padding: 8,
        backgroundColor: TextKitColor(238, 238, 238).withDark(TextKitColor(52, 54, 60)),
        borderWidth: 1, borderColor: TextKitColor(204, 204, 204).withDark(TextKitColor(72, 74, 80)),
        foregroundColor: TextKitColor(119, 119, 119).withDark(TextKitColor(150, 152, 158))))
    #expect(theme.paragraphAttributes(blocks: [code]) == codeParagraph)
    // In a quote the box starts at the block, inside the quote's indent.
    let quoted = theme.paragraphAttributes(blocks: [TextKitElement(type: "quote"), code])
    #expect(quoted.blockBox?.offset == 40)
    #expect(quoted.headIndent == 40 + 52)
    #expect(quoted.spacingBefore == 16)
    #expect(quoted.spacingAfter == 10 + 8)

    // Inline elements color the text and are skipped for the paragraph.
    let link = TextKitElement(type: "link", isInline: true)
    var linkText = plain
    linkText.foregroundColor = TextKitColor(33, 111, 219).withDark(TextKitColor(88, 166, 255))
    // `__link:hover { text-decoration: underline; cursor: pointer }`.
    linkText.cursor = .pointer
    linkText.hoverUnderline = true
    #expect(text(in: [paragraph, link]) == linkText)
    // An autolink takes the link's look, as its createDOM takes theme.link.
    #expect(text(in: [paragraph, TextKitElement(type: "autolink", isInline: true)]) == linkText)
    // Text formats come after the link: an underline format still shows.
    #expect(text(IS_UNDERLINE, in: [paragraph, link]).underline)
    let mark = TextKitElement(type: "mark", isInline: true)
    var markText = plain
    markText.backgroundColor = TextKitColor(255, 212, 0, alpha: 0.14)
    #expect(text(in: [paragraph, mark]) == markText)
    #expect(
      theme.paragraphAttributes(blocks: [paragraph, link, mark])
        == theme.paragraphAttributes(blocks: [paragraph]))
  }

  @Test func boldItalicCodeInAnH2InAnIndentedQuote() {
    let quote = TextKitElement(type: "quote", format: "right", indent: 2)
    let h2 = TextKitElement(type: "heading", tag: "h2", format: "center", indent: 1)
    let a = text(IS_BOLD | IS_ITALIC | IS_CODE, in: [root, quote, h2])
    var expected = plain
    expected.fontFamily = codeFamily
    expected.fontSize = 16 * 0.94
    expected.fontWeight = 700
    expected.italic = true
    expected.foregroundColor = secondary
    expected.backgroundColor = codeBackground
    expected.textTransform = .uppercase
    #expect(a == expected)
    // Lowercase on the text beats the heading's uppercase.
    #expect(text(IS_LOWERCASE, in: [quote, h2]).textTransform == .lowercase)

    let p = theme.paragraphAttributes(blocks: [root, quote, h2])
    #expect(p.alignment == .center)
    // The quote's 40 and its 2 levels inside its bar, then the h2's level.
    #expect(p.headIndent == 40 + 2 * 40 + 40)
    #expect(p.blockBars.map(\.offset) == [20])
    #expect(p.spacingAfter == 10)
    #expect(p.writingDirection == .natural)
  }

  @Test func nestedQuotesStackBars() {
    let outer = TextKitElement(type: "quote")
    let inner = TextKitElement(type: "quote", indent: 1)
    let p = theme.paragraphAttributes(blocks: [outer, inner, paragraph])
    #expect(p.blockBars.map(\.offset) == [20, 40 + 20])
    #expect(p.headIndent == 40 + 40 + 40)
  }

  @Test func listItemsShowTheirListsMarker() {
    let bullets = TextKitElement(type: "list", listType: "bullet")
    let numbers = TextKitElement(type: "list", listType: "number")
    let item = TextKitElement(type: "listitem", value: 3)
    let wrapper = TextKitElement(type: "listitem", value: 4)

    let top = theme.paragraphAttributes(blocks: [root, bullets, item])
    #expect(top.listMarker == .text("•"))
    #expect(top.headIndent == 32)
    #expect(theme.paragraphAttributes(blocks: [root, numbers, item]).listMarker == .text("3."))

    // A nested item: its wrapper item's margin adds up with its own, and
    // the depth picks the style.
    let nested = theme.paragraphAttributes(blocks: [root, numbers, wrapper, numbers, item])
    #expect(nested.listMarker == .text("C."))
    #expect(nested.headIndent == 64)
    #expect(
      theme.paragraphAttributes(blocks: [root, bullets, wrapper, bullets, item]).listMarker
        == .text("◦"))
    // A paragraph outside a list has none.
    #expect(theme.paragraphAttributes(blocks: [root, paragraph]).listMarker == nil)

    #expect(
      TextKitListMarker.marker(listType: "number", depth: 2, value: 28, checked: nil)
        == .text("ab."))
    #expect(
      TextKitListMarker.marker(listType: "number", depth: 3, value: 1994, checked: nil)
        == .text("MCMXCIV."))
    #expect(
      TextKitListMarker.marker(listType: "number", depth: 4, value: 9, checked: nil) == .text("ix.")
    )
    #expect(
      TextKitListMarker.marker(listType: "number", depth: 5, value: 9, checked: nil) == .text("9."))
    #expect(
      TextKitListMarker.marker(listType: "bullet", depth: 7, value: 1, checked: nil) == .text("▪"))
  }

  @Test func checkListItemsShowABoxAndStrikeThroughWhenChecked() {
    let checks = TextKitElement(type: "list", listType: "check")
    let done = TextKitElement(type: "listitem", value: 1, checked: true)
    let open = TextKitElement(type: "listitem", value: 2, checked: false)
    #expect(
      theme.paragraphAttributes(blocks: [root, checks, done]).listMarker == .checkbox(checked: true)
    )
    #expect(
      theme.paragraphAttributes(blocks: [root, checks, open]).listMarker
        == .checkbox(checked: false))
    #expect(text(in: [checks, done]).strikethrough)
    #expect(!text(in: [checks, open]).strikethrough)
  }

  @Test func inlineStyleWinsOverTheme() {
    let style =
      "color: #ff0000; font-size: 12pt; background-color: rgba(0, 0, 255, 0.5);"
      + " font-family: 'Courier New', monospace; font-weight: normal; font-style: normal;"
      + " text-decoration: none; text-transform: capitalize"
    let a = text(
      IS_BOLD | IS_ITALIC | IS_CODE | IS_UNDERLINE | IS_UPPERCASE, style,
      in: [TextKitElement(type: "heading", tag: "h1")])
    var expected = plain
    expected.fontFamily = ["Courier New", "monospace"]
    // Absolute: the 12pt is not scaled by `text.code`'s 94%.
    expected.fontSize = 16
    expected.foregroundColor = TextKitColor(255, 0, 0)
    expected.backgroundColor = TextKitColor(0, 0, 255, alpha: 0.5)
    expected.textTransform = .capitalize
    #expect(a == expected)
    #expect(a.monospace)
  }

  @Test func inlineStylesFromPatchStyleText() {
    #expect(text(0, "font-size: 15px").fontSize == 15)
    #expect(
      text(0, "font-size: 150%", in: [TextKitElement(type: "heading", tag: "h1")]).fontSize == 36)
    #expect(text(0, "font-family: Georgia").fontFamily == ["Georgia"])
    #expect(text(0, "color: rgb(65, 117, 5)").foregroundColor == TextKitColor(65, 117, 5))
    #expect(text(0, "background-color: #f8e71c").backgroundColor == TextKitColor(248, 231, 28))
    #expect(text(0, "background: yellow").backgroundColor == TextKitColor(255, 255, 0))
    #expect(text(0, "font-weight: 600").bold)
    #expect(text(0, "font-style: oblique").italic)
    let both = text(0, "text-decoration: line-through underline")
    #expect(both.underline && both.strikethrough)
  }

  @Test func unknownStyleValuesAreIgnored() {
    let style =
      "color: blurple; font-size: large; font-weight: heavy; font-style: wavy;"
      + " background-color: hsl(1, 2%, 3%); text-decoration: blink; text-transform: shout;"
      + " font-family: ''; unknown-property: 1; font-size: -4px"
    #expect(text(0, style) == plain)
    #expect(text(IS_CODE | IS_BOLD, style) == text(IS_CODE | IS_BOLD))
    // A later valid declaration of the same property still applies.
    #expect(text(0, "color: nope; color: navy").foregroundColor == TextKitColor(0, 0, 128))
  }

  @Test func underlineStrikethroughReplacesItsParts() {
    var custom = TextKitTheme.default
    custom["text.underlineStrikethrough"] = TextKitStyle(underline: true)
    let a = text(IS_UNDERLINE | IS_STRIKETHROUGH, theme: custom)
    #expect(a.underline && !a.strikethrough)
    custom["text.underlineStrikethrough"] = nil
    let b = text(IS_UNDERLINE | IS_STRIKETHROUGH, theme: custom)
    #expect(b.underline && b.strikethrough)
  }

  @Test func subscriptAndSuperscriptUseTheSurroundingSize() {
    let h1 = TextKitElement(type: "heading", tag: "h1")
    let sup = text(IS_SUPERSCRIPT, in: [h1])
    #expect(near(sup.fontSize, 24 * 0.8))
    #expect(near(sup.baselineOffset, 0.33 * 24))
    // One element's classes: the later key wins and sizes don't compound.
    let both = text(IS_SUBSCRIPT | IS_SUPERSCRIPT | IS_CODE)
    #expect(near(both.fontSize, 16 * 0.8))
    #expect(near(both.baselineOffset, 0.33 * 16))
  }

  @Test func customThemePerKey() {
    var custom = TextKitTheme(indentWidth: 24)
    custom["root"] = TextKitStyle(fontFamily: ["serif"], fontSize: .points(20))
    custom["text.bold"] = TextKitStyle(fontWeight: 900, foregroundColor: TextKitColor(1, 2, 3))
    custom["heading.h3"] = TextKitStyle(fontSize: .scale(2))
    custom["indent"] = TextKitStyle(foregroundColor: TextKitColor(9, 9, 9), headIndent: 4)
    let h3 = TextKitElement(type: "heading", tag: "h3", indent: 2)
    let a = text(IS_BOLD | IS_ITALIC, in: [h3], theme: custom)
    #expect(a.fontFamily == ["serif"])
    #expect(a.fontSize == 40)
    #expect(a.fontWeight == 900)
    #expect(!a.italic)
    #expect(a.foregroundColor == TextKitColor(1, 2, 3))
    #expect(text(in: [h3], theme: custom).foregroundColor == TextKitColor(9, 9, 9))
    #expect(custom.paragraphAttributes(blocks: [h3]).headIndent == 2 * 24 + 4)
    #expect(custom.paragraphAttributes(blocks: [h3]).lineHeightMultiple == nil)
    // An empty theme is the CSS initial look.
    #expect(text(in: [h3], theme: TextKitTheme()) == TextKitTextAttributes())
  }

  @Test func alignment() {
    for (format, alignment) in [
      ("", TextKitTextAlignment.natural), ("left", .left), ("center", .center),
      ("right", .right), ("justify", .justified), ("start", .natural), ("end", .end),
    ] {
      let block = TextKitElement(type: "paragraph", format: format)
      #expect(theme.paragraphAttributes(blocks: [block]).alignment == alignment, "\(format)")
    }
    // The innermost block that sets it wins; an unset one inherits.
    let quote = TextKitElement(type: "quote", format: "right")
    #expect(theme.paragraphAttributes(blocks: [quote, paragraph]).alignment == .right)
    let start = TextKitElement(type: "paragraph", format: "start")
    #expect(theme.paragraphAttributes(blocks: [quote, start]).alignment == .natural)
  }

  @Test func rightToLeft() {
    var custom = TextKitTheme.default
    custom["rtl"] = TextKitStyle(fontFamily: ["Geeza Pro"], headIndent: 2)
    custom["ltr"] = TextKitStyle(italic: true)
    let rtlQuote = TextKitElement(type: "quote", indent: 1, direction: "rtl")
    let p = custom.paragraphAttributes(blocks: [root, rtlQuote, paragraph])
    #expect(p.writingDirection == .rightToLeft)
    // Head indents are at the inline start, the right in RTL.
    #expect(p.headIndent == 40 + 40 + 2)
    #expect(p.blockBars.map(\.offset) == [20])
    #expect(text(in: [rtlQuote, paragraph], theme: custom).fontFamily == ["Geeza Pro"])
    #expect(!text(in: [rtlQuote, paragraph], theme: custom).italic)

    let ltr = TextKitElement(type: "paragraph", format: "end", direction: "ltr")
    let inner = custom.paragraphAttributes(blocks: [rtlQuote, ltr])
    #expect(inner.writingDirection == .leftToRight)
    #expect(inner.alignment == .end)
    #expect(text(in: [rtlQuote, ltr], theme: custom).italic)
    #expect(theme.paragraphAttributes(blocks: [paragraph]).writingDirection == .natural)
  }

  @Test func themeKeys() {
    #expect(TextKitElement(type: "heading", tag: "h5").themeKey == "heading.h5")
    #expect(TextKitElement(type: "heading").themeKey == "heading")
    #expect(TextKitElement(type: "paragraph").themeKey == "paragraph")
    #expect(TextKitElement(type: "quote", tag: "x").themeKey == "quote")
  }

  @Test func readsNodes() throws {
    let editor = try createEditor(
      CreateEditorArgs(
        namespace: "", nodes: [.klass(HeadingNode.self), .klass(QuoteNode.self)],
        onError: { throw $0 }))
    try editor.update {
      let quote = try createQuoteNode(shadowRoot: true).setIndent(1).setDirection("rtl")
      let heading = try createHeadingNode("h2").setFormat("center")
      let node = try createTextNode("hello").setFormat(IS_BOLD | IS_SUPERSCRIPT).setStyle(
        "color: #00f")
      try getRoot().clear().append(quote.append(heading.append(node)))

      #expect(
        try TextKitElement(heading)
          == TextKitElement(type: "heading", tag: "h2", format: "center"))
      #expect(
        try TextKitElement(quote) == TextKitElement(type: "quote", indent: 1, direction: "rtl"))

      let a = try theme.textAttributes(for: node)
      #expect(a.fontWeight == 700)
      #expect(near(a.fontSize, 16 * 0.8))
      #expect(near(a.baselineOffset, 0.33 * 16))
      #expect(a.foregroundColor == TextKitColor(0, 0, 255))
      #expect(a.textTransform == .uppercase)

      let p = try theme.paragraphAttributes(for: heading)
      #expect(p.alignment == .center)
      #expect(p.writingDirection == .rightToLeft)
      #expect(p.headIndent == 40 + 40)
      #expect(p.lineHeightMultiple == 1.7)
    }
  }
}

@Suite struct TextKitTextTypeStyleTests {
  /// `.PlaygroundEditorTheme__hashtag`'s background.
  private let hashtagBackground = TextKitColor(88, 144, 255, alpha: 0.15)

  @Test func aTextTypeTakesItsOwnStyle() {
    var expected = plain
    expected.backgroundColor = hashtagBackground
    #expect(
      theme.textAttributes(format: 0, style: "", ancestors: [paragraph], type: "hashtag")
        == expected)
    // A type without a style of its own is plain text.
    #expect(
      theme.textAttributes(format: 0, style: "", ancestors: [paragraph], type: "other")
        == plain)
  }

  @Test func aHashtagRunIsStyled() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "test", dependencies: [RichTextExtension, HashtagExtension]))
    let controller = TextKitInputController(editor: editor)
    var runs: [TextKitStyledRun] = []
    controller.onRender = { update in runs = update.edits.flatMap(\.runs) + update.restyled }
    try controller.setSelectedRange(location: 0, length: 0)
    try controller.insertText("say #tag")
    let tagged = runs.filter { near($0.attributes.backgroundColor, hashtagBackground) }
    #expect(tagged.map(\.range) == [TextKitRange(location: 4, length: 4)])
  }

  /// A code token takes the color of its type in `theme.codeHighlight`,
  /// `.PlaygroundEditorTheme__tokenAttr` for a keyword; one without a type
  /// keeps the code block's.
  @Test func aCodeTokenTakesItsTypesColor() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "test", dependencies: [RichTextExtension, CodeExtension]))
    defer { editor.dispose() }
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createCodeNode("js").append(
          createCodeHighlightNode("let", "keyword"), createCodeHighlightNode(" x")))
    }
    let colors = try editor.read {
      try getRoot().getAllTextNodes().map { try theme.textAttributes(for: $0).foregroundColor }
    }
    func light(_ color: TextKitColor) -> [Double] { [color.red, color.green, color.blue] }
    let keyword = TextKitColor(0, 119, 170)
    #expect(light(colors[0]) == light(keyword))
    #expect(light(colors[1]) != light(keyword))
    // Lighter on the dark code background.
    #expect(light(colors[0].darkAppearance) != light(keyword))
  }

  @Test func overflowTakesTheCharacterLimitStyle() {
    // OverflowNode's createDOM takes `theme.characterLimit`, whose
    // background is `.PlaygroundEditorTheme__characterLimit`'s.
    let overflow = TextKitElement(type: "overflow", isInline: true)
    #expect(overflow.themeKey == "characterLimit")
    var expected = plain
    expected.backgroundColor = TextKitColor(255, 187, 187)
    #expect(
      theme.textAttributes(format: 0, style: "", ancestors: [paragraph, overflow]) == expected)
  }

  @Test func textPastTheCharacterLimitStaysOnItsLine() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "test", dependencies: [RichTextExtension, OverflowExtension]))
    _ = try registerCharacterLimit(editor, maxCharacters: 5)
    let controller = TextKitInputController(editor: editor)
    try controller.setSelectedRange(location: 0, length: 0)
    try controller.insertText("hello world")
    try editor.read {}
    // A full render of the result, whose runs are where they end up.
    let update = try controller.renderAll(replacing: 11)
    let runs = update.edits.flatMap(\.runs)
    let paragraphs = update.paragraphs
    // OverflowNode is a block in the model, as in JS, where its <span> keeps
    // it inline: the text past the limit stays on its line, and only it takes
    // the background, as a run's.
    #expect(controller.reconciler.document.string == "hello world")
    let overflowing = runs.filter {
      near($0.attributes.backgroundColor, TextKitColor(255, 187, 187))
    }
    #expect(overflowing.map(\.range) == [TextKitRange(location: 5, length: 6)])
    #expect(paragraphs.map(\.range) == [TextKitRange(location: 0, length: 11)])
    #expect(paragraphs.allSatisfy { $0.attributes.backgroundColor == nil })
  }
}
