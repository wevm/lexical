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

    /// A `TextKitSuffix` is laid out with the text, so the node after it
    /// moves aside, and stays out of the model; a suggestion accepted
    /// mid-composition ends the keyboard's marked text with the model's.
    @Test func laysOutASuffixAndEndsTheCompositionWithTheModel() throws {
      let (view, window) = try makeView()
      try view.editor.update(discrete: true) {
        try getRoot().clear().append(
          createParagraphNode().append(
            createTextNode("a "), try createTextNode("cd").toggleFormat("bold")))
        _ = try (getRoot().getFirstDescendant() as! TextNode).select(2, 2)
      }
      let key = try view.editor.read { try getRoot().getFirstDescendant()!.getKey() }
      func x(_ location: Int) -> CGFloat {
        view.layoutIfNeeded()
        let position = view.position(from: view.beginningOfDocument, offset: location)!
        return view.caretRect(for: position).minX
      }
      let before = x(2)
      let model = try json(view)
      try view.controller.setSuffix(TextKitSuffix(key: key, text: "xyz", type: "autocomplete"))
      #expect(view.text == "a xyzcd")
      #expect(x(5) > before + 10)
      #expect(try json(view) == model)
      #expect(view.selectedRange == NSRange(location: 2, length: 0))
      view.setMarkedText("사", selectedRange: NSRange(location: 1, length: 0))
      #expect(view.text == "a 사cd")
      try view.controller.setSuffix(TextKitSuffix(key: key, text: "용", type: "autocomplete"))
      #expect(view.text == "a 사용cd")
      try view.editor.update(discrete: true) {
        let node = try getNodeByKey(key) as! TextNode
        try node.setTextContent(try node.getTextContent() + "용")
        try setCompositionKey(nil)
        _ = try node.select()
      }
      #expect(view.markedTextRange == nil)
      #expect(!view.editor.isComposing())
      #expect(view.text == "a 사용cd")
      view.setMarkedText("ㄱ", selectedRange: NSRange(location: 1, length: 0))
      #expect(view.selectedRange == NSRange(location: 5, length: 0))
      view.unmarkText()
      #expect(view.text == "a 사용ㄱcd")
      #expect(try json(view).contains(#""text":"a 사용ㄱ""#))
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

    /// A selected decorator shows only its own outline, as on the web: not
    /// UIKit's handles and highlight over its attachment character. A text
    /// selection afterwards has them again. `block` selects the demo's card,
    /// or else its inline badge.
    @Test(arguments: [true, false])
    func showsNoHandlesBesideASelectedDecorator(_ block: Bool) throws {
      let editor = try makeDemoEditor()
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 600))
      let view = LexicalUITextView(editor: editor, frame: window.bounds)
      window.addSubview(view)
      window.makeKeyAndVisible()
      _ = view.becomeFirstResponder()
      try loadDemoDocument(editor)
      /// The selection handles UIKit shows once they settle, and the most
      /// it showed meanwhile. UIKit adds and fades them in some run loop
      /// turns after the selection changes, later on some runtimes than
      /// others, so this watches for at least a second, until the count has
      /// held for half a second (or five seconds pass): long enough that
      /// handles shown late still count, in either direction.
      func settledHandles() -> (shown: [UIView], most: Int) {
        func visible(_ view: UIView) -> Bool {
          !view.isHidden && view.alpha > 0 && (view.superview.map(visible) ?? true)
        }
        func handles(_ view: UIView) -> [UIView] {
          (view is UITextSelectionHandleView ? [view] : []) + view.subviews.flatMap(handles)
        }
        func shown() -> [UIView] {
          window.layoutIfNeeded()
          return handles(view).filter(visible)
        }
        let start = Date()
        var current = shown()
        var most = current.count
        var since = start
        while Date().timeIntervalSince(start) < 5 {
          RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
          let next = shown()
          most = max(most, next.count)
          if next.count != current.count {
            current = next
            since = Date()
          } else if Date().timeIntervalSince(start) >= 1, Date().timeIntervalSince(since) >= 0.5 {
            return (next, most)
          }
        }
        return (current, most)
      }
      func select(_ body: @escaping () throws -> Void) throws {
        try editor.update(discrete: true) { try body() }
      }
      try select {
        let parent = try block ? getRoot() : #require(getRoot().getChildAtIndex(1) as? ElementNode)
        let node = try #require(
          try parent.getChildren().first { block ? $0 is CardNode : $0 is BadgeNode })
        let selection = createNodeSelection()
        selection.add(node.getKey())
        try setSelection(selection)
      }
      #expect(view.selectedRange.length == 1)
      let decorator = settledHandles()
      #expect(decorator.most == 0, "\(decorator.shown)")
      // Text selected again shows its handles.
      try select {
        let text = try #require(try getRoot().getFirstDescendant() as? TextNode)
        _ = try text.select(0, 5)
      }
      // Loading the document left the caret at its end, which the view
      // scrolled into view; a range selection doesn't scroll (as in JS), so
      // bring its text back on screen, where UIKit shows its handles.
      view.setContentOffset(.zero, animated: false)
      let textHandles = settledHandles()
      #expect(textHandles.shown.count == 2, "\(textHandles.shown)")
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

    /// A highlight (comment marks, find and replace matches) covers the
    /// same text box as the selection over the same range, the font's
    /// content area rather than the line box, as
    /// `.PlaygroundEditorTheme__mark` is an inline background on the web:
    /// in a 1.7 paragraph, in a 1.53 code block, and beside an inline
    /// decorator taller than the line, where both are on the text, which is
    /// on the decorator's baseline.
    @Test(arguments: ["paragraph", "code", "tall"])
    func paintsAHighlightOverTheSelectionsTextBox(_ block: String) throws {
      TallNode.size = CGSize(width: 24, height: 80)
      let editor = try buildEditorFromExtensions(
        defineExtension(
          name: "test", dependencies: [RichTextExtension, CodeExtension],
          nodes: { [.klass(TallNode.self)] }))
      let (view, window) = makeLightView(editor)
      var key = ""
      try editor.update(discrete: true) {
        // Spaces where the color is sampled, so no glyph is over it.
        let text = try createTextNode("ab    cd")
        key = text.getKey()
        let parent: ElementNode =
          block == "code"
          ? try createCodeNode().append(text)
          : block == "tall"
            ? try createParagraphNode().append(create(TallNode.self), text)
            : try createParagraphNode().append(text)
        try getRoot().clear().append(createParagraphNode().append(createTextNode("x")), parent)
      }
      view.controller.highlights["test"] = TextKitHighlight(
        ranges: [
          TextKitHighlightRange(anchorKey: key, anchorOffset: 2, focusKey: key, focusOffset: 6)
        ],
        backgroundColor: TextKitColor(255, 0, 0))
      let (color, near) = try render(view)
      let location = try #require(view.controller.document.span(key)).location + 2
      let start = try #require(view.position(from: view.beginningOfDocument, offset: location))
      let end = try #require(view.position(from: start, offset: 4))
      let range = try #require(view.textRange(from: start, to: end))
      let selection = view.selectionRects(for: range).map(\.rect)
      try #require(selection.count == 1, "\(selection)")
      let box = selection[0]
      // The selection is on the text: "ab"'s box has its ink.
      let letters = try #require(view.position(from: start, offset: -2))
      let lettersRange = try #require(view.textRange(from: letters, to: start))
      let text = try #require(view.selectionRects(for: lettersRange).first?.rect)
      // The highlight's color as drawn, in the bitmap's color space.
      let red = color(box.midX, box.midY)
      #expect(red.filter { $0 > 200 }.count == 1 && red.filter { $0 < 80 }.count == 2, "\(red)")
      #expect(
        near(color(box.midX, box.minY + 1.5), red), "\(box) \(color(box.midX, box.minY + 1.5))")
      #expect(
        near(color(box.midX, box.maxY - 1.5), red), "\(box) \(color(box.midX, box.maxY - 1.5))")
      #expect(
        !near(color(box.midX, box.minY - 1.5), red), "\(box) \(color(box.midX, box.minY - 1.5))")
      #expect(
        !near(color(box.midX, box.maxY + 1.5), red), "\(box) \(color(box.midX, box.maxY + 1.5))")
      let inked = stride(from: text.minY, to: text.maxY, by: 0.5).contains { y in
        stride(from: text.minX, to: text.maxX, by: 0.5).contains { x in
          color(x, y).allSatisfy { $0 < 100 }
        }
      }
      #expect(inked, "\(text)")
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

    /// Enter in a code block makes an empty line with the code style's line
    /// box and caret, at its end, in its middle and after several Enters,
    /// as the web's `<br>` takes its block's font, and the line above it
    /// keeps its text where it was. `trailing` puts a paragraph after the
    /// block.
    @Test(arguments: [false, true])
    func givesACodeBlocksEmptyLinesItsLineBox(_ trailing: Bool) throws {
      let editor = try buildEditorFromExtensions(
        defineExtension(name: "test", dependencies: [RichTextExtension, CodeExtension]))
      let (view, window) = makeLightView(editor)
      _ = view.becomeFirstResponder()
      try editor.update(discrete: true) {
        let code = try createCodeNode()
        try code.append(createTextNode("ab"))
        try getRoot().clear().append(createParagraphNode().append(createTextNode("x")), code)
        if trailing { try getRoot().append(createParagraphNode().append(createTextNode("y"))) }
        _ = try (code.getFirstChild() as? TextNode)?.select(2, 2)
      }
      let layoutManager = try #require(view.textLayoutManager)
      /// The code block's lines, in view coordinates.
      func lines() throws -> [CGRect] {
        view.layoutIfNeeded()
        layoutManager.ensureLayout(for: layoutManager.documentRange)
        let start = try #require(
          layoutManager.location(layoutManager.documentRange.location, offsetBy: 2))
        let fragment = try #require(layoutManager.textLayoutFragment(for: start))
        return fragment.textLineFragments.map {
          $0.typographicBounds.offsetBy(
            dx: view.textContainerInset.left + fragment.layoutFragmentFrame.minX,
            dy: view.textContainerInset.top + fragment.layoutFragmentFrame.minY)
        }
      }
      let left = view.textContainerInset.left + view.textContainer.lineFragmentPadding + 52
      /// The rows the first line's "a" is drawn on, left of any caret.
      func inkOfTheFirstLine() throws -> [CGFloat] {
        let line = try lines()[0]
        let (color, _) = try render(view)
        return stride(from: line.minY, to: line.maxY, by: 0.5).filter { row in
          stride(from: left, to: left + 6, by: 0.5).contains { color($0, row)[1] < 150 }
        }
      }
      let ink = try inkOfTheFirstLine()
      try #require(!ink.isEmpty)
      let font = try #require(UIFont(name: "Menlo-Regular", size: 13))
      let height = font.ascender - font.descender
      func expectCodeCaret(at offset: Int, line index: Int) throws {
        let all = try lines()
        try #require(index < all.count, "\(all) \(view.textStorage.string.debugDescription)")
        let line = all[index]
        let position = try #require(view.position(from: view.beginningOfDocument, offset: offset))
        // As tall as the code font, on the line's centered text.
        let caret = view.caretRect(for: position)
        #expect(abs(caret.height - height) <= 0.5, "\(offset): \(caret)")
        #expect(
          abs(caret.minY - (line.minY + (line.height - height) / 2)) <= 0.5,
          "\(offset): \(caret) in \(line)")
      }
      // "ab" is at 2..<4, so its line ends at 4 and the next starts at 5.
      // (A third Enter at the end would leave the block, as on the web.)
      view.insertText("\n")
      try expectCodeCaret(at: 5, line: 1)
      view.insertText("\n")
      try expectCodeCaret(at: 6, line: 2)
      view.insertText("c")
      // The empty line between "ab" and "c".
      try expectCodeCaret(at: 5, line: 1)
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
