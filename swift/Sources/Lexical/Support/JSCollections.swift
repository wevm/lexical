/// A JS `Map`: insertion-ordered, with reference semantics, and iterated live.
/// An iterator visits entries added while it runs and skips entries deleted
/// before it reaches them, which upstream relies on (the dirty-node walks add
/// to the set they are iterating).
///
/// Deleted entries leave a tombstone that is compacted away once no iterator
/// is open, so iteration indices stay valid while one is.
public final class JSMap<Key: Hashable, Value> {
  // Every access to the storage is a plain read or write inside one of
  // these methods, none of which calls out while it holds one, so the
  // dynamic exclusivity checks (a large share of an update's time) are off.
  @exclusivity(unchecked) private var keys: [Key?] = []
  @exclusivity(unchecked) private var values: [Value?] = []
  @exclusivity(unchecked) private var index: [FastHashKey<Key>: Int] = [:]
  @exclusivity(unchecked) private var tombstones = 0
  @exclusivity(unchecked) fileprivate var openIterators = 0
  /// Set on a committed node map, which JS's dev build freezes
  /// (`handleDEVOnlyPendingUpdateGuarantees`); the node map writers check it.
  package var _frozen = false

  public init() {}

  public init<S: Sequence>(_ entries: S) where S.Element == (Key, Value) {
    for (key, value) in entries { set(key, value) }
  }

  public var size: Int { index.count }
  public var isEmpty: Bool { index.isEmpty }

  public func has(_ key: Key) -> Bool { index[FastHashKey(key)] != nil }

  public func get(_ key: Key) -> Value? {
    guard let i = index[FastHashKey(key)] else { return nil }
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
    let count = keys.count
    // One hash: the defaulted subscript's modify inserts `count` if missing.
    let i = slot(&index[FastHashKey(key), default: count])
    if i < count {
      values[i] = value
    } else {
      keys.append(key)
      values.append(value)
    }
    return self
  }

  private func slot(_ value: inout Int) -> Int { value }

  @discardableResult
  public func delete(_ key: Key) -> Bool {
    guard let i = index.removeValue(forKey: FastHashKey(key)) else { return false }
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

  /// A copy, like `new Map(map)`. It shares the storage until either side
  /// writes, so the copy never rehashes; tombstones and order carry over.
  public func copy() -> JSMap {
    let copy = JSMap()
    copy.keys = keys
    copy.values = values
    copy.index = index
    copy.tombstones = tombstones
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
        index[FastHashKey(key)] = newKeys.count
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

/// A dictionary key that hashes a `String` with a cheap non-cryptographic
/// hash instead of SipHash, which dominated node-map lookups: Swift hashes a
/// string on every lookup, where JS caches the hash in the string. Node keys
/// and JSON property names are short, so they are usually small strings,
/// whose 15 bytes of UTF-8 are the string's own two words.
///
/// A `String` key compares by code units, as a JS `Map` or object does, not
/// by Unicode canonical equivalence as `String ==` does; the hash covers the
/// same bytes. Other keys hash and compare as usual.
@usableFromInline
package struct FastHashKey<Key: Hashable>: Hashable {
  @usableFromInline package let key: Key

  @inlinable package init(_ key: Key) { self.key = key }

  @inlinable @_optimize(speed) package static func == (lhs: Self, rhs: Self) -> Bool {
    guard Key.self == String.self else { return lhs.key == rhs.key }
    let a = unsafeBitCast(lhs.key, to: String.self)
    let b = unsafeBitCast(rhs.key, to: String.self)
    #if !_pointerBitWidth(_64)
      return a.utf8.elementsEqual(b.utf8)
    #else
      let aBits = unsafeBitCast(a, to: (UInt64, UInt64).self)
      let bBits = unsafeBitCast(b, to: (UInt64, UInt64).self)
      if aBits.0 == bBits.0 && aBits.1 == bBits.1 { return true }
      // A small string's bits are its contents, so two that differ differ.
      if aBits.1 & smallStringFlag != 0, bBits.1 & smallStringFlag != 0 { return false }
      return a.utf8.elementsEqual(b.utf8)
    #endif
  }

  @inlinable @_optimize(speed) package func hash(into hasher: inout Hasher) {
    hasher.combine(_rawHashValue(seed: 0))
  }

  @inlinable @_optimize(speed) package func _rawHashValue(seed: Int) -> Int {
    guard Key.self == String.self else { return key._rawHashValue(seed: seed) }
    var string = unsafeBitCast(key, to: String.self)
    var h: UInt64
    #if _pointerBitWidth(_64)
      let bits = unsafeBitCast(string, to: (UInt64, UInt64).self)
      let isSmall = bits.1 & smallStringFlag != 0
    #else
      // Another layout: every string hashes its bytes.
      let bits: (UInt64, UInt64) = (0, 0)
      let isSmall = false
    #endif
    if isSmall {
      // The UTF-8 bytes, little-endian and zero-padded, with the count and
      // flags in the top byte cleared.
      h = mixWords(bits.0, bits.1 & 0x00FF_FFFF_FFFF_FFFF)
    } else {
      h = string.withUTF8 { bytes in
        guard let base = bytes.baseAddress else { return mixWords(0, 0) }
        let count = bytes.count
        func load(_ from: Int, _ count: Int) -> UInt64 {
          var word: UInt64 = 0
          for i in 0..<count { word |= UInt64(base[from + i]) << (8 * UInt64(i)) }
          return word
        }
        if count <= 15 {
          // As the small form would hash.
          return mixWords(load(0, min(count, 8)), load(8, max(count - 8, 0)))
        }
        var h = mixWords(UInt64(count), 0)
        var i = 0
        while i + 8 <= count {
          h = mixWords(h, UnsafeRawPointer(base + i).loadUnaligned(as: UInt64.self))
          i += 8
        }
        return mixWords(h, load(i, count - i))
      }
    }
    h = mixWords(h, UInt64(UInt(bitPattern: seed)))
    return Int(truncatingIfNeeded: h)
  }
}

/// `_StringObject`'s small-string discriminator bit on 64-bit platforms,
/// where a `String` is two words and a small one (up to 15 UTF-8 bytes)
/// holds its bytes in them.
@usableFromInline let smallStringFlag: UInt64 = 0x2000_0000_0000_0000

@inlinable @_optimize(speed) func mixWords(_ a: UInt64, _ b: UInt64) -> UInt64 {
  var h = (a ^ 0x9E37_79B9_7F4A_7C15) &* 0xff51_afd7_ed55_8ccd
  h ^= b &+ (h >> 29)
  h = h &* 0xc4ce_b9fe_1a85_ec53
  h ^= h >> 32
  return h
}

extension FastHashKey: Sendable where Key: Sendable {}

extension Dictionary {
  /// The value for `key` in a dictionary keyed by `FastHashKey`.
  @inlinable package subscript<K>(key: K) -> Value? where Key == FastHashKey<K> {
    get { self[FastHashKey(key)] }
    set { self[FastHashKey(key)] = newValue }
  }
}
