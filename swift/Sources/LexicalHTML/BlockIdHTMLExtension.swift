import Lexical
import LexicalExtension

// Port of packages/lexical-html/src/BlockIdHTMLExtension.ts.

/// `BLOCK_ID_ATTRIBUTE` (`@experimental`): the attribute that carries a
/// block id in HTML.
public let BLOCK_ID_ATTRIBUTE = "data-block-id"

/// `BLOCK_HANDLE_ATTRIBUTE` (`@experimental`): the attribute that carries a
/// block handle (see `getBlockHandle`) in HTML exported with
/// `handles: true`. Import never reads it.
public let BLOCK_HANDLE_ATTRIBUTE = "data-block-handle"

/// `BLOCK_WRAPPER_ID_ATTRIBUTE` (`@experimental`): the attribute that
/// carries the block id of a block whose own element its export removes,
/// handing its children to another element: a list item that only wraps a
/// nested list is exported as that nested list, inside the previous item's
/// element. The id goes on the first element the block's element held (the
/// nested list), and import puts it back on the block that ends up holding
/// only what that element produced (the wrapper item), where the import of
/// that element, its parent or its grandparent makes it; otherwise that
/// block gets a new id.
public let BLOCK_WRAPPER_ID_ATTRIBUTE = "data-block-wrapper-id"

/// `BlockIdHTMLConfig` (`@experimental`).
public struct BlockIdHTMLConfig {
  /// Write `BLOCK_ID_ATTRIBUTE` on the exported element of every identified
  /// block (`generateHtmlFromNodes`, copy as HTML, …).
  public var export: Bool
  /// With `export` on, write each block's handle in
  /// `BLOCK_HANDLE_ATTRIBUTE` instead of its id in `BLOCK_ID_ATTRIBUTE`: a
  /// short opaque name derived from a hash of the id, for HTML shown to a
  /// model. Keep the handles of the export (`getBlockHandles()`, in the same
  /// read), and resolve what the model answers with
  /// `resolveBlockHandle(handle, BlockHandleOptions(read:))`. Import does not
  /// read handles. Default: `false`. (Optional in JS, so that a config
  /// written for an earlier version still type-checks.)
  public var handles: Bool
  /// Read `BLOCK_ID_ATTRIBUTE` back when HTML is imported through
  /// `DOMImportExtension`, onto the block that the element produced.
  /// Imported ids are never trusted: an invalid one is ignored, and a valid
  /// one goes through the uniqueness rule of `BlockIdExtension`. An imported
  /// id is dropped from a block that is not identified once it is inserted.
  public var `import`: Bool

  public init(export: Bool = true, handles: Bool = false, import: Bool = true) {
    self.export = export
    self.handles = handles
    self.import = `import`
  }
}

private func isBlockForId(_ node: LexicalNode) throws -> Bool {
  try (node is ElementNode || node is DecoratorNode) && !node.isInline()
}

private func parseAttribute(_ value: String?) -> String? {
  blockIdState.parse(value.map { .string($0) })
}

/// The per-editor state of `BlockIdHTMLExtension`, opaque outside this
/// module (`unknown` in JS).
package final class BlockIdHTMLRuntime {
  /// The keys of the blocks that got an id from HTML in this update.
  let imported = JSSet<NodeKey>()
  /// How many waiting wrapper ids the resolve rule looked at, for the tests
  /// of its cost (JS spies on `ElementNode.prototype.getParent`).
  package var wrapperIdLookups = 0
}

private func runtime() throws -> BlockIdHTMLRuntime {
  try getExtensionDependency(BlockIdHTMLExtension).initResult as! BlockIdHTMLRuntime
}

/// The keys of the blocks that got an id from HTML in this update.
private func importedKeys() throws -> JSSet<NodeKey> {
  try runtime().imported
}

nonisolated(unsafe) private let BlockIdImportRule = defineImportRule(
  name: "@lexical/html/block-id", match: sel.any().attr(BLOCK_ID_ATTRIBUTE)
) { _, el, next in
  let nodes = try next()
  if try !getExtensionOutput(BlockIdHTMLExtension).import.peek() {
    return nodes
  }
  if let id = parseAttribute(el.getAttribute(BLOCK_ID_ATTRIBUTE)) {
    // The element produced this block (and possibly inline siblings or
    // hoisted children after it): the first block is the one it describes.
    for node in nodes {
      if try isBlockForId(node) {
        if try getBlockId(node) == nil {
          try setState(node, blockIdState, id)
          try importedKeys().add(node.getKey())
        }
        break
      }
    }
  }
  return nodes
}

