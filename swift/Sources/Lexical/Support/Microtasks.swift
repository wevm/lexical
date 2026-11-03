/// JS microtasks, which the editor uses to batch synchronous updates into one
/// commit (`scheduleMicroTask` in LexicalUtils.ts).
///
/// Swift has no microtask checkpoint, so each thread keeps a queue that runs
/// when `drainMicrotasks()` is called: the UI layers call it once the current
/// event has been handled, and headless code and tests call it where JS would
/// `await` a microtask. Reading with `editor.read()` commits pending updates
/// first, as in JS, so most callers never need it.
///
/// Once the microtasks are done it runs the `setTimeout(fn, 0)` tasks, one
/// at a time and each followed by the microtasks it queues, so a drain
/// leaves the thread idle, as the end of a JS event-loop turn would.
public func drainMicrotasks() {
  let context = ThreadContext.current
  if context.drainingMicrotasks { return }
  context.drainingMicrotasks = true
  defer { context.drainingMicrotasks = false }
  while true {
    while !context.microtasks.isEmpty {
      let task = context.microtasks.removeFirst()
      task()
    }
    guard !context.macrotasks.isEmpty else { return }
    let task = context.macrotasks.removeFirst()
    task()
  }
}

/// Whether the calling thread has queued microtasks.
public var hasPendingMicrotasks: Bool {
  !ThreadContext.current.microtasks.isEmpty || !ThreadContext.current.macrotasks.isEmpty
}

package func scheduleMicroTask(_ task: @escaping () -> Void) {
  ThreadContext.current.microtasks.append(task)
}

/// `setTimeout(task, 0)`: runs after the queued microtasks.
package func scheduleMacroTask(_ task: @escaping () -> Void) {
  ThreadContext.current.macrotasks.append(task)
}

/// Runs the queued microtasks only, leaving `setTimeout(fn, 0)` work queued,
/// as `await Promise.resolve()` does in JS.
package func drainMicrotasksOnly() {
  let context = ThreadContext.current
  if context.drainingMicrotasks { return }
  context.drainingMicrotasks = true
  defer { context.drainingMicrotasks = false }
  while !context.microtasks.isEmpty {
    let task = context.microtasks.removeFirst()
    task()
  }
}
