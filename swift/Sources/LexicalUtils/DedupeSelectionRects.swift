import LexicalSelection

/// `dedupeSelectionRects`: drops zero-area rects and any rect that contains
/// another (within 1 point), keeping the smaller, text-hugging one.
public func dedupeSelectionRects(_ rects: [SelectionRect]) -> [SelectionRect] {
  func contains(_ a: SelectionRect, _ b: SelectionRect) -> Bool {
    b.left >= a.left - 1 && b.top >= a.top - 1 && b.right <= a.right + 1
      && b.bottom <= a.bottom + 1
  }
  var kept: [SelectionRect] = []
  for rect in rects {
    if rect.width < 0.5 || rect.height < 0.5 { continue }
    if kept.contains(where: { contains(rect, $0) }) { continue }
    kept.removeAll { contains($0, rect) }
    kept.append(rect)
  }
  return kept
}
