import Lexical

// Port of packages/lexical-extension/src/internal/positionalBlockId.ts.
//
// Positional block ids: the ids BlockIdExtension gives the blocks that a
// document stored without ids leaves unnamed. They are derived from the
// document and the block's place in it, so every reader and every replica of
// the same document derives the same ones. The derivation is part of the
// stored format: changing anything here changes the ids of every document
// that is read without ids, so it is pinned by tests.

/// `ID_ALPHABET`: the 64 characters of ids, in base64url order (RFC 4648,
/// section 5): one character per 6 bits.
package let ID_ALPHABET: [Character] = Array(
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_")

/// `ID_LENGTH`: characters in a generated or positional id: 126 bits.
package let ID_LENGTH = 21

// SHA-256 (FIPS 180-4) round constants: the first 32 bits of the fractional
// parts of the cube roots of the first 64 primes.
private let K: [UInt32] = [
  0x428a_2f98, 0x7137_4491, 0xb5c0_fbcf, 0xe9b5_dba5, 0x3956_c25b, 0x59f1_11f1,
  0x923f_82a4, 0xab1c_5ed5, 0xd807_aa98, 0x1283_5b01, 0x2431_85be, 0x550c_7dc3,
  0x72be_5d74, 0x80de_b1fe, 0x9bdc_06a7, 0xc19b_f174, 0xe49b_69c1, 0xefbe_4786,
  0x0fc1_9dc6, 0x240c_a1cc, 0x2de9_2c6f, 0x4a74_84aa, 0x5cb0_a9dc, 0x76f9_88da,
  0x983e_5152, 0xa831_c66d, 0xb003_27c8, 0xbf59_7fc7, 0xc6e0_0bf3, 0xd5a7_9147,
  0x06ca_6351, 0x1429_2967, 0x27b7_0a85, 0x2e1b_2138, 0x4d2c_6dfc, 0x5338_0d13,
  0x650a_7354, 0x766a_0abb, 0x81c2_c92e, 0x9272_2c85, 0xa2bf_e8a1, 0xa81a_664b,
  0xc24b_8b70, 0xc76c_51a3, 0xd192_e819, 0xd699_0624, 0xf40e_3585, 0x106a_a070,
  0x19a4_c116, 0x1e37_6c08, 0x2748_774c, 0x34b0_bcb5, 0x391c_0cb3, 0x4ed8_aa4a,
  0x5b9c_ca4f, 0x682e_6ff3, 0x748f_82ee, 0x78a5_636f, 0x84c8_7814, 0x8cc7_0208,
  0x90be_fffa, 0xa450_6ceb, 0xbef9_a3f7, 0xc671_78f2,
]

private func rotr(_ x: UInt32, _ n: UInt32) -> UInt32 {
  (x >> n) | (x << (32 - n))
}

/// Runs the compression function over the 64-byte block at `offset`.
private func compress(
  _ h: inout [UInt32], _ w: inout [UInt32], _ bytes: UnsafeBufferPointer<UInt8>, _ offset: Int
) {
  for t in 0..<16 {
    let i = offset + t * 4
    w[t] =
      (UInt32(bytes[i]) << 24) | (UInt32(bytes[i + 1]) << 16) | (UInt32(bytes[i + 2]) << 8)
      | UInt32(bytes[i + 3])
  }
  for t in 16..<64 {
    let x = w[t - 15]
    let y = w[t - 2]
    let s0 = rotr(x, 7) ^ rotr(x, 18) ^ (x >> 3)
    let s1 = rotr(y, 17) ^ rotr(y, 19) ^ (y >> 10)
    w[t] = w[t - 16] &+ s0 &+ w[t - 7] &+ s1
  }
  var a = h[0]
  var b = h[1]
  var c = h[2]
  var d = h[3]
  var e = h[4]
  var f = h[5]
  var g = h[6]
  var hh = h[7]
  for t in 0..<64 {
    let s1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)
    let ch = (e & f) ^ (~e & g)
    let t1 = hh &+ s1 &+ ch &+ K[t] &+ w[t]
    let s0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)
    let maj = (a & b) ^ (a & c) ^ (b & c)
    let t2 = s0 &+ maj
    hh = g
    g = f
    f = e
    e = d &+ t1
    d = c
    c = b
    b = a
    a = t1 &+ t2
  }
  h[0] = h[0] &+ a
  h[1] = h[1] &+ b
  h[2] = h[2] &+ c
  h[3] = h[3] &+ d
  h[4] = h[4] &+ e
  h[5] = h[5] &+ f
  h[6] = h[6] &+ g
  h[7] = h[7] &+ hh
}

/// The SHA-256 state words after hashing `message`. JS reuses scratch
/// buffers across calls; Swift allocates them per call.
private func sha256Words(_ message: [UInt8]) -> [UInt32] {
  let length = message.count
  var h: [UInt32] = [
    0x6a09_e667, 0xbb67_ae85, 0x3c6e_f372, 0xa54f_f53a, 0x510e_527f, 0x9b05_688c, 0x1f83_d9ab,
    0x5be0_cd19,
  ]
  var w = [UInt32](repeating: 0, count: 64)
  let full = length - (length % 64)
  message.withUnsafeBufferPointer { bytes in
    var offset = 0
    while offset < full {
      compress(&h, &w, bytes, offset)
      offset += 64
    }
  }
  // The rest of the message, a 1 bit, zeros, and the 64-bit length.
  var buffer = [UInt8](repeating: 0, count: 128)
  let end = length - full < 56 ? 64 : 128
  for i in full..<length {
    buffer[i - full] = message[i]
  }
  buffer[length - full] = 0x80
  let bits = UInt64(length) * 8
  for i in 0..<8 {
    buffer[end - 1 - i] = UInt8(truncatingIfNeeded: bits >> (8 * UInt64(i)))
  }
  buffer.withUnsafeBufferPointer { bytes in
    var offset = 0
    while offset < end {
      compress(&h, &w, bytes, offset)
      offset += 64
    }
  }
  return h
}

