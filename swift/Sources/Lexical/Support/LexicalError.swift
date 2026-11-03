/// An error Lexical raises where the JS implementation throws: a failed
/// `invariant`, or an `Error` thrown by a command, transform, or listener.
///
/// JS invariants become Swift `throws`, so `Editor.update` can recover the way
/// the JS editor does: it discards the pending state and reports the error to
/// `onError`.
public struct LexicalError: Error, CustomStringConvertible, Equatable, Sendable {
  public let message: String

  public init(_ message: String) {
    self.message = message
  }

  public var description: String { message }
}

/// `invariant(cond, message)` from `@lexical/internal/invariant`.
@inline(__always)
package func invariant(
  _ condition: Bool, _ message: @autoclosure () -> String
) throws {
  if !condition {
    throw LexicalError(message())
  }
}

/// `devInvariant` from `@lexical/internal/devInvariant`. JS throws in
/// development builds and warns in production. The port follows the
/// development build, which is the one the upstream tests and the
/// conformance vectors run.
@inline(__always)
package func devInvariant(
  _ condition: Bool, _ message: @autoclosure () -> String
) throws {
  if !condition {
    throw LexicalError(message())
  }
}

/// `LexicalWarning`: a warning passed to `onWarn`, with a code namespaced by
/// the package that reports it and optional details for programs (the
/// message is for people and may change).
public struct LexicalWarning: Error, CustomStringConvertible, Equatable, Sendable {
  public let name = "LexicalWarning"
  public let code: String
  public let message: String
  public let data: JSONValue?

  public init(_ code: String, _ message: String, data: JSONValue? = nil) {
    self.code = code
    self.message = message
    self.data = data
  }

  /// `String(warning)`: `LexicalWarning: <message>`.
  public var description: String { "\(name): \(message)" }
}
