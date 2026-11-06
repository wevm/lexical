import Foundation
import Lexical
import LexicalExtension
import LexicalUtils

// Dragging a selected block decorator other than an image (a page break)
// moves it, as `ImagesExtension` moves an image: the views drag a selected
// decorator's view through `DRAGSTART_COMMAND`, and the drop puts the same
// node at the drop point. The web playground drags only images; the other
// block decorators move the same way here.

/// `DecoratorDragExtension`.
let DecoratorDragExtension = defineExtension(
  name: "@lexical/playground-swift/DecoratorDrag",
  register: { editor, _, _ in
    mergeRegister([
      editor.registerCommand(
        DRAGSTART_COMMAND,
        { event, _ in
          guard let node = try draggedDecorator(), let dataTransfer = event.dataTransfer else {
            return false
          }
          let drag: [String: Any] = ["data": ["key": node.getKey()], "type": "decorator"]
          dataTransfer.setData("text/plain", "_")
          dataTransfer.setData(
            "application/x-lexical-drag",
            String(decoding: try JSONSerialization.data(withJSONObject: drag), as: UTF8.self))
          return true
        }, COMMAND_PRIORITY_LOW),
      editor.registerCommand(
        DROP_COMMAND,
        { event, editor in
          guard let node = try draggedDecorator(), let key = draggedKey(event),
            key == node.getKey()
          else { return false }
          event.preventDefault()
          // Not into itself, or into code.
          guard let caret = try dropCaret(event, editor), caret.origin.getKey() != node.getKey()
          else {
            return true
          }
          try node.remove()
          let selection = try setSelectionFromCaretRange(getCollapsedCaretRange(caret))
          try setSelection(selection)
          // Beside the block the drop caret is at the start or end of;
          // within one, it splits it, as inserting one does.
          let anchor = selection.anchor
          let top = try anchor.getNode().getTopLevelElementOrThrow()
          let block = top as? ElementNode
          let first = try block?.getFirstDescendant() ?? top
          let last = try block?.getLastDescendant() ?? top
          if anchor.offset == 0, try anchor.getNode().getKey() == first.getKey() {
            _ = try top.insertBefore(node)
          } else if try anchor.getNode().getKey() == last.getKey(),
            try anchor.offset == (anchor.type == .text ? last.getTextContentSize() : 0)
              || (block?.getChildrenSize() ?? 0) == 0
          {
            _ = try top.insertAfter(node)
          } else {
            _ = try insertNodeToNearestRoot(node)
          }
          let nodeSelection = createNodeSelection()
          nodeSelection.add(node.getKey())
          try setSelection(nodeSelection)
          return true
        }, COMMAND_PRIORITY_HIGH),
    ])
  })

/// The one block decorator in a node selection, other than an image.
private func draggedDecorator() throws -> DecoratorNode? {
  guard let selection = try getSelection() as? NodeSelection else { return nil }
  let nodes = try selection.getNodes()
  guard nodes.count == 1, let node = nodes[0] as? DecoratorNode, !(node is ImageNode),
    try !node.isInline()
  else { return nil }
  return node
}

/// The key of the decorator the drag carries.
private func draggedKey(_ event: DragEvent) -> NodeKey? {
  guard let json = event.dataTransfer?.getData("application/x-lexical-drag"), !json.isEmpty,
    let object = try? JSONSerialization.jsonObject(with: Data(json.utf8)) as? [String: Any],
    object["type"] as? String == "decorator", let data = object["data"] as? [String: Any]
  else { return nil }
  return data["key"] as? String
}
