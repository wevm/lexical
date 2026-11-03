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
