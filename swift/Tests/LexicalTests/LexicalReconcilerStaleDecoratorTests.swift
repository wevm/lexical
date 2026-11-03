import LexicalTesting
import Testing

@testable import Lexical

// Port of src/__tests__/unit/LexicalReconcilerStaleDecorator.test.ts.

@Suite struct LexicalReconcilerStaleDecoratorTests {
  @Test(
    .upstream(
      "lexical/src/__tests__/unit/LexicalReconcilerStaleDecorator.test.ts::LexicalReconciler — last-child decorator removal > does not throw when an inline DecoratorNode is removed as the last child of an element"
    ))
  func removeLastChildDecorator() throws {
    var errors: [any Error] = []
    let editor = try createEditor(
      CreateEditorArgs(
        namespace: "lexical-reconciler-stale-decorator", nodes: [.klass(TestDecoratorNode.self)],
        onError: { errors.append($0) }))
    var decoratorKey = ""
    try editor.update(discrete: true) {
      let decorator = try TestDecoratorNode()
      try getRoot().append(try createParagraphNode().append(decorator))
      decoratorKey = decorator.getKey()
    }
    try editor.update(discrete: true) {
      try #require(try getNodeByKey(decoratorKey)).remove()
    }
    #expect(errors.isEmpty)
  }
}
