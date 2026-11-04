import Testing

@testable import Lexical

// `JSMap` and `JSONObject` hash string keys themselves (`FastHashKey`):
// equal keys must find each other whatever their storage, and keys compare
// by code units, as a JS `Map` or object does.
@Suite struct JSCollectionsKeyTests {
  /// `"é"` precomposed and decomposed: `==` as Swift strings, distinct in JS.
  let composed = "\u{E9}"
  let decomposed = "e\u{301}"

  @Test func keysCompareByCodeUnits() throws {
    #expect(composed == decomposed)
    let map = JSMap<String, Int>()
    map.set(composed, 1)
    map.set(decomposed, 2)
    #expect(map.size == 2)
    #expect(map.get(composed) == 1)
    #expect(map.get(decomposed) == 2)

    var object = JSONObject()
    object[composed] = 1
    object[decomposed] = 2
    #expect(object.keys == [composed, decomposed])
  }

  @Test func shortKeysInLargeStorageFindTheirSmallTwins() throws {
    for key in ["", "1", "abc", "fifteen bytes!!", "é😀"] {
      // `reserveCapacity` moves the string to heap storage, which a short
      // literal never uses.
      var large = key
      large.reserveCapacity(64)
      let bits = unsafeBitCast(large, to: (UInt64, UInt64).self)
      try #require(bits.1 & 0x2000_0000_0000_0000 == 0, "not in large storage")
      let map = JSMap<String, Int>()
      map.set(key, 1)
      #expect(map.get(large) == 1)
      map.set(large, 2)
      #expect(map.size == 1 && map.get(key) == 2)
    }
  }

  @Test func longKeys() {
    let map = JSMap<String, Int>()
    let keys = (0..<1_000).map { "a long key, more than fifteen bytes, number \($0)" }
    for (i, key) in keys.enumerated() { map.set(key, i) }
    #expect(map.size == keys.count)
    for (i, key) in keys.enumerated() { #expect(map.get(key) == i) }
    #expect(map.get("a long key, more than fifteen bytes, number 1000") == nil)
  }
}
