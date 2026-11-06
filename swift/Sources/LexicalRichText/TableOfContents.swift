import Lexical
import LexicalUtils

// Port of packages/lexical-react/src/LexicalTableOfContentsPlugin.tsx: the
// logic of `TableOfContentsPlugin`, which keeps the document's headings in
// order for a view to list. React keeps it in a component whose children
// render the entries; here it is a registration that reports them to a
// closure.

/// `TableOfContentsEntry`: a heading's key, text and tag.
public struct TableOfContentsEntry: Equatable, Sendable {
  public var key: NodeKey
  public var text: String
  public var tag: HeadingTagType

  public init(key: NodeKey, text: String, tag: HeadingTagType) {
    self.key = key
    self.text = text
    self.tag = tag
  }
}

private func toEntry(_ heading: HeadingNode) throws -> TableOfContentsEntry {
  TableOfContentsEntry(
    key: heading.getKey(), text: try heading.getTextContent(), tag: try heading.getTag())
}

private func insertHeadingIntoTableOfContents(
  _ prevHeading: HeadingNode?, _ newHeading: HeadingNode,
  _ currentTableOfContents: [TableOfContentsEntry]
) throws -> [TableOfContentsEntry] {
  let newEntry = try toEntry(newHeading)
  guard let prevHeading else {
    // Check if the key already exists.
    if currentTableOfContents.first?.key == newHeading.getKey() {
      return currentTableOfContents
    }
    return [newEntry] + currentTableOfContents
  }
  var newTableOfContents: [TableOfContentsEntry] = []
  for (i, entry) in currentTableOfContents.enumerated() {
    newTableOfContents.append(entry)
    if entry.key == prevHeading.getKey() && entry.key != newHeading.getKey() {
      // Check if the key already exists.
      if i + 1 < currentTableOfContents.count
        && currentTableOfContents[i + 1].key == newHeading.getKey()
      {
        return currentTableOfContents
      }
      newTableOfContents.append(newEntry)
    }
  }
  return newTableOfContents
}

private func deleteHeadingFromTableOfContents(
  _ key: NodeKey, _ currentTableOfContents: [TableOfContentsEntry]
) -> [TableOfContentsEntry] {
  currentTableOfContents.filter { $0.key != key }
}

private func updateHeadingInTableOfContents(
  _ heading: HeadingNode, _ currentTableOfContents: [TableOfContentsEntry]
) throws -> [TableOfContentsEntry] {
  try currentTableOfContents.map { $0.key == heading.getKey() ? try toEntry(heading) : $0 }
}

private func updateHeadingPosition(
  _ prevHeading: HeadingNode?, _ heading: HeadingNode,
  _ currentTableOfContents: [TableOfContentsEntry]
) throws -> [TableOfContentsEntry] {
  var newTableOfContents: [TableOfContentsEntry] = []
  let newEntry = try toEntry(heading)
  if prevHeading == nil {
    newTableOfContents.append(newEntry)
  }
  for entry in currentTableOfContents {
    if entry.key == heading.getKey() { continue }
    newTableOfContents.append(entry)
    if let prevHeading, entry.key == prevHeading.getKey() {
      newTableOfContents.append(newEntry)
    }
  }
  return newTableOfContents
}

/// `$getPreviousHeading`. Upstream walks with `$getNextRightPreorderNode`,
/// which goes backwards through the document.
private func getPreviousHeading(_ node: HeadingNode) throws -> HeadingNode? {
  var prevHeading = try getNextRightPreorderNode(node)
  while let node = prevHeading, !(node is HeadingNode) {
    prevHeading = try getNextRightPreorderNode(node)
  }
  return prevHeading as? HeadingNode
}

/// `TableOfContentsPlugin`: reports the document's headings, in order, to
/// `onChange` at once and after each change to them. Returns the clean-up.
public func registerTableOfContents(
  _ editor: Editor, _ onChange: @escaping ([TableOfContentsEntry]) -> Void
) throws -> () -> Void {
  // Set the table of contents' initial state.
  var currentTableOfContents: [TableOfContentsEntry] = []
  try editor.read(.latest) {
    func updateCurrentTableOfContents(_ node: ElementNode) throws {
      for child in try node.getChildren() {
        if let heading = child as? HeadingNode {
          currentTableOfContents.append(try toEntry(heading))
        } else if let element = child as? ElementNode {
          try updateCurrentTableOfContents(element)
        }
      }
    }
    try updateCurrentTableOfContents(getRoot())
  }
  onChange(currentTableOfContents)

  let removeRootUpdateListener = editor.registerUpdateListener { payload in
    try payload.editorState.read {
      func updateChildHeadings(_ node: ElementNode) throws {
        for child in try node.getChildren() {
          if let heading = child as? HeadingNode {
            let prevHeading = try getPreviousHeading(heading)
            currentTableOfContents = try updateHeadingPosition(
              prevHeading, heading, currentTableOfContents)
            onChange(currentTableOfContents)
          } else if let element = child as? ElementNode {
            try updateChildHeadings(element)
          }
        }
      }
      // If a node changes, all of its child headings' positions need to be
      // updated.
      for node in try getRoot().getChildren() {
        if let element = node as? ElementNode, payload.dirtyElements.get(node.getKey()) == true {
          try updateChildHeadings(element)
        }
      }
    }
  }

  // Listen to heading mutations and update the state.
  let removeHeaderMutationListener = try editor.registerMutationListener(
    HeadingNode.self,
    { mutatedNodes, _ in
      try editor.read(.latest) {
        for (nodeKey, mutation) in mutatedNodes {
          if mutation == "created" {
            if let newHeading = try getNodeByKey(nodeKey) as? HeadingNode {
              let prevHeading = try getPreviousHeading(newHeading)
              currentTableOfContents = try insertHeadingIntoTableOfContents(
                prevHeading, newHeading, currentTableOfContents)
            }
          } else if mutation == "destroyed" {
            currentTableOfContents = deleteHeadingFromTableOfContents(
              nodeKey, currentTableOfContents)
          } else if mutation == "updated" {
            if let newHeading = try getNodeByKey(nodeKey) as? HeadingNode {
              let prevHeading = try getPreviousHeading(newHeading)
              currentTableOfContents = try updateHeadingPosition(
                prevHeading, newHeading, currentTableOfContents)
            }
          }
        }
        onChange(currentTableOfContents)
      }
    }, skipInitialization: true)

  // Listen to text node mutations.
  let removeTextNodeMutationListener = try editor.registerMutationListener(
    TextNode.self,
    { mutatedNodes, _ in
      try editor.read(.latest) {
        for (nodeKey, mutation) in mutatedNodes where mutation == "updated" {
          if let currNode = try getNodeByKey(nodeKey),
            let parentNode = try currNode.getParentOrThrow() as? HeadingNode
          {
            currentTableOfContents = try updateHeadingInTableOfContents(
              parentNode, currentTableOfContents)
            onChange(currentTableOfContents)
          }
        }
      }
    }, skipInitialization: true)

  return {
    removeHeaderMutationListener()
    removeTextNodeMutationListener()
    removeRootUpdateListener()
  }
}
