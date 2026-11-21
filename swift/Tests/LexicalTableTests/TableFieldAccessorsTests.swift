import Lexical
import LexicalExtension
import LexicalTable
import LexicalTesting
import Testing

// Port of packages/lexical-table/src/__tests__/unit/TableFieldAccessors.test.ts.
// A Swift schema's accessors are closures that call the setter methods, so
// a subclass that overrides one decides, whether the declaration names it
// or JS finds it by convention.

private final class CappedTableNode: TableNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(CappedTableNode.self, type: "capped-table", extends: TableNode.self)
  }
  override func setFrozenColumns(_ columnCount: Int) throws -> Self {
    try super.setFrozenColumns(min(columnCount, 2))
  }
  override func setFrozenRows(_ rowCount: Int) throws -> Self {
    try super.setFrozenRows(min(rowCount, 3))
  }
  // Named by convention rather than by the declaration, which is the case
  // the frozen counts could not use.
  override func setRowStriping(_ newRowStriping: Bool) throws -> Self {
    try super.setRowStriping(!newRowStriping)
  }
}

private final class AlignCellNode: TableCellNode {
  override class func config() throws -> NodeConfig? {
    NodeConfig(AlignCellNode.self, type: "align-cell", extends: TableCellNode.self)
  }
  override func setVerticalAlign(_ newVerticalAlign: String?) throws -> Self {
    try super.setVerticalAlign(newVerticalAlign == "middle" ? "bottom" : newVerticalAlign)
  }
}

/// An editor that knows the two subclasses and their bases, and nothing
/// else.
private func withEditor(_ fn: @escaping () throws -> Void) throws {
  let editor = try buildEditorFromExtensions(
    defineExtension(
      name: "[table-field-accessors]",
      nodes: {
        [
          .klass(TableNode.self), .klass(CappedTableNode.self), .klass(TableCellNode.self),
          .klass(AlignCellNode.self),
        ]
      }))
  defer { editor.dispose() }
  try editor.update(discrete: true, fn)
}

@Suite struct TableFieldAccessorsTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/TableFieldAccessors.test.ts::the accessors the table field declarations stand in for > a subclass override of a named setter still decides"
    ))
  func namedSetter() throws {
    try withEditor {
      let node = try #require(
        CappedTableNode.importJSON(
          JSONObject([
            ("children", []), ("direction", nil), ("format", ""), ("frozenColumnCount", 9),
            ("frozenRowCount", 9), ("indent", 0), ("type", "capped-table"), ("version", 1),
          ])) as? CappedTableNode)
      #expect(try node.getFrozenColumns() == 2)
      #expect(try node.getFrozenRows() == 3)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/TableFieldAccessors.test.ts::the accessors the table field declarations stand in for > and one of a setter named only by convention"
    ))
  func conventionalSetter() throws {
    try withEditor {
      let node = try #require(
        CappedTableNode.importJSON(
          JSONObject([
            ("children", []), ("direction", nil), ("format", ""), ("indent", 0),
            ("rowStriping", true), ("type", "capped-table"), ("version", 1),
          ])) as? CappedTableNode)
      #expect(try node.getRowStriping() == false)
    }
  }

  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/TableFieldAccessors.test.ts::the accessors the table field declarations stand in for > including a cell whose vertical alignment is overridden"
    ))
  func verticalAlignSetter() throws {
    try withEditor {
      let node = try #require(
        AlignCellNode.importJSON(
          JSONObject([
            ("children", []), ("colSpan", 1), ("direction", nil), ("format", ""),
            ("headerState", 0), ("indent", 0), ("rowSpan", 1), ("type", "align-cell"),
            ("version", 1), ("verticalAlign", "middle"),
          ])) as? AlignCellNode)
      #expect(try node.getVerticalAlign() == "bottom")
    }
  }
}
