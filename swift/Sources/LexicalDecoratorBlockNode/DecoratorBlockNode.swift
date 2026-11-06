import Lexical
import LexicalExtension
import LexicalUtils

// Port of packages/lexical-react/src/LexicalDecoratorBlockNode.ts, and the
// commands of LexicalBlockWithAlignableContents.tsx, which are not React's:
// the node is the base class of block embeds (videos, tweets), and the
// extension aligns and selects them.

/// `DecoratorBlockNode`: a decorator on its own line that keeps an element
/// alignment (`format`), is not indentable and renders into a `<div>`.
/// Extend it for block embeds, with `BlockWithAlignableContentsExtension`
/// for their alignment and selection.
open class DecoratorBlockNode: DecoratorNode {
  public var __format: ElementFormatType = ""

  open override class func config() throws -> NodeConfig? {
    try NodeConfig(
      DecoratorBlockNode.self, extends: DecoratorNode.self,
      json: [
        SchemaField(
          "format",
          enumValue(
            ["", "left", "start", "center", "right", "end", "justify"].map { .string($0) }),
          field: \DecoratorBlockNode.__format)
      ])
  }

  open func canIndent() -> Bool { false }

  open override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement
  {
    getDocument().createElement("div")
  }

  open override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool {
    false
  }

  @discardableResult
  open func setFormat(_ format: ElementFormatType) throws -> Self {
    let writable = try getWritable()
    writable.__format = format
    return writable
  }

  open func getFormat() throws -> ElementFormatType {
    try getLatest().__format
  }

  open override func isInline() throws -> Bool { false }
}

/// `$isDecoratorBlockNode(node)`.
public func isDecoratorBlockNode(_ node: LexicalNode?) -> Bool {
  node is DecoratorBlockNode
}

/// The commands `BlockWithAlignableContents` registers for each block it
/// draws, for every `DecoratorBlockNode`. While a block is selected
/// (`isSelected()`, which a range over it is too), `FORMAT_ELEMENT_COMMAND`
/// aligns it: under a node selection the selected blocks, under a range
/// each block in it and the nearest block of every other node. A click on
/// a block (the views report it with its key) selects only it, or
/// deselects it when it was selected; a Shift click adds it to the node
/// selection or takes it out. React registers one handler per block, so of
/// several selected blocks only the first is aligned; this aligns them all.
public let BlockWithAlignableContentsExtension = defineExtension(
  name: "@lexical/react/BlockWithAlignableContents",
  register: { editor, _, _ in
    mergeRegister([
      editor.registerCommand(
        FORMAT_ELEMENT_COMMAND,
        { formatType, _ in
          guard let selection = try getSelection() else { return false }
          let nodes = try selection.getNodes()
          let selected = try nodes.compactMap { $0 as? DecoratorBlockNode }.filter {
            try $0.isSelected()
          }
          guard !selected.isEmpty else { return false }
          if selection is NodeSelection {
            for block in selected { try block.setFormat(formatType) }
          } else if selection is RangeSelection {
            for node in nodes {
              if let block = node as? DecoratorBlockNode {
                try block.setFormat(formatType)
              } else {
                _ = try getNearestBlockElementAncestorOrThrow(node).setFormat(formatType)
              }
            }
          }
          return true
        }, COMMAND_PRIORITY_LOW),
      editor.registerCommand(
        CLICK_COMMAND,
        { event, _ in
          guard let key = event.targetKey, let block = try getNodeByKey(key) as? DecoratorBlockNode
          else { return false }
          event.preventDefault()
          // `clearSelection(); setSelected(!isSelected)`.
          let isSelected = try block.isSelected()
          let selection: NodeSelection
          if let current = try getSelection() as? NodeSelection {
            if !event.shiftKey { current.clear() }
            selection = current
          } else {
            // A range that covers the block selects it too, so even its
            // deselection replaces the range with a node selection.
            selection = createNodeSelection()
            try setSelection(selection)
          }
          if isSelected {
            selection.delete(key)
          } else {
            selection.add(key)
          }
          return true
        }, COMMAND_PRIORITY_LOW),
    ])
  })
