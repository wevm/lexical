import Foundation
import Lexical
import LexicalExtension
import LexicalHTML
import LexicalUtils
import SwiftUI

// Ports of packages/lexical-playground/src/nodes/EquationNode.tsx and
// plugins/EquationsExtension: an inline or block KaTeX equation, with its
// JSON, its HTML (KaTeX's own markup, as `katex.render` builds it),
// `INSERT_EQUATION_COMMAND` and the import rule. `EquationComponent` draws
// it.

/// `encodeEquation`: base64 of the equation's UTF-8 bytes.
func encodeEquation(_ equation: String) -> String {
  Data(equation.utf8).base64EncodedString()
}

/// `decodeEquation`, the inverse of `encodeEquation`. `atob` throws on what
/// is not base64; that is `nil` here.
func decodeEquation(_ encoded: String) -> String? {
  Data(base64Encoded: encoded).map { String(decoding: $0, as: UTF8.self) }
}

/// `EquationNode`.
final class EquationNode: DecoratorNode {
  var __equation = ""
  var __inline = false

  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      EquationNode.self, type: "equation", extends: DecoratorNode.self,
      json: [
        SchemaField("equation", stringValue(), field: \EquationNode.__equation),
        SchemaField("inline", booleanValue(), field: \EquationNode.__inline),
      ])
  }

  /// The Swift playground imports HTML with `generateNodesFromDOM`, which
  /// `EquationsExtension`'s rule does not reach, so the node converts its
  /// element itself as well.
  override class func importDOM() -> DOMConversionMap? {
    let convert: (DOMNode) throws -> DOMConversion? = { node in
      guard let el = node as? DOMElement, el.getAttribute("data-lexical-equation") != nil else {
        return nil
      }
      return DOMConversion(priority: 1) { node in
        DOMConversionOutput(
          node: [try convertEquationElement(node as! DOMElement)].compactMap { $0 })
      }
    }
    return ["div": convert, "span": convert]
  }

  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    let element = getDocument().createElement(__inline ? "span" : "div")
    // EquationNodes should implement `user-action:none` in their CSS to
    // avoid issues with deletion on Android.
    element.className = "editor-equation"
    try element.setAttribute("role", "math")
    try element.setAttribute("aria-label", "Equation: \(try getEquation())")
    return element
  }

  override func exportDOM(_ editor: Editor) throws -> DOMExportOutput {
    let document = getDocument()
    let element = document.createElement(__inline ? "span" : "div")
    // Encode the equation as base64 to avoid issues with special characters
    try element.setAttribute("data-lexical-equation", encodeEquation(__equation))
    try element.setAttribute("data-lexical-inline", "\(__inline)")
    for node in renderKatex(__equation, displayMode: !__inline) {
      _ = try element.appendChild(node.dom(document))
    }
    try element.setAttribute("role", "math")
    try element.setAttribute("aria-label", "Equation: \(__equation)")
    return DOMExportOutput(element: element)
  }

  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    guard let prevNode = prevNode as? EquationNode else { return false }
    // If the inline property changes, replace the element
    if __inline != prevNode.__inline { return true }
    if __equation != prevNode.__equation {
      try dom.setAttribute("aria-label", "Equation: \(try getEquation())")
    }
    return false
  }

  override func getTextContent() throws -> String { try getEquation() }

  override func isInline() throws -> Bool { try getLatest().__inline }

  func getEquation() throws -> String { try getLatest().__equation }

  @discardableResult
  func setInline(_ inline: Bool) throws -> Self {
    let writable = try getWritable()
    writable.__inline = inline
    return writable
  }

  @discardableResult
  func setEquation(_ equation: String) throws -> Self {
    let writable = try getWritable()
    writable.__equation = equation
    return writable
  }

  override func decorate(_ editor: Editor) throws -> Any? {
    let (key, equation, inline) = (getKey(), __equation, __inline)
    // The views decorate on the main thread.
    nonisolated(unsafe) let editor = editor
    return MainActor.assumeIsolated {
      EquationComponent(editor: editor, nodeKey: key, equation: equation, inline: inline)
    }
  }
}

