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

  /// What `element.style.setProperty(name, value)` reads back, as
  /// `css.test.ts`'s `probe` asks jsdom.
  private func probe(_ name: String, _ value: String) -> String {
    let element = DOMDocument().createElement("div")
    element.style.setProperty(name, value)
    return element.style.getPropertyValue(name)
  }

  /// Each property behaves as the probes `css.test.ts` ran in jsdom found,
  /// not only as the table says.
  @Test func propertiesNormalizeAsJSDOM() throws {
    for c in try Vectors.load("css/properties", as: PropertyCase.self) {
      #expect(!probe(c.name, "initial").isEmpty, "\(c.name) accepts initial")
      let color =
        probe(c.name, "#f00") == "rgb(255, 0, 0)"
        || probe(c.name, "1px solid #f00") == "1px solid rgb(255, 0, 0)"
      #expect(color == c.color, "\(c.name) color")
      #expect((probe(c.name, "Foo") == "Foo") == c.preserveCase, "\(c.name) preserveCase")
      #expect((probe(c.name, "1PX 1px RED") == "1PX 1px RED") == c.verbatim, "\(c.name) verbatim")
      #expect((probe(c.name, "0") == "0px") == c.zeroLength, "\(c.name) zeroLength")
    }
  }

  /// jsdom lowercases a CSS-wide keyword for every property, and rejects a
  /// shadow of nothing but identifiers.
  @Test func cssWideKeywordsAndShadows() {
    #expect(probe("font-family", "INHERIT") == "inherit")
    #expect(probe("box-shadow", "Unset") == "unset")
    #expect(probe("box-shadow", "NONE") == "none")
    #expect(probe("box-shadow", "Foo") == "")
    #expect(probe("text-shadow", "red, blue") == "")
    #expect(probe("text-shadow", "1px 1px RED") == "1px 1px RED")
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