/// The nodes that elements carrying `BLOCK_WRAPPER_ID_ATTRIBUTE` produced in
/// an import, with that id: the block that ends up holding one of them
/// alone gets the id (see `BlockWrapperIdResolveRule`). Per import session.
private final class PendingWrapperIdsState {
  /// The waiting ids, by the key of the node they wait for.
  let pending = JSMap<NodeKey, String>()
  /// The ids still looked for, in the order they were found: the node's key
  /// and the element that carried the id. An id is looked for by the
  /// elements around that element up to its grandparent, and dropped after.
  var active: [(key: NodeKey, el: DOMElement)] = []
}

nonisolated(unsafe) private let PendingWrapperIds = createImportState("blockWrapperIds") {
  PendingWrapperIdsState?.none
}

nonisolated(unsafe) private let BlockWrapperIdImportRule = defineImportRule(
  name: "@lexical/html/block-wrapper-id", match: sel.any().attr(BLOCK_WRAPPER_ID_ATTRIBUTE)
) { ctx, el, next in
  let nodes = try next()
  if try !getExtensionOutput(BlockIdHTMLExtension).import.peek() {
    return nodes
  }
  if let id = parseAttribute(el.getAttribute(BLOCK_WRAPPER_ID_ATTRIBUTE)),
    let waiting = ctx.session.get(PendingWrapperIds)
  {
    for node in nodes {
      if try isBlockForId(node) {
        waiting.pending.set(node.getKey(), id)
        waiting.active.append((node.getKey(), el))
        break
      }
    }
  }
  return nodes
}

/// Around each element's import: of the wrapper ids found in its children
/// or grandchildren, each one's node, if its parent now holds it alone and
/// is a block without an id, gives that parent the id, through the same
/// rules as an imported id. An id is looked at by three elements at most,
/// so the cost is linear in the ids.
nonisolated(unsafe) private let BlockWrapperIdResolveRule = defineImportRule(
  name: "@lexical/html/block-wrapper-id-resolve", match: sel.any()
) { ctx, el, next in
  let waiting = ctx.session.get(PendingWrapperIds)
  let from = waiting?.active.count ?? 0
  let nodes = try next()
  guard let waiting, waiting.active.count != from else {
    return nodes
  }
  var kept = from
  let runtime = try runtime()
  for k in from..<waiting.active.count {
    runtime.wrapperIdLookups += 1
    let entry = waiting.active[k]
    guard let id = waiting.pending.get(entry.key) else {
      continue
    }
    let holder = try getNodeByKey(entry.key)?.getParent()
    if let holder, try holder.getChildrenSize() == 1, try !isRootOrShadowRoot(holder),
      try isBlockForId(holder), try getBlockId(holder) == nil
    {
      try setState(holder, blockIdState, id)
      try importedKeys().add(holder.getKey())
      waiting.pending.delete(entry.key)
    } else if entry.el === el || entry.el.parentElement === el {
      // Its parent, or its grandparent, looks again.
      waiting.active[kept] = entry
      kept += 1
    } else {
      waiting.pending.delete(entry.key)
    }
  }
  waiting.active.removeLast(waiting.active.count - kept)
  return nodes
}

nonisolated(unsafe) private let BlockIdImportOverlay = defineOverlayRules([
  BlockIdImportRule, BlockWrapperIdImportRule,
])
nonisolated(unsafe) private let BlockWrapperIdResolveOverlay = defineOverlayRules([
  BlockWrapperIdResolveRule
])

/// `dom.querySelector('[data-block-wrapper-id]') !== null`: whether an
/// element below `node` carries a wrapper id.
private func containsWrapperId(_ node: DOMNode) -> Bool {
  node.childNodes.contains { child in
    (child as? DOMElement)?.hasAttribute(BLOCK_WRAPPER_ID_ATTRIBUTE) == true
      || containsWrapperId(child)
  }
}

