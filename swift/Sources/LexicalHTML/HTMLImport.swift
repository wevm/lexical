import Lexical

// Port of the legacy import half of packages/lexical-html/src/index.ts:
// `$generateNodesFromDOM`, which converts with each node class's
// `importDOM`. Inlining `<style>` sheets into elements
// (`$inlineStylesFromStyleSheetsDOM`) is not ported yet.

private let ignoreTags: Set<String> = ["STYLE", "SCRIPT"]

/// `$generateNodesFromDOM(editor, dom)`: the Lexical nodes for a parsed
/// document's body, or for a node's children. Call it inside an update.
public func generateNodesFromDOM(_ editor: Editor, _ dom: DOMNode) throws -> [LexicalNode] {
  let elements = (dom as? DOMDocument)?.body?.childNodes ?? dom.childNodes
  var lexicalNodes: [LexicalNode] = []
  var allArtificialNodes: [ArtificialNode__DO_NOT_USE] = []
  for element in elements where !ignoreTags.contains(element.nodeName) {
    lexicalNodes += try createNodesFromDOM(element, editor, &allArtificialNodes, false)
  }
  try unwrapArtificialNodes(allArtificialNodes)
  return lexicalNodes
}

private func getConversionFunction(
  _ domNode: DOMNode, _ editor: Editor
) throws
  -> DOMConversionFn?
{
  var current: DOMConversion? = nil
  for cached in editor._htmlConversions[domNode.nodeName.lowercased()] ?? [] {
    // Among equal priorities the last registered importer wins.
    if let conversion = try cached(domNode),
      current == nil || current!.priority <= conversion.priority
    {
      current = conversion
    }
  }
  return current?.conversion
}

/// Ordered by the DOM node names that set them, as JS's `Map` is.
private typealias ChildConversions = [(String, DOMChildConversion)]

private func createNodesFromDOM(
  _ node: DOMNode, _ editor: Editor, _ allArtificialNodes: inout [ArtificialNode__DO_NOT_USE],
  _ hasBlockAncestorLexicalNode: Bool, _ forChildMap: ChildConversions = [],
  _ parentLexicalNode: LexicalNode? = nil
) throws -> [LexicalNode] {
  var lexicalNodes: [LexicalNode] = []
  if ignoreTags.contains(node.nodeName) { return lexicalNodes }
  var forChildMap = forChildMap
  var currentLexicalNode: LexicalNode? = nil
  let transformOutput = try getConversionFunction(node, editor).flatMap { try $0(node) }
  if let transformOutput {
    currentLexicalNode = transformOutput.node.last
    if currentLexicalNode != nil {
      for (_, forChild) in forChildMap {
        currentLexicalNode = try forChild(currentLexicalNode!, parentLexicalNode)
        if currentLexicalNode == nil { break }
      }
      if let current = currentLexicalNode {
        lexicalNodes += transformOutput.node.count > 1 ? transformOutput.node : [current]
      }
    }
    if let forChild = transformOutput.forChild {
      if let index = forChildMap.firstIndex(where: { $0.0 == node.nodeName }) {
        forChildMap[index].1 = forChild
      } else {
        forChildMap.append((node.nodeName, forChild))
      }
    }
  }
  // A DOM node without a conversion still has its children converted.
  let hasBlockAncestorForChildren: Bool
  if let current = currentLexicalNode, try isRootOrShadowRoot(current) {
    hasBlockAncestorForChildren = false
  } else {
    hasBlockAncestorForChildren =
      try isBlockElementNode(currentLexicalNode) || hasBlockAncestorLexicalNode
  }
  var childLexicalNodes: [LexicalNode] = []
  for child in node.childNodes {
    childLexicalNodes += try createNodesFromDOM(
      child, editor, &allArtificialNodes, hasBlockAncestorForChildren, forChildMap,
      currentLexicalNode)
  }
  if let after = transformOutput?.after {
    childLexicalNodes = try after(childLexicalNodes)
  }
  if isBlockDomNode(node) {
    if !hasBlockAncestorForChildren {
      childLexicalNodes = try wrapContinuousInlines(node, childLexicalNodes) {
        try createParagraphNode()
      }
    } else {
      childLexicalNodes = try wrapContinuousInlines(node, childLexicalNodes) {
        let artificialNode = try ArtificialNode__DO_NOT_USE()
        allArtificialNodes.append(artificialNode)
        return artificialNode
      }
    }
  }
  if let current = currentLexicalNode {
    if let element = current as? ElementNode {
      try element.append(contentsOf: childLexicalNodes)
    }
  } else if !childLexicalNodes.isEmpty {
    // An unconverted DOM node's children take its place.
    lexicalNodes += childLexicalNodes
  } else if isBlockDomNode(node) && isDomNodeBetweenTwoInlineNodes(node) {
    // An empty, unconverted block between inline nodes is a line break.
    lexicalNodes.append(try createLineBreakNode())
  }
  return lexicalNodes
}

private func wrapContinuousInlines(
  _ domNode: DOMNode, _ nodes: [LexicalNode], _ createWrapper: () throws -> ElementNode
) throws -> [LexicalNode] {
  let textAlign = (domNode as? DOMElement)?.style["text-align"] ?? ""
  var out: [LexicalNode] = []
  var continuousInlines: [LexicalNode] = []
  for (i, node) in nodes.enumerated() {
    if try isBlockElementNode(node) {
      let block = node as! ElementNode
      if !textAlign.isEmpty, try block.getFormat() == 0 {
        try block.setFormat(textAlign)
      }
      out.append(node)
    } else {
      continuousInlines.append(node)
      if try i == nodes.count - 1 || isBlockElementNode(nodes[i + 1]) {
        let wrapper = try createWrapper()
        try wrapper.setFormat(textAlign)
        try wrapper.append(contentsOf: continuousInlines)
        out.append(wrapper)
        continuousInlines = []
      }
    }
  }
  return out
}

/// Replaces each artificial node with its children, with a line break
/// between adjacent ones.
private func unwrapArtificialNodes(_ allArtificialNodes: [ArtificialNode__DO_NOT_USE]) throws {
  for node in allArtificialNodes {
    if try node.getParent() != nil, try node.getNextSibling() is ArtificialNode__DO_NOT_USE {
      try node.insertAfter(createLineBreakNode())
    }
  }
  for node in allArtificialNodes {
    if let parent = try node.getParent() {
      try parent.splice(node.getIndexWithinParent(), 1, node.getChildren())
    }
  }
}

private func isDomNodeBetweenTwoInlineNodes(_ node: DOMNode) -> Bool {
  guard let next = node.nextSibling, let previous = node.previousSibling else { return false }
  return isInlineDomNode(next) && isInlineDomNode(previous)
}
