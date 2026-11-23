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

    /// A view showing a table of `rows` by `columns` whose cells read "r,c"
    /// (from 0), with `headers` as `createTableNodeWithDimensions` takes them.
    private func tableView(
      _ rows: Int, _ columns: Int, headers: InsertTableCommandPayloadHeaders = false
    ) throws -> (Editor, UIWindow, LexicalUITextView) {
      let editor = try buildEditorFromExtensions(
        defineExtension(name: "test", dependencies: [RichTextExtension, TableExtension]))
      let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 390, height: 520))
      let view = LexicalUITextView(editor: editor, frame: window.bounds)
      window.addSubview(view)
      window.makeKeyAndVisible()
      try editor.update(discrete: true) {
        let table = try createTableNodeWithDimensions(rows, columns, headers)
        for (r, row) in try table.getChildren().enumerated() {
          for (c, cell) in try (row as! ElementNode).getChildren().enumerated() {
            try ((cell as! ElementNode).getFirstChild() as! ElementNode).append(
              createTextNode("\(r),\(c)"))
          }
        }
        try getRoot().clear().append(table, createParagraphNode())
        try getRoot().getLastChild()!.selectStart()
      }
      settle(window)
      return (editor, window, view)
    }

    private func dataTable(in view: UIView) -> (any UIAccessibilityContainerDataTable)? {
      if let table = view as? any UIAccessibilityContainerDataTable,
        view.accessibilityContainerType == .dataTable
      {
        return table
      }
      for subview in view.subviews {
        if let table = dataTable(in: subview) { return table }
      }
      return nil
    }

    /// VoiceOver reads a table cell by cell: the grid is a data table whose
    /// elements are the cells' text views, at their rows and columns, and a
    /// header row's cells head their columns, a header column's their rows.
    @Test func voiceOverReadsTheGridAsADataTable() throws {
      let (_, window, view) = try tableView(
        3, 2, headers: .rowsAndColumns(rows: true, columns: false))
      _ = window
      let table = try #require(dataTable(in: view))
      #expect(table.accessibilityRowCount() == 3)
      #expect(table.accessibilityColumnCount() == 2)
      let cell = try #require(table.accessibilityDataTableCellElement(forRow: 2, column: 1))
      #expect((cell as? UITextView)?.text == "2,1")
      #expect(cell.accessibilityRowRange() == NSRange(location: 2, length: 1))
      #expect(cell.accessibilityColumnRange() == NSRange(location: 1, length: 1))
      let columnHeaders = table.accessibilityHeaderElements?(forColumn: 1) ?? []
      #expect(columnHeaders.compactMap { ($0 as? UITextView)?.text } == ["0,1"])
      #expect((table.accessibilityHeaderElements?(forRow: 2) ?? []).isEmpty)
      let elements = ((table as? UIView)?.accessibilityElements ?? []).compactMap {
        ($0 as? UITextView)?.text
      }
      #expect(elements == ["0,0", "0,1", "1,0", "1,1", "2,0", "2,1"])

      let (_, rowWindow, rowView) = try tableView(
        2, 3, headers: .rowsAndColumns(rows: false, columns: true))
      _ = rowWindow
      let rowTable = try #require(dataTable(in: rowView))
      let rowHeaders = rowTable.accessibilityHeaderElements?(forRow: 1) ?? []
      #expect(rowHeaders.compactMap { ($0 as? UITextView)?.text } == ["1,0"])
      #expect((rowTable.accessibilityHeaderElements?(forColumn: 2) ?? []).isEmpty)
    }

    /// A drag of a handle selects from one cell to another in one commit:
    /// the far cell keeps the anchor, and the table extension makes the
    /// range a table selection of the cells between.
    @Test func selectingFromOneCellToAnotherMakesATableSelection() throws {
      let (editor, window, view) = try tableView(3, 3)
      let cells = cellViews(in: view)
      let anchor = try #require(cells["2,2"])
      let focus = try #require(cells["1,0"])
      try focus.controller.select(from: 3, of: anchor.controller, to: 0)
      settle(window)
      let selected = try editor.read(.latest) {
        let selection = try #require(getSelection() as? TableSelection)
        return try selection.getNodes().compactMap { node -> String? in
          node is TableCellNode ? try node.getTextContent() : nil
        }.sorted()
      }
      #expect(selected == ["1,0", "1,1", "1,2", "2,0", "2,1", "2,2"])
    }

    private func node(_ key: NodeKey) throws -> TextNode {
      try getNodeByKey(key) as! TextNode
    }
  }
#endif
