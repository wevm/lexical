#if canImport(UIKit)
  import Lexical
  import LexicalA11y
  import LexicalCodeCore
  import LexicalExtension
  import LexicalHistory
  import LexicalRichText
  import LexicalTextKit
  import LexicalUIKit
  import Testing
  import UIKit

  // The UIKit view on the real text system: UIKeyInput and UITextInput
  // calls, marked text, the pasteboard and hit-testing, each checked against
  // the model the editor's extensions produce.

  @MainActor
  /// `frozenClock` stops history's clock, so changes merge however long a
  /// loaded simulator takes between them.
  private func makeView(
    delay: Double = 300, frozenClock: Bool = false
  ) throws -> (
    LexicalUITextView, UIWindow
  ) {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "test",
        dependencies: [
          RichTextExtension,
          configExtension(HistoryExtension) {
            $0.delay = delay
            if frozenClock { $0.now = { 0 } }
          },
        ]))
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 600))
    let view = LexicalUITextView(editor: editor, frame: window.bounds)
    window.addSubview(view)
    window.makeKeyAndVisible()
    _ = view.becomeFirstResponder()
    view.pasteboard = UIPasteboard(name: UIPasteboard.Name("lexical-test-\(UUID())"), create: true)!
    return (view, window)
  }

  @MainActor
  private func json(_ view: LexicalUITextView) throws -> String {
    try view.editor.getEditorState().toJSON(true).stringify()
  }

  @MainActor
  @Suite struct LexicalUITextViewTests {
    @Test func usesTextKit2AndIsTheRootElement() throws {
      let (view, window) = try makeView()
      #expect(view.textLayoutManager != nil, "expected TextKit 2")
      #expect(view.editor.getRootElement() === view)
      #expect(view.isConnected)
      // RichTextExtension's announcers speak through the view.
      let region = try #require(view.liveRegion)
      #expect(region.host === view)
      region.textContent = "Heading 1\u{200B}"
      view.blur()
      #expect(!view.hasFocus)
      view.focus()
      #expect(view.hasFocus)
      _ = window
    }

    @Test func typesDeletesAndEnters() throws {
      let (view, window) = try makeView()
      view.insertText("Hello")
      view.insertText("\n")
      view.insertText("world")
      view.deleteBackward()
      #expect(view.text == "Hello\nworl")
      #expect(view.selectedRange == NSRange(location: 10, length: 0))
      #expect(try json(view).contains(#""text":"worl""#))
      view.selectedRange = NSRange(location: 0, length: 5)
      view.insertText("Bye")
      #expect(view.text == "Bye\nworl")
      // Autocorrection replaces a range through UITextInput.
      let start = try #require(view.position(from: view.beginningOfDocument, offset: 4))
      let end = try #require(view.position(from: start, offset: 4))
      view.replace(try #require(view.textRange(from: start, to: end)), withText: "world")
      #expect(view.text == "Bye\nworld")
      // The keyboard's Return replaces the selection with a newline: Enter.
      let caret = try #require(view.selectedTextRange)
      view.replace(caret, withText: "\n")
      #expect(view.text == "Bye\nworld\n")
      #expect(try view.editor.getEditorState().read { try getRoot().getChildrenSize() } == 3)
      // The system's undo (Command-Z, shake) goes to the editor's history.
      view.undoManager?.undo()
      #expect(view.text == "Bye\nworld")
      #expect(view.undoManager?.canRedo == true)
      view.undoManager?.redo()
      #expect(view.text == "Bye\nworld\n")
      _ = window
    }

    @Test func formatsWithTheEditMenu() throws {
      let (view, window) = try makeView()
      view.insertText("Hello world")
      view.selectedRange = NSRange(location: 6, length: 5)
      view.toggleBoldface(nil)
      #expect(try json(view).contains(#"{"type":"text","format":1,"text":"world"}"#))
      let font = try #require(
        view.textStorage.attribute(.font, at: 7, effectiveRange: nil) as? UIFont)
      #expect(font.fontDescriptor.symbolicTraits.contains(.traitBold))
      _ = window
    }

    @Test func composesMarkedTextThroughTheModel() throws {
      let (view, window) = try makeView()
      view.insertText("a")
      view.setMarkedText("k", selectedRange: NSRange(location: 1, length: 0))
      #expect(view.markedTextRange != nil)
      #expect(view.editor.isComposing())
      view.setMarkedText("かな", selectedRange: NSRange(location: 2, length: 0))
      #expect(view.text == "aかな")
      #expect(view.selectedRange == NSRange(location: 3, length: 0))
      view.insertText("仮名")
      #expect(view.markedTextRange == nil)
      #expect(!view.editor.isComposing())
      #expect(view.text == "a仮名")
      view.setMarkedText("x", selectedRange: NSRange(location: 1, length: 0))
      view.unmarkText()
      #expect(view.text == "a仮名x")
      #expect(!view.editor.isComposing())
      _ = window
    }

    @Test func undoesThroughHistory() throws {
      let (view, window) = try makeView(delay: 0)
      view.insertText("Hello")
      view.insertText("\n")
      view.insertText("World")
      view.undo()
      #expect(view.text == "Hello\n")
      view.undo()
      #expect(view.text == "Hello")
      view.redo()
      view.redo()
      #expect(view.text == "Hello\nWorld")
      _ = window
    }

    @Test func replaysEditsTheTextViewMakesToItsOwnText() throws {
      let (view, window) = try makeView()
      view.insertText("ㅎ")
      // The Korean keyboard deletes the jamo in the text storage itself,
      // then inserts the syllable.
      view.textStorage.replaceCharacters(in: NSRange(location: 0, length: 1), with: "")
      view.insertText("하")
      #expect(view.text == "하")
      #expect(try json(view).contains(#""text":"하""#))
      _ = window
    }

    @Test func mergesKeysTypedAsReplacementsOfTheSelection() throws {
      // With the wall clock, a key more than 300 ms after the previous one
      // starts a new undo entry, which a loaded full-suite run can hit.
      let (view, window) = try makeView(frozenClock: true)
      // The iOS keyboard types each key by replacing the selection.
      for key in ["O", "n", "e"] {
        view.replace(try #require(view.selectedTextRange), withText: key)
      }
      #expect(view.text == "One")
      #expect(view.selectedRange == NSRange(location: 3, length: 0))
      view.undoManager?.undo()
      #expect(view.text == "")
      _ = window
    }

    @Test func copiesAndPastesThroughThePasteboard() throws {
      let (view, window) = try makeView()
      view.insertText("Hello world")
      view.selectedRange = NSRange(location: 0, length: 5)
      view.copy(nil)
      #expect(view.pasteboard.types.contains("dev.lexical.editor"))
      #expect(view.pasteboard.types.contains("public.html"))
      view.cut(nil)
      #expect(view.text == " world")
      view.selectedRange = NSRange(location: 6, length: 0)
      view.paste(nil)
      #expect(view.text == " worldHello")
      _ = window
    }

    @Test func pastesHTML() throws {
      let (view, window) = try makeView()
      view.pasteboard.setItems([
        [
          "public.html":
            "<h1>Title</h1><blockquote>Quoted</blockquote><p>Some <b>bold</b> text</p>",
          "public.utf8-plain-text": "Title\nQuoted\nSome bold text",
        ]
      ])
      view.paste(nil)
      #expect(view.text == "Title\nQuoted\nSome bold text")
      let state = try json(view)
      #expect(state.contains(#""type":"heading""#))
      #expect(state.contains(#""type":"quote""#))
      #expect(state.contains(#"{"type":"text","format":1,"text":"bold"}"#))
      let heading = try #require(
        view.textStorage.attribute(.font, at: 0, effectiveRange: nil) as? UIFont)
      #expect(heading.pointSize == 24)
      _ = window
    }

    @Test func centersTextInCSSLineBoxes() throws {
      let (view, window) = try makeView()
      // Lines of capitals, whose ink runs from the cap height to the
      // baseline.
      view.insertText(String(repeating: "HHHH ", count: 16))
      view.layoutIfNeeded()
      let layoutManager = try #require(view.textLayoutManager)
      layoutManager.ensureLayout(for: layoutManager.documentRange)
      view.layoutIfNeeded()
      let format = UIGraphicsImageRendererFormat()
      format.scale = 2
      let image = UIGraphicsImageRenderer(bounds: view.bounds, format: format).image { context in
        UIColor.white.setFill()
        context.fill(view.bounds)
        view.layer.render(in: context.cgContext)
      }
      let cgImage = try #require(image.cgImage)
      let bytes = try #require(cgImage.dataProvider?.data.flatMap(CFDataGetBytePtr))
      var lines: [(top: CGFloat, bottom: CGFloat)] = []
      for row in 0..<cgImage.height {
        let dark = (0..<min(cgImage.width, 200)).contains { column in
          let pixel = bytes + row * cgImage.bytesPerRow + column * cgImage.bitsPerPixel / 8
          // The black text, not the blue caret.
          return max(pixel[0], pixel[1], pixel[2]) < 100
        }
        guard dark else { continue }
        if let last = lines.last, last.bottom == CGFloat(row) / 2 {
          lines[lines.count - 1].bottom = CGFloat(row + 1) / 2
        } else {
          lines.append((CGFloat(row) / 2, CGFloat(row + 1) / 2))
        }
      }
      // The playground's `line-height: 1.7` at 16 points: 27.2-point line
      // boxes, each with half the space the font leaves above its text.
      let font = UIFont.systemFont(ofSize: 16)
      let halfLeading = (27.2 - (font.ascender - font.descender)) / 2
      try #require(lines.count >= 2, "\(lines)")
      for (index, line) in lines.enumerated() {
        let baseline =
          view.textContainerInset.top + 27.2 * CGFloat(index) + halfLeading + font.ascender
        #expect(abs(line.bottom - baseline) <= 1, "line \(index): \(line), baseline \(baseline)")
        #expect(abs(line.top - (baseline - font.capHeight)) <= 1, "line \(index): \(line)")
      }
      // The caret is as tall as the font, on the centered text.
      let position = try #require(view.position(from: view.beginningOfDocument, offset: 2))
      let caret = view.caretRect(for: position)
      #expect(abs(caret.minY - (view.textContainerInset.top + halfLeading)) <= 0.5, "\(caret)")
      #expect(abs(caret.height - (font.ascender - font.descender)) <= 0.5, "\(caret)")
      _ = window
    }

    @Test func fitsTheCaretAtTheEndOfTheText() throws {
      let (view, window) = try makeView()
      view.insertText("Hello")
      view.insertText("\n")
      view.insertText("world")
      view.layoutIfNeeded()
      let font = UIFont.systemFont(ofSize: 16)
      let halfLeading = (27.2 - (font.ascender - font.descender)) / 2
      func expectFitted(at position: UITextPosition, line: Int) {
        // As tall as the font, on the line's centered text.
        let caret = view.caretRect(for: position)
        #expect(abs(caret.height - (font.ascender - font.descender)) <= 0.5, "\(caret)")
        #expect(
          abs(caret.minY - (view.textContainerInset.top + 27.2 * CGFloat(line) + halfLeading))
            <= 0.5, "\(caret)")
      }
      let middle = try #require(view.position(from: view.beginningOfDocument, offset: 2))
      expectFitted(at: middle, line: 0)
      expectFitted(at: view.endOfDocument, line: 1)
      // An empty last block: the extra line after the final newline.
      view.insertText("\n")
      view.layoutIfNeeded()
      expectFitted(at: view.endOfDocument, line: 2)
      _ = window
    }

    @Test func hitTestsCarets() throws {
      let (view, window) = try makeView()
      view.insertText("Hello")
      view.insertText("\n")
      view.insertText("world")
      view.layoutIfNeeded()
      let start = try #require(view.position(from: view.beginningOfDocument, offset: 6))
      let rect = view.caretRect(for: start)
      try view.editor.update(discrete: true) {
        let caret = try #require(try view.caretFromPoint(rect.midX + 1, rect.midY))
        #expect(caret.isText)
        #expect(try caret.origin.getTextContent() == "world")
        #expect(caret.offset == 0)
      }
      _ = window
    }

    @Test func selectsOnlyTheText() throws {
      let (view, window) = try makeView()
      view.insertText(String(repeating: "word ", count: 12) + "end")
      view.insertText("\n")
      view.insertText("Second")
      try view.editor.update(discrete: true) {
        for child in try getRoot().getChildren() { try (child as? ElementNode)?.setIndent(2) }
      }
      view.layoutIfNeeded()
      let all = try #require(
        view.textRange(from: view.beginningOfDocument, to: view.endOfDocument))
      let rects = view.selectionRects(for: all)
      // Three lines: the first paragraph wraps once.
      try #require(rects.count == 3, "\(rects.map(\.rect))")
      #expect(rects.first?.containsStart == true)
      #expect(rects.last?.containsEnd == true)
      // Each line's text, as TextKit lays it out.
      let layoutManager = try #require(view.textLayoutManager)
      var text: [CGRect] = []
      layoutManager.enumerateTextSegments(
        in: layoutManager.documentRange, type: .standard, options: .rangeNotRequired
      ) { _, segment, _, _ in
        text.append(
          segment.offsetBy(dx: view.textContainerInset.left, dy: view.textContainerInset.top))
        return true
      }
      try #require(text.count >= 3, "\(text)")
      let font = UIFont.systemFont(ofSize: 16)
      let left = view.textContainerInset.left + view.textContainer.lineFragmentPadding
      for (index, (rect, text)) in zip(rects.map(\.rect), text).enumerated() {
        // From the indented text's start to its end, not the container's
        // edges.
        #expect(abs(rect.minX - (left + 80)) <= 0.5, "\(index): \(rect)")
        #expect(abs(rect.maxX - text.maxX) <= 0.5, "\(index): \(rect) \(text)")
        // Over the font's content area, centered in the 27.2-point line.
        #expect(abs(rect.height - (font.ascender - font.descender)) <= 0.5, "\(rect)")
        #expect(
          abs(rect.midY - (view.textContainerInset.top + 27.2 * (CGFloat(index) + 0.5))) <= 1,
          "\(index): \(rect)")
      }
      _ = window
    }

    @Test func drawsHorizontalRulesFromTheTheme() throws {
      let editor = try buildEditorFromExtensions(
        defineExtension(name: "test", dependencies: [RichTextExtension, HorizontalRuleExtension]))
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 600))
      window.overrideUserInterfaceStyle = .light
      let view = LexicalUITextView(editor: editor, frame: window.bounds)
      window.addSubview(view)
      window.makeKeyAndVisible()
      var rule: NodeKey = ""
      try editor.update(discrete: true) {
        let node = try createHorizontalRuleNode()
        rule = node.getKey()
        try getRoot().clear().append(
          createParagraphNode().append(createTextNode("a")), node,
          createParagraphNode().append(createTextNode("b")))
      }
      func render() -> (CGFloat, CGFloat) -> [Int] {
        view.layoutIfNeeded()
        let format = UIGraphicsImageRendererFormat()
        format.scale = 2
        // Each view draws its layer's contents before the layer tree is
        // rendered over white.
        func display(_ layer: CALayer) {
          layer.displayIfNeeded()
          layer.sublayers?.forEach(display)
        }
        display(view.layer)
        let image = UIGraphicsImageRenderer(bounds: view.bounds, format: format).image { context in
          UIColor.white.setFill()
          context.fill(view.bounds)
          view.layer.render(in: context.cgContext)
        }
        let width = Int(view.bounds.width * 2)
        let height = Int(view.bounds.height * 2)
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        let context = CGContext(
          data: &pixels, width: width, height: height, bitsPerComponent: 8,
          bytesPerRow: width * 4, space: CGColorSpace(name: CGColorSpace.sRGB)!,
          bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.draw(image.cgImage!, in: CGRect(x: 0, y: 0, width: width, height: height))
        return { x, y in
          let offset = (Int(y * 2) * width + Int(x * 2)) * 4
          return [Int(pixels[offset]), Int(pixels[offset + 1]), Int(pixels[offset + 2])]
        }
      }
      func near(_ a: [Int], _ b: [Int]) -> Bool { zip(a, b).allSatisfy { abs($0 - $1) <= 28 } }
      // The playground's `__hr`: a 2-point #ccc line 18 points (margin and
      // padding) below the first paragraph's 27.2-point line, in a
      // 38-point line of its own.
      var color = render()
      let lineTop = view.textContainerInset.top + 27.2 + 18
      #expect(near(color(200, lineTop + 1), [204, 204, 204]), "\(color(200, lineTop + 1))")
      #expect(near(color(200, lineTop - 1), [255, 255, 255]), "\(color(200, lineTop - 1))")
      #expect(near(color(200, lineTop + 3), [255, 255, 255]), "\(color(200, lineTop + 3))")
      let layoutManager = try #require(view.textLayoutManager)
      var fragments: [CGRect] = []
      layoutManager.enumerateTextLayoutFragments(from: layoutManager.documentRange.location) {
        fragments.append($0.layoutFragmentFrame)
        return true
      }
      try #require(fragments.count == 3)
      #expect(abs(fragments[1].height - 38) <= 1, "\(fragments)")

      // Selected, it is outlined in blue outside its padded box, and the
      // selection highlight leaves it alone (`user-select: none`).
      try editor.update(discrete: true) {
        let selection = createNodeSelection()
        selection.add(rule)
        try setSelection(selection)
      }
      color = render()
      #expect(near(color(200, lineTop - 3), [60, 132, 244]), "\(color(200, lineTop - 3))")
      #expect(near(color(200, lineTop + 5), [60, 132, 244]), "\(color(200, lineTop + 5))")
      let start = try #require(view.position(from: view.beginningOfDocument, offset: 2))
      let end = try #require(view.position(from: start, offset: 1))
      let rects = view.selectionRects(for: try #require(view.textRange(from: start, to: end)))
      #expect(rects.isEmpty, "\(rects.map(\.rect))")
      _ = window
    }

    /// A code block's lines, `|` for a line break, then a paragraph so the
    /// block isn't the text's last.
    @Test(arguments: [("a|b", 2), ("a||b", 3), ("a||", 3), ("", 1)])
    func drawsACodeBlocksBoxAndGutter(_ lines: String, count: Int) throws {
      let editor = try buildEditorFromExtensions(
        defineExtension(name: "test", dependencies: [RichTextExtension, CodeExtension]))
      let (view, window) = makeLightView(editor)
      try editor.update(discrete: true) {
        let code = try createCodeNode()
        for (index, text) in lines.split(separator: "|", omittingEmptySubsequences: false)
          .enumerated()
        {
          if index > 0 { try code.append(createLineBreakNode()) }
          if !text.isEmpty { try code.append(createTextNode(String(text))) }
        }
        // TextKit leaves out the spacing before the first paragraph.
        try getRoot().clear().append(
          createParagraphNode().append(createTextNode("x")), code,
          createParagraphNode().append(createTextNode("y")))
      }
      let (color, near) = try render(view)
      // The playground's `__code`: an 8-point margin, then the box, whose
      // 8-point padding is above its 1.53 × 13-point lines and below them.
      // A trailing line break's empty line is the block's too, as the
      // web's `<br>` gives it a line box, and so is an empty block's line.
      let left = view.textContainerInset.left + view.textContainer.lineFragmentPadding
      let top = view.textContainerInset.top + 27.2 + 8
      let line = 1.53 * 13
      let bottom = top + 8 + CGFloat(count) * line + 8
      let background = [240, 242, 245]
      #expect(near(color(left + 200, top + 1), background), "\(color(left + 200, top + 1))")
      #expect(near(color(left + 200, bottom - 1), background), "\(color(left + 200, bottom - 1))")
      #expect(!near(color(left + 200, top - 1), background))
      #expect(!near(color(left + 200, bottom + 1), background))
      // The 41-point gutter and its 1-point border at the box's start.
      #expect(near(color(left + 2, top + 2), [238, 238, 238]), "\(color(left + 2, top + 2))")
      #expect(near(color(left + 41.5, top + 2), [204, 204, 204]), "\(color(left + 41.5, top + 2))")
      // One number on each line, empty or not, centered in it like the
      // line's text.
      var inked: [CGFloat] = []
      for row in stride(from: top, to: bottom, by: 0.5) {
        let dark = stride(from: left + 8, to: left + 41 - 4, by: 0.5).contains { x in
          color(x, row)[1] < 180
        }
        if dark { inked.append(row) }
      }
      let runs =
        zip(inked, inked.dropFirst()).filter { $1 - $0 > 1 }.count + (inked.isEmpty ? 0 : 1)
      #expect(runs == count, "\(inked)")
      for index in 0..<count {
        let middle = top + 8 + line * (CGFloat(index) + 0.5)
        let ink = inked.filter { abs($0 - middle) < line / 2 }
        try #require(!ink.isEmpty, "line \(index): \(inked)")
        #expect(abs((ink.min()! + ink.max()!) / 2 - middle) <= 2, "line \(index): \(ink)")
      }
      _ = window
    }

    /// A quote that ends with a line break, followed by a paragraph: its
    /// bar covers the empty line after the break, as the web's does.
    @Test func drawsAQuotesBarOverItsTrailingEmptyLine() throws {
      let editor = try buildEditorFromExtensions(
        defineExtension(name: "test", dependencies: [RichTextExtension]))
      let (view, window) = makeLightView(editor)
      try editor.update(discrete: true) {
        try getRoot().clear().append(
          createQuoteNode().append(createTextNode("a"), createLineBreakNode()),
          createParagraphNode().append(createTextNode("y")))
      }
      let (color, near) = try render(view)
      let layoutManager = try #require(view.textLayoutManager)
      var lines: [CGRect] = []
      layoutManager.enumerateTextLayoutFragments(
        from: layoutManager.documentRange.location, options: [.ensuresLayout]
      ) { fragment in
        lines += fragment.textLineFragments.map {
          $0.typographicBounds.offsetBy(
            dx: fragment.layoutFragmentFrame.minX, dy: fragment.layoutFragmentFrame.minY)
        }
        return true
      }
      // The quote's two lines, then the paragraph's.
      try #require(lines.count >= 3, "\(lines)")
      let origin = view.textContainerInset.top
      let bar = try #require(
        stride(from: 0, to: 60, by: 0.5).first {
          near(color($0, origin + lines[0].midY), [206, 208, 212])
        }, "no bar on the first line")
      #expect(near(color(bar + 1, origin + lines[1].midY), [206, 208, 212]))
      #expect(!near(color(bar + 1, origin + lines[2].midY), [206, 208, 212]))
      _ = window
    }

    /// `.ContentEditable__placeholder`: #999 root text over the first line,
    /// centered in it as typed text is, while `canShowPlaceholder` holds.
    @Test func showsThePlaceholderWhereTypedTextWouldBe() throws {
      let editor = try buildEditorFromExtensions(
        defineExtension(name: "test", dependencies: [RichTextExtension]))
      let (view, window) = makeLightView(editor)
      let text = "Enter some rich text..."
      view.placeholder = text
      #expect(view.accessibilityValue == text)
      // The rows of the first line's ink of a color: at least half
      // covered, so text of either color measures alike.
      func rows(_ isInk: @escaping ([Int]) -> Bool) throws -> ClosedRange<CGFloat>? {
        view.layoutIfNeeded()
        func display(_ layer: CALayer) {
          layer.displayIfNeeded()
          layer.sublayers?.forEach(display)
        }
        display(view.layer)
        let (color, _) = try render(view)
        let x = view.textContainerInset.left
        let rows = stride(from: 0, to: 50, by: 0.5).filter { y in
          stride(from: x, to: x + 150, by: 0.5).contains { isInk(color($0, y)) }
        }
        return rows.first.map { $0...rows.last! }
      }
      let placeholder = try #require(
        try rows { $0.allSatisfy { $0 < 204 } && $0.max()! - $0.min()! < 8 }, "no placeholder")
      try editor.update(discrete: true) {
        try getRoot().clear().append(createParagraphNode().append(createTextNode(text)))
      }
      #expect(view.accessibilityValue != text)
      let typed = try #require(try rows { $0.allSatisfy { $0 < 128 } }, "no text")
      #expect(abs(placeholder.lowerBound - typed.lowerBound) <= 1, "\(placeholder) \(typed)")
      #expect(abs(placeholder.upperBound - typed.upperBound) <= 1, "\(placeholder) \(typed)")
      // `$canShowPlaceholder`: only an unindented paragraph of text.
      let blocks: [(() throws -> ElementNode, Bool)] = [
        ({ try createHeadingNode() }, false),
        ({ try createParagraphNode().setIndent(1) }, false),
        ({ try createParagraphNode() }, true),
      ]
      for (block, shows) in blocks {
        try editor.update(discrete: true) { try getRoot().clear().append(try block()) }
        #expect((view.accessibilityValue == text) == shows)
      }
      _ = window
    }

    private func makeLightView(_ editor: Editor) -> (LexicalUITextView, UIWindow) {
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 600))
      window.overrideUserInterfaceStyle = .light
      let view = LexicalUITextView(editor: editor, frame: window.bounds)
      window.addSubview(view)
      window.makeKeyAndVisible()
      return (view, window)
    }

    /// Renders `view` at 2×, returning its color at a point and a
    /// comparison that takes either byte order.
    private func render(
      _ view: LexicalUITextView
    ) throws -> (
      (CGFloat, CGFloat) -> [Int], ([Int], [Int]) -> Bool
    ) {
      view.layoutIfNeeded()
      let format = UIGraphicsImageRendererFormat()
      format.scale = 2
      let image = UIGraphicsImageRenderer(bounds: view.bounds, format: format).image { context in
        UIColor.white.setFill()
        context.fill(view.bounds)
        view.layer.render(in: context.cgContext)
      }
      let cgImage = try #require(image.cgImage)
      let data = try #require(cgImage.dataProvider?.data)
      let bytes = try #require(CFDataGetBytePtr(data))
      let bytesPerRow = cgImage.bytesPerRow
      let bytesPerPixel = cgImage.bitsPerPixel / 8
      return (
        { x, y in
          _ = data
          let pixel = bytes + Int(y * 2) * bytesPerRow + Int(x * 2) * bytesPerPixel
          return [Int(pixel[0]), Int(pixel[1]), Int(pixel[2])]
        },
        { a, b in
          let close = { (c: [Int]) in zip(c, b).allSatisfy { abs($0 - $1) <= 6 } }
          return close(a) || close(a.reversed())
        }
      )
    }
  }
#endif
