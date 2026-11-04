#if os(macOS)
  import AppKit
  import Lexical
  import LexicalA11y
  import LexicalAppKit
  import LexicalCodeCore
  import LexicalExtension
  import LexicalHistory
  import LexicalRichText
  import LexicalTextKit
  import Testing

  // The AppKit view on the real text system: key events, NSResponder
  // selectors, marked text, the pasteboard and hit-testing, each checked
  // against the model the editor's extensions produce.

  @MainActor
  func makeRichTextEditor(delay: Double = 300) throws -> Editor {
    try buildEditorFromExtensions(
      defineExtension(
        name: "test",
        dependencies: [
          RichTextExtension,
          configExtension(HistoryExtension) { $0.delay = delay },
        ]))
  }

  /// A view in an offscreen window, as first responder.
  @MainActor
  private func makeView(_ editor: Editor? = nil) throws -> (LexicalNSTextView, NSWindow) {
    let view = LexicalNSTextView(
      editor: try editor ?? makeRichTextEditor(), frame: NSRect(x: 0, y: 0, width: 400, height: 300)
    )
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled],
      backing: .buffered, defer: false)
    window.contentView = view
    window.makeFirstResponder(view)
    view.pasteboard = NSPasteboard(name: NSPasteboard.Name("lexical-test-\(UUID().uuidString)"))
    return (view, window)
  }

  @MainActor
  private func key(
    _ characters: String, _ keyCode: UInt16, _ flags: NSEvent.ModifierFlags = [],
    ignoringModifiers: String? = nil
  ) -> NSEvent {
    NSEvent.keyEvent(
      with: .keyDown, location: .zero, modifierFlags: flags, timestamp: 0, windowNumber: 0,
      context: nil, characters: characters,
      charactersIgnoringModifiers: ignoringModifiers ?? characters, isARepeat: false,
      keyCode: keyCode)!
  }

  @MainActor
  private func json(_ view: LexicalNSTextView) throws -> String {
    try view.editor.getEditorState().toJSON(true).stringify()
  }

  @MainActor
  private func insert(_ view: LexicalNSTextView, _ text: String) {
    view.insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
  }

  @MainActor
  @Suite struct LexicalNSTextViewTests {
    @Test func usesTextKit2AndIsTheRootElement() throws {
      let (view, window) = try makeView()
      #expect(view.textLayoutManager != nil, "expected TextKit 2")
      #expect(view.editor.getRootElement() === view)
      #expect(view.isConnected)
      // RichTextExtension's announcers speak through the view.
      let region = try #require(view.liveRegion)
      #expect(region.host === view)
      region.textContent = "Heading 1\u{200B}"
      #expect(view.hasFocus)
      view.blur()
      #expect(!view.hasFocus)
      view.focus()
      #expect(view.hasFocus)
      _ = window
    }

    @Test func tabMovesFocusThroughTheKeyViewLoop() throws {
      // Full keyboard access: the editor is in the window's key view loop,
      // and Tab, which no KEY_TAB_COMMAND handler takes here, moves the
      // focus on, as it does out of a contenteditable.
      let view = LexicalNSTextView(
        editor: try makeRichTextEditor(), frame: NSRect(x: 0, y: 0, width: 400, height: 260))
      let field = NSTextField(frame: NSRect(x: 0, y: 270, width: 400, height: 24))
      let window = NSWindow(
        contentRect: NSRect(x: 0, y: 0, width: 400, height: 300), styleMask: [.titled],
        backing: .buffered, defer: false)
      window.contentView?.addSubview(view)
      window.contentView?.addSubview(field)
      view.nextKeyView = field
      field.nextKeyView = view
      #expect(view.canBecomeKeyView)
      window.makeFirstResponder(view)
      #expect(view.hasFocus)
      view.keyDown(with: key("\t", 48))
      #expect((window.firstResponder as? NSText)?.delegate === field)
      #expect(!view.hasFocus)
      #expect(view.string.isEmpty)
      window.selectKeyView(following: field)
      #expect(window.firstResponder === view)
      #expect(view.hasFocus)
      view.keyDown(with: key("\u{19}", 48, .shift, ignoringModifiers: "\t"))
      #expect((window.firstResponder as? NSText)?.delegate === field)
    }

    @Test func typesWithKeyEvents() throws {
      let (view, window) = try makeView()
      for (character, code) in [("H", UInt16(0x04)), ("i", 0x22)] {
        view.keyDown(with: key(character, code, character == "H" ? .shift : []))
      }
      #expect(view.string == "Hi")
      view.keyDown(with: key("\r", 0x24))
      insert(view, "there")
      view.keyDown(with: key("\u{7F}", 0x33))
      #expect(view.string == "Hi\nther")
      #expect(view.selectedRange() == NSRange(location: 7, length: 0))
      // Option-Backspace deletes a word at keydown (DELETE_WORD_COMMAND).
      view.keyDown(with: key("\u{7F}", 0x33, .option))
      #expect(view.string == "Hi\n")
      // Shift-Enter is a line break.
      insert(view, "a")
      view.keyDown(with: key("\r", 0x24, .shift))
      #expect(view.string == "Hi\na\u{2028}")
      #expect(try json(view).contains(#""type":"linebreak""#))
      _ = window
    }

    @Test func formatsWithCommandKeys() throws {
      let (view, window) = try makeView()
      insert(view, "Hello world")
      view.setSelectedRange(NSRange(location: 6, length: 5))
      #expect(view.performKeyEquivalent(with: key("b", 0x0B, .command)))
      #expect(try json(view).contains(#"{"type":"text","format":1,"text":"world"}"#))
      view.setSelectedRange(NSRange(location: 0, length: 5))
      view.keyDown(with: key("i", 0x22, .command))
      #expect(try json(view).contains(#"{"type":"text","format":2,"text":"Hello"}"#))
      // The view's storage shows the model's formats.
      let font = try #require(
        view.textStorage?.attribute(.font, at: 7, effectiveRange: nil) as? NSFont)
      #expect(font.fontDescriptor.symbolicTraits.contains(.bold))
      _ = window
    }

    @Test func mapsResponderSelectors() throws {
      let (view, window) = try makeView()
      insert(view, "one two three")
      view.deleteWordBackward(nil)
      #expect(view.string == "one two ")
      view.insertNewline(nil)
      insert(view, "x")
      view.insertLineBreak(nil)
      insert(view, "y")
      #expect(view.string == "one two \nx\u{2028}y")
      // Movement is the text view's own, synced to the model.
      view.moveToBeginningOfParagraph(nil)
      view.moveRight(nil)
      #expect(view.selectedRange() == NSRange(location: 10, length: 0))
      view.deleteBackward(nil)
      #expect(view.string == "one two \n\u{2028}y")
      view.deleteForward(nil)
      #expect(view.string == "one two \ny")
      view.selectAll(nil)
      #expect(view.selectedRange() == NSRange(location: 0, length: view.string.utf16.count))
      try view.editor.read { () throws in
        let selection = try #require(try getSelection() as? RangeSelection)
        #expect(try selection.getTextContent() == "one two \ny")
      }
      view.deleteBackward(nil)
      #expect(view.string == "")
      _ = window
    }

    @Test func selectionSyncsBothWays() throws {
      let (view, window) = try makeView()
      insert(view, "abc")
      view.setSelectedRange(NSRange(location: 1, length: 2))
      try view.editor.read { () throws in
        let selection = try #require(try getSelection() as? RangeSelection)
        #expect(selection.anchor.offset == 1 && selection.focus.offset == 3)
      }
      try view.editor.update(discrete: true) {
        _ = try (getRoot().getFirstDescendant() as? TextNode)?.select(0, 1)
      }
      #expect(view.selectedRange() == NSRange(location: 0, length: 1))
      _ = window
    }

    @Test func caretOnAnEmptyLastBlockFollowsItsIndent() throws {
      let (view, window) = try makeView()
      insert(view, "Hello")
      view.insertNewline(nil)
      // The caret is on the empty last line, which no character carries.
      func caretX() throws -> CGFloat {
        let layoutManager = try #require(view.textLayoutManager)
        let end = NSTextRange(location: layoutManager.documentRange.endLocation)
        layoutManager.ensureLayout(for: layoutManager.documentRange)
        var x: CGFloat?
        layoutManager.enumerateTextSegments(in: end, type: .standard, options: []) {
          _, segment, _, _ in
          x = segment.minX
          return false
        }
        return try #require(x)
      }
      let start = try caretX()
      // Only the block's look changes: the text and the selection stay.
      func indentLastBlock(_ indent: Int) throws {
        try view.editor.update(discrete: true) {
          try (getRoot().getLastChild() as? ElementNode)?.setIndent(indent)
        }
      }
      try indentLastBlock(2)
      #expect(try caretX() == start + 80)
      try indentLastBlock(0)
      #expect(try caretX() == start)
      _ = window
    }

    @Test func fitsTheCaretAtTheEndOfTheText() throws {
      let (view, window) = try makeView()
      insert(view, "Hello")
      view.insertNewline(nil)
      insert(view, "world")
      let font = NSFont.systemFont(ofSize: 16)
      let inset = view.textContainerInset.height
      let halfLeading = (27.2 - (font.ascender - font.descender)) / 2
      // The offscreen window shows no caret, so one stands in for
      // NSTextView's, which is as tall as its line.
      let indicator = NSTextInsertionIndicator(frame: .zero)
      view.addSubview(indicator)
      func expectFitted(at location: Int, line: Int) {
        view.setSelectedRange(NSRange(location: location, length: 0))
        indicator.frame = NSRect(x: 40, y: inset + 27.2 * CGFloat(line), width: 2, height: 27.2)
        view.layout()
        // As tall as the font, on the line's centered text.
        let frame = indicator.frame
        #expect(
          abs(frame.height - (font.ascender - font.descender)) <= 0.5, "\(location): \(frame)")
        #expect(
          abs(frame.minY - (inset + 27.2 * CGFloat(line) + halfLeading)) <= 0.5,
          "\(location): \(frame)")
      }
      expectFitted(at: 2, line: 0)
      expectFitted(at: view.string.utf16.count, line: 1)
      // An empty last block: the extra line after the final newline.
      view.insertNewline(nil)
      expectFitted(at: view.string.utf16.count, line: 2)
      _ = window
    }

    @Test func centersTextInCSSLineBoxes() throws {
      let (view, window) = try makeView()
      // Lines of capitals, whose ink runs from the cap height to the
      // baseline.
      insert(view, String(repeating: "HHHH ", count: 16))
      let layoutManager = try #require(view.textLayoutManager)
      layoutManager.ensureLayout(for: layoutManager.documentRange)
      view.layoutSubtreeIfNeeded()
      let rep = try #require(view.bitmapImageRepForCachingDisplay(in: view.bounds))
      view.cacheDisplay(in: view.bounds, to: rep)
      let scale = CGFloat(rep.pixelsHigh) / view.bounds.height
      var inked: [Int] = []
      for row in 0..<rep.pixelsHigh {
        let dark = (0..<min(rep.pixelsWide, 200)).contains { column in
          (rep.colorAt(x: column, y: row)?.brightnessComponent ?? 1) < 0.5
        }
        if dark { inked.append(row) }
      }
      var lines: [(top: CGFloat, bottom: CGFloat)] = []
      for row in inked {
        if let last = lines.last, last.bottom == CGFloat(row) / scale {
          lines[lines.count - 1].bottom = CGFloat(row + 1) / scale
        } else {
          lines.append((CGFloat(row) / scale, CGFloat(row + 1) / scale))
        }
      }
      // The playground's `line-height: 1.7` at 16 points: 27.2-point line
      // boxes, each with half the space the font leaves above its text.
      let font = NSFont.systemFont(ofSize: 16)
      let halfLeading = (27.2 - (font.ascender - font.descender)) / 2
      try #require(lines.count >= 2, "\(lines)")
      for (index, line) in lines.enumerated() {
        let baseline =
          view.textContainerInset.height + 27.2 * CGFloat(index) + halfLeading + font.ascender
        #expect(abs(line.bottom - baseline) <= 1, "line \(index): \(line), baseline \(baseline)")
        #expect(abs(line.top - (baseline - font.capHeight)) <= 1, "line \(index): \(line)")
      }
      _ = window
    }

    @Test func shiftArrowsExtendBackwardAndProgramsSelectForward() throws {
      let (view, window) = try makeView()
      insert(view, "abc")
      func selection() throws -> (anchor: Int, focus: Int) {
        try view.editor.read {
          let selection = try #require(try getSelection() as? RangeSelection)
          return (selection.anchor.offset, selection.focus.offset)
        }
      }
      view.keyDown(with: key("\u{F702}", 123, [.shift, .function, .numericPad]))
      #expect(view.selectedRange() == NSRange(location: 2, length: 1))
      #expect(try selection() == (3, 2))
      // A program's range has no direction: forward, though it ends at the
      // anchor.
      view.setSelectedRange(NSRange(location: 3, length: 0))
      view.setSelectedRange(NSRange(location: 1, length: 2))
      #expect(try selection() == (1, 3))
      _ = window
    }

    @Test func composesMarkedTextThroughTheModel() throws {
      let (view, window) = try makeView()
      insert(view, "a")
      view.setMarkedText(
        "k", selectedRange: NSRange(location: 1, length: 0),
        replacementRange: NSRange(location: NSNotFound, length: 0))
      #expect(view.hasMarkedText())
      #expect(view.markedRange() == NSRange(location: 1, length: 1))
      #expect(view.editor.isComposing())
      view.setMarkedText(
        "かな", selectedRange: NSRange(location: 2, length: 0),
        replacementRange: NSRange(location: NSNotFound, length: 0))
      #expect(view.string == "aかな")
      // The marked text is underlined, without changing the storage.
      let layoutManager = try #require(view.textLayoutManager)
      var underlined: [Int] = []
      layoutManager.enumerateRenderingAttributes(
        from: layoutManager.documentRange.location, reverse: false
      ) { manager, attributes, range in
        if attributes[.underlineStyle] != nil {
          underlined.append(
            manager.offset(from: manager.documentRange.location, to: range.location))
          underlined.append(
            manager.offset(from: manager.documentRange.location, to: range.endLocation))
        }
        return true
      }
      #expect(underlined == [1, 3])
      #expect(view.textStorage?.attribute(.underlineStyle, at: 2, effectiveRange: nil) == nil)
      // A key press while composing is the IME's, not the editor's.
      #expect(try json(view).contains(#""text":"aかな""#))
      insert(view, "仮名")
      #expect(!view.hasMarkedText())
      #expect(!view.editor.isComposing())
      #expect(view.string == "a仮名")
      #expect(view.selectedRange() == NSRange(location: 3, length: 0))
      _ = window
    }

    @Test func undoesThroughHistory() throws {
      let (view, window) = try makeView(try makeRichTextEditor(delay: 0))
      insert(view, "Hello")
      view.insertNewline(nil)
      insert(view, "World")
      view.undo(nil)
      #expect(view.string == "Hello\n")
      #expect(view.performKeyEquivalent(with: key("z", 0x06, .command)))
      #expect(view.string == "Hello")
      view.performKeyEquivalent(with: key("Z", 0x06, [.command, .shift], ignoringModifiers: "Z"))
      #expect(view.string == "Hello\n")
      view.redo(nil)
      #expect(view.string == "Hello\nWorld")
      _ = window
    }

    @Test func copiesAndPastesThroughThePasteboard() throws {
      let (view, window) = try makeView()
      insert(view, "Hello world")
      view.setSelectedRange(NSRange(location: 0, length: 5))
      view.copy(nil)
      let types = view.pasteboard.types ?? []
      #expect(types.contains(NSPasteboard.PasteboardType("dev.lexical.editor")))
      #expect(types.contains(.html))
      #expect(view.pasteboard.string(forType: .string) == "Hello")
      view.cut(nil)
      #expect(view.string == " world")
      view.setSelectedRange(NSRange(location: 6, length: 0))
      view.paste(nil)
      #expect(view.string == " worldHello")
      _ = window
    }

    @Test func pastesHTML() throws {
      let (view, window) = try makeView()
      view.pasteboard.clearContents()
      view.pasteboard.setString(
        "<h1>Title</h1><blockquote>Quoted</blockquote><p>Some <b>bold</b> text</p>",
        forType: .html)
      view.pasteboard.setString("Title\nQuoted\nSome bold text", forType: .string)
      view.paste(nil)
      #expect(view.string == "Title\nQuoted\nSome bold text")
      let state = try json(view)
      #expect(state.contains(#""type":"heading""#))
      #expect(state.contains(#""type":"quote""#))
      #expect(state.contains(#"{"type":"text","format":1,"text":"bold"}"#))
      let heading = try #require(
        view.textStorage?.attribute(.font, at: 0, effectiveRange: nil) as? NSFont)
      #expect(heading.pointSize == 24)
      // Paste as plain text drops the formatting.
      view.selectAll(nil)
      view.pasteAsPlainText(nil)
      #expect(try !json(view).contains(#""format":1"#))
      #expect(view.string == "Title\nQuoted\nSome bold text")
      _ = window
    }

    @Test func pastesRichTextFromNativeApps() throws {
      let (view, window) = try makeView()
      let rich = NSAttributedString(
        string: "Bold", attributes: [.font: NSFont.boldSystemFont(ofSize: 12)])
      let rtf = try #require(
        rich.rtf(from: NSRange(location: 0, length: rich.length), documentAttributes: [:]))
      view.pasteboard.clearContents()
      view.pasteboard.setData(rtf, forType: .rtf)
      view.paste(nil)
      #expect(view.string == "Bold")
      #expect(try json(view).contains(#""format":1"#))
      _ = window
    }

    @Test func refusesEditsThatBypassTheModel() throws {
      let (view, window) = try makeView()
      insert(view, "teh cat")
      // Autocorrection and other storage edits become model replacements.
      #expect(!view.shouldChangeText(in: NSRange(location: 0, length: 3), replacementString: "the"))
      #expect(view.string == "the cat")
      // The caret stays where it was, after the replaced word.
      insert(view, "!")
      #expect(view.string == "the cat!")
      _ = window
    }

    @Test func hitTestsCarets() throws {
      let (view, window) = try makeView()
      insert(view, "Hello")
      view.insertNewline(nil)
      insert(view, "world")
      view.layoutSubtreeIfNeeded()
      let layoutManager = try #require(view.textLayoutManager)
      layoutManager.ensureLayout(for: layoutManager.documentRange)
      // The middle of "w" in the second paragraph.
      let rect = view.firstRect(
        forCharacterRange: NSRange(location: 6, length: 1), actualRange: nil)
      let local = view.convert(window.convertFromScreen(rect), from: nil)
      try view.editor.update(discrete: true) {
        let caret = try #require(try view.caretFromPoint(local.minX + 1, local.midY))
        #expect(caret.isText)
        #expect(try caret.origin.getTextContent() == "world")
        #expect(caret.offset == 0)
      }
      _ = window
    }

    @Test func selectsOnlyTheText() throws {
      let (view, window) = try makeView()
      insert(view, String(repeating: "word ", count: 12) + "end")
      view.insertNewline(nil)
      insert(view, "Second")
      try view.editor.update(discrete: true) {
        for child in try getRoot().getChildren() { try (child as? ElementNode)?.setIndent(2) }
      }
      let layoutManager = try #require(view.textLayoutManager)
      layoutManager.ensureLayout(for: layoutManager.documentRange)
      // What NSTextView highlights for a selection of everything.
      var segments: [CGRect] = []
      layoutManager.enumerateTextSegments(
        in: layoutManager.documentRange, type: .selection, options: .rangeNotRequired
      ) { _, segment, _, _ in
        segments.append(segment)
        return true
      }
      // Three lines: the first paragraph wraps once.
      try #require(segments.count == 3, "\(segments)")
      let font = NSFont.systemFont(ofSize: 16)
      let padding = try #require(view.textContainer).lineFragmentPadding
      for (index, segment) in segments.enumerated() {
        // From the indented text's start, not the container's edge.
        #expect(abs(segment.minX - (padding + 80)) <= 0.5, "\(index): \(segment)")
        // Over the font's content area, centered in the 27.2-point line.
        #expect(abs(segment.height - (font.ascender - font.descender)) <= 0.5, "\(segment)")
        #expect(abs(segment.midY - 27.2 * (CGFloat(index) + 0.5)) <= 1, "\(index): \(segment)")
      }
      // Each line ends where its text does, not at the container's edge.
      var text: [CGRect] = []
      layoutManager.enumerateTextSegments(
        in: layoutManager.documentRange, type: .standard, options: .rangeNotRequired
      ) { _, segment, _, _ in
        text.append(segment)
        return true
      }
      // (TextKit reports the last line's twice.)
      try #require(text.count >= 3, "\(text)")
      for (segment, text) in zip(segments, text) {
        #expect(abs(segment.maxX - text.maxX) <= 0.5, "\(segment) \(text)")
      }
      _ = window
    }

    @Test func drawsHorizontalRulesFromTheTheme() throws {
      let editor = try buildEditorFromExtensions(
        defineExtension(name: "test", dependencies: [RichTextExtension, HorizontalRuleExtension]))
      let (view, window) = try makeView(editor)
      window.appearance = NSAppearance(named: .aqua)
      var rule: NodeKey = ""
      try editor.update(discrete: true) {
        let node = try createHorizontalRuleNode()
        rule = node.getKey()
        try getRoot().clear().append(
          createParagraphNode().append(createTextNode("a")), node,
          createParagraphNode().append(createTextNode("b")))
      }
      func render() throws -> (CGFloat, CGFloat) -> [Int] {
        let layoutManager = try #require(view.textLayoutManager)
        layoutManager.ensureLayout(for: layoutManager.documentRange)
        view.layoutSubtreeIfNeeded()
        let rep = try #require(view.bitmapImageRepForCachingDisplay(in: view.bounds))
        view.cacheDisplay(in: view.bounds, to: rep)
        let scale = CGFloat(rep.pixelsHigh) / view.bounds.height
        return { x, y in
          let c = rep.colorAt(x: Int(x * scale), y: Int(y * scale))?.usingColorSpace(.sRGB)
          return [c?.redComponent, c?.greenComponent, c?.blueComponent].map {
            Int((($0 ?? 0) * 255).rounded())
          }
        }
      }
      // The offscreen bitmap's color space shifts the colors, the blue most.
      func near(_ a: [Int], _ b: [Int]) -> Bool { zip(a, b).allSatisfy { abs($0 - $1) <= 28 } }
      // The playground's `__hr`: a 2-point #ccc line, 2 points of padding
      // and 16 points of margin above and below it.
      var color = try render()
      let rows = stride(from: CGFloat(0), to: 150, by: 0.5)
      let line = rows.filter { near(color(200, $0), [204, 204, 204]) }
      try #require(!line.isEmpty, "no rule drawn")
      #expect(line.max()! - line.min()! <= 2, "\(line)")
      let lineTop = line.min()!
      // The line between the paragraphs, 18 points (margin and padding)
      // below the first and above the second.
      let layoutManager = try #require(view.textLayoutManager)
      var fragments: [CGRect] = []
      layoutManager.enumerateTextLayoutFragments(from: layoutManager.documentRange.location) {
        fragments.append($0.layoutFragmentFrame)
        return true
      }
      try #require(fragments.count == 3)
      let origin = view.textContainerOrigin.y
      #expect(abs(fragments[1].height - 38) <= 1, "\(fragments)")
      #expect(abs(lineTop - (origin + fragments[1].minY + 18)) <= 1, "\(lineTop) \(fragments)")
      #expect(!rows.contains { near(color(200, $0), [60, 132, 244]) })

      // Selected, it is outlined in blue outside its padded box.
      try editor.update(discrete: true) {
        let selection = createNodeSelection()
        selection.add(rule)
        try setSelection(selection)
      }
      color = try render()
      let outline = rows.filter { near(color(200, $0), [60, 132, 244]) }
      try #require(!outline.isEmpty, "not outlined")
      #expect(abs(outline.min()! - (lineTop - 4)) <= 1, "\(outline) \(lineTop)")
      #expect(abs(outline.max()! - (lineTop + 5.5)) <= 1, "\(outline) \(lineTop)")
      // The selection highlight leaves it alone (`user-select: none`).
      let ruleStart = try #require(
        layoutManager.location(layoutManager.documentRange.location, offsetBy: 2))
      let ruleRange = try #require(
        NSTextRange(location: ruleStart, end: layoutManager.location(ruleStart, offsetBy: 1)))
      var highlighted: [CGRect] = []
      layoutManager.enumerateTextSegments(in: ruleRange, type: .selection, options: []) {
        _, segment, _, _ in
        highlighted.append(segment)
        return true
      }
      #expect(highlighted.isEmpty, "\(highlighted)")
      _ = window
    }

    @Test func drawsACodeBlocksBoxAndGutter() throws {
      let editor = try buildEditorFromExtensions(
        defineExtension(name: "test", dependencies: [RichTextExtension, CodeExtension]))
      let (view, window) = try makeView(editor)
      window.appearance = NSAppearance(named: .aqua)
      try editor.update(discrete: true) {
        // TextKit leaves out the spacing before the first paragraph.
        try getRoot().clear().append(
          createParagraphNode().append(createTextNode("x")),
          createCodeNode().append(createTextNode("a"), createLineBreakNode(), createTextNode("b")))
      }
      let layoutManager = try #require(view.textLayoutManager)
      layoutManager.ensureLayout(for: layoutManager.documentRange)
      view.layoutSubtreeIfNeeded()
      let rep = try #require(view.bitmapImageRepForCachingDisplay(in: view.bounds))
      view.cacheDisplay(in: view.bounds, to: rep)
      let scale = CGFloat(rep.pixelsHigh) / view.bounds.height
      func color(_ x: CGFloat, _ y: CGFloat) -> [Int] {
        let pixel = rep.colorAt(x: Int(x * scale), y: Int(y * scale))?.usingColorSpace(.sRGB)
        return [pixel?.redComponent, pixel?.greenComponent, pixel?.blueComponent].map {
          Int((($0 ?? 0) * 255).rounded())
        }
      }
      // The offscreen bitmap's color space shifts the colors a little.
      func near(_ a: [Int], _ b: [Int]) -> Bool { zip(a, b).allSatisfy { abs($0 - $1) <= 12 } }
      // The playground's `__code`: an 8-point margin, then the box, whose
      // 8-point padding is above two 1.53 × 13-point lines and below them.
      let padding = try #require(view.textContainer).lineFragmentPadding
      let left = view.textContainerInset.width + padding
      let top = view.textContainerInset.height + 27.2 + 8
      let line = 1.53 * 13
      let bottom = top + 8 + 2 * line + 8
      let background = [240, 242, 245]
      #expect(near(color(left + 200, top + 1), background), "\(color(left + 200, top + 1))")
      #expect(near(color(left + 200, bottom - 1), background), "\(color(left + 200, bottom - 1))")
      #expect(!near(color(left + 200, top - 1), background))
      #expect(!near(color(left + 200, bottom + 1), background))
      // The 41-point gutter and its 1-point border at the box's start.
      #expect(near(color(left + 2, top + 2), [238, 238, 238]), "\(color(left + 2, top + 2))")
      #expect(near(color(left + 41.5, top + 2), [204, 204, 204]), "\(color(left + 41.5, top + 2))")
      #expect(color(left + 41.5, top + 2)[0] < color(left + 2, top + 2)[0] - 10)
      // A number on each line, centered in it like the line's text.
      var inked: [CGFloat] = []
      for row in Int(top * scale)..<Int(bottom * scale) {
        let dark = stride(from: left + 8, to: left + 41 - 4, by: 0.5).contains { x in
          color(x, CGFloat(row) / scale)[0] < 180
        }
        if dark { inked.append(CGFloat(row) / scale) }
      }
      for index in 0..<2 {
        let middle = top + 8 + line * (CGFloat(index) + 0.5)
        let ink = inked.filter { abs($0 - middle) < line / 2 }
        try #require(!ink.isEmpty, "line \(index): \(inked)")
        #expect(abs((ink.min()! + ink.max()!) / 2 - middle) <= 2, "line \(index): \(ink)")
      }
      _ = window
    }
  }
#endif
