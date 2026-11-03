/// A JS `Map`: insertion-ordered, with reference semantics, and iterated live.
/// An iterator visits entries added while it runs and skips entries deleted
/// before it reaches them, which upstream relies on (the dirty-node walks add
/// to the set they are iterating).
///
/// Deleted entries leave a tombstone that is compacted away once no iterator
/// is open, so iteration indices stay valid while one is.
public final class JSMap<Key: Hashable, Value> {
  private var keys: [Key?] = []
  private var values: [Value?] = []
  private var index: [Key: Int] = [:]
  private var tombstones = 0
  fileprivate var openIterators = 0

  public init() {}

  public init<S: Sequence>(_ entries: S) where S.Element == (Key, Value) {
    for (key, value) in entries { set(key, value) }
  }

  public var size: Int { index.count }
  public var isEmpty: Bool { index.isEmpty }

  public func has(_ key: Key) -> Bool { index[key] != nil }

  public func get(_ key: Key) -> Value? {
    guard let i = index[key] else { return nil }
    return values[i]
  }

  public subscript(key: Key) -> Value? {
    get { get(key) }
    set {
      if let newValue { set(key, newValue) } else { delete(key) }
    }
  }

  @discardableResult
  public func set(_ key: Key, _ value: Value) -> JSMap {
    if let i = index[key] {
      values[i] = value
    } else {
      index[key] = keys.count
      keys.append(key)
      values.append(value)
    }
    return self
  }

  @discardableResult
  public func delete(_ key: Key) -> Bool {
    guard let i = index.removeValue(forKey: key) else { return false }
    keys[i] = nil
    values[i] = nil
    tombstones += 1
    compactIfNeeded()
    return true
  }

  public func clear() {
    keys.removeAll()
    values.removeAll()
    index.removeAll()
    tombstones = 0
  }

  /// A copy, like `new Map(map)`.
  public func copy() -> JSMap {
    let copy = JSMap()
    for (key, value) in self { copy.set(key, value) }
    return copy
  }

  public var keysArray: [Key] { compactMap(\.0) }
  public var valuesArray: [Value] { map(\.1) }

  fileprivate func entry(at i: Int) -> (Key, Value)? {
    guard i < keys.count, let key = keys[i] else { return nil }
    return (key, values[i]!)
  }

  fileprivate var storageCount: Int { keys.count }

  fileprivate func compactIfNeeded() {
    guard openIterators == 0, tombstones > 16, tombstones * 2 > keys.count else { return }
    var newKeys: [Key?] = []
    var newValues: [Value?] = []
    newKeys.reserveCapacity(index.count)
    newValues.reserveCapacity(index.count)
    for i in keys.indices {
      if let key = keys[i] {
        index[key] = newKeys.count
        newKeys.append(key)
        newValues.append(values[i])
      }
    }
    keys = newKeys
    values = newValues
    tombstones = 0
  }
}

extension JSMap: Sequence {
  public final class Iterator: IteratorProtocol {
    private let map: JSMap
    private var position = 0

    init(_ map: JSMap) {
      self.map = map
      map.openIterators += 1
    }

    deinit {
      map.openIterators -= 1
      map.compactIfNeeded()
    }

    public func next() -> (Key, Value)? {
      while position < map.storageCount {
        let i = position
        position += 1
        if let entry = map.entry(at: i) { return entry }
      }
      return nil
    }
  }

  public func makeIterator() -> Iterator { Iterator(self) }
}

/// A JS `Set`: insertion-ordered, with reference semantics, iterated live.
public final class JSSet<Element: Hashable> {
  private let storage = JSMap<Element, Void>()

  public init() {}

  public init<S: Sequence>(_ elements: S) where S.Element == Element {
    for element in elements { add(element) }
  }

  public var size: Int { storage.size }
  public var isEmpty: Bool { storage.isEmpty }

  public func has(_ element: Element) -> Bool { storage.has(element) }

  @discardableResult
  public func add(_ element: Element) -> JSSet {
    if !storage.has(element) { storage.set(element, ()) }
    return self
  }

  @discardableResult
  public func delete(_ element: Element) -> Bool { storage.delete(element) }

  public func clear() { storage.clear() }

  public func copy() -> JSSet { JSSet(self) }

  public var array: [Element] { Array(self) }
}

extension JSSet: Sequence {
  public struct Iterator: IteratorProtocol {
    fileprivate let base: JSMap<Element, Void>.Iterator
    public mutating func next() -> Element? { base.next()?.0 }
  }

  public func makeIterator() -> Iterator { Iterator(base: storage.makeIterator()) }
}