/// Installs the import rule as a walk-wide overlay, so it wraps whichever
/// rule converts the element, whatever the order of the extensions that
/// contribute import rules.
private func installBlockIdImportOverlay(
  _ dom: DOMNode, _ ctx: DOMPreprocessContext, _ next: () throws -> Void
) throws {
  // The resolver runs for every element, so only where the HTML carries a
  // wrapper id.
  let wrapped = containsWrapperId(dom)
  if wrapped {
    ctx.session.set(PendingWrapperIds, PendingWrapperIdsState())
  }
  ctx.session.update(ImportOverlays) { prev in
    wrapped
      ? prev + [BlockIdImportOverlay, BlockWrapperIdResolveOverlay] : prev + [BlockIdImportOverlay]
  }
  try next()
}

nonisolated(unsafe) private let BlockIdExportOverride = domOverride(
  all: DOMRenderMatchConfig(exportDOM: { node, next, _ in
    var output = try next()
    let html = try getExtensionOutput(BlockIdHTMLExtension)
    guard let element = output.element as? DOMElement, html.export.peek(),
      try getExtensionOutput(BlockIdExtension).isIdentifiedBlock(node)
    else {
      return output
    }
    if html.handles.peek() {
      if let handle = try getBlockHandle(node) {
        element.setAttribute(BLOCK_HANDLE_ATTRIBUTE, handle)
      }
    } else if let id = try getBlockId(node) {
      element.setAttribute(BLOCK_ID_ATTRIBUTE, id)
      if let after = output.after {
        // Where the export removes the block's element and hands its
        // children to another element (a list item that only wraps a
        // nested list), the id goes on the first element it held
        // (BLOCK_WRAPPER_ID_ATTRIBUTE).
        output.after = { generated in
          let generatedElement = generated as? DOMElement
          let first = generatedElement?.children.first
          let result = try after(generated)
          if let generatedElement, generatedElement.parentNode == nil, result === generated,
            let first, first.parentNode !== generatedElement, first.parentNode != nil
          {
            first.setAttribute(BLOCK_WRAPPER_ID_ATTRIBUTE, id)
          }
          return result
        }
      }
    }
    return output
  }),
  DOMOverrideOptions(
    // Left out of export sessions while `export` is off, so that nothing
    // runs for each exported element.
    disabledForSession: { _ in
      (try? getExtensionOutput(BlockIdHTMLExtension).export.peek()) != true
    }))

/// `BlockIdHTMLExtension` (`@experimental`): opt-in HTML support for
/// `BlockIdExtension`: exported HTML carries each identified block's id in a
/// `data-block-id` attribute, and HTML imported through
/// `DOMImportExtension` reads it back (through the uniqueness rule, so ids
/// in pasted HTML never clash with ids in the document).
///
/// For HTML shown to a model, `handles: true` writes short handles in
/// `data-block-handle` instead. The legacy `generateNodesFromDOM` path
/// (`importDOM`) does not read the attribute.
public let BlockIdHTMLExtension:
  LexicalExtension<BlockIdHTMLConfig, NamedSignals<BlockIdHTMLConfig>, AnyObject> =
    defineExtension(
      name: "@lexical/html/BlockIdHTML", config: BlockIdHTMLConfig(),
      dependencies: [
        BlockIdExtension,
        configExtension(DOMRenderExtension) { $0.overrides = [BlockIdExportOverride] },
        configExtension(DOMImportExtension) { $0.preprocess = [installBlockIdImportOverlay] },
      ],
      init: { _, _, _ -> AnyObject in BlockIdHTMLRuntime() },
      build: { _, config, _ in namedSignals(config) },
      register: { editor, _, state in
        let imported = (try state.getInitResult() as! BlockIdHTMLRuntime).imported
        // Root transforms run after the other transforms of a pass, once the
        // imported blocks are inserted, so the predicate can see where they are.
        return try editor.registerNodeTransform(RootNode.self) { _ in
          if imported.isEmpty {
            return
          }
          let isIdentifiedBlock = try getExtensionOutput(BlockIdExtension).isIdentifiedBlock
          for key in imported {
            if let node = try getNodeByKey(key), try node.isAttached(), try !isIdentifiedBlock(node)
            {
              try setState(node, blockIdState, nil)
            }
          }
          imported.clear()
        }
      })
