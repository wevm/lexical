import Lexical
import Testing

/// Replays `vectors/html/parse.json` and `vectors/html/entities.json`: the
/// Swift `DOMParser` builds the same documents as jsdom's.
@Suite(.enabled(if: Vectors.available))
struct HTMLParseTests {
  struct Case: Decodable, CustomTestStringConvertible, Sendable {
    let html: String
    let head: String
    let body: String
    var testDescription: String { html.isEmpty ? "(empty)" : String(html.prefix(60)) }
  }

  struct EntityCase: Decodable, Sendable {
    let name: String
    let value: String
    let legacy: Bool
  }

  static let cases: [Case] = (try? Vectors.load("html/parse")) ?? []

  @Test func vectorsLoad() throws {
    #expect(try Vectors.load("html/parse", as: Case.self).count == Self.cases.count)
    #expect(!Self.cases.isEmpty)
  }

  @Test(arguments: cases)
  func parse(_ vector: Case) throws {
    let document = DOMParser().parseFromString(vector.html, "text/html")
    #expect(document.head?.innerHTML == vector.head)
    #expect(document.body?.innerHTML == vector.body)
  }

  /// `HTMLEntities.swift` is generated from the vectors; regenerate it with
  /// `pnpm --filter lexical-conformance run html-entities`.
  @Test func entityTableMatchesVectors() throws {
    let cases = try Vectors.load("html/entities", as: EntityCase.self)
    #expect(
      htmlNamedEntities == Dictionary(uniqueKeysWithValues: cases.map { ($0.name, $0.value) }))
    #expect(htmlLegacyEntityNames == Set(cases.filter(\.legacy).map(\.name)))
  }

  @Test func entitiesDecode() throws {
    for c in try Vectors.load("html/entities", as: EntityCase.self) {
      // Without its semicolon, a name that isn't legacy may still start
      // with one that is (`&centerdot` is `&cent` + "erdot").
      let html = c.legacy ? "<p>&\(c.name);|&\(c.name)</p>" : "<p>&\(c.name);</p>"
      let expected = c.legacy ? c.value + "|" + c.value : c.value
      #expect(DOMParser().parseFromString(html).body?.textContent == expected, "\(c.name)")
    }
  }
}
