#if canImport(UIKit)
  import UIKit

  // CSS `line-height` centers a line's text in its line box, with half the
  // extra space (the half-leading) above the text and half below. TextKit
  // puts a paragraph's whole minimum line height above the text, and
  // `baselineOffset` cannot move it below (a raised baseline shortens the
  // line from the bottom), so each paragraph's layout fragment draws its
  // lines' text centered instead. Hit-testing and the selection keep
  // TextKit's line boxes, which are the CSS ones.

  /// A paragraph's layout fragment that draws each line's text centered in
  /// its line, as CSS does.
  final class CenteredLineLayoutFragment: NSTextLayoutFragment {
    override func draw(at point: CGPoint, in context: CGContext) {
      let lines = textLineFragments
      let offsets = lines.map(\.textOffset)
      guard let first = offsets.first, offsets.contains(where: { $0 > 0 }) else {
        super.draw(at: point, in: context)
        return
      }
      if offsets.allSatisfy({ abs($0 - first) < 0.01 }) {
        super.draw(at: CGPoint(x: point.x, y: point.y - first), in: context)
        return
      }
      // Lines with different offsets (one with a larger font): each is
      // drawn with its own, clipped to its line.
      let surface = renderingSurfaceBounds
      for (line, offset) in zip(lines, offsets) {
        context.saveGState()
        context.clip(
          to: CGRect(
            x: point.x + surface.minX, y: point.y + line.typographicBounds.minY,
            width: surface.width, height: line.typographicBounds.height))
        super.draw(at: CGPoint(x: point.x, y: point.y - offset), in: context)
        context.restoreGState()
      }
    }

    override var renderingSurfaceBounds: CGRect {
      let bounds = super.renderingSurfaceBounds
      let offset = textLineFragments.map(\.textOffset).max() ?? 0
      return bounds.union(bounds.offsetBy(dx: 0, dy: -offset))
    }
  }

  /// Gives each paragraph a `CenteredLineLayoutFragment`.
  final class CenteredLineLayoutDelegate: NSObject, NSTextLayoutManagerDelegate {
    func textLayoutManager(
      _ textLayoutManager: NSTextLayoutManager, textLayoutFragmentFor location: any NSTextLocation,
      in textElement: NSTextElement
    ) -> NSTextLayoutFragment {
      CenteredLineLayoutFragment(textElement: textElement, range: textElement.elementRange)
    }
  }

  extension NSTextLineFragment {
    /// The largest ascent and descent of the line's fonts, with their
    /// baseline offsets.
    var fontExtents: (ascent: CGFloat, descent: CGFloat)? {
      var extents: (ascent: CGFloat, descent: CGFloat)?
      let range = NSIntersectionRange(
        characterRange, NSRange(location: 0, length: attributedString.length))
      guard range.length > 0 else { return nil }
      attributedString.enumerateAttributes(in: range) { attributes, _, _ in
        guard let font = attributes[.font] as? UIFont else { return }
        let shift = (attributes[.baselineOffset] as? NSNumber).map { CGFloat($0.doubleValue) } ?? 0
        let ascent = font.ascender + shift
        let descent = -font.descender - shift
        extents = (
          max(extents?.ascent ?? ascent, ascent), max(extents?.descent ?? descent, descent)
        )
      }
      return extents
    }

    /// How far above where TextKit put it the line's text is drawn, so it
    /// is centered in the line.
    var textOffset: CGFloat {
      guard let (ascent, descent) = fontExtents else { return 0 }
      let height = typographicBounds.height
      return max(0, glyphOrigin.y - (height + ascent - descent) / 2)
    }
  }
#endif
