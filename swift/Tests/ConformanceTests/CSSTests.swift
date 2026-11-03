import Lexical
import Testing

@Suite(.enabled(if: Vectors.available))
struct CSSTests {
  struct PropertyCase: Decodable {
    let name: String
    let color: Bool
    let zeroLength: Bool
    let preserveCase: Bool
    let verbatim: Bool
  }

  struct ValueCase: Decodable {
    let css: String
    let cssText: String
  }

  /// `CSSProperties.swift` is generated from the vectors; regenerate it
  /// with `pnpm --filter lexical-conformance run css-properties`.
  @Test func propertyTableMatchesVectors() throws {
    let cases = try Vectors.load("css/properties", as: PropertyCase.self)
    var expected: [String: UInt8] = [:]
    for c in cases {
      expected[c.name] =
        (c.color ? 1 : 0) | (c.zeroLength ? 2 : 0) | (c.preserveCase ? 4 : 0) | (c.verbatim ? 8 : 0)
    }
    #expect(cssPropertyTraitsByName == expected)
  }

  @Test func cssTextNormalizesAsJSDOM() throws {
    for c in try Vectors.load("css/values", as: ValueCase.self) {
      let element = DOMDocument().createElement("span")
      element.style.cssText = c.css
      #expect(element.style.cssText == c.cssText, "\(c.css)")
      #expect(element.getAttribute("style") == c.cssText)
    }
  }
}
