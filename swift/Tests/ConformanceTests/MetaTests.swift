import Foundation
import Lexical
import Testing

@Suite(.enabled(if: Vectors.available))
struct MetaTests {
  struct Meta: Decodable { let lexicalVersion: String }

  /// The vectors, `swift/UPSTREAM_VERSION`, and `lexicalUpstreamVersion` all
  /// name the same upstream release, so an upstream sync can't move one alone.
  @Test func upstreamVersionMatchesVectors() throws {
    let meta = try #require(try Vectors.load("meta", as: Meta.self).first)
    let declared = try String(
      contentsOf: Vectors.repositoryRoot.appendingPathComponent("swift/UPSTREAM_VERSION"),
      encoding: .utf8
    ).trimmingCharacters(in: .whitespacesAndNewlines)
    #expect(meta.lexicalVersion == declared)
    #expect(lexicalUpstreamVersion == declared)
  }
}
