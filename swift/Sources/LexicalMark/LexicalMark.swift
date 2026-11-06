import Lexical

// Port of packages/lexical-mark/src/index.ts.

/// `$unwrapMarkNode(node)`: puts the mark's children in its place.
public func unwrapMarkNode(_ node: MarkNode) throws {
  let children = try node.getChildren()
  var target: LexicalNode? = nil
  for child in children {
    if let target {
      try target.insertAfter(child)
    } else {
      try node.insertBefore(child)
    }
    target = child
  }
  try node.remove()
}

/// `$wrapSelectionInMarkNode(selection, isBackward, id, createNode)`:
/// wraps the text, line breaks and inline nodes `selection` covers in mark
/// nodes with `id`, one per run of siblings, and collapses the selection at
/// the end (the start when `isBackward`).
public func wrapSelectionInMarkNode(
  _ selection: RangeSelection, _ isBackward: Bool, _ id: String,
  _ createNode: (([String]) throws -> MarkNode)? = nil
) throws {
  // Force a forwards selection since append is used, ignore the argument.
  // A new selection is used to avoid side-effects of flipping the given
  // selection
  let forwardSelection = createRangeSelection()
  let (startPoint, endPoint) =
    try selection.isBackward()
    ? (selection.focus, selection.anchor) : (selection.anchor, selection.focus)
  try forwardSelection.anchor.set(startPoint.key, startPoint.offset, startPoint.type)
  try forwardSelection.focus.set(endPoint.key, endPoint.offset, endPoint.type)

  var currentNodeParent: ElementNode? = nil
  var lastCreatedMarkNode: MarkNode? = nil

  // Note that extract will split text nodes at the boundaries
  let nodes = try forwardSelection.extract()
  // We only want wrap adjacent text nodes, line break nodes
  // and inline element nodes. For decorator nodes and block
  // element nodes, we step out of their boundary and start
  // again after, if there are more nodes.
  for node in nodes {
    if let lastCreatedMarkNode, try lastCreatedMarkNode.isParentOf(node) {
      // If the current node is a child of the last created mark node, there is nothing to do here
      continue
    }
    var targetNode: LexicalNode? = nil

    if node is TextNode {
      // Case 1: The node is a text node and we can include it
      targetNode = node
    } else if node is MarkNode {
      // Case 2: the node is a mark node and we can ignore it as a target,
      // moving on to its children. Note that when we make a mark inside
      // another mark, it may ultimately be unnested by a call to
      // `registerNestedElementResolver<MarkNode>` somewhere else in the
      // codebase.
      continue
    } else if node is ElementNode || node is DecoratorNode, try node.isInline() {
      // Case 3: inline element/decorator nodes can be added in their entirety
      // to the new mark
      targetNode = node
    }

    if let targetNode {
      // Now that we have a target node for wrapping with a mark, we can run
      // through special cases.
      if targetNode.is(currentNodeParent) {
        // The current node is a child of the target node to be wrapped, there
        // is nothing to do here.
        continue
      }
      let parentNode = try targetNode.getParent()
      if parentNode == nil || !parentNode!.is(currentNodeParent) {
        // If the parent node is not the current node's parent node, we can
        // clear the last created mark node.
        lastCreatedMarkNode = nil
      }

      currentNodeParent = parentNode

      if lastCreatedMarkNode == nil {
        // If we don't have a created mark node, we can make one
        let markNode = try (createNode ?? { try createMarkNode($0) })([id])
        try targetNode.insertBefore(markNode)
        lastCreatedMarkNode = markNode
      }

      // Add the target node to be wrapped in the latest created mark node
      try lastCreatedMarkNode!.append(targetNode)
    } else {
      // If we don't have a target node to wrap we can clear our state and
      // continue on with the next node
      currentNodeParent = nil
      lastCreatedMarkNode = nil
    }
  }
  // Make selection collapsed at the end
  if let lastCreatedMarkNode {
    if isBackward {
      try lastCreatedMarkNode.selectStart()
    } else {
      try lastCreatedMarkNode.selectEnd()
    }
  }
}

/// `$getMarkIDs(node, offset)`: the ids of the mark `node` is in, or of the
/// mark right after it when `offset` is at its end; `nil` when there is
/// none.
public func getMarkIDs(_ node: TextNode, _ offset: Int) throws -> [String]? {
  var currentNode: LexicalNode? = node
  while let current = currentNode {
    if let mark = current as? MarkNode {
      return try mark.getIDs()
    } else if let text = current as? TextNode, try offset == text.getTextContentSize() {
      if let nextSibling = try text.getNextSibling() as? MarkNode {
        return try nextSibling.getIDs()
      }
    }
    currentNode = try current.getParent()
  }
  return nil
}

/// `MarkExtension`: registers `MarkNode`.
public let MarkExtension = defineExtension(
  name: "@lexical/mark", nodes: { [.klass(MarkNode.self)] })
