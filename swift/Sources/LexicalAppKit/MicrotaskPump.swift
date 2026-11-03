#if os(macOS)
  import Foundation
  import Lexical

  /// Runs the main thread's queued microtasks before the run loop sleeps, as
  /// a browser's event loop does at the end of each task, so updates an app
  /// makes outside the views (`editor.update` from a toolbar) commit.
  @MainActor
  enum MicrotaskPump {
    private static var observer: CFRunLoopObserver?

    static func install() {
      guard observer == nil else { return }
      let observer = CFRunLoopObserverCreateWithHandler(
        nil, CFRunLoopActivity.beforeWaiting.rawValue, true, 0
      ) { _, _ in
        MainActor.assumeIsolated { drainMicrotasks() }
      }
      CFRunLoopAddObserver(CFRunLoopGetMain(), observer, .commonModes)
      self.observer = observer
    }
  }
#endif
