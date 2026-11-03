// Port of packages/lexical/src/LexicalEditorState.ts.

/// The node map of an editor state: insertion-ordered, as a JS `Map`.
public typealias NodeMap = JSMap<NodeKey, LexicalNode>

/// Throws as a frozen JS node map's `set`, `clear` and `delete` do.
package func errorOnFrozenNodeMap(_ nodeMap: NodeMap, _ method: String) throws {
  if nodeMap._frozen {
    throw LexicalError("Cannot call \(method)() on a frozen Lexical node map")
  }
}

package func editorStateHasDirtySelection(_ editorState: EditorState, _ editor: Editor) -> Bool {
  let currentSelection = editor.getEditorState()._selection
  if let pendingSelection = editorState._selection {
    if pendingSelection.dirty || !pendingSelection.is(currentSelection) {
      return true
    }
  } else if currentSelection != nil {
    return true
  }
  return false
}

package func cloneEditorState(_ current: EditorState) -> EditorState {
  EditorState(current._nodeMap.copy(), nil, current._slotsUsed)
}

package func createEmptyEditorState() -> EditorState {
  EditorState(NodeMap([("root", try! createRootNode())]), nil, false)
}

func exportNodeToJSON(_ node: LexicalNode) throws -> JSONObject {
  var serializedNode = try exportNodeJSON(node)
  if let element = node as? ElementNode {
    var children: [JSONValue] = serializedNode["children"]?.arrayValue ?? []
    for child in try element.getChildren() {
      children.append(.object(try exportNodeToJSON(child)))
    }
    serializedNode["children"] = .array(children)
  }
  let slotNames = try getSlotNames(node)
  if !slotNames.isEmpty {
    var serializedSlots = JSONObject()
    for name in slotNames {
      guard let slotNode = try getSlot(node, name) else {
        throw LexicalError(
          "LexicalNode: Node \(type(of: node)) has slot \"\(name)\" but it resolved to no node during export."
        )
      }
      serializedSlots[name] = .object(try exportNodeToJSON(slotNode))
    }
    serializedNode["$slots"] = .object(serializedSlots)
  }
  return serializedNode
}

/// An immutable snapshot of the editor content: the node map and the
/// selection. Serializes to JSON in the legacy and compact forms.
public final class EditorState {
  package var _nodeMap: NodeMap
  package var _selection: BaseSelection?
  package var _flushSync = false
  package var _readOnly = false
  package var _parsed = false
  package var _slotsUsed: Bool
  /// The keys of nodes moved during the update, keyed by the previous node
  /// map, for the reconciler.
  package var _movedNodes: JSMap<ObjectIdentifier, JSSet<NodeKey>>?
  /// See `$getTextInsideCache`.
  package var _textInsideCache: JSMap<NodeKey, Bool>?
  /// See `getCachedTypeToNodeMap`.
  package var _typeToNodeMap: TypeToNodeMap?

  package init(_ nodeMap: NodeMap, _ selection: BaseSelection? = nil, _ slotsUsed: Bool = false) {
    self._nodeMap = nodeMap
    self._selection = selection
    self._slotsUsed = slotsUsed
  }

  public func isEmpty() -> Bool {
    _nodeMap.size <= 1 && _selection == nil
  }

  /// Reads the state in `callback`, where `$functions` resolve against it.
  /// Pass `editor` for the functions that also need the editor.
  public func read<V>(editor: Editor? = nil, _ callback: () throws -> V) throws -> V {
    try readEditorState(editor, self, callback)
  }

  public func clone(_ selection: BaseSelection?? = .none) -> EditorState {
    let editorState = EditorState(
      _nodeMap,
      {
        switch selection {
        case .none: return _selection
        case .some(let selection): return selection
        }
      }(), _slotsUsed)
    editorState._readOnly = true
    editorState._parsed = _parsed
    return editorState
  }

  /// The serialized state: the legacy form, or with `compact` the compact
  /// form, which omits what parsing restores.
  public func toJSON(_ compact: Bool = false) throws -> JSONObject {
    try withCompactExport(compact) {
      try readEditorState(nil, self) {
        try INTERNAL_withExportMemo {
          JSONObject([("root", .object(try exportNodeToJSON(getRoot())))])
        }
      }
    }
  }
}

/// `$isEditorState`.
public func isEditorState(_ x: Any?) -> Bool { x is EditorState }
