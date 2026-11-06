import Lexical
import LexicalRichText
import LexicalTextKit
import Observation
import SwiftUI

#if os(macOS)
  import AppKit
  import LexicalAppKit
  private typealias TOCTextView = LexicalNSTextView
#else
  import UIKit
  import LexicalUIKit
  private typealias TOCTextView = LexicalUITextView
#endif

// Port of packages/lexical-playground/src/plugins/TableOfContentsPlugin:
// the "Table Of Contents" setting lists the document's headings, kept by
// `registerTableOfContents` (`@lexical/react`'s TableOfContentsPlugin), and
// a click on one scrolls the editor to it and marks it selected.

/// The headings the panel lists, while the setting is on.
@MainActor
@Observable
final class TableOfContentsModel {
  /// The "Table Of Contents" setting (`showTableOfContents`); off by
  /// default, as on the web.
  var isShown = false {
    didSet { if isShown != oldValue { sync() } }
  }
  private(set) var entries: [TableOfContentsEntry] = []
  /// The heading last scrolled to (`selectedKey`).
  private(set) var selectedKey: NodeKey?
  @ObservationIgnored private weak var editor: Editor?
  @ObservationIgnored private var unregisterTableOfContents: (() -> Void)?

  /// Follows the headings of `editor` while the setting is on.
  func register(_ editor: Editor) -> () -> Void {
    self.editor = editor
    sync()
    return { [weak self] in
      self?.editor = nil
      self?.sync()
    }
  }

  private func sync() {
    unregisterTableOfContents?()
    unregisterTableOfContents = nil
    entries = []
    selectedKey = nil
    // A plain text editor has no headings to list.
    guard isShown, let editor, editor.hasNodes([HeadingNode.self]) else { return }
    unregisterTableOfContents = try? registerTableOfContents(editor) { [weak self] entries in
      MainActor.assumeIsolated { self?.entries = entries }
    }
  }

  /// `scrollToNode`: the heading `key` scrolled into view and selected.
  func scrollToNode(_ key: NodeKey) {
    guard let textView = editor?.getRootElement() as? TOCTextView,
      let location = textView.controller.document.location(key, 0, .element)
    else { return }
    scrollToCenter(textView, location)
    selectedKey = key
  }

  /// `scrollIntoView({block: 'center'})` for the text at `location`.
  private func scrollToCenter(_ textView: TOCTextView, _ location: Int) {
    #if os(macOS)
      guard let window = textView.window, let scrollView = textView.enclosingScrollView else {
        return
      }
      textView.scrollRangeToVisible(NSRange(location: location, length: 0))
      let screenRect = textView.firstRect(
        forCharacterRange: NSRange(location: location, length: 0), actualRange: nil)
      let rect = textView.convert(window.convertFromScreen(screenRect), from: nil)
      let visible = scrollView.contentView.bounds
      let y = min(
        max(rect.midY - visible.height / 2, 0), max(textView.bounds.height - visible.height, 0))
      scrollView.contentView.scroll(to: NSPoint(x: visible.minX, y: y))
      scrollView.reflectScrolledClipView(scrollView.contentView)
    #else
      guard
        let position = textView.position(
          from: textView.beginningOfDocument, offset: location)
      else { return }
      let rect = textView.caretRect(for: position)
      let height = textView.bounds.height
      let maxY = max(
        textView.contentSize.height + textView.adjustedContentInset.bottom - height,
        -textView.adjustedContentInset.top)
      let y = min(max(rect.midY - height / 2, -textView.adjustedContentInset.top), maxY)
      textView.setContentOffset(CGPoint(x: textView.contentOffset.x, y: y), animated: true)
    #endif
  }
}

/// `TableOfContentsList`: the headings beside a rail, the first in bold,
/// the others indented by level, the selected one in blue with a dot on
/// the rail.
struct TableOfContentsPanel: View {
  let model: TableOfContentsModel
  /// Sized to its headings, up to the web's height, where the editor is
  /// short.
  var isCompact = false

  /// The web's 200 by 220 points, grown with the text up to about half
  /// as much again, where the headings wrap.
  @ScaledMetric(relativeTo: .callout) private var width: CGFloat = 200
  @ScaledMetric(relativeTo: .callout) private var height: CGFloat = 220
  @Environment(\.dynamicTypeSize) private var dynamicTypeSize

  private static let blue = Color(red: 0x35 / 255, green: 0x78 / 255, blue: 0xe5 / 255)
  private static let gray = Color(red: 0x65 / 255, green: 0x67 / 255, blue: 0x6b / 255)

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 0) {
        ForEach(Array(model.entries.enumerated()), id: \.element.key) { index, entry in
          row(entry, isFirst: index == 0)
        }
      }
      .frame(maxWidth: .infinity, alignment: .leading)
    }
    .scrollIndicators(.hidden)
    .frame(width: min(width, 300))
    .frame(height: isCompact ? nil : min(height, 440))
    .frame(maxHeight: min(height, 440))
    .fixedSize(horizontal: false, vertical: isCompact)
    .background(alignment: .leading) {
      // `.headings::before`.
      RoundedRectangle(cornerRadius: 2).fill(
        Color(red: 0xcc / 255, green: 0xd0 / 255, blue: 0xd5 / 255)
      )
      .frame(width: 4)
      .padding(.top, 5)
    }
    .padding(10)
    .accessibilityElement(children: .contain)
    .accessibilityLabel("Table of contents")
    .accessibilityIdentifier("table-of-contents")
  }

  @ViewBuilder
  private func row(_ entry: TableOfContentsEntry, isFirst: Bool) -> some View {
    let isSelected = !isFirst && model.selectedKey == entry.key
    Button {
      model.scrollToNode(entry.key)
    } label: {
      Text(truncated(entry.text, isFirst ? 20 : 27))
        .scaledFont(size: 16, weight: isFirst ? .bold : .regular, relativeTo: .callout)
        .foregroundStyle(isFirst ? .black : isSelected ? Self.blue : Self.gray)
        .lineLimit(dynamicTypeSize.isAccessibilitySize ? nil : 1)
        .frame(minHeight: 20)
        // `.first-heading` sits a line above the next (its `<br />`).
        .padding(.bottom, isFirst ? 20 : 0)
        .padding(.leading, 32 + indent(entry.tag))
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .overlay(alignment: .topLeading) {
      // `.selected-heading-wrapper::before`.
      if isSelected {
        Circle().fill(Self.blue).frame(width: 4, height: 4)
          .padding(4).background(Circle().fill(.white))
          .offset(x: -2, y: 4)
      }
    }
    .accessibilityAddTraits(isSelected ? .isSelected : [])
  }

  /// `.heading2` and `.heading3`.
  private func indent(_ tag: HeadingTagType) -> CGFloat {
    tag == "h2" ? 10 : tag == "h3" ? 20 : 0
  }

  /// The text's first `length` UTF-16 code units and "...", past that.
  private func truncated(_ text: String, _ length: Int) -> String {
    guard text.utf16.count > length else { return text }
    return (String(text.utf16.prefix(length)) ?? String(text.prefix(length))) + "..."
  }
}
