import Lexical
import LexicalRichText
import LexicalSelection
import LexicalTable
import LexicalTesting
import Testing

// Port of the `$setBlocksType` cases of
// packages/lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx
// that build a table, whose nodes upstream's `createTestEditor` registers.

private func mountedTableEditor() throws -> Editor {
  try createMountedTestEditor(nodes: [
    .klass(HeadingNode.self), .klass(TableNode.self), .klass(TableRowNode.self),
    .klass(TableCellNode.self),
  ])
}

@Suite struct LexicalSelectionTableTests {
  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Collapsed in element inside top-element"
    ))
  func setBlocksTypeCollapsedInElementInsideTopElement() throws {
    let testEditor = try mountedTableEditor()
    defer { testEditor.dispose() }
    try update(testEditor) { () throws in
      let root = try getRoot()
      let table = try createTableNodeWithDimensions(1, 1)
      let row = try #require(try table.getFirstChild() as? ElementNode)
      let column = try #require(try row.getFirstChild() as? ElementNode)
      let paragraph = try #require(try column.getFirstChild() as? ElementNode)
      if let first = try paragraph.getFirstChild() {
        try first.remove()
      }
      try root.append(table)

      let selection = createRangeSelection()
      try setSelection(selection)
      try setAnchorPoint(paragraph.__key, 0, .element)
      try setFocusPoint(paragraph.__key, 0, .element)

      #expect(try column.getChildren()[0].__type == "paragraph")
      try setBlocksType(selection) { try createHeadingNode("h1") }

      let columnChildrenAfter = try column.getChildren()
      #expect(columnChildrenAfter[0].__type == "heading")
      #expect(columnChildrenAfter.count == 1)
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Collapsed in text inside top-element"
    ))
  func setBlocksTypeCollapsedInTextInsideTopElement() throws {
    let testEditor = try mountedTableEditor()
    defer { testEditor.dispose() }
    try update(testEditor) { () throws in
      let root = try getRoot()
      let table = try createTableNodeWithDimensions(1, 1)
      let row = try #require(try table.getFirstChild() as? ElementNode)
      let column = try #require(try row.getFirstChild() as? ElementNode)
      let paragraph = try #require(try column.getFirstChild() as? ElementNode)
      let text = try createTextNode("foo")
      try root.append(table)
      try paragraph.append(text)

      try setSelection(createRangeSelection())
      try setAnchorPoint(text.__key, text.__text.utf16.count, .text)
      try setFocusPoint(text.__key, text.__text.utf16.count, .text)
      let selection = try #require(try getSelection() as? RangeSelection)

      #expect(try column.getChildren()[0].__type == "paragraph")
      try setBlocksType(selection) { try createHeadingNode("h1") }

      let columnChildrenAfter = try column.getChildren()
      #expect(columnChildrenAfter[0].__type == "heading")
      #expect(columnChildrenAfter.count == 1)
    }
  }

  @Test(
    .upstream(
      "lexical-selection/src/__tests__/unit/LexicalSelection.test.tsx::LexicalSelection tests > $setBlocksType > Full editor selection with a mix of top-elements"
    ))
  func setBlocksTypeFullSelectionMixOfTopElements() throws {
    let testEditor = try mountedTableEditor()
    defer { testEditor.dispose() }
    try update(testEditor) { () throws in
      let root = try getRoot()

      let paragraph1 = try createParagraphNode()
      let paragraph2 = try createParagraphNode()
      let text1 = try createTextNode()
      let text2 = try createTextNode()
      try paragraph1.append(text1)
      try paragraph2.append(text2)
      try root.append(paragraph1, paragraph2)

      let table = try createTableNodeWithDimensions(1, 2)
      let row = try #require(try table.getFirstChild() as? ElementNode)
      let columns = try row.getChildren()
      try root.append(table)

      let column1 = try #require(columns[0] as? ElementNode)
      let paragraph3 = try createParagraphNode()
      let paragraph4 = try createParagraphNode()
      let text3 = try createTextNode()
      let text4 = try createTextNode()
      try paragraph1.append(text3)
      try paragraph2.append(text4)
      try column1.append(paragraph3, paragraph4)

      let column2 = try #require(columns[1] as? ElementNode)
      let paragraph5 = try createParagraphNode()
      let paragraph6 = try createParagraphNode()
      try column2.append(paragraph5, paragraph6)

      let paragraph7 = try createParagraphNode()
      try root.append(paragraph7)

      try setSelection(createRangeSelection())
      try setAnchorPoint(paragraph1.__key, 0, .element)
      try setFocusPoint(paragraph7.__key, 0, .element)
      let selection = try #require(try getSelection() as? RangeSelection)

      try setBlocksType(selection) { try createHeadingNode("h1") }
      let json = JSONValue.object(try #require(testEditor._pendingEditorState).toJSON())
      #expect(
        json.stringify()
          == #"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"","type":"text","version":1},{"detail":0,"format":0,"mode":"normal","style":"","text":"","type":"text","version":1}],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"","type":"text","version":1},{"detail":0,"format":0,"mode":"normal","style":"","text":"","type":"text","version":1}],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1},{"children":[{"children":[{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"","type":"text","version":1}],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1},{"children":[],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1},{"children":[],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1}],"backgroundColor":null,"colSpan":1,"headerState":3,"rowSpan":1,"direction":null,"format":"","indent":0,"type":"tablecell","version":1},{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"","type":"text","version":1}],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1},{"children":[],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1},{"children":[],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1}],"backgroundColor":null,"colSpan":1,"headerState":1,"rowSpan":1,"direction":null,"format":"","indent":0,"type":"tablecell","version":1}],"direction":null,"format":"","indent":0,"type":"tablerow","version":1}],"direction":null,"format":"","indent":0,"type":"table","version":1},{"children":[],"tag":"h1","direction":null,"format":"","indent":0,"type":"heading","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#
      )
    }
  }
}
