import Lexical
import LexicalSelection

// Port of the export half of packages/lexical-html/src/index.ts. An export
// runs with `RenderContextExport` set and walks with the session's render
// config (`DOMRenderExtension`'s overrides, less those disabled for it).

/// `$generateHtmlFromNodes(editor, selection)`: the HTML of the editor's
/// state, or of `selection`. Call it inside `editor.read` or `update`.
public func generateHtmlFromNodes(
  _ editor: Editor, _ selection: BaseSelection? = nil
) throws
  -> String
{
  // BC: a legacy `editorState.read(cb)` scope has no active editor; establish
  // one, as JS does since #8589.
  try assumeActiveEditor(editor)
  return try generateDOMFromNodes(getDocument().createElement("div"), selection, editor)
    .innerHTML
}

/// `$generateDOMFromNodes(container, selection, editor)` (`@experimental`):
/// appends the DOM of the top-level nodes (of the selection's slot frame
/// when it is inside a slot) to `container`.
@discardableResult
public func generateDOMFromNodes<T: DOMNode>(
  _ container: T, _ selection: BaseSelection? = nil, _ editor: Editor? = nil
) throws -> T {
  let editor = try editor ?? getEditor()
  return try withRenderContext([contextValue(RenderContextExport, true)], editor) {
    let domConfig = try getSessionDOMRenderConfig(editor)
    // A selection wholly inside a slot never includes its host, so walk the
    // selection's slot frame; outside slots this is the root.
    let frame = try getSelectionSlotFrame(selection) as? ElementNode
    let parent: ElementNode = try frame ?? getRoot()
    for topLevelNode in try parent.getChildren() {
      try appendNodesToHTML(
        editor, topLevelNode, { container.appendChild($0) }, selection, domConfig)
    }
    return container
  }
}

/// `$generateDOMFromRoot(container, root)` (`@experimental`): appends the
/// DOM of `root` itself and its subtree.
@discardableResult
public func generateDOMFromRoot<T: DOMNode>(
  _ container: T, _ root: LexicalNode? = nil
) throws
  -> T
{
  let editor = try getEditor()
  let root = try root ?? getRoot()
  return try withRenderContext(
    [contextValue(RenderContextExport, true), contextValue(RenderContextRoot, true)], editor
  ) {
    try appendNodesToHTML(
      editor, root, { container.appendChild($0) }, nil, getSessionDOMRenderConfig(editor))
    return container
  }
}

/// `$appendNodeToHTML(editor, node, parentElement, selection)`
/// (`@experimental`): serializes one node and its subtree into
/// `parentElement`, as the exporter does for the nodes it walks.
@discardableResult
public func appendNodeToHTML(
  _ editor: Editor, _ node: LexicalNode, _ parentElement: DOMNode,
  _ selection: BaseSelection? = nil
) throws -> Bool {
  // Through the session, so session-disabled and export-only overrides apply
  // to slot subtrees as to the content around them.
  try appendNodesToHTML(
    editor, node, { parentElement.appendChild($0) }, selection, getSessionDOMRenderConfig(editor))
}

/// A `<br>` ending a block is not rendered, so importers drop it; after a
/// trailing line break the exporter adds the reconciler's managed
/// terminator, which importers drop in its place.
private func appendTerminatingLineBreak(_ element: DOMNode, _ lastIncludedChild: LexicalNode?) {
  guard isLineBreakNode(lastIncludedChild), let element = element as? DOMElement,
    isBlockDomNode(element), element.lastChild?.nodeName == "BR"
  else { return }
  let br = getDocument().createElement("br")
  br.setAttribute("data-lexical-managed-linebreak", "true")
  element.append(br)
}

@discardableResult
private func appendNodesToHTML(
  _ editor: Editor, _ currentNode: LexicalNode, _ parentAppend: (DOMNode) -> Void,
  _ selection: BaseSelection?, _ domConfig: EditorDOMRenderConfig
) throws -> Bool {
  var shouldInclude = try domConfig.shouldInclude(currentNode, selection, editor)
  let shouldExclude = try domConfig.shouldExclude(currentNode, selection, editor)
  var target = currentNode
  if let selection, let text = currentNode as? TextNode {
    target = try sliceSelectedTextNodeContent(selection, text, .clone)
  }
  let exportProps = try domConfig.exportDOM(target, editor)
  guard let element = exportProps.element else { return false }

  let fragment = getDocument().createDocumentFragment()
  let children: [LexicalNode] =
    if let getChildNodes = exportProps.getChildNodes {
      try getChildNodes()
    } else if let element = target as? ElementNode {
      try element.getChildren()
    } else {
      []
    }
  // A whole element host in a NodeSelection exports its whole subtree.
  let childSelection: BaseSelection? =
    shouldInclude && selection is NodeSelection && currentNode is ElementNode ? nil : selection
  var lastIncludedChild: LexicalNode? = nil
  for childNode in children {
    let shouldIncludeChild = try appendNodesToHTML(
      editor, childNode, { fragment.appendChild($0) }, childSelection, domConfig)
    if shouldIncludeChild { lastIncludedChild = childNode }
    if !shouldInclude && shouldIncludeChild,
      try domConfig.extractWithChild(currentNode, childNode, selection, "html", editor)
    {
      shouldInclude = true
    }
  }

  guard shouldInclude && !shouldExclude else {
    parentAppend(fragment)
    return shouldInclude
  }
  if element is DOMElement || element is DOMDocumentFragment {
    if let append = exportProps.append {
      try append(fragment)
    } else {
      element.appendChild(fragment)
    }
    appendTerminatingLineBreak(element, lastIncludedChild)
  }
  if element is DOMDocumentFragment {
    // Run `after` before the fragment is drained into its parent.
    if let after = exportProps.after, let replacement = try after(element) {
      element.replaceChildren(replacement)
    }
    parentAppend(element)
  } else {
    parentAppend(element)
    if let after = exportProps.after, let replacement = try after(element) {
      element.replaceWith(replacement)
    }
  }
  return shouldInclude
}
