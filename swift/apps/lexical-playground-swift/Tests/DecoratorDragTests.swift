import Lexical
import LexicalExtension
import LexicalRichText
import LexicalTextKit
import Testing

@testable import LexicalPlayground

#if os(macOS)
  import AppKit
  import LexicalAppKit
  private typealias TextView = LexicalNSTextView
#else
  import UIKit
  import LexicalUIKit
  private typealias TextView = LexicalUITextView
#endif

/// Dragging a selected image or page break in a text view moves it to the
/// drop point (`ImagesExtension`'s `$onDragStart` and `$onDrop`, and
/// `DecoratorDragExtension`).
@MainActor
struct DecoratorDragTests {
  private func makeView() throws -> (TextView, AnyObject) {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "@lexical/playground-swift/DecoratorDragTests",
        dependencies: [
          RichTextExtension, ImagesExtension, PageBreakExtension, DecoratorDragExtension,
        ]))
    #if os(macOS)
      let view = TextView(editor: editor, frame: NSRect(x: 0, y: 0, width: 400, height: 400))
      let window = NSWindow(
        contentRect: NSRect(x: 0, y: 0, width: 400, height: 400), styleMask: [.titled],
        backing: .buffered, defer: false)
      window.contentView = view
      window.makeFirstResponder(view)
    #else
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 600))
      let view = TextView(editor: editor, frame: window.bounds)
      window.addSubview(view)
      window.makeKeyAndVisible()
      _ = view.becomeFirstResponder()
    #endif
    return (view, window)
  }

  /// The point of the view just inside the start of the character at
  /// `location`, where a drop puts the caret before it.
  private func point(_ view: TextView, before location: Int) -> CGPoint {
    #if os(macOS)
      view.layoutSubtreeIfNeeded()
      view.textLayoutManager?.ensureLayout(for: view.textLayoutManager!.documentRange)
      let rect = view.firstRect(
        forCharacterRange: NSRange(location: location, length: 1), actualRange: nil)
      let local = view.convert(view.window!.convertFromScreen(rect), from: nil)
      return CGPoint(x: local.minX + 1, y: local.midY)
    #else
      view.layoutIfNeeded()
      let caret = view.caretRect(
        for: view.position(from: view.beginningOfDocument, offset: location)!)
      return CGPoint(x: caret.minX + 1, y: caret.midY)
    #endif
  }

  private func select(_ editor: Editor, _ key: NodeKey) throws {
    try editor.update(discrete: true) {
      let selection = createNodeSelection()
      selection.add(key)
      try setSelection(selection)
    }
  }

  /// Drags what is selected and drops it at `point`.
  private func drag(_ view: TextView, to point: CGPoint) throws {
    let data = try #require(try view.controller.dragStart(x: 0, y: 0))
    #expect(!data.getData("application/x-lexical-drag").isEmpty)
    #expect(try view.controller.drop(data, x: point.x, y: point.y))
  }

  @Test func movesAnImageWithItsCaption() throws {
    let (view, window) = try makeView()
    let editor = view.editor
    var caption: Editor?
    try editor.update(discrete: true) {
      let image = try createImageNode(ImagePayload(altText: "a", src: "x.png", width: 40))
      caption = image.caption
      try getRoot().clear().append(
        createParagraphNode().append(createTextNode("first")),
        createParagraphNode().append(image),
        createParagraphNode().append(createTextNode("third")))
    }
    try caption?.update(discrete: true) {
      try getRoot().clear().append(createParagraphNode().append(createTextNode("cap")))
    }
    let key = try editor.read {
      try (getRoot().getChildAtIndex(1) as! ElementNode).getFirstDescendant()!.getKey()
    }
    try select(editor, key)
    // Between "fi" and "rst".
    try drag(view, to: point(view, before: 2))
    let (first, second, image) = try editor.read {
      let first = try #require(getRoot().getFirstChild() as? ElementNode)
      let image = try #require(first.getChildren().first { $0 is ImageNode } as? ImageNode)
      return (
        try first.getChildren().map { try $0.getTextContent() },
        try getRoot().getChildAtIndex(1)!.getTextContent(), image
      )
    }
    #expect(first == ["fi", "", "rst"])
    #expect(second == "")
    #expect(image.__src == "x.png")
    #expect(image.__width == .points(40))
    // The caption moves with it: the same nested editor.
    #expect(image.caption === caption)
    #expect(try caption?.read { try getRoot().getTextContent() } == "cap")
    _ = window
  }

  @Test func movesAPageBreak() throws {
    let (view, window) = try makeView()
    let editor = view.editor
    try editor.update(discrete: true) {
      try getRoot().clear().append(
        createParagraphNode().append(createTextNode("one")), createPageBreakNode(),
        createParagraphNode().append(createTextNode("two")),
        createParagraphNode().append(createTextNode("three")))
    }
    let key = try editor.read { try getRoot().getChildAtIndex(1)!.getKey() }
    try select(editor, key)
    // Before "three": one \n [break] \n two \n three.
    let location = view.controller.document.string.utf16.count - 5
    try drag(view, to: point(view, before: location))
    let children = try editor.read {
      try getRoot().getChildren().map { $0 is PageBreakNode ? "|" : try $0.getTextContent() }
    }
    #expect(children == ["one", "two", "|", "three"])
    // It stays selected, the same node.
    #expect(
      try editor.read { (try getSelection() as? NodeSelection)?.has(key) } == true)
    _ = window
  }

  /// A decorator that isn't selected, or a caret, starts no move.
  @Test func dragsOnlyASelectedDecorator() throws {
    let (view, window) = try makeView()
    try view.editor.update(discrete: true) {
      try getRoot().clear().append(
        createParagraphNode().append(createTextNode("one")), createPageBreakNode())
      _ = try (getRoot().getFirstDescendant() as! TextNode).select(1, 1)
    }
    #expect(try view.controller.dragStart(x: 0, y: 0) == nil)
    _ = window
  }
}
