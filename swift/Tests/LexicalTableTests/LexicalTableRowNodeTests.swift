import Lexical
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-table/src/__tests__/unit/LexicalTableRowNode.test.ts.

private var editorConfig: EditorConfig {
  EditorConfig(namespace: "", theme: ["tableRow": "test-table-row-class"])
}

@Suite struct LexicalTableRowNodeTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableRowNode.test.ts::LexicalTableRowNode tests > TableRowNode.constructor"
    ))
  func constructor() throws {
    let editor = try createMountedTestEditor(nodes: tableTestNodes)
    try update(editor) {
      #expect(throws: Never.self) { try createTableRowNode() }
    }
    #expect(throws: (any Error).self) { try createTableRowNode() }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/LexicalTableRowNode.test.ts::LexicalTableRowNode tests > TableRowNode.createDOM()"
    ))
  func createDOM() throws {
    let editor = try createMountedTestEditor(nodes: tableTestNodes)
    try update(editor) {
      let rowNode = try createTableRowNode()
      #expect(
        try rowNode.createDOM(editorConfig).outerHTML == #"<tr class="test-table-row-class"></tr>"#)

      let rowWithCustomHeightNode = try createTableRowNode(36)
      #expect(
        try rowWithCustomHeightNode.createDOM(editorConfig).outerHTML
          == #"<tr style="height: 36px;" class="test-table-row-class"></tr>"#)
    }
  }
}
