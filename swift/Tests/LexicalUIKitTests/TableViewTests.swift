#if canImport(UIKit)
  import Lexical
  import LexicalExtension
  import LexicalRichText
  import LexicalTable
  import LexicalUIKit
  import Testing
  import UIKit

  // Tables in the UIKit view (DESIGN.md, The table view): each cell is a
  // text view of its own over the root's editor.

  @MainActor
  @Suite struct TableViewTests {
    /// The cell views under `view`, by the text they show.
    private func cellViews(in view: UIView) -> [String: LexicalUITextView] {
      var found: [String: LexicalUITextView] = [:]
      func visit(_ view: UIView) {
        for subview in view.subviews {
          if let cell = subview as? LexicalUITextView { found[cell.text] = cell }
          visit(subview)
        }
      }
      visit(view)
      return found
    }

    private func settle(_ window: UIWindow) {
      for _ in 0..<5 {
        window.layoutIfNeeded()
        RunLoop.main.run(until: Date(timeIntervalSinceNow: 0.05))
      }
    }

    /// A tap in another cell makes that cell's view first responder; when
    /// the caret UIKit places there is where that view's own was, no
    /// selection change follows, so taking the focus must move the
    /// selection into the cell.
    @Test func cellViewTakingTheFocusTakesTheSelection() throws {
      let editor = try buildEditorFromExtensions(
        defineExtension(name: "test", dependencies: [RichTextExtension, TableExtension]))
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 520))
      let view = LexicalUITextView(editor: editor, frame: window.bounds)
      window.addSubview(view)
      window.makeKeyAndVisible()
      var textKeys: [String: NodeKey] = [:]
      try editor.update(discrete: true) {
        let table = try createTableNodeWithDimensions(1, 2, false)
        let row = try table.getFirstChild() as! ElementNode
        for (cell, text) in zip(try row.getChildren(), ["one", "two"]) {
          let node = try createTextNode(text)
          textKeys[text] = node.getKey()
          try ((cell as! ElementNode).getFirstChild() as! ElementNode).append(node)
        }
        try getRoot().clear().append(table, createParagraphNode())
        try node(textKeys["one"]!).select(3, 3)
      }
      settle(window)
      let cells = cellViews(in: view)
      let one = try #require(cells["one"])
      let two = try #require(cells["two"])
      _ = one.becomeFirstResponder()
      settle(window)
      #expect(one.isFirstResponder)
      _ = two.becomeFirstResponder()
      settle(window)
      let anchor = try editor.read(.latest) {
        (try getSelection() as? RangeSelection)?.anchor.key
      }
      #expect(anchor == textKeys["two"])
      #expect(view.hasFocus)
    }

    private func node(_ key: NodeKey) throws -> TextNode {
      try getNodeByKey(key) as! TextNode
    }
  }
#endif