/// `$createEquationNode(equation, inline)`.
func createEquationNode(_ equation: String = "", _ inline: Bool = false) throws -> EquationNode {
  try applyNodeReplacement(EquationNode().setEquation(equation).setInline(inline))
}

func isEquationNode(_ node: LexicalNode?) -> Bool { node is EquationNode }

/// `INSERT_EQUATION_COMMAND`'s payload.
struct EquationPayload {
  let equation: String
  let inline: Bool
}

/// `INSERT_EQUATION_COMMAND`.
let INSERT_EQUATION_COMMAND = LexicalCommand<EquationPayload>("INSERT_EQUATION_COMMAND")

/// `$convertEquationElement`.
private func convertEquationElement(_ el: DOMElement) throws -> EquationNode? {
  guard let encoded = el.getAttribute("data-lexical-equation"), !encoded.isEmpty,
    let equation = decodeEquation(encoded), !equation.isEmpty
  else { return nil }
  return try createEquationNode(equation, el.getAttribute("data-lexical-inline") == "true")
}

nonisolated(unsafe) private let EquationImportRule = defineImportRule(
  name: "@lexical/playground/equation",
  match: sel.tag("div", "span").attr("data-lexical-equation")
) { _, el, next in
  if let node = try convertEquationElement(el) { return [node] }
  return try next()
}

/// `EquationsExtension`, with `EquationComponent`'s `CLICK_COMMAND` and
/// `KEY_ENTER_COMMAND` handlers: the views report a click on a decorator
/// with its key, and a click selects only the equation (a Shift click
/// toggles it in the selection); Enter while it is the only node selected
/// adds a paragraph after it, with the caret in it.
let EquationsExtension = defineExtension(
  name: "@lexical/playground/Equations",
  dependencies: [configExtension(DOMImportExtension) { $0.rules = [EquationImportRule.entry] }],
  nodes: { [.klass(EquationNode.self)] },
  register: { editor, _, _ in
    mergeRegister([
      editor.registerCommand(
        INSERT_EQUATION_COMMAND,
        { payload, _ in
          let equationNode = try createEquationNode(payload.equation, payload.inline)
          if payload.inline {
            try insertNodeIntoLeaf(equationNode)
            if try isRootOrShadowRoot(equationNode.getParent()) {
              _ = try wrapNodeInElement(equationNode) { try createParagraphNode() }.selectEnd()
            }
          } else {
            _ = try insertNodeToNearestRoot(equationNode)
          }
          return true
        }, COMMAND_PRIORITY_EDITOR),
      editor.registerCommand(
        CLICK_COMMAND,
        { event, _ in
          guard let key = event.targetKey, try getNodeByKey(key) is EquationNode else {
            return false
          }
          let current = try getSelection() as? NodeSelection
          if event.shiftKey {
            let selection = current ?? createNodeSelection()
            if selection.has(key) { selection.delete(key) } else { selection.add(key) }
            try setSelection(selection)
          } else {
            let selection = createNodeSelection()
            selection.add(key)
            try setSelection(selection)
          }
          return true
        }, COMMAND_PRIORITY_LOW),
      editor.registerCommand(
        KEY_ENTER_COMMAND,
        { event, _ in
          guard let selection = try getSelection() as? NodeSelection,
            let node = try selection.getNodes().first as? EquationNode,
            try selection.getNodes().count == 1
          else { return false }
          let paragraph = try createParagraphNode()
          if try node.isInline() {
            guard let parent = try node.getParent() else { return false }
            try parent.insertAfter(paragraph)
          } else {
            try node.insertAfter(paragraph)
          }
          try paragraph.select()
          event?.preventDefault()
          return true
        }, COMMAND_PRIORITY_LOW),
    ])
  })
