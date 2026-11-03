import Lexical

// Port of packages/lexical-extension/src/internal/blockHandles.ts.
//
// Block handles: the shortest prefix, at least MIN_BLOCK_HANDLE_LENGTH
// characters long, of a hash of a block's id that no other id's hash of the
// document starts with, like a short git hash. Hashing first makes handles
// short and typo-safe whatever the ids look like (random, a timestamp first,
// a counter, @lexical/y's derived ids). Handles are derived from the ids of
// one document and never stored. These helpers are pure: BlockIdExtension
// feeds them the ids and decides which blocks are live.

/// `MIN_BLOCK_HANDLE_LENGTH` (`@experimental`): the shortest handle.
public let MIN_BLOCK_HANDLE_LENGTH = 4

private let ALPHABET: [Character] = Array(
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_")

/// `HANDLE_KEY_LENGTH`: the length of a hash key, and so the longest
/// handle: 64 bits in base64url. A longer string is looked up as a whole id.
package let HANDLE_KEY_LENGTH = 11

/// `handleKey(id)`: the hash key of an id: 64 bits of a fast
/// non-cryptographic hash (cyrb53's mixing, kept to 64 bits), in base64url,
/// most significant bits first. `Math.imul` is a wrapping 32-bit multiply.
package func handleKey(_ id: String) -> String {
  var h1: UInt32 = 0xdead_beef
  var h2: UInt32 = 0x41c6_ce57
  for c in id.utf16 {
    h1 = (h1 ^ UInt32(c)) &* 2_654_435_761
    h2 = (h2 ^ UInt32(c)) &* 1_597_334_677
  }
  h1 = (h1 ^ (h1 >> 16)) &* 2_246_822_507
  h1 ^= (h2 ^ (h2 >> 13)) &* 3_266_489_909
  h2 = (h2 ^ (h2 >> 16)) &* 2_246_822_507
  h2 ^= (h1 ^ (h1 >> 13)) &* 3_266_489_909
  // h2 then h1, 6 bits at a time: 10 whole characters and 4 bits.
  var key = ""
  var bit = 0
  while bit < 64 {
    let value: UInt32
    if bit + 6 <= 32 {
      value = h2 >> UInt32(26 - bit)
    } else if bit >= 32 {
      let shift = 26 - (bit - 32)
      value = shift >= 0 ? h1 >> UInt32(shift) : h1 << UInt32(-shift)
    } else {
      let low = UInt32(bit + 6 - 32)
      value = (h2 << low) | (h1 >> (32 - low))
    }
    key.append(ALPHABET[Int(value & 63)])
    bit += 6
  }
  return key
}

/// `HandleTable`: the handles of a set of ids.
package struct HandleTable {
  /// The distinct hash keys, in ascending UTF-16 code unit order.
  package let sorted: [String]
  /// Hash key → the ids that have it (one, unless the hash collides).
  package let idsOfKey: JSMap<String, [String]>
  /// Id → its handle.
  package let handles: JSMap<String, String>
}

private func commonPrefixLength(_ a: String, _ b: String) -> Int {
  var i = 0
  for (x, y) in zip(a.utf16, b.utf16) {
    if x != y { break }
    i += 1
  }
  return i
}

/// The first `length` UTF-16 code units of `key` (hash keys are ASCII).
private func prefix(_ key: String, _ length: Int) -> String {
  String(key.prefix(length))
}

/// The handle of `key` between its neighbours in sorted order: long enough
/// to differ from both, so from every other key.
private func handleBetween(_ key: String, _ prev: String?, _ next: String?) -> String {
  var length = MIN_BLOCK_HANDLE_LENGTH
  if let prev {
    length = max(length, commonPrefixLength(key, prev) + 1)
  }
  if let next {
    length = max(length, commonPrefixLength(key, next) + 1)
  }
  return prefix(key, length)
}

/// JS `a < b` for strings: UTF-16 code unit order.
package func jsStringLess(_ a: String, _ b: String) -> Bool {
  a.utf16.lexicographicallyPrecedes(b.utf16)
}

/// The first index of `sorted` whose key is not below `value`.
private func lowerBound(_ sorted: [String], _ value: String) -> Int {
  var lo = 0
  var hi = sorted.count
  while lo < hi {
    let mid = (lo + hi) >> 1
    if jsStringLess(sorted[mid], value) {
      lo = mid + 1
    } else {
      hi = mid
    }
  }
  return lo
}

/// `buildHandleTable(ids)`: the handles of `ids` (duplicates allowed), in
/// O(n log n): after sorting their hash keys, only a key's neighbours can
/// share a longer prefix with it than any other key.
package func buildHandleTable<S: Sequence<String>>(_ ids: S) -> HandleTable {
  let idsOfKey = JSMap<String, [String]>()
  for id in JSSet(ids) {
    let key = handleKey(id)
    idsOfKey.set(key, (idsOfKey.get(key) ?? []) + [id])
  }
  let sorted = idsOfKey.keysArray.sorted(by: jsStringLess)
  let handles = JSMap<String, String>()
  for i in sorted.indices {
    let handle = handleBetween(
      sorted[i], i > 0 ? sorted[i - 1] : nil, i + 1 < sorted.count ? sorted[i + 1] : nil)
    for id in idsOfKey.get(sorted[i])! {
      handles.set(id, handle)
    }
  }
  return HandleTable(sorted: sorted, idsOfKey: idsOfKey, handles: handles)
}

/// `handleInTable(table, id)`: the handle of `id` against the ids of
/// `table`, whether or not `id` is one of them.
package func handleInTable(_ table: HandleTable, _ id: String) -> String {
  if let known = table.handles.get(id) {
    return known
  }
  let key = handleKey(id)
  let sorted = table.sorted
  let i = lowerBound(sorted, key)
  let at = i < sorted.count ? sorted[i] : nil
  return handleBetween(
    key, i > 0 ? sorted[i - 1] : nil,
    at == key ? (i + 1 < sorted.count ? sorted[i + 1] : nil) : at)
}

/// `idsWithPrefix(table, prefix)`: the ids of `table` whose hash key starts
/// with `prefix`, by key.
package func idsWithPrefix(_ table: HandleTable, _ prefix: String) -> [String] {
  var result: [String] = []
  var i = lowerBound(table.sorted, prefix)
  while i < table.sorted.count && table.sorted[i].hasPrefix(prefix) {
    result.append(contentsOf: table.idsOfKey.get(table.sorted[i])!)
    i += 1
  }
  return result
}
