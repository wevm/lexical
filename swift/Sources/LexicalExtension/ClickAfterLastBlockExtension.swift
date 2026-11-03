import Lexical

// Port of packages/lexical-extension/src/ClickAfterLastBlockExtension.ts.

/// `$defaultShouldInsertAfter`: a paragraph is inserted after a decorator
/// or a shadow root (a table), which a caret cannot be placed after.
public func defaultShouldInsertAfter(_ node: LexicalNode) throws -> Bool {
  if node is DecoratorNode {
    return true
  }
  if let element = node as? ElementNode, element.isShadowRoot() {
    return true
  }
  return false
}

/// `ClickAfterLastBlockConfig`.
public struct ClickAfterLastBlockConfig {
  /// Set to `true` to disable this extension.
  public var disabled: Bool
  /// `$shouldInsertAfter`: called inside the editor update with the last
  /// child of the root when the user clicks the empty area below it. Return
  /// `true` to insert a new paragraph after that node and select it.
  public var shouldInsertAfter: (LexicalNode) throws -> Bool

  public init(
    disabled: Bool = false,
    shouldInsertAfter: @escaping (LexicalNode) throws -> Bool = defaultShouldInsertAfter
  ) {
    self.disabled = disabled
    self.shouldInsertAfter = shouldInsertAfter
  }
}

/// `shouldClaimClick`. JS reacts only to a click whose target is the root
/// element itself, below the bottom of the last block's DOM. The view
/// measures that and reports the click with the root's key as its
/// `targetKey`. The predicate runs once, in the update that handles the
/// command, where JS runs it in a read and again in its update.
private func shouldClaimClick(
  _ editor: Editor, _ event: MouseEvent, _ shouldInsertAfter: (LexicalNode) throws -> Bool
) throws -> LexicalNode? {
  if !editor.isEditable() || event.targetKey != "root" {
    return nil
  }
  guard let lastChild = try getRoot().getLastChild(), try shouldInsertAfter(lastChild) else {
    return nil
  }
  return lastChild
}

/// `ClickAfterLastBlockExtension`: a click in the empty area below the last
/// block inserts a paragraph after it, when the last block is one a caret
/// cannot be placed after (a block decorator, a table).
///
/// JS listens to `mousedown` and `click` on the root element in the
/// capture phase and stops Lexical's own click handling. Native views
/// report the click as `CLICK_COMMAND`, so the extension handles it at
/// `COMMAND_PRIORITY_CRITICAL`, before the other click handlers.
public let ClickAfterLastBlockExtension = defineExtension(
  name: "@lexical/ClickAfterLastBlock", config: ClickAfterLastBlockConfig(),
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let output = state.getOutput()
    return try effect { () -> (() -> Void)? in
      if output.disabled.value {
        return nil
      }
      return editor.registerCommand(
        CLICK_COMMAND,
        { event, _ in
          guard
            let lastChild = try shouldClaimClick(editor, event, output.shouldInsertAfter.peek())
          else {
            return false
          }
          event.preventDefault()
          let paragraph = try createParagraphNode()
          _ = try lastChild.insertAfter(paragraph)
          _ = try paragraph.select()
          return true
        }, COMMAND_PRIORITY_CRITICAL)
    }
  })
