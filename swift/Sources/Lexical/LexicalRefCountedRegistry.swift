/// `RefCountedRegistry`: registrations reference counted by key. The first
/// registration of a key runs the activation; the disposer it returns runs
/// once the last registration for that key is released. `options` are read
/// only on the activating registration.
public final class RefCountedRegistry<Key: Hashable, Options> {
  private final class Entry {
    // Each registration's disposer has its own token; the activation is torn
    // down once the set empties.
    var holders: Set<ObjectIdentifier> = []
    let dispose: () -> Void
    init(_ dispose: @escaping () -> Void) { self.dispose = dispose }
  }
  private final class Token {}

  private let activate: (Key, Options?) -> () -> Void
  private let entries = JSMap<Key, Entry>()

  init(_ activate: @escaping (Key, Options?) -> () -> Void) {
    self.activate = activate
  }

  /// Registers `key` and returns an idempotent disposer. A stale disposer (a
  /// second call, or one after teardown or re-registration) does nothing.
  public func register(_ key: Key, _ options: Options? = nil) -> () -> Void {
    let entry: Entry
    if let existing = entries.get(key) {
      entry = existing
    } else {
      entry = Entry(activate(key, options))
      entries.set(key, entry)
    }
    let token = Token()
    let id = ObjectIdentifier(token)
    entry.holders.insert(id)
    return { [weak self] in
      _ = token
      guard let self, let current = self.entries.get(key),
        current.holders.remove(id) != nil, current.holders.isEmpty
      else { return }
      self.entries.delete(key)
      current.dispose()
    }
  }

  /// Disposes every live registration and clears the registry.
  public func dispose() {
    for (_, entry) in entries {
      entry.dispose()
    }
    entries.clear()
  }
}

/// `createRefCountedRegistry(activate)`.
public func createRefCountedRegistry<Key: Hashable, Options>(
  _ activate: @escaping (Key, Options?) -> () -> Void
) -> RefCountedRegistry<Key, Options> {
  RefCountedRegistry(activate)
}

/// `createRefCountedRegistry(activate)` without options.
public func createRefCountedRegistry<Key: Hashable>(
  _ activate: @escaping (Key) -> () -> Void
) -> RefCountedRegistry<Key, Void> {
  RefCountedRegistry { key, _ in activate(key) }
}
