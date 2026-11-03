import Lexical
import LexicalExtension
import LexicalTesting
import Testing

// Port of packages/lexical-extension/src/__tests__/unit/deepThemeMergeInPlace.test.ts.
// Swift themes are `[String: JSONValue]` values, so there is no prototype
// to pollute; the reserved keys are still skipped as in JS.

@Suite struct DeepThemeMergeInPlaceTests {
  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/deepThemeMergeInPlace.test.ts::deepThemeMergeInPlace > merges recursively"
    ))
  func mergesRecursively() {
    var a: EditorThemeClasses = ["a": "a", "nested": ["a": 1]]
    deepThemeMergeInPlace(&a, ["b": "b", "nested": ["b": 2]])
    #expect(a == ["a": "a", "b": "b", "nested": ["a": 1, "b": 2]])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/deepThemeMergeInPlace.test.ts::deepThemeMergeInPlace > does not pollute the prototype via __proto__"
    ))
  func skipsProto() throws {
    guard
      case .object(let malicious) = try JSONValue.parse(
        #"{"paragraph":"x","__proto__":{"polluted":1}}"#)
    else { Issue.record("not an object"); return }
    var target: EditorThemeClasses = ["paragraph": "base"]
    deepThemeMergeInPlace(
      &target, Dictionary(uniqueKeysWithValues: malicious.map { ($0.key, $0.value) }))
    #expect(target == ["paragraph": "x"])
  }

  @Test(
    .upstream(
      "lexical-extension/src/__tests__/unit/deepThemeMergeInPlace.test.ts::deepThemeMergeInPlace > does not pollute via constructor/prototype keys"
    ))
  func skipsConstructorAndPrototype() {
    var target: EditorThemeClasses = [:]
    deepThemeMergeInPlace(
      &target, ["constructor": ["prototype": ["polluted": 1]], "prototype": ["polluted": 1]])
    #expect(target.isEmpty)
  }
}
