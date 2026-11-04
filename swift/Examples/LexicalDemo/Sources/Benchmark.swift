import Lexical
import SwiftUI

#if os(macOS)
  import LexicalAppKit
#elseif canImport(UIKit)
  import LexicalUIKit
#endif

// `-benchmark`: 200 keystrokes typed through the view into the middle of
// 5,000 paragraphs, timed from the view's insertText to its text storage
// holding the change (the editor update, the reconcile and the storage
// edit). Layout is lazy and not included. The result is the p95 and the
// median in milliseconds, as the "benchmark" label.

@MainActor
func runKeystrokeBenchmark(_ editor: Editor) throws -> String {
  try editor.update(discrete: true) {
    let root = try getRoot().clear()
    for index in 0..<5_000 {
      try root.append(
        createParagraphNode().append(
          createTextNode("Paragraph \(index) with a few words of text in it.")))
    }
    _ = try (root.getChildAtIndex(2_500) as? ElementNode)?.selectEnd()
  }
  #if os(macOS)
    guard let view = editor.getRootElement() as? LexicalNSTextView else { return "no view" }
    let type = { view.insertText("a", replacementRange: NSRange(location: NSNotFound, length: 0)) }
    let length = { view.textStorage?.length ?? 0 }
  #else
    guard let view = editor.getRootElement() as? LexicalUITextView else { return "no view" }
    let type = { view.insertText("a") }
    let length = { view.textStorage.length }
  #endif
  var times: [Double] = []
  // 20 untimed keystrokes warm the caches first.
  for index in 0..<220 {
    let before = length()
    let start = ContinuousClock.now
    type()
    let elapsed = ContinuousClock.now - start
    guard length() == before + 1 else { return "keystroke \(index) did not reach the view" }
    if index >= 20 {
      times.append(
        Double(elapsed.components.attoseconds) / 1e15 + Double(elapsed.components.seconds) * 1e3)
    }
  }
  times.sort()
  let p95 = times[Int(Double(times.count) * 0.95) - 1]
  let median = times[times.count / 2]
  return String(format: "p95 %.2f ms, median %.2f ms over %d keystrokes", p95, median, times.count)
}
