#if canImport(UIKit)
  import UIKit

  /// Reports a third (or later) tap in a row, with its tap count and
  /// location, without taking part in gesture recognition: it never
  /// recognizes, so the text view's own taps (word and paragraph selection)
  /// keep working, and nothing prevents it from seeing the taps.
  final class TapCountRecognizer: UIGestureRecognizer {
    var onTap: ((_ count: Int, _ location: CGPoint) -> Void)?

    override init(target: Any?, action: Selector?) {
      super.init(target: target, action: action)
      cancelsTouchesInView = false
      delaysTouchesBegan = false
      delaysTouchesEnded = false
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
      if let touch = touches.first, touch.tapCount >= 3 {
        onTap?(touch.tapCount, touch.location(in: view))
      }
      state = .failed
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) {
      state = .failed
    }

    override func canBePrevented(by preventingGestureRecognizer: UIGestureRecognizer) -> Bool {
      false
    }

    override func canPrevent(_ preventedGestureRecognizer: UIGestureRecognizer) -> Bool {
      false
    }
  }
#endif
