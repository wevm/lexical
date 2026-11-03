/// JS microtasks, which the editor uses to batch synchronous updates into one
/// commit (`scheduleMicroTask` in LexicalUtils.ts).
///
/// Swift has no microtask checkpoint, so each thread keeps a queue that runs
/// when `drainMicrotasks()` is called: the UI layers call it once the current
/// event has been handled, and headless code and tests call it where JS would
/// `await` a microtask. Reading with `editor.read()` commits pending updates
/// first, as in JS, so most callers never need it.
public func drainMicrotasks() {
  let context = ThreadContext.current
  if context.drainingMicrotasks { return }
  context.drainingMicrotasks = true
  defer { context.drainingMicrotasks = false }
  while !context.microtasks.isEmpty {
    let task = context.microtasks.removeFirst()
    task()
  }
}

/// Whether the calling thread has queued microtasks.
public var hasPendingMicrotasks: Bool { !ThreadContext.current.microtasks.isEmpty }

package func scheduleMicroTask(_ task: @escaping () -> Void) {
  ThreadContext.current.microtasks.append(task)
}
