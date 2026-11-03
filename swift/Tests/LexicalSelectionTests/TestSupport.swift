import Lexical
import LexicalSelection
import LexicalTesting

/// A root element that reports a left-to-right computed style for each
/// node the reconciler has rendered, as `getComputedStyle` does for a
/// node's DOM in JS.
final class StyledRootElement: ComputedStyleProvider {
  weak var editor: Editor?
  var style = ComputedStyle()
  func computedStyle(forKey key: NodeKey) -> ComputedStyle? {
    editor?._keyToDOMMap.has(key) == true ? style : nil
  }
}

/// `$setAnchorPoint` from the upstream selection test utils: selects a
/// detached text node first when there is no range selection.
func setAnchorPoint(_ key: NodeKey, _ offset: Int, _ type: PointType) throws {
  guard let selection = try getSelection() as? RangeSelection else {
    try createTextNode().select()
    return try setAnchorPoint(key, offset, type)
  }
  try selection.anchor.set(key, offset, type)
}

/// `$setFocusPoint` from the upstream selection test utils.
func setFocusPoint(_ key: NodeKey, _ offset: Int, _ type: PointType) throws {
  guard let selection = try getSelection() as? RangeSelection else {
    try createTextNode().select()
    return try setFocusPoint(key, offset, type)
  }
  try selection.focus.set(key, offset, type)
}

/// A point as `key:offset:type`, standing in for upstream's
/// `expect(point).toEqual(expect.objectContaining({key, offset, type}))`.
func pointString(_ point: Point) -> String {
  "\(point.key):\(point.offset):\(point.type.rawValue)"
}

/// The DOM the upstream reconciler mounts for the committed document, as
/// `rootElement.innerHTML`: each node's `createDOM`, with the attributes
/// `$createNode` adds (`data-lexical-text`, `data-lexical-decorator` and
/// `contenteditable`, `dir="auto"` on a block in the root or a shadow root)
/// and the managed `<br>` of an empty block or one ending in a line break.
func renderedHTML(_ editor: Editor) throws -> String {
  try editor.read(.latest) {
    let container = getDocument().createElement("div")
    for child in try getRoot().getChildren() {
      _ = container.appendChild(try renderDOM(child, editor))
    }
    return container.innerHTML
  }
}

private func renderDOM(_ node: LexicalNode, _ editor: Editor) throws -> DOMElement {
  let dom = try node.createDOM(editor._config, editor)
  if node is TextNode {
    dom.setAttribute("data-lexical-text", "true")
  } else if node is DecoratorNode {
    dom.setAttribute("data-lexical-decorator", "true")
    dom.setAttribute("contenteditable", "false")
  }
  guard let element = node as? ElementNode else { return dom }
  if let direction = element.__dir {
    dom.setAttribute("dir", direction)
  } else if let parent = try element.getParent(), try isRootOrShadowRoot(parent),
    parent.__dir == nil
  {
    dom.setAttribute("dir", "auto")
  }
  let children = try element.getChildren()
  for child in children {
    _ = dom.appendChild(try renderDOM(child, editor))
  }
  if try !element.isInline() {
    let last = children.last
    if try last == nil || last is LineBreakNode || (last is DecoratorNode && last!.isInline()) {
      let br = getDocument().createElement("br")
      br.setAttribute("data-lexical-managed-linebreak", "true")
      _ = dom.appendChild(br)
    }
  }
  return dom
}
