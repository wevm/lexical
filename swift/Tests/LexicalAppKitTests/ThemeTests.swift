#if os(macOS)
  import AppKit
  import Lexical
  import LexicalAppKit
  import LexicalExtension
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
  }
#endif
