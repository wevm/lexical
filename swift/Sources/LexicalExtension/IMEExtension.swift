import Lexical

// Port of packages/lexical-extension/src/IMEExtension.ts.

/// The output of `IMEExtension`.
public struct IMEOutput {
  /// The key of the node an IME composition started in, `nil` when no
  /// composition is in progress.
  public let compositionKey: Signal<NodeKey?>
  /// The text node being composed, once there is one: a composition that
  /// starts on an element point gets its text node with the first marked
  /// text.
  public let composingTextNode: Signal<TextNode?>
}

/// `IMEExtension`: signals for the IME composition in progress.
///
/// JS clears the composition on the root element's `compositionend` event.
/// The views report the end of a composition as `COMPOSITION_END_COMMAND`
/// (where Lexical's own handler takes it at `COMMAND_PRIORITY_EDITOR`), so
/// the extension clears it there, at `COMMAND_PRIORITY_BEFORE_EDITOR`.
public let IMEExtension = defineExtension(
  name: "@lexical/extension/IME",
  build: { _, _, _ in
    IMEOutput(compositionKey: signal(nil), composingTextNode: signal(nil))
  },
  register: { editor, _, state in
    let output = state.getOutput()
    let compositionKey = output.compositionKey
    let composingTextNode = output.composingTextNode

    let removeStartCommand = editor.registerCommand(
      COMPOSITION_START_COMMAND,
      { _, _ in
        // Runs just before Lexical's own `$handleCompositionStart`, which
        // marks the same anchor key as composing.
        if let selection = try getSelection() as? RangeSelection {
          compositionKey.value = selection.anchor.key
        }
        return false
      }, COMMAND_PRIORITY_BEFORE_EDITOR)

    // Stage 1: resolve the key to a text node when it is one. An element
    // point start resolves to nil until stage 2.
    let stopKeyEffect = try effect {
      guard let key = compositionKey.value else {
        composingTextNode.value = nil
        return
      }
      composingTextNode.value = try editor.read(.latest) { try getNodeByKey(key) as? TextNode }
    }

    // Stage 2: the update tagged COMPOSITION_START_TAG has the selection in
    // the composing text node.
    let removeUpdateListener = editor.registerUpdateListener { payload in
      guard payload.tags.contains(COMPOSITION_START_TAG) else { return }
      try payload.editorState.read {
        if let selection = try getSelection() as? RangeSelection,
          let node = try selection.anchor.getNode() as? TextNode
        {
          composingTextNode.value = node
        }
      }
    }

    let removeEndCommand = editor.registerCommand(
      COMPOSITION_END_COMMAND,
      { _, _ in
        compositionKey.value = nil
        return false
      }, COMMAND_PRIORITY_BEFORE_EDITOR)

    let removeRootListener = editor.registerRootListener { rootElement, _ in
      if rootElement == nil {
        compositionKey.value = nil
      }
      return nil
    }

    return mergeRegister(
      removeStartCommand, stopKeyEffect, removeUpdateListener, removeEndCommand,
      removeRootListener)
  })
