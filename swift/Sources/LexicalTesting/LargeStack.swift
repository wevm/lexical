import Foundation

/// Runs `body` to completion on a new thread with a stack of `stackSize`
/// bytes, and returns its result or rethrows its error.
///
/// Swift Testing runs tests on secondary threads, whose stack is 512 KB on
/// Darwin (8 MB on Linux). A test whose input nests deeply enough for the
/// recursion of an upstream algorithm (HTML import recurses once per DOM
/// level, as in JS) to need more runs its body here, so that it behaves the
/// same on every platform.
public func withLargeStack<T>(
  _ stackSize: Int = 8 << 20, _ body: @escaping () throws -> T
) throws -> T {
  nonisolated(unsafe) let body = body
  nonisolated(unsafe) var result: Result<T, any Error>?
  let done = DispatchSemaphore(value: 0)
  let thread = Thread {
    result = Result { try body() }
    done.signal()
  }
  thread.stackSize = stackSize
  thread.start()
  done.wait()
  return try result!.get()
}
