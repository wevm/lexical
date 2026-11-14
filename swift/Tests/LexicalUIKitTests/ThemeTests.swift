#if canImport(UIKit)
  import Lexical
  import LexicalExtension
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
