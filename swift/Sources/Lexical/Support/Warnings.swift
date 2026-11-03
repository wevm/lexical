#if canImport(Darwin)
  import Darwin
#elseif canImport(Glibc)
  import Glibc
#elseif canImport(Musl)
  import Musl
#endif

/// `console.warn` in the JS implementation: a developer warning that is not
/// an error. Written to standard error unless a handler is installed with
/// `withWarningHandler`.
package func consoleWarn(_ message: String) {
  if let handler = ThreadContext.current.warningHandler {
    handler(message)
  } else {
    let line = Array("lexical: \(message)\n".utf8)
    _ = line.withUnsafeBytes { write(2, $0.baseAddress, $0.count) }
  }
}

/// Runs `body` with `handler` receiving the warnings `console.warn` would
/// print, on this thread.
public func withWarningHandler<T>(
  _ handler: @escaping (String) -> Void, _ body: () throws -> T
) rethrows -> T {
  let context = ThreadContext.current
  let previous = context.warningHandler
  context.warningHandler = handler
  defer { context.warningHandler = previous }
  return try body()
}

/// An error thrown out of a microtask, which JS leaves uncaught (an unhandled
/// exception). Written to standard error unless a handler is installed with
/// `withUncaughtErrorHandler`.
package func reportUncaughtError(_ error: any Error) {
  if let handler = ThreadContext.current.uncaughtErrorHandler {
    handler(error)
  } else {
    let line = Array("lexical: uncaught error: \(error)\n".utf8)
    _ = line.withUnsafeBytes { write(2, $0.baseAddress, $0.count) }
  }
}

/// Runs `body` with `handler` receiving the errors that microtasks throw,
/// which JS would leave uncaught, on this thread.
public func withUncaughtErrorHandler<T>(
  _ handler: @escaping (any Error) -> Void, _ body: () throws -> T
) rethrows -> T {
  let context = ThreadContext.current
  let previous = context.uncaughtErrorHandler
  context.uncaughtErrorHandler = handler
  defer { context.uncaughtErrorHandler = previous }
  return try body()
}
