import Foundation

/// Locates and decodes the vector files in `packages/lexical-conformance/vectors`.
///
/// The vectors live in the monorepo, outside this package. The mirror
/// repository doesn't carry them, so every conformance suite is skipped when
/// they aren't found.
enum Vectors {
  static let directory: URL? = {
    let url =
      ProcessInfo.processInfo.environment["LEXICAL_CONFORMANCE_VECTORS"].map {
        URL(fileURLWithPath: $0, isDirectory: true)
      }
      ?? repositoryRoot.appendingPathComponent(
        "packages/lexical-conformance/vectors", isDirectory: true)
    return FileManager.default.fileExists(atPath: url.path) ? url : nil
  }()

  static var available: Bool { directory != nil }

  /// The monorepo root, three levels above this file's directory.
  static let repositoryRoot = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent()  // ConformanceTests
    .deletingLastPathComponent()  // Tests
    .deletingLastPathComponent()  // swift
    .deletingLastPathComponent()

  static let supportedFormatVersion = 1

  struct Header: Decodable { let formatVersion: Int }

  struct File<Case: Decodable>: Decodable {
    let formatVersion: Int
    let kind: String
    let description: String
    let cases: [Case]
  }

  /// The cases of `vectors/<name>.json`, which must have a supported
  /// `formatVersion`.
  static func load<Case: Decodable>(_ name: String, as _: Case.Type = Case.self) throws -> [Case] {
    guard let directory else { throw VectorError.missingDirectory }
    let data = try Data(contentsOf: directory.appendingPathComponent("\(name).json"))
    let version = try JSONDecoder().decode(Header.self, from: data).formatVersion
    guard version == supportedFormatVersion else {
      throw VectorError.unsupportedFormat(name: name, version: version)
    }
    return try JSONDecoder().decode(File<Case>.self, from: data).cases
  }
}

enum VectorError: Error, CustomStringConvertible {
  case missingDirectory
  case unsupportedFormat(name: String, version: Int)

  var description: String {
    switch self {
    case .missingDirectory:
      "packages/lexical-conformance/vectors not found; set LEXICAL_CONFORMANCE_VECTORS"
    case .unsupportedFormat(let name, let version):
      "\(name).json has format version \(version); this runner supports \(Vectors.supportedFormatVersion)"
    }
  }
}
