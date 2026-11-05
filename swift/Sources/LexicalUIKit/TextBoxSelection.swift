#if canImport(UIKit)
  import UIKit

  // A browser highlights a selection over the text's inline boxes: from a
  // line's first selected character to its last, over the font's content
  // area. TextKit's selection rects are whole line boxes, and a line the
  // selection runs through starts at the container's edge and ends at its
  // other edge, so they cover list markers, indents, quote bars and the
  // line-height's leading. `LexicalUITextView.selectionRects(for:)` trims
  // each one to its line's text.

  extension NSTextLayoutManager {
    /// `segment`, a selection segment in container coordinates, trimmed to
    /// its line's text box, or `nil` when none of the line's text is in
    /// it. A caret-wide segment keeps its place, and one off any line is
    /// left as it is.
    func selectionSegment(trimming segment: CGRect) -> CGRect? {
      guard let box = textBox(at: CGPoint(x: segment.minX, y: segment.midY)) else {
        return segment
      }
      var trimmed = CGRect(x: segment.minX, y: box.minY, width: segment.width, height: box.height)
      if segment.width > 0 {
        let minX = max(segment.minX, box.minX)
        let maxX = min(segment.maxX, box.maxX)
        guard maxX > minX else { return nil }
        trimmed.origin.x = minX
        trimmed.size.width = maxX - minX
      }
      return trimmed
    }

    /// The text box of the line at `point`, in container coordinates: from
    /// its first character to its last, without the newline, and over its
    /// fonts' ascent and descent at the baseline `CenteredLineLayoutFragment`
    /// draws them on. A line without text is a sliver at its start, as
    /// tall as its newline's font.
    func textBox(at point: CGPoint) -> CGRect? {
      guard let fragment = textLayoutFragment(for: point) else { return nil }
      let frame = fragment.layoutFragmentFrame
      guard
        let line = fragment.textLineFragments.first(where: { line in
          let bounds = line.typographicBounds.offsetBy(dx: frame.minX, dy: frame.minY)
          return bounds.minY <= point.y && point.y < bounds.maxY
        })
      else { return nil }
      let string = line.attributedString.string as NSString
      let range = NSIntersectionRange(
        line.characterRange, NSRange(location: 0, length: string.length))
      var end = range.upperBound
      while end > range.location, [0x0A, 0x2028, 0x2029].contains(string.character(at: end - 1)) {
        end -= 1
      }
      let extents: (ascent: CGFloat, descent: CGFloat)
      if let lineExtents = line.fontExtents {
        extents = lineExtents
      } else if range.location < string.length,
        let font = line.attributedString.attribute(.font, at: range.location, effectiveRange: nil)
          as? UIFont
      {
        extents = (font.ascender, -font.descender)
      } else {
        return nil
      }
      let origin = CGPoint(
        x: frame.minX + line.typographicBounds.minX, y: frame.minY + line.typographicBounds.minY)
      let start = line.locationForCharacter(at: range.location).x
      let stop = end > range.location ? line.locationForCharacter(at: end).x : start + 4
      // Where `CenteredLineLayoutFragment` draws the baseline.
      let offset = max(
        0,
        line.glyphOrigin.y
          - (line.typographicBounds.height + extents.ascent - extents.descent) / 2)
      let baseline = origin.y + line.glyphOrigin.y - offset
      return CGRect(
        x: origin.x + min(start, stop), y: baseline - extents.ascent, width: abs(stop - start),
        height: extents.ascent + extents.descent)
    }
  }
#endif
