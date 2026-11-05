import Lexical
import LexicalExtension
import LexicalHTML
import LexicalUtils
import SwiftUI

// Ports of packages/lexical-playground/src/nodes/PageBreakNode and
// plugins/PageBreakExtension/index.ts: a page break between blocks, drawn
// as the web's `hr[data-lexical-page-break]`: a dashed band across the
// editor with the scissors at its start and the "PAGE BREAK" label in its
// middle, outlined in the primary color while it is selected.

/// `PageBreakNode`.
final class PageBreakNode: DecoratorNode {
  override class func config() -> NodeConfig? {
    NodeConfig(PageBreakNode.self, type: "page-break", extends: DecoratorNode.self)
  }

  /// The Swift playground imports HTML with `generateNodesFromDOM`, which
  /// `PageBreakExtension`'s rules do not reach, so the node converts its
  /// `<hr>` (and the older `<figure type="page-break">`) itself as well.
  override class func importDOM() -> DOMConversionMap? {
    func convert(
      _ tag: String, _ attribute: String, _ value: String
    ) -> (DOMNode) throws -> DOMConversion? {
      { node in
        guard (node as? DOMElement)?.getAttribute(attribute) == value else { return nil }
        return DOMConversion(priority: 2) { _ in
          DOMConversionOutput(node: [try createPageBreakNode()])
        }
      }
    }
    return [
      "hr": convert("hr", "data-lexical-page-break", "true"),
      "figure": convert("figure", "type", "page-break"),
    ]
  }

  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    let el = getDocument().createElement("hr")
    el.style.setProperty("page-break-after", "always")
    el.setAttribute("data-lexical-page-break", "true")
    return el
  }

  override func getTextContent() throws -> String { "\n" }

  override func isInline() throws -> Bool { false }

  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool { false }

  override func decorate(_ editor: Editor) throws -> Any? {
    let key = getKey()
    nonisolated(unsafe) let editor = editor
    return MainActor.assumeIsolated { PageBreakComponent(editor: editor, nodeKey: key) }
  }
}

/// `$createPageBreakNode()`.
func createPageBreakNode() throws -> PageBreakNode { try PageBreakNode() }

func isPageBreakNode(_ node: LexicalNode?) -> Bool { node is PageBreakNode }

/// `INSERT_PAGE_BREAK`.
let INSERT_PAGE_BREAK = LexicalCommand<Void>("INSERT_PAGE_BREAK")

nonisolated(unsafe) private let PageBreakImportRule = defineImportRule(
  name: "@lexical/playground/page-break",
  match: sel.tag("hr").attr("data-lexical-page-break", "true")
) { _, _, _ in [try createPageBreakNode()] }

/// Older playground exports wrote the page break as `<figure
/// type="page-break">`.
nonisolated(unsafe) private let PageBreakLegacyImportRule = defineImportRule(
  name: "@lexical/playground/page-break-legacy", match: sel.tag("figure").attr("type", "page-break")
) { _, _, _ in [try createPageBreakNode()] }

/// `PageBreakExtension`, with `PageBreakComponent`'s `CLICK_COMMAND`
/// handler: the views report a click on a decorator with its key. A click
/// selects only the page break, or deselects it when it was selected; a
/// Shift click adds it to the node selection or takes it out.
let PageBreakExtension = defineExtension(
  name: "@lexical/playground/PageBreak",
  dependencies: [
    configExtension(DOMImportExtension) {
      $0.rules = [PageBreakImportRule, PageBreakLegacyImportRule].map(\.entry)
    }
  ],
  nodes: { [.klass(PageBreakNode.self)] },
  register: { editor, _, _ in
    mergeRegister([
      editor.registerCommand(
        INSERT_PAGE_BREAK,
        { _, _ in
          guard try getSelection() is RangeSelection else { return false }
          _ = try insertNodeToNearestRoot(createPageBreakNode())
          return true
        }, COMMAND_PRIORITY_EDITOR),
      editor.registerCommand(
        CLICK_COMMAND,
        { event, _ in
          guard let key = event.targetKey, try getNodeByKey(key) is PageBreakNode else {
            return false
          }
          let current = try getSelection() as? NodeSelection
          let isSelected = current?.has(key) ?? false
          let selection =
            event.shiftKey ? (current ?? createNodeSelection()) : createNodeSelection()
          if isSelected && event.shiftKey {
            selection.delete(key)
          } else if !isSelected {
            selection.add(key)
          }
          try setSelection(selection)
          return true
        }, COMMAND_PRIORITY_LOW),
    ])
  })

/// `PageBreakComponent`, with the look of `hr[data-lexical-page-break]`:
/// 28 points above and below, a #eee band between dashed borders, the
/// scissors 12 points in, and the label in a white box in the middle.
struct PageBreakComponent: View {
  let editor: Editor
  let nodeKey: NodeKey
  @State private var selection = NodeSelectionState()

  var body: some View {
    let border =
      selection.isSelected
      ? Color(red: 71 / 255, green: 102 / 255, blue: 203 / 255) : Color(white: 0.933)
    ZStack {
      VStack(spacing: 0) {
        DashedLine().stroke(border, style: StrokeStyle(lineWidth: 1, dash: [3, 3])).frame(height: 1)
        DashedLine().stroke(border, style: StrokeStyle(lineWidth: 1, dash: [3, 3])).frame(height: 1)
      }
      .background(Color(white: 0.933))
      HStack {
        Image(systemName: "scissors")
          .font(.system(size: 13))
          .foregroundStyle(.black)
          .opacity(selection.isSelected ? 1 : 0.5)
          .padding(.leading, 12)
        Spacer()
      }
      Text("PAGE BREAK")
        .font(.system(size: 12, weight: .semibold))
        .foregroundStyle(.black)
        .padding(.horizontal, 6).padding(.vertical, 2)
        .background(Color.white)
        .border(Color(white: 0.8))
    }
    .frame(maxWidth: .infinity)
    .padding(.vertical, 28)
    .contentShape(Rectangle())
    .onAppear { selection.observe(editor, nodeKey) }
    .accessibilityElement(children: .ignore)
    .accessibilityLabel("Page break")
  }
}

/// A horizontal line through the middle of its frame.
private struct DashedLine: Shape {
  func path(in rect: CGRect) -> Path {
    Path { path in
      path.move(to: CGPoint(x: rect.minX, y: rect.midY))
      path.addLine(to: CGPoint(x: rect.maxX, y: rect.midY))
    }
  }
}
