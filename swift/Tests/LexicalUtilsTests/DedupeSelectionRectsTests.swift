import LexicalSelection
import LexicalTesting
import LexicalUtils
import Testing

// Port of packages/lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts.
// createRectsFromDOMRange's rect filter is `filterSelectionRects`, given the
// root's width (1200) and padding (0) the upstream fake root reports.

private func rect(_ left: Double, _ top: Double, _ width: Double, _ height: Double) -> SelectionRect
{
  SelectionRect(left: left, top: top, width: width, height: height)
}

private func widths(_ rects: [SelectionRect]) -> [Double] { rects.map(\.width).sorted() }

private func createRects(_ rects: [SelectionRect]) -> [SelectionRect] {
  filterSelectionRects(rects, rootWidth: 1200, rootPadding: 0)
}

private func text() -> SelectionRect { rect(128, 90.5, 368, 18) }
private func wider() -> SelectionRect { rect(128, 90.0, 900, 18) }

@Suite struct DedupeSelectionRectsTests {
  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > keeps one of two identical rects (the doubled-opacity duplicate)"
    ))
  func case0() {
    let kept = dedupeSelectionRects([rect(128, 166, 394, 18), rect(128, 166, 394, 18)])
    #expect(kept.count == 1)
    #expect(kept[0].width == 394)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > keeps the smaller text rect when a wider rect contains it (#7106 extra area)"
    ))
  func case1() {
    let kept = dedupeSelectionRects([rect(128, 90, 368, 40), rect(128, 90, 1024, 40)])
    #expect(kept.count == 1)
    #expect(kept[0].width == 368)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > keeps the smaller rect regardless of input order"
    ))
  func case2() {
    let text = rect(128, 90, 368, 40)
    let wider = rect(128, 90, 1024, 40)
    #expect(dedupeSelectionRects([wider, text])[0].width == 368)
    #expect(dedupeSelectionRects([text, wider])[0].width == 368)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > preserves genuine multi-line rects (different rows are not contained)"
    ))
  func case3() {
    let row1 = rect(128, 90, 368, 18)
    let row2 = rect(128, 112, 200, 18)
    let kept = dedupeSelectionRects([row1, row1, row2, row2])
    #expect(kept.count == 2)
    #expect(kept.map(\.top).sorted() == [90, 112])
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > drops zero-area rects"
    ))
  func case4() {
    #expect(
      dedupeSelectionRects([rect(0, 0, 0, 18), rect(0, 0, 100, 0), rect(0, 0, 100, 18)]).count == 1)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > returns empty for an empty list"
    ))
  func case5() {
    #expect(dedupeSelectionRects([]).isEmpty)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > treats near-identical rects within 1px as duplicates"
    ))
  func case6() {
    let kept = dedupeSelectionRects([
      rect(128, 166, 394, 18),
      SelectionRect(
        left: 128.4, top: 166.3, right: 522.1, bottom: 184.5, width: 393.7, height: 18.2),
    ])
    #expect(kept.count == 1)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > keeps horizontally disjoint rects on the same row (no false containment)"
    ))
  func case7() {
    #expect(dedupeSelectionRects([rect(0, 0, 50, 18), rect(100, 0, 50, 18)]).count == 2)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > does not clip a wide rect (e.g. trailing whitespace) against a disjoint narrower row"
    ))
  func case8() {
    let kept = dedupeSelectionRects([rect(0, 0, 300, 18), rect(0, 22, 80, 18)])
    #expect(widths(kept) == [80, 300])
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects > keeps every row of a ragged multi-row selection (varying offsets, e.g. RTL)"
    ))
  func case9() {
    #expect(
      dedupeSelectionRects([rect(200, 0, 100, 18), rect(150, 22, 150, 18), rect(180, 44, 120, 18)])
        .count == 3)
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects + createRectsFromDOMRange > createRectsFromDOMRange leaves the contained pair (its adjacent-only pass does not catch it)"
    ))
  func case10() {
    #expect(widths(createRects([text(), wider()])) == [368, 900])
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects + createRectsFromDOMRange > dedupeSelectionRects collapses that survivor pair to the text-hugging rect"
    ))
  func case11() {
    #expect(widths(dedupeSelectionRects(createRects([text(), wider()]))) == [368])
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects + createRectsFromDOMRange > preserves the partially overlapping pair in either order before tolerant deduplication"
    ))
  func case12() {
    #expect(widths(createRects([text(), wider()])) == [368, 900])
    #expect(widths(createRects([wider(), text()])) == [368, 900])
    #expect(widths(dedupeSelectionRects([text(), wider()])) == [368])
    #expect(widths(dedupeSelectionRects([wider(), text()])) == [368])
  }

  @Test(
    .upstream(
      "lexical-utils/src/__tests__/unit/dedupeSelectionRects.test.ts::dedupeSelectionRects + createRectsFromDOMRange > keeps the smaller (text) rect by design — keeping the wider one re-introduces the #7106 extra-area paint"
    ))
  func case13() {
    let kept = dedupeSelectionRects(createRects([text(), wider()]))
    #expect(kept.count == 1)
    #expect(kept[0].width == 368)
  }

}
