#if os(macOS)
  import AppKit
  import Lexical
  import LexicalA11y
  import LexicalAppKit
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
  }
#endif
