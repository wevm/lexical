#if os(macOS)
  import AppKit
  import Lexical
  import LexicalA11y
  import LexicalAppKit
  import LexicalCodeCore
  import LexicalExtension
  import LexicalHistory
  import LexicalLink
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

  /// A rich text editor with history, and the history state it records
  /// into, so a test can count its entries.
  @MainActor
  func makeEditorWithHistory(
    _ dependencies: [any AnyLexicalExtensionArgument] = []
  ) throws -> (Editor, HistoryState) {
    let history = createEmptyHistoryState()
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "test",
        dependencies: [
          RichTextExtension,
          configExtension(HistoryExtension) { $0.createInitialHistoryState = { _ in history } },
        ] + dependencies))
    return (editor, history)
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

    /// A shortcut's `key` is Safari's on macOS: with Command, Option
    /// applies and Shift doesn't; with Control alone, Shift applies. The
    /// characters are AppKit's for each key on a US layout.
    @Test func keyIsSafarisWithModifiers() throws {
      let cases: [(String, String, UInt16, NSEvent.ModifierFlags, String)] = [
        ("7", "&", 0x1A, [.command, .shift], "7"),
        ("\u{A1}", "1", 0x12, [.command, .option], "\u{A1}"),
        (".", ">", 0x2F, [.command, .shift], "."),
        ("1", "!", 0x12, [.control, .shift], "!"),
        ("\u{11}", "Q", 0x0C, [.control, .shift], "Q"),
        ("b", "b", 0x0B, [.command], "b"),
      ]
      for (characters, ignoringModifiers, keyCode, flags, expected) in cases {
        let event = try #require(
          KeyboardEvent(key(characters, keyCode, flags, ignoringModifiers: ignoringModifiers)))
        #expect(event.key == expected, "\(characters) \(flags)")
      }
    }

    /// ⇧⌘7 reaches a shortcut bound to Shift+Command+7, as in Safari.
    @Test func shiftedCommandDigitMatchesItsShortcut() throws {
      let command = createCommand("test", payload: KeyboardEvent.self)
      let editor = try buildEditorFromExtensions(
        defineExtension(
          name: "test",
          dependencies: [
            RichTextExtension,
            configExtension(KeyboardShortcutsExtension) {
              $0.shortcuts = [
                "LIST": [
                  KeyboardShortcut(
                    key: "7", modifiers: CONTROL_OR_META.with(shiftKey: true), command: command)
                ]
              ]
            },
          ]))
      let (view, window) = try makeView(editor)
      var fired = 0
      let remove = editor.registerCommand(
        command,
        { event, _ in
          fired += 1
          event.preventDefault()
          return true
        }, COMMAND_PRIORITY_EDITOR)
      defer { remove() }
      #expect(
        view.performKeyEquivalent(with: key("7", 0x1A, [.command, .shift], ignoringModifiers: "&")))
      #expect(fired == 1)
      _ = window
    }

    /// Shift+Tab is a `keydown` of Tab with `shiftKey`, as a browser
    /// reports it: it outdents a code line (`CodeIndentExtension`) and a
    /// paragraph at its start (`TabIndentationExtension`), instead of
    /// moving the focus.
    @Test func shiftTabOutdents() throws {
      let editor = try buildEditorFromExtensions(
        defineExtension(
          name: "test",
          dependencies: [RichTextExtension, CodeIndentExtension, TabIndentationExtension]))
      let (view, window) = try makeView(editor)
      try editor.update(discrete: true) {
        try getRoot().clear().append(
          createCodeNode().append(createTabNode(), createCodeHighlightNode("foo")),
          createParagraphNode().setIndent(1).append(createTextNode("bar")))
      }
      let backtab = key("\u{19}", 0x30, .shift, ignoringModifiers: "\u{19}")
      view.setSelectedRange(NSRange(location: 2, length: 0))
      view.keyDown(with: backtab)
      #expect(window.firstResponder === view)
      try editor.read { () throws in
        #expect(try getRoot().getFirstChild()?.getTextContent() == "foo")
      }
      view.setSelectedRange(NSRange(location: (view.string as NSString).length - 3, length: 0))
      view.keyDown(with: backtab)
      try editor.read { () throws in
        #expect(try (getRoot().getLastChild() as? ElementNode)?.getIndent() == 0)
      }
      #expect(window.firstResponder === view)
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

    /// The selection `NSTextView` makes while it tracks the mouse reaches
    /// the model at each step, as `selectionchange` does from the press on,
    /// with the caret in the font there. Selection-only updates are not
    /// history: one undo still takes back the last edit.
    @Test func selectionReachesTheModelWhileTheMouseIsTracked() throws {
      let (editor, history) = try makeEditorWithHistory()
      let (view, window) = try makeView(editor)
      try view.editor.update(discrete: true) {
        try getRoot().clear().append(
          createHeadingNode("h1").append(createTextNode("Title")),
          createParagraphNode().append(createTextNode("body")))
      }
      view.setSelectedRange(NSRange(location: 2, length: 0))
      let undoDepth = history.undoStack.count
      // What each commit's update listener sees: the committed selection.
      var committed: [(String, Int, String, Int)] = []
      let remove = editor.registerUpdateListener { payload in
        try payload.editorState.read {
          guard let selection = try getSelection() as? RangeSelection else { return }
          committed.append(
            (
              try selection.anchor.getNode().getTextContent(), selection.anchor.offset,
              try selection.focus.getNode().getTextContent(), selection.focus.offset
            ))
        }
      }
      defer { remove() }
      let headingFont = try #require(view.typingAttributes[.font] as? NSFont)
      func step(_ range: NSRange) {
        view.setSelectedRanges(
          [NSValue(range: range)], affinity: .downstream, stillSelecting: true)
      }
      func model() throws -> (String, Int, String, Int) {
        try view.editor.read {
          let selection = try #require(try getSelection() as? RangeSelection)
          return (
            try selection.anchor.getNode().getTextContent(), selection.anchor.offset,
            try selection.focus.getNode().getTextContent(), selection.focus.offset
          )
        }
      }
      // Pressing in the paragraph ("bo|dy"), then dragging to "bod".
      step(NSRange(location: 8, length: 0))
      #expect(try model() == ("body", 2, "body", 2))
      #expect(committed.last.map { $0 == ("body", 2, "body", 2) } == true, "\(committed)")
      let font = try #require(view.typingAttributes[.font] as? NSFont)
      #expect(font.pointSize < headingFont.pointSize, "\(font)")
      step(NSRange(location: 8, length: 1))
      #expect(try model() == ("body", 2, "body", 3))
      #expect(committed.last.map { $0 == ("body", 2, "body", 3) } == true, "\(committed)")
      // Back into the heading, before the mouse is released.
      step(NSRange(location: 1, length: 0))
      #expect(try model() == ("Title", 1, "Title", 1))
      #expect(committed.last.map { $0 == ("Title", 1, "Title", 1) } == true, "\(committed)")
      // One commit per step, all before the mouse is released.
      #expect(committed.count == 3, "\(committed)")
      #expect((view.typingAttributes[.font] as? NSFont)?.pointSize == headingFont.pointSize)
      view.setSelectedRanges(
        [NSValue(range: NSRange(location: 1, length: 0))], affinity: .downstream,
        stillSelecting: false)
      // Selection-only commits record no history: undo still takes out the
      // content the test set up.
      #expect(history.undoStack.count == undoDepth)
      #expect(history.redoStack.isEmpty)
      view.undo(nil)
      #expect(view.string == "")
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

    /// A code block's lines, `|` for a line break, then a paragraph so the
    /// block isn't the text's last.
    @Test(arguments: [("a|b", 2), ("a||b", 3), ("a||", 3), ("", 1)])
    func drawsACodeBlocksBoxAndGutter(_ lines: String, count: Int) throws {
      let editor = try buildEditorFromExtensions(
        defineExtension(name: "test", dependencies: [RichTextExtension, CodeExtension]))
      let (view, window) = try makeView(editor)
      window.appearance = NSAppearance(named: .aqua)
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
      let padding = try #require(view.textContainer).lineFragmentPadding
      let left = view.textContainerInset.width + padding
      let top = view.textContainerInset.height + 27.2 + 8
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
      #expect(color(left + 41.5, top + 2)[0] < color(left + 2, top + 2)[0] - 10)
      // One number on each line, empty or not, centered in it like the
      // line's text.
      var inked: [CGFloat] = []
      for row in stride(from: top, to: bottom, by: 0.5) {
        let dark = stride(from: left + 8, to: left + 41 - 4, by: 0.5).contains { x in
          color(x, row)[0] < 180
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

    /// Enter in a code block makes an empty line with the code style's line
    /// box and caret, at its end, in its middle and after several Enters,
    /// as the web's `<br>` takes its block's font, and the line above it
    /// keeps its text where it was. `trailing` puts a paragraph after the
    /// block.
    @Test(arguments: [false, true])
    func givesACodeBlocksEmptyLinesItsLineBox(_ trailing: Bool) throws {
      let editor = try buildEditorFromExtensions(
        defineExtension(name: "test", dependencies: [RichTextExtension, CodeExtension]))
      let (view, window) = try makeView(editor)
      window.appearance = NSAppearance(named: .aqua)
      try editor.update(discrete: true) {
        let code = try createCodeNode()
        try code.append(createTextNode("ab"))
        try getRoot().clear().append(createParagraphNode().append(createTextNode("x")), code)
        if trailing { try getRoot().append(createParagraphNode().append(createTextNode("y"))) }
        _ = try (code.getFirstChild() as? TextNode)?.select(2, 2)
      }
      let layoutManager = try #require(view.textLayoutManager)
      let origin = view.textContainerOrigin
      /// The code block's lines, in view coordinates.
      func lines() throws -> [CGRect] {
        layoutManager.ensureLayout(for: layoutManager.documentRange)
        let start = try #require(
          layoutManager.location(layoutManager.documentRange.location, offsetBy: 2))
        let fragment = try #require(layoutManager.textLayoutFragment(for: start))
        return fragment.textLineFragments.map {
          $0.typographicBounds.offsetBy(
            dx: origin.x + fragment.layoutFragmentFrame.minX,
            dy: origin.y + fragment.layoutFragmentFrame.minY)
        }
      }
      let left = try origin.x + #require(view.textContainer).lineFragmentPadding + 52
      /// The rows the first line's "a" is drawn on, left of any caret.
      func inkOfTheFirstLine() throws -> [CGFloat] {
        let (color, _) = try render(view)
        let line = try lines()[0]
        return stride(from: line.minY, to: line.maxY, by: 0.5).filter { row in
          stride(from: left, to: left + 6, by: 0.5).contains { color($0, row)[0] < 150 }
        }
      }
      let ink = try inkOfTheFirstLine()
      try #require(!ink.isEmpty)
      let font = try #require(NSFont(name: "Menlo-Regular", size: 13))
      let height = font.ascender - font.descender
      // The offscreen window shows no caret, so one stands in for
      // NSTextView's, which is as tall as its line.
      let indicator = NSTextInsertionIndicator(frame: .zero)
      view.addSubview(indicator)
      func expectCodeCaret(at location: Int, line index: Int) throws {
        view.setSelectedRange(NSRange(location: location, length: 0))
        let all = try lines()
        try #require(index < all.count, "\(all) \(view.string.debugDescription)")
        let line = all[index]
        indicator.frame = NSRect(x: left, y: line.minY, width: 2, height: line.height)
        view.layout()
        // As tall as the code font, on the line's centered text.
        let frame = indicator.frame
        #expect(abs(frame.height - height) <= 0.5, "\(location): \(frame)")
        #expect(
          abs(frame.minY - (line.minY + (line.height - height) / 2)) <= 0.5,
          "\(location): \(frame) in \(line)")
      }
      // "ab" is at 2..<4, so its line ends at 4 and the next starts at 5.
      // (A third Enter at the end would leave the block, as on the web.)
      view.insertNewline(nil)
      try expectCodeCaret(at: 5, line: 1)
      view.insertNewline(nil)
      try expectCodeCaret(at: 6, line: 2)
      insert(view, "c")
      // The empty line between "ab" and "c".
      try expectCodeCaret(at: 5, line: 1)
      indicator.removeFromSuperview()
      // Every line is 1.53 × 13 points, empty or not.
      let all = try lines()
      #expect(all.count == 3, "\(all)")
      for line in all { #expect(abs(line.height - 1.53 * 13) <= 0.5, "\(all)") }
      // The first line's text did not move.
      let after = try inkOfTheFirstLine()
      #expect(after == ink, "\(ink) → \(after)")
      _ = window
    }

    /// A quote that ends with a line break, followed by a paragraph: its
    /// bar covers the empty line after the break, as the web's does.
    @Test func drawsAQuotesBarOverItsTrailingEmptyLine() throws {
      let editor = try buildEditorFromExtensions(
        defineExtension(name: "test", dependencies: [RichTextExtension]))
      let (view, window) = try makeView(editor)
      window.appearance = NSAppearance(named: .aqua)
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
      let origin = view.textContainerOrigin.y
      let first = origin + lines[0].midY
      let bar = try #require(
        stride(from: 0, to: 60, by: 0.5).first { near(color($0, first), [206, 208, 212]) },
        "no bar on the first line")
      #expect(near(color(bar + 1, origin + lines[1].midY), [206, 208, 212]))
      #expect(!near(color(bar + 1, origin + lines[2].midY), [206, 208, 212]))
      _ = window
    }

    /// `.ContentEditable__placeholder`: #999 root text over the first line,
    /// centered in it as typed text is, while `canShowPlaceholder` holds.
    @Test func drawsThePlaceholderWhereTypedTextWouldBe() throws {
      let editor = try buildEditorFromExtensions(
        defineExtension(name: "test", dependencies: [RichTextExtension]))
      let (view, window) = try makeView(editor)
      window.appearance = NSAppearance(named: .aqua)
      let text = "Enter some rich text..."
      view.placeholder = text
      #expect(view.accessibilityPlaceholderValue() == text)
      // The rows of the first line's ink of a color: at least half
      // covered, so text of either color measures alike.
      func rows(_ isInk: @escaping ([Int]) -> Bool) throws -> ClosedRange<CGFloat>? {
        let (color, _) = try render(view)
        let x = view.textContainerOrigin.x
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
      #expect(view.accessibilityPlaceholderValue() == nil)
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
        #expect((view.accessibilityPlaceholderValue() == text) == shows)
      }
      _ = window
    }

    /// Renders `view`, returning its color at a point and a comparison
    /// that allows for the offscreen bitmap's color space.
    private func render(
      _ view: LexicalNSTextView
    ) throws -> (
      (CGFloat, CGFloat) -> [Int], ([Int], [Int]) -> Bool
    ) {
      let layoutManager = try #require(view.textLayoutManager)
      layoutManager.ensureLayout(for: layoutManager.documentRange)
      view.layoutSubtreeIfNeeded()
      let rep = try #require(view.bitmapImageRepForCachingDisplay(in: view.bounds))
      view.cacheDisplay(in: view.bounds, to: rep)
      let scale = CGFloat(rep.pixelsHigh) / view.bounds.height
      return (
        { x, y in
          let pixel = rep.colorAt(x: Int(x * scale), y: Int(y * scale))?.usingColorSpace(.sRGB)
          return [pixel?.redComponent, pixel?.greenComponent, pixel?.blueComponent].map {
            Int((($0 ?? 0) * 255).rounded())
          }
        },
        { a, b in zip(a, b).allSatisfy { abs($0 - $1) <= 12 } }
      )
    }

    /// `__link:hover { text-decoration: underline }`: only the link under
    /// the pointer is underlined, as view state that sends no update.
    @Test func underlinesTheHoveredLink() throws {
      let (editor, history) = try makeEditorWithHistory([LinkExtension])
      let (view, window) = try makeView(editor)
      try editor.update(discrete: true) {
        try getRoot().clear().append(
          createParagraphNode().append(
            createTextNode("a "),
            createLinkNode("https://lexical.dev").append(createTextNode("bb")),
            createLinkNode("https://lexical.dev/docs").append(createTextNode("cc"))))
      }
      let layoutManager = try #require(view.textLayoutManager)
      layoutManager.ensureLayout(for: layoutManager.documentRange)
      func point(at index: Int) throws -> NSPoint {
        let start = try #require(
          layoutManager.location(layoutManager.documentRange.location, offsetBy: index))
        let range = try #require(
          NSTextRange(location: start, end: layoutManager.location(start, offsetBy: 1)))
        var frame = NSRect.zero
        layoutManager.enumerateTextSegments(in: range, type: .standard, options: []) {
          _, segment, _, _ in
          frame = segment
          return false
        }
        return NSPoint(
          x: frame.midX + view.textContainerOrigin.x, y: frame.midY + view.textContainerOrigin.y)
      }
      func underlined() throws -> [NSRange] {
        let storage = try #require(view.textStorage)
        var ranges: [NSRange] = []
        storage.enumerateAttribute(
          .underlineStyle, in: NSRange(location: 0, length: storage.length)
        ) { value, range, _ in
          if value != nil { ranges.append(range) }
        }
        return ranges
      }
      func hover(_ point: NSPoint?) {
        guard let point else {
          view.mouseExited(
            with: NSEvent.enterExitEvent(
              with: .mouseExited, location: .zero, modifierFlags: [], timestamp: 0,
              windowNumber: window.windowNumber, context: nil, eventNumber: 0, trackingNumber: 0,
              userData: nil)!)
          return
        }
        view.mouseMoved(
          with: NSEvent.mouseEvent(
            with: .mouseMoved, location: view.convert(point, to: nil), modifierFlags: [],
            timestamp: 0, windowNumber: window.windowNumber, context: nil, eventNumber: 0,
            clickCount: 0, pressure: 0)!)
      }
      let undoDepth = history.undoStack.count
      let state = editor.getEditorState()
      var updates = 0
      let remove = editor.registerUpdateListener { _ in updates += 1 }
      defer { remove() }
      hover(try point(at: 0))
      #expect(try underlined().isEmpty)
      hover(try point(at: 3))
      #expect(try underlined() == [NSRange(location: 2, length: 2)])
      // The underline is the view's: the link's text keeps no underline
      // format.
      try editor.read {
        let link = try #require(
          try getRoot().getFirstDescendant()?.getNextSibling() as? ElementNode)
        #expect(try (link.getFirstChild() as? TextNode)?.getFormat() == 0)
      }
      hover(try point(at: 4))
      #expect(try underlined() == [NSRange(location: 4, length: 2)])
      hover(nil)
      #expect(try underlined().isEmpty)
      // Hovering touches neither the model nor the history.
      #expect(updates == 0)
      #expect(editor.getEditorState() === state)
      #expect(history.undoStack.count == undoDepth)
      #expect(history.redoStack.isEmpty)
      _ = window
    }

    @Test func showsThePointingHandOverLinks() throws {
      let editor = try buildEditorFromExtensions(
        defineExtension(name: "test", dependencies: [RichTextExtension, LinkExtension]))
      let (view, window) = try makeView(editor)
      try editor.update(discrete: true) {
        try getRoot().clear().append(
          createParagraphNode().append(
            createTextNode("a "), createLinkNode("https://lexical.dev").append(createTextNode("b"))
          ))
      }
      // `__link:hover { cursor: pointer }`; other text keeps the I-beam.
      let storage = try #require(view.textStorage)
      #expect(storage.attribute(.cursor, at: 2, effectiveRange: nil) as? NSCursor == .pointingHand)
      #expect(storage.attribute(.cursor, at: 0, effectiveRange: nil) == nil)
      _ = window
    }
  }
#endif