/// `sha256(message)`: the SHA-256 digest of `message`. Synchronous, because
/// ids are derived inside editor updates.
package func sha256(_ message: [UInt8]) -> [UInt8] {
  let h = sha256Words(message)
  var digest = [UInt8](repeating: 0, count: 32)
  for i in 0..<32 {
    digest[i] = UInt8(truncatingIfNeeded: h[i >> 2] >> (24 - UInt32(i & 3) * 8))
  }
  return digest
}

/// `utf8(text)`: the UTF-8 bytes of `text`. JS turns a lone surrogate into
/// U+FFFD, as `TextEncoder` does; a Swift string holds none.
package func utf8(_ text: String) -> [UInt8] {
  Array(text.utf8)
}

/// A deep copy of `value` whose objects have their keys inserted sorted, in
/// UTF-16 code unit order.
private func sortKeys(_ value: JSONValue) -> JSONValue {
  switch value {
  case .array(let items):
    return .array(items.map(sortKeys))
  case .object(let record):
    var sorted = JSONObject()
    let keys = record.keys.sorted { $0.utf16.lexicographicallyPrecedes($1.utf16) }
    for key in keys {
      sorted.set(key, record[key].map(sortKeys))
    }
    return .object(sorted)
  default:
    return value
  }
}

/// `canonicalJSON(value)`: `JSON.stringify` (no whitespace) of a deep copy
/// of `value` in which the keys of every object were inserted in UTF-16 code
/// unit order. A JS object (and `JSONObject`) then writes the keys that are
/// array indices first, in numeric order, then the others in that order.
package func canonicalJSON(_ value: JSONValue) -> String {
  sortKeys(value).stringify()
}

/// A node's compact JSON with its children and slots, without its block id
/// (and without a `$` that held only that).
private func nodeJSON(_ node: LexicalNode) throws -> JSONObject {
  var json = try exportNodeJSON(node)
  if case .object(var state)? = json["$"], state.has("blockId") {
    state.remove("blockId")
    if !state.keys.isEmpty {
      json["$"] = .object(state)
    } else {
      json.remove("$")
    }
  }
  if let element = node as? ElementNode {
    json["children"] = .array(try element.getChildren().map { .object(try nodeJSON($0)) })
  }
  let names = try getSlotNames(node)
  if !names.isEmpty {
    var slots = JSONObject()
    for name in names {
      if let slot = try getSlot(node, name) {
        slots[name] = .object(try nodeJSON(slot))
      }
    }
    json["$slots"] = .object(slots)
  }
  return json
}

/// `$canonicalDocument()`: the active document as the canonical JSON its key
/// is hashed from: the compact export of the root, without any `blockId`
/// (and without a `$` that held only that), in `canonicalJSON`.
package func canonicalDocument() throws -> String {
  try withCompactExport(true) { canonicalJSON(.object(try nodeJSON(getRoot()))) }
}

/// `$documentKey()`: the key of the active document: SHA-256 of the UTF-8
/// bytes of `canonicalDocument()`. Ids are left out, so naming blocks does
/// not change it.
package func documentKey() throws -> [UInt8] {
  sha256(utf8(try canonicalDocument()))
}

/// An id from the SHA-256 of `input`: the first 21 characters (126 bits) of
/// the digest's unpadded base64url encoding.
private func idFromDigestOf(_ input: [UInt8]) -> String {
  let h = sha256Words(input)
  // The digest's bits, 6 at a time from the most significant.
  var id = ""
  var bit = 0
  while id.count < ID_LENGTH {
    let word = bit >> 5
    let offset = UInt32(bit & 31)
    let pair =
      offset <= 26
      ? h[word] >> (26 - offset)
      : (h[word] << (offset - 26)) | (h[word + 1] >> (58 - offset))
    id.append(ID_ALPHABET[Int(pair & 63)])
    bit += 6
  }
  return id
}

/// `derivePositionalId(documentKey, place)`: the positional id of the block
/// at `place` in the document with key `documentKey`: the first 21
/// characters of the unpadded base64url encoding of
/// SHA-256(`documentKey` ‖ UTF-8(`place`)). `place` is the block's path
/// among identified blocks, its indices joined by `.` (`"3"`, `"0.2"`), with
/// `#n` appended for the n-th retry after a collision.
package func derivePositionalId(_ documentKey: [UInt8], _ place: String) -> String {
  idFromDigestOf(documentKey + utf8(place))
}

/// `deriveDuplicateId(id, ordinal)`: the id of the k-th later holder of the
/// repeated id `id`: the first 21 characters of the unpadded base64url
/// encoding of SHA-256(UTF-8(`"dup"` + U+0000 + `id` + U+0000 + `ordinal`)).
package func deriveDuplicateId(_ id: String, _ ordinal: String) -> String {
  idFromDigestOf(utf8("dup\u{0000}\(id)\u{0000}\(ordinal)"))
}
