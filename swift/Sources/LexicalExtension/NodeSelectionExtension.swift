import Lexical

// Port of packages/lexical-extension/src/NodeSelectionExtension.ts.

/// The output of `NodeSelectionExtension`.
public struct NodeSelectionOutput {
  /// `watchNodeKey(key)`: a signal of whether the node with `key` is
  /// selected, kept up to date while something watches it.
  public let watchNodeKey: (NodeKey) -> ReadonlySignal<Bool>
}

/// The watched keys and their watchers. A new box is stored whenever a key
/// is added, so the signal holding it notifies (JS stores a new object).
private final class WatchedNodeKeys {
  let watchedNodeKeys: JSMap<NodeKey, [ReadonlySignal<Bool>]>
  init(_ watchedNodeKeys: JSMap<NodeKey, [ReadonlySignal<Bool>]>) {
    self.watchedNodeKeys = watchedNodeKeys
  }
}

/// `NodeSelectionExtension`: tracks whether watched nodes are selected.
public let NodeSelectionExtension = defineExtension(
  name: "@lexical/extension/NodeSelection", dependencies: [EditorStateExtension],
  build: { editor, _, state -> NodeSelectionOutput in
    let editorStateStore = try state.getDependency(EditorStateExtension).output
    let watchedNodeStore = signal(WatchedNodeKeys(JSMap()))
    let selectedNodeKeys = watchedSignal(
      { () -> Set<NodeKey>? in nil },
      { selectedNodeKeys in
        (try? effect {
          let prevSelectedNodeKeys = selectedNodeKeys.peek()
          let watchedNodeKeys = watchedNodeStore.value.watchedNodeKeys
          var nextSelectedNodeKeys: Set<NodeKey>?
          var didChange = false
          try editorStateStore.value.read {
            guard try getSelection() != nil else { return }
            for (key, listeners) in Array(watchedNodeKeys) {
              if listeners.isEmpty {
                // Mutated without firing a signal, to avoid re-triggering
                // this effect: there are no subscribers.
                _ = watchedNodeKeys.delete(key)
                continue
              }
              let node = try getNodeByKey(key)
              let isSelected = try node?.isSelected() ?? false
              didChange =
                didChange || isSelected != (prevSelectedNodeKeys?.contains(key) ?? false)
              if isSelected {
                nextSelectedNodeKeys = nextSelectedNodeKeys ?? Set()
                nextSelectedNodeKeys!.insert(key)
              }
            }
          }
          if !(!didChange && nextSelectedNodeKeys != nil && prevSelectedNodeKeys != nil
            && nextSelectedNodeKeys!.count == prevSelectedNodeKeys!.count)
          {
            try selectedNodeKeys.set(nextSelectedNodeKeys)
          }
        }) ?? {}
      })
    func watchNodeKey(_ key: NodeKey) -> ReadonlySignal<Bool> {
      let watcher = computed { (selectedNodeKeys.value ?? []).contains(key) }
      let watchedNodeKeys = watchedNodeStore.peek().watchedNodeKeys
      let listeners = watchedNodeKeys.get(key)
      _ = watchedNodeKeys.set(key, (listeners ?? []) + [watcher])
      if listeners == nil {
        watchedNodeStore.value = WatchedNodeKeys(watchedNodeKeys)
      }
      return watcher
    }
    return NodeSelectionOutput(watchNodeKey: watchNodeKey)
  })
