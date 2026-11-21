import Lexical
import LexicalExtension
import LexicalTable
import Testing

// Port of packages/lexical-table/src/__tests__/unit/TableSelectionObserverBeforeDOM.test.ts.
// The other three cases find the observer from the rendered `<table>`
// with `getTableObserverFromTableElement`, a DOM getter of the views.

private func appendTable() throws {
  try getRoot().append(createTableNodeWithDimensions(2, 2, false), createParagraphNode())
}

private func getTableKey() throws -> NodeKey? {
  try getRoot().getChildren().first { $0 is TableNode }?.getKey()
}

/// Serialized editor state holding one 2x2 table, made by a separate editor.
private func tableStateJSON() throws -> String {
  let source = try buildEditorFromExtensions(
    defineExtension(name: "table-source", dependencies: [TableExtension]))
  defer { source.dispose() }
  try source.update(discrete: true) { try appendTable() }
  return JSONValue.object(try source.getEditorState().toJSON()).stringify()
}

/// An extension listed before TableExtension that replaces the editor state
/// from its register, as an extension that loads stored content does.
private func setStateOnRegister(_ json: String) -> LexicalExtension<EmptyConfig, Void, Void> {
  defineExtension(
    name: "set-state-on-register",
    register: { editor, _, _ in
      try editor.setEditorState(editor.parseEditorState(json))
      return {}
    })
}

@Suite struct TableSelectionObserverBeforeDOMTests {
  @Test(
    .upstream(
      "lexical-table/src/__tests__/unit/TableSelectionObserverBeforeDOM.test.ts::table selection observer for tables that exist before their DOM > builds when an earlier extension sets a state holding a table during register"
    ))
  func buildsWithStateSetOnRegister() throws {
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "state-before-table",
        dependencies: [setStateOnRegister(try tableStateJSON()), TableExtension]))
    defer { editor.dispose() }
    #expect(try editor.read { try getTableKey() } != nil)
  }
}
