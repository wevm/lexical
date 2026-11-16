import Lexical
import LexicalExtension
import LexicalHTML
import Testing

/// The render states of the scenarios (`RENDER_FLAG` and `RENDER_LABEL` in
/// html.test.ts).
nonisolated(unsafe) private let renderFlag = createRenderState("flag") { false }
nonisolated(unsafe) private let renderLabel = createRenderState("label") { "none" }

/// Replays `vectors/html/export-render.json`: an editor built with
/// `DOMRenderExtension` and the case's scenario (`RENDER_SCENARIOS` in
/// html.test.ts, repeated here) exports the HTML jsdom gave.
@Suite(.enabled(if: Vectors.available))
struct HTMLExportRenderTests {
  struct Case: Decodable, CustomTestStringConvertible, Sendable {
    let name: String
    let scenario: String
    let flagOff: Bool?
    let op: String
    let editorState: String
    let html: String
    var testDescription: String { name }
  }

  static let cases: [Case] = (try? Vectors.load("html/export-render")) ?? []

  @Test func vectorsLoad() throws {
    #expect(try Vectors.load("html/export-render", as: Case.self).count == Self.cases.count)
    #expect(!Self.cases.isEmpty)
  }

  /// `$tagExport`: adds `value` to the element's `attr`, space-separated.
  static func tag(_ output: DOMExportOutput, _ attr: String, _ value: String) -> DOMExportOutput {
    if let element = output.element as? DOMElement {
      let prev = element.getAttribute(attr) ?? ""
      element.setAttribute(attr, prev.isEmpty ? value : "\(prev) \(value)")
    }
    return output
  }

  static func tagging(_ attr: String, _ value: String) -> DOMRenderMatchConfig {
    DOMRenderMatchConfig(exportDOM: { _, next, _ in tag(try next(), attr, value) })
  }

  static func scenario(_ name: String) throws -> DOMRenderConfig {
    switch name {
    case "context":
      return DOMRenderConfig(
        overrides: [
          domOverride(
            [ParagraphNode.self],
            DOMRenderMatchConfig(exportDOM: { _, next, editor in
              let value = [
                try getRenderContextValue(renderLabel, editor),
                String(try getRenderContextValue(RenderContextExport, editor)),
                String(try getRenderContextValue(RenderContextRoot, editor)),
              ].joined(separator: " ")
              return tag(try next(), "data-label", value)
            }))
        ], contextDefaults: [contextValue(renderLabel, "default")])
    case "create-dom":
      return DOMRenderConfig(overrides: [
        domOverride(
          [ParagraphNode.self],
          DOMRenderMatchConfig(createDOM: { node, next, _ in
            let element = try next()
            element.setAttribute(
              "data-size", String(try (node as? ElementNode)?.getChildrenSize() ?? 0))
            return element
          }))
      ])
    case "editor-gate":
      return DOMRenderConfig(
        overrides: [
          domOverride(
            all: tagging("data-gate", "on"),
            DOMOverrideOptions(disabledForEditor: { $0.get(renderFlag) }))
        ], contextDefaults: [contextValue(renderFlag, true)])
    case "exclude":
      return DOMRenderConfig(overrides: [
        domOverride(
          [ParagraphNode.self],
          DOMRenderMatchConfig(shouldExclude: { node, _, next, _ in
            try ((node as? ElementNode)?.getChildrenSize() ?? 0) > 2 || next()
          }))
      ])
    case "include":
      return DOMRenderConfig(overrides: [
        domOverride(
          all: DOMRenderMatchConfig(
            shouldInclude: { node, _, next, _ in try !isParagraphNode(node) && next() },
            extractWithChild: { node, child, _, _, next, _ in
              let bold = try (child as? TextNode)?.hasFormat("bold") ?? false
              return try (isParagraphNode(node) && bold) || next()
            }))
      ])
    case "none":
      return DOMRenderConfig()
    case "order":
      return DOMRenderConfig(overrides: [
        domOverride(all: tagging("data-order", "wildcard")),
        domOverride(
          [.predicate { node in ((try? node.getTextContent()) ?? "").utf16.count > 2 }],
          tagging("data-order", "predicate")),
        domOverride([TabNode.self], tagging("data-order", "tab")),
        domOverride([TextNode.self], tagging("data-order", "text")),
      ])
    case "session-gate":
      return DOMRenderConfig(overrides: [
        domOverride(
          all: tagging("data-session", "on"),
          DOMOverrideOptions(disabledForSession: { $0.get(RenderContextRoot) }))
      ])
    default:
      throw LexicalError("unknown scenario \(name)")
    }
  }

  @Test(arguments: cases)
  func export(_ vector: Case) throws {
    let config = try Self.scenario(vector.scenario)
    let editor = try buildEditorFromExtensions(
      defineExtension(
        name: "[conformance]",
        dependencies: [
          configExtension(DOMRenderExtension) {
            $0.overrides = config.overrides
            $0.contextDefaults = config.contextDefaults
          }
        ]))
    defer { editor.dispose() }
    try editor.setEditorState(editor.parseEditorState(vector.editorState))
    if vector.flagOff == true {
      try setRenderContextValue(renderFlag, false, editor)
    }
    let html = try editor.read { () throws -> String in
      switch vector.op {
      case "root":
        return try generateDOMFromRoot(getDocument().createElement("div")).innerHTML
      case "scoped":
        return try withRenderContext([contextUpdater(renderLabel) { "\($0)+scoped" }], editor) {
          try generateHtmlFromNodes(editor)
        }
      default:
        return try generateHtmlFromNodes(editor)
      }
    }
    #expect(html == vector.html)
  }
}
