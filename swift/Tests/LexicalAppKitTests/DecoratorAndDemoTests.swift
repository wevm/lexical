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
          if String(describing: type(of: subview)).contains("NSHostingView") {
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
  }
#endif
