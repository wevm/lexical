#if os(macOS)
  import AppKit
  import Lexical
  import LexicalAppKit
  import LexicalSwiftUI
  import LexicalTextKit
  import SwiftUI
  import Testing

  // DecoratorNodes as SwiftUI views, and the demo document in the AppKit
  // view. Set LEXICAL_SCREENSHOT_DIR to write the screenshot there.

  @MainActor
  private func showDemo() throws -> (LexicalNSTextView, NSWindow) {
    NSApplication.shared.setActivationPolicy(.accessory)
    let editor = try makeDemoEditor()
    let window = NSWindow(
      contentRect: NSRect(x: 0, y: 0, width: 640, height: 520), styleMask: [.titled],
      backing: .buffered, defer: false)
    window.appearance = NSAppearance(named: .aqua)
    let view = LexicalNSTextView(editor: editor, frame: window.contentLayoutRect)
    view.autoresizingMask = [.width, .height]
    window.contentView = view
    try loadDemoDocument(editor)
    window.makeFirstResponder(view)
    window.orderFrontRegardless()
    settle(view)
    return (view, window)
  }

  /// Lays the view out and lets the hosted SwiftUI views render.
  @MainActor
  private func settle(_ view: NSView) {
    for _ in 0..<5 {
      view.window?.displayIfNeeded()
      view.layoutSubtreeIfNeeded()
      if let textView = view as? NSTextView, let layoutManager = textView.textLayoutManager {
        layoutManager.ensureLayout(for: layoutManager.documentRange)
      }
      RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
    }
  }

  @MainActor
  private func writePNG(_ view: NSView, _ name: String) throws {
    guard let directory = ProcessInfo.processInfo.environment["LEXICAL_SCREENSHOT_DIR"] else {
      return
    }
    let rep = try #require(view.bitmapImageRepForCachingDisplay(in: view.bounds))
    view.cacheDisplay(in: view.bounds, to: rep)
    let data = try #require(rep.representation(using: .png, properties: [:]))
    try data.write(to: URL(fileURLWithPath: directory).appendingPathComponent(name))
  }

  /// Whether `view`, drawn into an sRGB bitmap, has a pixel within 6 of
  /// `rgb` (0...255) in `rect` (view coordinates).
  @MainActor
  private func draws(_ view: NSView, _ rgb: (Int, Int, Int), in rect: NSRect) -> Bool {
    guard
      let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: Int(view.bounds.width) * 2,
        pixelsHigh: Int(view.bounds.height) * 2, bitsPerSample: 8, samplesPerPixel: 4,
        hasAlpha: true, isPlanar: false, colorSpaceName: .calibratedRGB, bytesPerRow: 0,
        bitsPerPixel: 0)?.retagging(with: .sRGB)
    else { return false }
    rep.size = view.bounds.size
    view.cacheDisplay(in: view.bounds, to: rep)
    let scale = CGFloat(rep.pixelsWide) / view.bounds.width
    let rect = rect.intersection(view.bounds)
    for y in stride(from: rect.minY, to: rect.maxY, by: 1) {
      for x in stride(from: rect.minX, to: rect.maxX, by: 1) {
        // The bitmap's rows run top-down, as a flipped view's do.
        let row = view.isFlipped ? y : view.bounds.height - y
        // The components as stored: sRGB, which `usingColorSpace` would
        // convert again from the rep's calibrated color space name.
        guard let color = rep.colorAt(x: Int(x * scale), y: Int(row * scale)) else { continue }
        let (r, g, b) = (
          Int(color.redComponent * 255), Int(color.greenComponent * 255),
          Int(color.blueComponent * 255)
        )
        if abs(r - rgb.0) <= 6 && abs(g - rgb.1) <= 6 && abs(b - rgb.2) <= 6 { return true }
      }
    }
    return false
  }

  /// The views hosting SwiftUI content in `view` and under it.
  @MainActor
  private func hostingViews(in view: NSView) -> [NSView] {
    view is NSHostingView<AnyView> ? [view] : view.subviews.flatMap { hostingViews(in: $0) }
  }

  @MainActor
  @Suite struct DecoratorAndDemoTests {
    @Test func rendersDecoratorsAsSwiftUIViews() throws {
      let (view, window) = try showDemo()
      let text = view.string as NSString
      #expect(view.editor.getDecorators().count == 2)
      var attachments: [NSTextAttachment] = []
      view.textStorage?.enumerateAttribute(
        .attachment, in: NSRange(location: 0, length: text.length)
      ) { value, _, _ in
        if let attachment = value as? NSTextAttachment { attachments.append(attachment) }
      }
      #expect(attachments.count == 2)
      #expect(attachments.allSatisfy { $0.allowsTextAttachmentView })
      // Their hosting views are in the text view once laid out.
      var hosted: [NSView] = []
      func collect(_ view: NSView) {
        for subview in view.subviews {
          if subview is NSHostingView<AnyView> {
            hosted.append(subview)
          } else {
            collect(subview)
          }
        }
      }
      collect(view)
      #expect(hosted.count == 2)
      // The block card takes the line's width; the badge its own.
      let widths = hosted.map(\.frame.width).sorted()
      #expect(widths[0] < 120 && widths[1] > 400)
      // VoiceOver's label for a decorator is its text content when the
      // SwiftUI view gives none (as here: SwiftUI builds its accessibility
      // elements only for an assistive technology). The card has no text.
      let labels = hosted.sorted { $0.frame.width < $1.frame.width }.map {
        $0.accessibilityLabel()
      }
      #expect(labels == ["SwiftUI", nil])
      // Editing next to a decorator keeps its view.
      let before = Set(hosted.map(ObjectIdentifier.init))
      view.insertText(" More.", replacementRange: NSRange(location: NSNotFound, length: 0))
      settle(view)
      hosted = []
      collect(view)
      #expect(Set(hosted.map(ObjectIdentifier.init)) == before)
      // Deleting a decorator removes its view.
      try view.editor.update(discrete: true) {
        for node in try getRoot().getChildren() where node is CardNode {
          try node.remove()
        }
      }
      settle(view)
      #expect(view.editor.getDecorators().count == 1)
      _ = window
    }

    /// TextKit makes a new view provider for a decorator each time it lays
    /// the decorator's line out again, and releases the old one, which
    /// takes its view out of the text, after it shows the new one's: the
    /// decorator stays in the new one.
    @Test func keepsDecoratorsShownWhenTextKitReplacesTheirProviders() throws {
      let (view, window) = try showDemo()
      _ = window
      let storage = try #require(view.textStorage)
      let layoutManager = try #require(view.textLayoutManager)
      // The viewport the providers' views are shown in.
      let viewport = NSWindow(
        contentRect: NSRect(x: 0, y: 0, width: 640, height: 200), styleMask: [.titled],
        backing: .buffered, defer: false)
      viewport.isReleasedWhenClosed = false
      viewport.orderFrontRegardless()
      defer { viewport.close() }
      let content = try #require(viewport.contentView)
      var shown = 0
      storage.enumerateAttribute(
        .attachment, in: NSRange(location: 0, length: storage.length)
      ) { value, range, _ in
        guard let attachment = value as? NSTextAttachment,
          let location = layoutManager.location(
            layoutManager.documentRange.location, offsetBy: range.location)
        else { return }
        func show() -> NSTextAttachmentViewProvider? {
          let provider = attachment.viewProvider(
            for: view, location: location, textContainer: view.textContainer)
          provider?.loadView()
          if let view = provider?.view {
            let fragment = NSView(frame: NSRect(x: 0, y: 0, width: 200, height: 60))
            content.addSubview(fragment)
            fragment.addSubview(view)
          }
          return provider
        }
        var old = show()
        let new = show()
        old = nil
        _ = old
        // Without its own container, the new view is the old one, now gone.
        if let view = new?.view, view.window != nil, !hostingViews(in: view).isEmpty { shown += 1 }
      }
      #expect(shown == 2)
    }

    @Test func rendersTheDemoDocument() throws {
      let (view, window) = try showDemo()
      #expect(view.string.hasPrefix("Lexical for Swift\nNative editing with bold"))
      let heading = try #require(
        view.textStorage?.attribute(.font, at: 0, effectiveRange: nil) as? NSFont)
      #expect(heading.pointSize == 24)
      let code = (view.string as NSString).range(of: "inline code")
      let codeFont = try #require(
        view.textStorage?.attribute(.font, at: code.location, effectiveRange: nil) as? NSFont)
      #expect(codeFont.isFixedPitch)
      let quote = (view.string as NSString).range(of: "A quote")
      let style = try #require(
        view.textStorage?.attribute(.paragraphStyle, at: quote.location, effectiveRange: nil)
          as? NSParagraphStyle)
      #expect(style.headIndent == 40)
      try writePNG(view, "appkit-demo.png")
      _ = window
    }

    @Test func followsTheAppearance() throws {
      let (view, window) = try showDemo()
      let text = view.string as NSString
      func rect(_ string: String) -> NSRect {
        let screen = view.firstRect(forCharacterRange: text.range(of: string), actualRange: nil)
        return view.convert(window.convertFromScreen(screen), from: nil)
      }
      // The quote's bar is left of its text; the code's background behind it.
      let quote = rect("A quote")
      let bar = NSRect(x: 0, y: quote.midY - 2, width: quote.minX, height: 4)
      let code = rect("inline")
      #expect(draws(view, (206, 208, 212), in: bar))
      #expect(draws(view, (240, 242, 245), in: code))
      window.appearance = NSAppearance(named: .darkAqua)
      settle(view)
      #expect(draws(view, (84, 86, 92), in: bar))
      #expect(!draws(view, (206, 208, 212), in: bar))
      #expect(draws(view, (44, 46, 51), in: code))
      try writePNG(view, "appkit-demo-dark.png")
      window.appearance = NSAppearance(named: .aqua)
      settle(view)
      #expect(draws(view, (206, 208, 212), in: bar))
      _ = window
    }

    @Test func rendersInSwiftUI() throws {
      let editor = try makeDemoEditor()
      try loadDemoDocument(editor)
      let host = NSHostingView(rootView: LexicalEditor(editor).frame(width: 640, height: 520))
      host.frame = NSRect(x: 0, y: 0, width: 640, height: 520)
      let window = NSWindow(
        contentRect: host.frame, styleMask: [.titled], backing: .buffered, defer: false)
      window.appearance = NSAppearance(named: .aqua)
      window.contentView = host
      settle(host)
      #expect(editor.getRootElement() is LexicalNSTextView)
      let textView = try #require(editor.getRootElement() as? LexicalNSTextView)
      #expect(textView.string.hasPrefix("Lexical for Swift"))
      try writePNG(host, "swiftui-demo.png")
      _ = window
    }

    /// `sizedToContent`: 8pt insets around the lines, each 1.7 × 16 =
    /// 27.2pt as in the playground's CSS, up to the maximum, past which it
    /// scrolls; and as short again once the text is.
    @Test func sizesToItsContentInSwiftUI() throws {
      let editor = try makeDemoEditor()
      let host = NSHostingView(
        rootView: VStack(spacing: 0) {
          LexicalEditor(editor).sizedToContent(maxHeight: 150)
          Spacer(minLength: 0)
        })
      host.frame = NSRect(x: 0, y: 0, width: 400, height: 400)
      let window = NSWindow(
        contentRect: host.frame, styleMask: [.titled], backing: .buffered, defer: false)
      window.contentView = host
      func height(lines: Int) throws -> CGFloat {
        try editor.update(discrete: true) {
          let root = try getRoot().clear()
          if lines == 0 { try root.append(createParagraphNode()) }
          for line in 0..<lines {
            try root.append(createParagraphNode().append(createTextNode("Line \(line)")))
          }
        }
        settle(host)
        let textView = try #require(editor.getRootElement() as? LexicalNSTextView)
        return try #require(textView.enclosingScrollView).frame.height
      }
      #expect(abs(try height(lines: 0) - 43.2) <= 1.5)
      #expect(abs(try height(lines: 3) - 97.6) <= 1.5)
      #expect(try height(lines: 10) == 150)
      #expect(abs(try height(lines: 1) - 43.2) <= 1.5)
      _ = window
    }
  }
#endif
