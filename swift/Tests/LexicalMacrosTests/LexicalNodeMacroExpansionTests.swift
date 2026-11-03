import LexicalMacros
import SwiftSyntaxMacroExpansion
import SwiftSyntaxMacrosGenericTestSupport
import Testing

private let macros: [String: MacroSpec] = [
  "LexicalNode": MacroSpec(type: LexicalNodeMacro.self),
  "Field": MacroSpec(type: FieldMacro.self),
]

private func fail(_ failure: TestFailureSpec) {
  Issue.record(
    Comment(rawValue: failure.message),
    sourceLocation: SourceLocation(
      fileID: failure.location.fileID, filePath: failure.location.filePath,
      line: failure.location.line, column: failure.location.column))
}

@Suite struct LexicalNodeMacroExpansionTests {
  @Test func expandsConfigAndAccessors() {
    assertMacroExpansion(
      """
      @LexicalNode(type: "custom", extends: ElementNode.self, inline: true)
      public final class CustomNode: ElementNode {
        @Field(stringValue()) var __label: String = ""
        @Field("n", numberValue(), accessors: false) var __count: Double = 0
      }
      """,
      expandedSource: """
        public final class CustomNode: ElementNode {
          var __label: String = ""
          var __count: Double = 0

            public override class func config() throws -> NodeConfig? {
              try NodeConfig(
                CustomNode.self,
                type: "custom",
                extends: ElementNode.self,
                json: [
                  SchemaField("label", stringValue(), field: \\CustomNode.__label),
                  SchemaField("n", numberValue(), field: \\CustomNode.__count),
                ],
                inline: true)
            }

            public func getLabel() throws -> String {
              try getLatest().__label
            }

            @discardableResult
            public func setLabel(_ value: String) throws -> Self {
              let writable = try getWritable()
              writable.__label = value
              return writable
            }
        }
        """,
      macroSpecs: macros, failureHandler: fail)
  }
}
