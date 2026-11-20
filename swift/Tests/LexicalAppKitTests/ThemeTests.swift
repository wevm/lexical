#if os(macOS)
  import AppKit
  import Lexical
  import LexicalAppKit
  import LexicalCodeCore
  import LexicalExtension
  import LexicalList
  import LexicalRichText
  import LexicalTextKit
  import Testing

  // Theming a live view: platform colors and swapping the theme.

  @MainActor
  private func makeView() throws -> (LexicalNSTextView, NSWindow) {
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "theme-test", dependencies: [RichTextExtension]))
    try editor.update(discrete: true) {
      try getRoot().clear().append(createParagraphNode().append(createTextNode("Body text")))
    }
    let view = LexicalNSTextView(editor: editor, frame: NSRect(x: 0, y: 0, width: 400, height: 300))
    let window = NSWindow(
      contentRect: view.frame, styleMask: [.titled], backing: .buffered, defer: false)
    window.contentView = view
    return (view, window)
  }

  @MainActor
  private func font(_ view: LexicalNSTextView) throws -> NSFont {
    try #require(view.textStorage?.attribute(.font, at: 0, effectiveRange: nil) as? NSFont)
  }

  @MainActor
  private func color(_ view: LexicalNSTextView, _ name: NSAppearance.Name) throws -> NSColor {
    let color = try #require(
      view.textStorage?.attribute(.foregroundColor, at: 0, effectiveRange: nil) as? NSColor)
    var resolved: NSColor?
    NSAppearance(named: name)?.performAsCurrentDrawingAppearance {
      resolved = color.usingColorSpace(.sRGB)
    }
    return try #require(resolved)
  }

  /// Blue at half opacity in light, red in dark.
  private let blueOrRed = NSColor(name: nil) { appearance in
    appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
      ? NSColor(srgbRed: 1, green: 0, blue: 0, alpha: 1)
      : NSColor(srgbRed: 0, green: 0, blue: 1, alpha: 0.5)
  }

  /// `view` drawn into an sRGB bitmap at 2×: its color at a point, in
  /// 0...255.
  @MainActor
  private func pixels(_ view: NSView) throws -> (CGFloat, CGFloat) -> [Int] {
    let rep = try #require(
      NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: Int(view.bounds.width) * 2,
        pixelsHigh: Int(view.bounds.height) * 2, bitsPerSample: 8, samplesPerPixel: 4,
        hasAlpha: true, isPlanar: false, colorSpaceName: .calibratedRGB, bytesPerRow: 0,
        bitsPerPixel: 0)?.retagging(with: .sRGB))
    rep.size = view.bounds.size
    view.cacheDisplay(in: view.bounds, to: rep)
    return { x, y in
      // The components as stored: sRGB, which `usingColorSpace` would
      // convert again from the rep's calibrated color space name.
      guard let color = rep.colorAt(x: Int(x * 2), y: Int(y * 2)) else { return [] }
      return [color.redComponent, color.greenComponent, color.blueComponent].map {
        Int(($0 * 255).rounded())
      }
    }
  }

  /// Whether `a` is within 12 of `b` in each channel.
  private func near(_ a: [Int], _ b: [Int]) -> Bool {
    a.count == b.count && zip(a, b).allSatisfy { abs($0 - $1) <= 12 }
  }

  /// The smallest rect around the points of `rect` that are `rgb`: found
  /// in 1-point steps along every fourth row, then its edges to a quarter
  /// point along its middle row and column (a box's widest, rounded
  /// corners or not).
  private func bounds(
    of rgb: [Int], in rect: NSRect, _ color: (CGFloat, CGFloat) -> [Int]
  ) -> NSRect? {
    var found: NSRect?
    for y in stride(from: rect.minY, to: rect.maxY, by: 4) {
      for x in stride(from: rect.minX, to: rect.maxX, by: 1) where near(color(x, y), rgb) {
        let point = NSRect(x: x, y: y, width: 1, height: 4)
        found = found.map { $0.union(point) } ?? point
      }
    }
    guard let coarse = found else { return nil }
    let (midX, midY) = (coarse.midX, coarse.midY)
    let xs = stride(from: coarse.minX - 1.5, to: coarse.maxX + 1.5, by: 0.25)
      .filter { near(color($0, midY), rgb) }
    let ys = stride(from: coarse.minY - 4.5, to: coarse.maxY + 4.5, by: 0.25)
      .filter { near(color(midX, $0), rgb) }
    guard let minX = xs.min(), let maxX = xs.max(), let minY = ys.min(), let maxY = ys.max()
    else { return coarse }
    return NSRect(x: minX, y: minY, width: maxX + 0.25 - minX, height: maxY + 0.25 - minY)
  }

  @MainActor
  @Suite struct ThemeTests {
    @Test func initializesAColorFromAnNSColorInLightAndDark() {
      #expect(
        TextKitColor(nsColor: blueOrRed)
          == TextKitColor(red: 0, green: 0, blue: 1, alpha: 0.5)
          .withDark(TextKitColor(red: 1, green: 0, blue: 0)))
      // A color that is the same in both has no dark variant.
      #expect(
        TextKitColor(nsColor: NSColor(srgbRed: 0.25, green: 0.5, blue: 1, alpha: 1)).dark == nil)
      // System colors are dynamic.
      let text = TextKitColor(nsColor: .textColor)
      #expect(text.red == 0 && text.alpha == 1)
      #expect(text.darkAppearance.red == 1)
    }

    /// A new theme draws the text again, and its dynamic colors still
    /// follow the appearance; an equal theme edits nothing.
    @Test func drawsAgainWhenTheThemeChanges() throws {
      let (view, window) = try makeView()
      #expect(try font(view).pointSize == 16)
      var theme = TextKitTheme.default
      theme["root"]?.fontSize = .points(20)
      theme["root"]?.foregroundColor = TextKitColor(nsColor: blueOrRed)
      view.theme = theme
      #expect(view.controller.theme == theme)
      #expect(try font(view).pointSize == 20)
      #expect(try color(view, .aqua).blueComponent == 1)
      #expect(try color(view, .aqua).alphaComponent == 0.5)
      #expect(try color(view, .darkAqua).redComponent == 1)

      nonisolated(unsafe) var edits = 0
      let observer = NotificationCenter.default.addObserver(
        forName: NSTextStorage.didProcessEditingNotification, object: view.textStorage, queue: nil
      ) { _ in edits += 1 }
      defer { NotificationCenter.default.removeObserver(observer) }
      view.theme = theme
      #expect(edits == 0)
      view.theme = .default
      #expect(edits > 0)
      #expect(try font(view).pointSize == 16)
      _ = window
    }

    /// A theme's border and corner radius draw around a code block's box,
    /// over its gutter and clipping it, and a theme's check boxes draw at
    /// their size in their colors, which follow the appearance.
    @Test func drawsThemedBordersAndCheckBoxes() throws {
      let editor = try buildEditorFromExtensions(
        defineExtension(
          name: "theme-test", dependencies: [RichTextExtension, CodeExtension, CheckListExtension]))
      try editor.update(discrete: true) {
        try getRoot().clear().append(
          createParagraphNode().append(createTextNode("x")),
          createListNode("check").append(
            createListItemNode(true).append(createTextNode("Done")),
            createListItemNode(false).append(createTextNode("Todo"))),
          createCodeNode().append(createTextNode("let a = 1")),
          createParagraphNode().append(createTextNode("y")))
      }
      var theme = TextKitTheme.default
      let red = [200, 0, 0]
      let (green, darkGreen, yellow, magenta) = (
        [0, 160, 0], [0, 80, 0], [255, 255, 0], [255, 0, 255]
      )
      theme["code"]?.border = TextKitBorder(
        width: 2, color: TextKitColor(200, 0, 0), cornerRadius: 10)
      theme["listitemChecked"]?.checkbox = TextKitCheckbox(
        size: .points(20),
        backgroundColor: TextKitColor(0, 160, 0).withDark(TextKitColor(0, 80, 0)),
        border: TextKitBorder(cornerRadius: 4), checkmarkColor: TextKitColor(255, 255, 0))
      theme["listitemUnchecked"]?.checkbox = TextKitCheckbox(
        size: .points(20), border: TextKitBorder(width: 3, color: TextKitColor(255, 0, 255)))
      let view = LexicalNSTextView(
        editor: editor, frame: NSRect(x: 0, y: 0, width: 400, height: 300))
      view.theme = theme
      let window = NSWindow(
        contentRect: view.frame, styleMask: [.titled], backing: .buffered, defer: false)
      window.appearance = NSAppearance(named: .aqua)
      window.contentView = view
      let layoutManager = try #require(view.textLayoutManager)
      layoutManager.ensureLayout(for: layoutManager.documentRange)
      var color = try pixels(view)
      let padding = try #require(view.textContainer).lineFragmentPadding
      let start = view.textContainerOrigin.x + padding

      // The code box: its border on each side, its background and gutter
      // inside, and nothing in the corner its radius rounds off.
      let box = try #require(bounds(of: red, in: view.bounds, color))
      #expect(abs(box.minX - start) <= 0.5, "\(box)")
      #expect(near(color(box.midX, box.minY + 1), red))
      #expect(near(color(box.midX, box.maxY - 1), red))
      #expect(near(color(box.minX + 1, box.midY), red))
      #expect(near(color(box.maxX - 1, box.midY), red))
      #expect(
        near(color(box.midX, box.minY + 4), [240, 242, 245]), "\(color(box.midX, box.minY + 4))")
      #expect(
        near(color(box.minX + 4, box.midY), [238, 238, 238]), "\(color(box.minX + 4, box.midY))")
      #expect(
        near(color(box.minX + 1, box.minY + 1), [255, 255, 255]),
        "\(color(box.minX + 1, box.minY + 1))")

      // The boxes are 20 points, 8 points into the items' 32-point indent.
      let column = NSRect(x: 0, y: 0, width: start + 40, height: view.bounds.height)
      let checked = try #require(bounds(of: green, in: column, color))
      #expect(abs(checked.minX - (start + 8)) <= 1, "\(checked)")
      #expect(abs(checked.width - 20) <= 1, "\(checked)")
      #expect(
        stride(from: checked.minX, to: checked.maxX, by: 0.5).contains { x in
          stride(from: checked.minY, to: checked.maxY, by: 0.5).contains {
            near(color(x, $0), yellow)
          }
        })
      let unchecked = try #require(bounds(of: magenta, in: column, color))
      #expect(abs(unchecked.minX - (start + 8)) <= 1, "\(unchecked)")
      #expect(abs(unchecked.width - 20) <= 1 && abs(unchecked.height - 20) <= 1, "\(unchecked)")
      #expect(near(color(unchecked.midX, unchecked.midY), [255, 255, 255]))
      #expect(unchecked.minY > checked.maxY)

      window.appearance = NSAppearance(named: .darkAqua)
      color = try pixels(view)
      #expect(bounds(of: green, in: checked, color) == nil)
      #expect(bounds(of: darkGreen, in: checked, color) != nil)
      _ = window
    }

    /// A theme's tracking is the text's `.tracking`, which narrows it, and
    /// goes away with a theme that has none.
    @Test func tracksTheText() throws {
      let (view, window) = try makeView()
      func tracking() -> Double? {
        (view.textStorage?.attribute(.tracking, at: 0, effectiveRange: nil) as? NSNumber)?
          .doubleValue
      }
      let wide = try #require(view.textStorage).size().width
      #expect(tracking() == nil)
      var theme = TextKitTheme.default
      theme["root"]?.tracking = -1
      view.theme = theme
      #expect(tracking() == -1)
      let narrow = try #require(view.textStorage).size().width
      // "Body text" is 9 characters.
      #expect(abs(wide - narrow - 9) < 0.5)
      view.theme = .default
      #expect(tracking() == nil)
      _ = window
    }
  }
#endif
