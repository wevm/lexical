import Lexical
import LexicalTable
import LexicalTesting

// Shared fixtures of the table tests.

/// The table nodes the upstream test editor registers.
var tableTestNodes: [LexicalNodeConfig] {
  [.klass(TableNode.self), .klass(TableRowNode.self), .klass(TableCellNode.self)]
}

/// `$runDOMConversion(editor, element)` from the upstream test utils: the
/// highest-priority conversion the editor offers for `element`, run.
func runDOMConversion(_ editor: Editor, _ element: DOMElement) throws -> DOMConversionOutput? {
  var match: DOMConversion? = nil
  for conversion in editor._htmlConversions[element.tagName.lowercased()] ?? [] {
    if let candidate = try conversion(element),
      match == nil || match!.priority <= candidate.priority
    {
      match = candidate
    }
  }
  return try match.flatMap { try $0.conversion(element) }
}
