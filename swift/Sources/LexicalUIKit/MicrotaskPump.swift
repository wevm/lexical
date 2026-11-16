#if canImport(UIKit)
  import Foundation
  import Lexical

  /// Runs the main thread's queued microtasks after each pass of the run
  /// loop, as a browser's event loop does at the end of each task, so
  /// updates an app makes outside the views (`editor.update` from a
  /// toolbar) commit. A pass ends before the next one starts its timers,
  /// before the run loop sleeps, or when it exits: a busy main thread (a
  /// sheet's dismissal, an animation's events) can keep it from sleeping
  /// for seconds.
  @MainActor
  enum MicrotaskPump {
    private static var observer: CFRunLoopObserver?

    static func install() {
      guard observer == nil else { return }
      let activities: CFRunLoopActivity = [.beforeTimers, .beforeWaiting, .exit]
      let observer = CFRunLoopObserverCreateWithHandler(nil, activities.rawValue, true, 0) { _, _ in
        MainActor.assumeIsolated { drainMicrotasks() }
      }
      CFRunLoopAddObserver(CFRunLoopGetMain(), observer, .commonModes)
      self.observer = observer
    }
  }
#endif
