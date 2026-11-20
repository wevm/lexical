#if canImport(UIKit)
  import Lexical
  import LexicalCodeCore
  import LexicalExtension
  import LexicalList
  import LexicalRichText
  import LexicalSwiftUI
  import LexicalTextKit
  import LexicalUIKit
  import SwiftUI
  import Testing
  import UIKit

  // Theming a live view: platform colors, swapping the theme, and pinning
  // Dynamic Type with the platform's own traits.

  @MainActor
  private func settle(_ view: UIView) {
    for _ in 0..<5 {
      view.window?.layoutIfNeeded()
      RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
    }
  }

  @MainActor
  private func makeEditor() throws -> Editor {
    let editor = try buildEditorFromExtensions(
      defineExtension(name: "theme-test", dependencies: [RichTextExtension]))
    try editor.update(discrete: true) {
      try getRoot().clear().append(createParagraphNode().append(createTextNode("Body text")))
    }
    return editor
  }

  @MainActor
  private func font(_ textView: LexicalUITextView) throws -> UIFont {
    try #require(textView.textStorage.attribute(.font, at: 0, effectiveRange: nil) as? UIFont)
  }

  @MainActor
  private func color(_ textView: LexicalUITextView, _ style: UIUserInterfaceStyle) throws -> UIColor
  {
    let color = textView.textStorage.attribute(.foregroundColor, at: 0, effectiveRange: nil)
    return try #require(color as? UIColor).resolvedColor(
      with: UITraitCollection(userInterfaceStyle: style))
  }

  /// Blue at half opacity in light, red in dark.
  private let blueOrRed = UIColor { traits in
    traits.userInterfaceStyle == .dark
      ? UIColor(red: 1, green: 0, blue: 0, alpha: 1)
      : UIColor(red: 0, green: 0, blue: 1, alpha: 0.5)
  }

  /// `view` rendered at 2× over white: its color at a point, in 0...255,
  /// in either byte order.
  @MainActor
  private func pixels(_ view: UIView) throws -> (CGFloat, CGFloat) -> [Int] {
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
    let (bytesPerRow, bytesPerPixel) = (cgImage.bytesPerRow, cgImage.bitsPerPixel / 8)
    return { x, y in
      _ = data
      let pixel = bytes + Int(y * 2) * bytesPerRow + Int(x * 2) * bytesPerPixel
      return [Int(pixel[0]), Int(pixel[1]), Int(pixel[2])]
    }
  }

  /// Whether `a` is within 12 of `b` in each channel, in either byte order.
  private func near(_ a: [Int], _ b: [Int]) -> Bool {
    let close = { (c: [Int]) in c.count == b.count && zip(c, b).allSatisfy { abs($0 - $1) <= 12 } }
    return close(a) || close(a.reversed())
  }

  /// The smallest rect around the points of `rect` that are `rgb`: found
  /// in 1-point steps along every fourth row, then its edges to a quarter
  /// point along its middle row and column (a box's widest, rounded
  /// corners or not).
  private func bounds(
    of rgb: [Int], in rect: CGRect, _ color: (CGFloat, CGFloat) -> [Int]
  ) -> CGRect? {
    var found: CGRect?
    for y in stride(from: rect.minY, to: rect.maxY, by: 4) {
      for x in stride(from: rect.minX, to: rect.maxX, by: 1) where near(color(x, y), rgb) {
        let point = CGRect(x: x, y: y, width: 1, height: 4)
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
    return CGRect(x: minX, y: minY, width: maxX + 0.25 - minX, height: maxY + 0.25 - minY)
  }

  @MainActor
  @Suite struct ThemeTests {
    @Test func initializesAColorFromAUIColorInLightAndDark() {
      #expect(
        TextKitColor(uiColor: blueOrRed)
          == TextKitColor(red: 0, green: 0, blue: 1, alpha: 0.5)
          .withDark(TextKitColor(red: 1, green: 0, blue: 0)))
      // A color that is the same in both has no dark variant.
      #expect(TextKitColor(uiColor: UIColor(red: 0.25, green: 0.5, blue: 1, alpha: 1)).dark == nil)
      // System colors are dynamic.
      let label = TextKitColor(uiColor: .label)
      #expect(label.red == 0 && label.alpha == 1)
      // White in gray converted to RGB is a float's step past 1.
      #expect(abs(label.darkAppearance.red - 1) < 1e-6)
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
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 400))
      window.overrideUserInterfaceStyle = .light
      let view = LexicalUITextView(editor: editor, frame: window.bounds)
      view.theme = theme
      window.addSubview(view)
      window.makeKeyAndVisible()
      settle(view)
      var color = try pixels(view)
      let start = view.textContainerInset.left + view.textContainer.lineFragmentPadding

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
      let column = CGRect(x: 0, y: 0, width: start + 40, height: view.bounds.height)
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

      window.overrideUserInterfaceStyle = .dark
      settle(view)
      color = try pixels(view)
      #expect(bounds(of: green, in: checked, color) == nil)
      #expect(bounds(of: darkGreen, in: checked, color) != nil)
      _ = window
    }

    /// A new theme draws the text again, and its dynamic colors still
    /// follow the appearance; an equal theme edits nothing.
    @Test func drawsAgainWhenTheThemeChanges() throws {
      let editor = try makeEditor()
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 520))
      let view = LexicalUITextView(editor: editor, frame: window.bounds)
      window.addSubview(view)
      window.makeKeyAndVisible()
      settle(view)
      #expect(try font(view).pointSize == 16)
      var theme = TextKitTheme.default
      theme["root"]?.fontSize = .points(20)
      theme["root"]?.foregroundColor = TextKitColor(uiColor: blueOrRed)
      view.theme = theme
      #expect(view.controller.theme == theme)
      #expect(try font(view).pointSize == 20)
      #expect(try color(view, .light) == UIColor(red: 0, green: 0, blue: 1, alpha: 0.5))
      #expect(try color(view, .dark) == UIColor(red: 1, green: 0, blue: 0, alpha: 1))

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

    @Test func swiftUIThemeChangesTheLiveView() throws {
      let editor = try makeEditor()
      var large = TextKitTheme.default
      large["root"]?.fontSize = .points(20)
      let host = UIHostingController(rootView: LexicalEditor(editor).theme(large))
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 520))
      window.rootViewController = host
      window.makeKeyAndVisible()
      settle(host.view)
      let textView = try #require(editor.getRootElement() as? LexicalUITextView)
      #expect(try font(textView).pointSize == 20)
      host.rootView = LexicalEditor(editor)
      settle(host.view)
      #expect(textView.theme == .default)
      #expect(try font(textView).pointSize == 16)
      _ = window
    }

    /// The documented Dynamic Type opt-out: SwiftUI's `dynamicTypeSize`
    /// reaches the text view's traits, so the theme keeps its points while
    /// the rest of the window is at the largest accessibility size.
    @Test func swiftUIDynamicTypeSizeKeepsTheThemesSizes() throws {
      func bodySize(_ content: some View, _ editor: Editor) throws -> CGFloat {
        let host = UIHostingController(rootView: content)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 520))
        window.traitOverrides.preferredContentSizeCategory = .accessibilityExtraExtraExtraLarge
        window.rootViewController = host
        window.makeKeyAndVisible()
        settle(host.view)
        defer { window.isHidden = true }
        return try font(try #require(editor.getRootElement() as? LexicalUITextView)).pointSize
      }
      let scaled = try makeEditor()
      #expect(try bodySize(LexicalEditor(scaled), scaled) > 40)
      let pinned = try makeEditor()
      #expect(try bodySize(LexicalEditor(pinned).dynamicTypeSize(.large), pinned) == 16)
    }

    /// The UIKit form: the view's own trait override, set while the view
    /// is in a window at the largest accessibility size.
    @Test func traitOverridesKeepTheThemesSizes() throws {
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 520))
      window.traitOverrides.preferredContentSizeCategory = .accessibilityExtraExtraExtraLarge
      let view = LexicalUITextView(editor: try makeEditor(), frame: window.bounds)
      window.addSubview(view)
      window.makeKeyAndVisible()
      settle(view)
      #expect(try font(view).pointSize > 40)
      view.traitOverrides.preferredContentSizeCategory = .large
      settle(view)
      #expect(try font(view).pointSize == 16)
      _ = window
    }
  }
#endif
