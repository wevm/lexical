#if canImport(UIKit)
  import Lexical
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
  private func writePNG(_ view: UIView, _ name: String) throws {
    guard let directory = ProcessInfo.processInfo.environment["LEXICAL_SCREENSHOT_DIR"] else {
      return
    }
    let format = UIGraphicsImageRendererFormat()
    format.scale = 2
    // The test bundle has no host app, so there is no screen to snapshot;
    // the layers render themselves.
    let image = UIGraphicsImageRenderer(bounds: view.bounds, format: format).image { context in
      UIColor.white.setFill()
      context.fill(view.bounds)
      view.layer.render(in: context.cgContext)
    }
    let data = try #require(image.pngData())
    try data.write(to: URL(fileURLWithPath: directory).appendingPathComponent(name))
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
  }
#endif
