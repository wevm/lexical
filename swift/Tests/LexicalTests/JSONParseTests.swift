import Testing

@testable import Lexical

// `JSONValue.parse` reads the text's UTF-8 but must read what JS's
// `JSON.parse` reads from its UTF-16: these are the cases where the two
// encodings, or the object fast path, could disagree.
@Suite struct JSONParseTests {
  @Test func repeatedKeyKeepsFirstPositionAndLastValue() throws {
    let value = try JSONValue.parse(#"{"a":1,"b":2,"a":3}"#)
    #expect(value.objectValue?.keys == ["a", "b"])
    #expect(value.stringify() == #"{"a":3,"b":2}"#)
  }

  @Test func arrayIndexKeysComeFirstInAscendingOrder() throws {
    let value = try JSONValue.parse(#"{"b":1,"10":2,"2":3,"01":4}"#)
    #expect(value.stringify() == #"{"2":3,"10":2,"b":1,"01":4}"#)
  }

  @Test func largeObjectFindsEveryKey() throws {
    let keys = (0..<40).map { "key\($0)" }
    let text = "{" + keys.enumerated().map { #""\#($1)":\#($0)"# }.joined(separator: ",") + "}"
    var object = try #require(try JSONValue.parse(text).objectValue)
    for (i, key) in keys.enumerated() { #expect(object[key] == .int(i)) }
    object.remove("key3")
    object["key3"] = "again"
    #expect(object.keys.last == "key3")
    #expect(object["key4"] == .int(4))
    #expect(!object.has("key40"))
  }

  @Test func stringsWithAndWithoutEscapes() throws {
    #expect(try JSONValue.parse(#""héllo 😀""#) == "héllo 😀")
    #expect(
      try JSONValue.parse(#""a\"b\\c\/d\n\t \u00e9 \ud83d\ude00 é😀""#)
        == "a\"b\\c/d\n\t é 😀 é😀")
  }

  @Test func numbers() throws {
    let negativeZero = try #require(try JSONValue.parse("-0").numberValue)
    #expect(negativeZero == 0 && negativeZero.sign == .minus)
    #expect(try JSONValue.parse("123456789012345") == .number(123_456_789_012_345))
    #expect(try JSONValue.parse("12345678901234567890") == .number(12_345_678_901_234_567_890))
    #expect(try JSONValue.parse("-42") == .number(-42))
    #expect(try JSONValue.parse("1.5e3") == .number(1500))
    #expect(try JSONValue.parse("0.1") == .number(0.1))
  }

  @Test func errorPositionsCountUTF16CodeUnits() {
    // `é` is two UTF-8 bytes and one UTF-16 unit, `😀` four and two.
    #expect(throws: LexicalError.self) { try JSONValue.parse(#"{"é😀": x}"#) }
    do {
      _ = try JSONValue.parse(#"{"é😀": x}"#)
    } catch {
      #expect("\(error)".contains("position 8"))
    }
    #expect(throws: LexicalError.self) { try JSONValue.parse("\"a\u{1}\"") }
    #expect(throws: LexicalError.self) { try JSONValue.parse(#""abc"#) }
  }
}
