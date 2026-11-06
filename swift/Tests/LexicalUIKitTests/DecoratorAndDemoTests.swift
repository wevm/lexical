#if canImport(UIKit)
  import Lexical
  import LexicalExtension
  import LexicalList
  import LexicalRichText
  import LexicalSwiftUI
  import LexicalTextKit
  import LexicalUIKit
  import SwiftUI
  import Testing
  import UIKit

  // DecoratorNodes as SwiftUI views, and the demo document in the UIKit
  // view. Set LEXICAL_SCREENSHOT_DIR (TEST_RUNNER_LEXICAL_SCREENSHOT_DIR
  // through xcodebuild) to write the screenshot there.

  @MainActor
  private func showDemo() throws -> (LexicalUITextView, UIWindow) {
    let editor = try makeDemoEditor()
    let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 520))
    window.overrideUserInterfaceStyle = .light
    let view = LexicalUITextView(editor: editor, frame: window.bounds)
    window.addSubview(view)
    window.makeKeyAndVisible()
    try loadDemoDocument(editor)
    settle(view)
    return (view, window)
  }

  /// Lays the view out and lets the hosted SwiftUI views render.
  @MainActor
  private func settle(_ view: UIView) {
    for _ in 0..<5 {
      view.window?.layoutIfNeeded()
      if let textView = view as? UITextView, let layoutManager = textView.textLayoutManager {
        layoutManager.ensureLayout(for: layoutManager.documentRange)
      }
      RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
    }
  }

  @MainActor
  private func writePNG(_ view: UIView, _ name: String, background: UIColor = .white) throws {
    guard let directory = ProcessInfo.processInfo.environment["LEXICAL_SCREENSHOT_DIR"] else {
      return
    }
    let format = UIGraphicsImageRendererFormat()
    format.scale = 2
    // The test bundle has no host app, so there is no screen to snapshot;
    // the layers render themselves.
    let image = UIGraphicsImageRenderer(bounds: view.bounds, format: format).image { context in
      background.setFill()
      context.fill(view.bounds)
      view.layer.render(in: context.cgContext)
    }
    let data = try #require(image.pngData())
    try data.write(to: URL(fileURLWithPath: directory).appendingPathComponent(name))
  }

  /// Whether `view`, as drawn, has a pixel within 6 of `rgb` (0...255) in
  /// `rect`.
  @MainActor
  private func draws(_ view: UIView, _ rgb: (Int, Int, Int), in rect: CGRect) -> Bool {
    let format = UIGraphicsImageRendererFormat()
    format.scale = 1
    let image = UIGraphicsImageRenderer(bounds: view.bounds, format: format).image { context in
      view.layer.render(in: context.cgContext)
    }
    guard let cgImage = image.cgImage, let data = cgImage.dataProvider?.data,
      let bytes = CFDataGetBytePtr(data)
    else { return false }
    // The image starts at the bounds' origin, which a scroll moves.
    let rect = rect.integral.intersection(view.bounds).offsetBy(
      dx: -view.bounds.minX, dy: -view.bounds.minY)
    for y in Int(rect.minY)..<Int(rect.maxY) {
      for x in Int(rect.minX)..<Int(rect.maxX) {
        let pixel = bytes + y * cgImage.bytesPerRow + x * cgImage.bitsPerPixel / 8
        // BGRA or RGBA: compare both orders.
        let (a, b, c) = (Int(pixel[0]), Int(pixel[1]), Int(pixel[2]))
        func near(_ p: Int, _ q: Int, _ r: Int) -> Bool {
          abs(p - rgb.0) <= 6 && abs(q - rgb.1) <= 6 && abs(r - rgb.2) <= 6
        }
        if near(a, b, c) || near(c, b, a) { return true }
      }
    }
    return false
  }

  /// The views hosting SwiftUI content under `view`.
  @MainActor
  private func hostedViews(_ view: UIView) -> [UIView] {
    view.subviews.flatMap { subview in
      String(describing: type(of: subview)).contains("HostingView")
        ? [subview] : hostedViews(subview)
    }
  }

  @MainActor
  @Suite struct UIKitDecoratorAndDemoTests {
    @Test func rendersDecoratorsAsSwiftUIViews() throws {
      let (view, window) = try showDemo()
      #expect(view.editor.getDecorators().count == 2)
      var hosted = hostedViews(view)
      #expect(hosted.count == 2)
      // The block card takes the line's width; the badge its own.
      let widths = hosted.map(\.frame.width).sorted()
      #expect(widths.count == 2 && widths[0] < 120 && widths[1] > 300)
      // VoiceOver's label for a decorator's character is its text content
      // when the SwiftUI view gives none (as here: SwiftUI builds its
      // accessibility elements only for an assistive technology). The card
      // has no text.
      var labels: [String?] = []
      view.textStorage.enumerateAttribute(
        .attachment, in: NSRange(location: 0, length: view.textStorage.length)
      ) { value, _, _ in
        if let attachment = value as? NSTextAttachment {
          labels.append(attachment.accessibilityLabel)
        }
      }
      #expect(labels == ["SwiftUI", nil])
      // Editing next to a decorator keeps its view.
      let before = Set(hosted.map(ObjectIdentifier.init))
      view.insertText(" More.")
      settle(view)
      hosted = hostedViews(view)
      #expect(Set(hosted.map(ObjectIdentifier.init)) == before)
      // Deleting a decorator removes its view.
      try view.editor.update(discrete: true) {
        for node in try getRoot().getChildren() where node is CardNode {
          try node.remove()
        }
      }
      settle(view)
      #expect(view.editor.getDecorators().count == 1)
      #expect(hostedViews(view).count == 1)
      _ = window
    }

    /// TextKit makes a new view provider for a decorator each time it lays
    /// the decorator's line out again, and releases the old one, which
    /// takes its view out of the text, after it shows the new one's: the
    /// decorator stays in the new one. It goes there without leaving the
    /// window, so a text field in it stays the first responder: leaving
    /// the window ended its editing, and the keyboard change that follows
    /// resized the text view during its own layout, which TextKit 2 raises
    /// on.
    @Test func keepsDecoratorsShownWhenTextKitReplacesTheirProviders() throws {
      let (view, window) = try showDemo()
      let storage = view.textStorage
      let layoutManager = try #require(view.textLayoutManager)
      // The viewport the providers' views are shown in.
      let content = UIView(frame: CGRect(x: 0, y: 0, width: 390, height: 200))
      window.addSubview(content)
      defer { content.removeFromSuperview() }
      var shown = 0
      var keptEditing = 0
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
            let fragment = UIView(frame: CGRect(x: 0, y: 0, width: 200, height: 60))
            content.addSubview(fragment)
            fragment.addSubview(view)
          }
          return provider
        }
        var old = show()
        let field = UITextField(frame: CGRect(x: 0, y: 0, width: 40, height: 20))
        if let hosted = old?.view.flatMap({ hostedViews($0).first }) { hosted.addSubview(field) }
        let editing = field.becomeFirstResponder()
        let new = show()
        if editing && field.isFirstResponder { keptEditing += 1 }
        field.removeFromSuperview()
        old = nil
        _ = old
        // Without its own container, the new view is the old one, now gone.
        if let view = new?.view, view.window != nil, !hostedViews(view).isEmpty { shown += 1 }
      }
      #expect(shown == 2)
      #expect(keptEditing == 2)
    }

    /// A line with an inline decorator taller than the line height, as an
    /// image is, in a paragraph or a list item: its text sits on the
    /// decorator's baseline, as an inline image's line's does in CSS, and
    /// the next line's text is drawn on its own line only.
    @Test(arguments: [(false, 80.0), (true, 250.0)])
    func drawsTextBesideATallInlineDecoratorOnItsBaseline(_ list: Bool, _ height: Double) throws {
      TallNode.size = CGSize(width: height == 80 ? 24 : 200, height: height)
      let editor = try buildEditorFromExtensions(
        defineExtension(
          name: "tall", dependencies: [RichTextExtension, ListExtension],
          nodes: { [.klass(TallNode.self)] }))
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 520))
      window.overrideUserInterfaceStyle = .light
      let view = LexicalUITextView(editor: editor, frame: window.bounds)
      window.addSubview(view)
      window.makeKeyAndVisible()
      try editor.update(discrete: true) {
        let children: [LexicalNode] = try [
          createTextNode("Ab"), create(TallNode.self),
          createTextNode(" cd" + String(repeating: " mmmm", count: 16)),
        ]
        let block: ElementNode =
          list
          ? try createListNode("bullet").append(createListItemNode().append(contentsOf: children))
          : try createParagraphNode().append(contentsOf: children)
        try getRoot().clear().append(block)
      }
      settle(view)
      try writePNG(view, "uikit-tall-inline-\(list)-\(Int(height)).png")
      let hosted = try #require(hostedViews(view).first)
      let image = hosted.convert(hosted.bounds, to: view)
      try #require(image.height > 70, "\(image)")
      let format = UIGraphicsImageRendererFormat()
      format.scale = 2
      let rendered = UIGraphicsImageRenderer(bounds: view.bounds, format: format).image {
        context in
        UIColor.white.setFill()
        context.fill(view.bounds)
        view.layer.render(in: context.cgContext)
      }
      let cgImage = try #require(rendered.cgImage)
      let data = try #require(cgImage.dataProvider?.data)
      let bytes = try #require(CFDataGetBytePtr(data))
      /// The rows of `rect` (view coordinates) with text's dark ink.
      func inkedRows(_ rect: CGRect) -> [CGFloat] {
        let rect = rect.intersection(view.bounds)
        return stride(from: rect.minY, to: rect.maxY, by: 0.5).filter { y in
          stride(from: rect.minX, to: rect.maxX, by: 0.5).contains { x in
            let px = Int((x - view.bounds.minX) * 2)
            let py = Int((y - view.bounds.minY) * 2)
            guard px >= 0, py >= 0, px < cgImage.width, py < cgImage.height else { return false }
            let pixel = bytes + py * cgImage.bytesPerRow + px * cgImage.bitsPerPixel / 8
            return pixel[0] < 100 && pixel[1] < 100 && pixel[2] < 100
          }
        }
      }
      // Beside the decorator, before and after it, the text stands on the
      // decorator's bottom edge, the baseline.
      let band = { (minX: CGFloat, maxX: CGFloat) in
        CGRect(x: minX, y: image.minY, width: maxX - minX, height: image.height)
      }
      for (side, rect) in [
        ("before", band(0, image.minX - 1)), ("after", band(image.maxX + 1, view.bounds.maxX - 8)),
      ] {
        let ink = inkedRows(rect)
        try #require(!ink.isEmpty, "\(side): no text beside \(image)")
        #expect(ink.min()! >= image.maxY - 18, "\(side): ink at \(ink.min()!) in \(image)")
        #expect(ink.max()! <= image.maxY + 5, "\(side): ink at \(ink.max()!) in \(image)")
      }
      _ = window
    }

    @Test func rendersTheDemoDocument() throws {
      let (view, window) = try showDemo()
      #expect(view.text.hasPrefix("Lexical for Swift\nNative editing with bold"))
      let heading = try #require(
        view.textStorage.attribute(.font, at: 0, effectiveRange: nil) as? UIFont)
      #expect(heading.pointSize == 24)
      let code = (view.text as NSString).range(of: "inline code")
      let codeFont = try #require(
        view.textStorage.attribute(.font, at: code.location, effectiveRange: nil) as? UIFont)
      #expect(codeFont.fontDescriptor.symbolicTraits.contains(.traitMonoSpace))
      try writePNG(view, "uikit-demo.png")
      _ = window
    }

    @Test func followsTheAppearance() throws {
      let (view, window) = try showDemo()
      let text = view.text as NSString
      func rect(_ string: String) throws -> CGRect {
        let range = text.range(of: string)
        let start = try #require(
          view.position(from: view.beginningOfDocument, offset: range.location))
        let end = try #require(view.position(from: start, offset: range.length))
        return view.firstRect(for: try #require(view.textRange(from: start, to: end)))
      }
      // The quote's bar is left of its text; the code's background behind it.
      let quote = try rect("A quote")
      let bar = CGRect(x: 0, y: quote.midY - 2, width: quote.minX, height: 4)
      let code = try rect("inline")
      #expect(draws(view, (206, 208, 212), in: bar))
      #expect(draws(view, (240, 242, 245), in: code))
      // Switching to dark while shown redraws the bars and the text.
      window.overrideUserInterfaceStyle = .dark
      settle(view)
      #expect(draws(view, (84, 86, 92), in: bar))
      #expect(!draws(view, (206, 208, 212), in: bar))
      #expect(draws(view, (44, 46, 51), in: code))
      try writePNG(view, "uikit-demo-dark.png", background: .black)
      window.overrideUserInterfaceStyle = .light
      settle(view)
      #expect(draws(view, (206, 208, 212), in: bar))
      #expect(draws(view, (240, 242, 245), in: code))
    }

    @Test func keepsQuoteBarsBesideTheirTextWhenPulledPastTheTop() throws {
      let (view, window) = try showDemo()
      // A pull down past the top, as while rubber-banding.
      view.contentOffset = CGPoint(x: 0, y: -80)
      settle(view)
      #expect(view.bounds.minY == -80)
      let range = (view.text as NSString).range(of: "A quote")
      let start = try #require(
        view.position(from: view.beginningOfDocument, offset: range.location))
      let end = try #require(view.position(from: start, offset: range.length))
      let quote = view.firstRect(for: try #require(view.textRange(from: start, to: end)))
      let bar = CGRect(x: 0, y: quote.midY - 2, width: quote.minX, height: 4)
      #expect(draws(view, (206, 208, 212), in: bar))
      _ = window
    }

    @Test func scalesFontsWithDynamicType() throws {
      let (view, window) = try showDemo()
      func size(_ text: String) throws -> CGFloat {
        let location = (view.text as NSString).range(of: text).location
        let font = view.textStorage.attribute(.font, at: location, effectiveRange: nil) as? UIFont
        return try #require(font).pointSize
      }
      #expect(try size("Lexical for Swift") == 24)
      #expect(try size("Native editing") == 16)
      window.traitOverrides.preferredContentSizeCategory = .extraExtraExtraLarge
      settle(view)
      try writePNG(view, "uikit-demo-xxxl.png")
      window.traitOverrides.preferredContentSizeCategory = .accessibilityExtraExtraExtraLarge
      settle(view)
      try writePNG(view, "uikit-demo-ax5.png")
      // The body text style grows about threefold at the largest
      // accessibility size (16 to 45 points), and every theme size scales
      // as body text, so a heading keeps its ratio to the body text.
      let body = try size("Native editing")
      #expect(body > 40)
      #expect(abs(try size("Lexical for Swift") / body - 1.5) < 0.05)
      window.traitOverrides.preferredContentSizeCategory = .large
      settle(view)
      #expect(try size("Lexical for Swift") == 24)
      #expect(try size("Native editing") == 16)
      _ = window
    }

    @Test func rendersInSwiftUI() throws {
      let editor = try makeDemoEditor()
      try loadDemoDocument(editor)
      let host = UIHostingController(rootView: LexicalEditor(editor))
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 520))
      window.rootViewController = host
      window.makeKeyAndVisible()
      settle(host.view)
      let textView = try #require(editor.getRootElement() as? LexicalUITextView)
      #expect(textView.text.hasPrefix("Lexical for Swift"))
      _ = window
    }

    /// `sizedToContent`: 8pt insets around the lines, each 1.7 × 16 =
    /// 27.2pt as in the playground's CSS, up to the maximum, past which it
    /// scrolls; and as short again once the text is.
    @Test func sizesToItsContentInSwiftUI() throws {
      let editor = try makeDemoEditor()
      let host = UIHostingController(
        rootView: VStack(spacing: 0) {
          LexicalEditor(editor).sizedToContent(maxHeight: 150)
          Spacer(minLength: 0)
        })
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 520))
      window.rootViewController = host
      window.makeKeyAndVisible()
      func height(lines: Int) throws -> CGFloat {
        try editor.update(discrete: true) {
          let root = try getRoot().clear()
          if lines == 0 { try root.append(createParagraphNode()) }
          for line in 0..<lines {
            try root.append(createParagraphNode().append(createTextNode("Line \(line)")))
          }
        }
        settle(host.view)
        return try #require(editor.getRootElement() as? LexicalUITextView).frame.height
      }
      #expect(abs(try height(lines: 0) - 43.2) <= 1.5)
      #expect(abs(try height(lines: 3) - 97.6) <= 1.5)
      #expect(try height(lines: 10) == 150)
      #expect(abs(try height(lines: 1) - 43.2) <= 1.5)
      _ = window
    }
  }
#endif
