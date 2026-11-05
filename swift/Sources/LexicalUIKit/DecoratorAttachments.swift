#if canImport(UIKit)
  import Lexical
  import LexicalTextKit
  import SwiftUI
  import UIKit

  // DecoratorNodes as SwiftUI views: each decorator's attachment character
  // carries a text attachment whose view provider hosts what the node's
  // `decorate(_:)` returned (Q12). Inline decorators size to their view;
  // block decorators take the line's width.

  /// The attachments of one view's decorators, by node key, so a view
  /// survives the re-renders that keep its node.
  @MainActor
  final class DecoratorAttachments {
    private var attachments: [NodeKey: LexicalDecoratorAttachment] = [:]
    /// The keys in the editor's node selection.
    private var selectedKeys: Set<NodeKey> = []
    /// Called with the key of a decorator that was tapped.
    var onClick: ((NodeKey) -> Void)?

    func attachment(for span: TextKitDecoratorSpan, editor: Editor) -> NSTextAttachment? {
      let view = (editor.getDecorators()[span.key]).flatMap { $0 as? any View }
      guard view != nil || span.rule != nil else {
        attachments[span.key] = nil
        return nil
      }
      let attachment =
        attachments[span.key]
        ?? LexicalDecoratorAttachment(key: span.key) { [weak self] key in self?.onClick?(key) }
      if let view {
        attachment.update(AnyView(view), isInline: span.isInline)
      } else if let rule = span.rule {
        attachment.update(rule, isSelected: selectedKeys.contains(span.key))
      }
      attachment.textContent =
        (try? editor.getEditorState().read { try getNodeByKey(span.key)?.getTextContent() }) ?? ""
      attachments[span.key] = attachment
      return attachment
    }

    /// Outlines the horizontal rules in a node selection of `keys`, as JS
    /// adds the theme's `hrSelected` class to their `<hr>`.
    func select(_ keys: Set<NodeKey>) {
      guard keys != selectedKeys else { return }
      let changed = keys.symmetricDifference(selectedKeys)
      selectedKeys = keys
      for key in changed {
        attachments[key]?.ruleView?.isSelected = keys.contains(key)
      }
    }

    /// Forgets the attachments of decorators that are gone: those not in
    /// `keys`, the decorators with views, and rules `document` no longer
    /// shows.
    func retain(_ keys: Set<NodeKey>, document: TextKitDocument) {
      attachments = attachments.filter {
        keys.contains($0.key) || ($0.value.ruleView != nil && document.span($0.key) != nil)
      }
    }
  }

  /// A horizontal rule, drawn from the theme's `TextKitRule`: its line in
  /// the padded box between its margins, and while selected, an outline
  /// outside the box.
  final class HorizontalRuleView: UIView {
    var rule: TextKitRule? {
      didSet { setNeedsDisplay() }
    }
    var isSelected = false {
      didSet { setNeedsDisplay() }
    }

    override init(frame: CGRect) {
      super.init(frame: frame)
      isOpaque = false
      backgroundColor = .clear
      contentMode = .redraw
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override func draw(_ rect: CGRect) {
      guard let rule else { return }
      // The box is inset by the outline, which CSS draws outside it, so
      // the view's edges do not clip it.
      let inset = rule.selectedOutlineWidth
      let box = CGRect(
        x: inset, y: rule.margin, width: bounds.width - 2 * inset,
        height: 2 * rule.padding + rule.thickness)
      rule.color.uiColor.setFill()
      UIRectFill(box.insetBy(dx: rule.padding, dy: rule.padding))
      if isSelected {
        let width = rule.selectedOutlineWidth
        let outline = UIBezierPath(rect: box.insetBy(dx: -width / 2, dy: -width / 2))
        outline.lineWidth = width
        rule.selectedOutlineColor.uiColor.setStroke()
        outline.stroke()
      }
    }
  }

  /// A decorator's attachment: a hosting view for its SwiftUI view.
  final class LexicalDecoratorAttachment: NSTextAttachment {
    let key: NodeKey
    nonisolated(unsafe) let host: UIHostingController<AnyView>
    /// Read by layout, which TextKit runs on the main thread.
    nonisolated(unsafe) private var size = CGSize.zero
    nonisolated(unsafe) private var isInline = true
    /// A horizontal rule's view, which TextKit shows in place of the
    /// hosting view; its line is only as tall as its box.
    nonisolated(unsafe) private(set) var ruleView: HorizontalRuleView?
    nonisolated(unsafe) var isRule: Bool { ruleView != nil }
    private let onClick: () -> Void
    /// The decorator's `getTextContent()`.
    nonisolated(unsafe) var textContent = ""

    @MainActor
    init(key: NodeKey, onClick: @escaping (NodeKey) -> Void) {
      self.key = key
      self.onClick = { onClick(key) }
      host = UIHostingController(rootView: AnyView(EmptyView()))
      host.view.backgroundColor = .clear
      super.init(data: nil, ofType: nil)
      allowsTextAttachmentView = true
      // What TextKit draws where it does not show the view, in place of
      // its file icon.
      image = UIGraphicsImageRenderer(size: CGSize(width: 1, height: 1)).image { _ in }
      let tap = DecoratorTapRecognizer()
      tap.onTap = self.onClick
      host.view.addGestureRecognizer(tap)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    @MainActor
    func update(_ view: AnyView, isInline: Bool) {
      host.rootView = view
      size = host.sizeThatFits(in: CGSize(width: 10_000, height: 10_000))
      self.isInline = isInline
    }

    /// Shows `rule` in place of a SwiftUI view.
    @MainActor
    func update(_ rule: TextKitRule, isSelected: Bool) {
      let view = ruleView ?? HorizontalRuleView()
      if ruleView == nil {
        let tap = DecoratorTapRecognizer()
        tap.onTap = onClick
        view.addGestureRecognizer(tap)
        ruleView = view
      }
      view.rule = rule
      view.isSelected = isSelected
      size = CGSize(width: 1, height: rule.height)
      isInline = false
    }

    /// What VoiceOver says for the decorator's character: the labels of
    /// the hosted SwiftUI view's accessibility elements, which SwiftUI
    /// builds only while an assistive technology is running, or else the
    /// decorator's text content.
    override var accessibilityLabel: String? {
      get {
        MainActor.assumeIsolated { hostedAccessibilityLabel(host.view) }
          ?? (textContent.isEmpty ? nil : textContent)
      }
      set {}
    }

    override func viewProvider(
      for parentView: UIView?, location: any NSTextLocation, textContainer: NSTextContainer?
    ) -> NSTextAttachmentViewProvider? {
      nonisolated(unsafe) let layoutManager = textContainer?.textLayoutManager
      nonisolated(unsafe) let attachment = self
      let provider = DecoratorViewProvider(
        textAttachment: attachment, parentView: parentView, textLayoutManager: layoutManager,
        location: location)
      provider.tracksTextAttachmentViewBounds = true
      return provider
    }

    /// The decorator's frame on its line: an inline decorator takes its
    /// view's size, a block decorator the line's width. A rule sits on the
    /// line's descent, so the line is no taller than its box, as an `<hr>`.
    func bounds(proposedLineFragment: CGRect, font: UIFont?) -> CGRect {
      if isInline {
        return CGRect(origin: .zero, size: size)
      }
      return CGRect(
        x: 0, y: isRule ? font?.descender ?? 0 : 0, width: max(proposedLineFragment.width, 1),
        height: size.height)
    }
  }

  /// The labels of the accessibility elements under `object`, joined as
  /// VoiceOver reads combined children; `nil` when they have none.
  @MainActor
  func hostedAccessibilityLabel(_ object: NSObject) -> String? {
    func labels(_ object: NSObject) -> [String] {
      if object.isAccessibilityElement {
        return object.accessibilityLabel.map { [$0] }?.filter { !$0.isEmpty } ?? []
      }
      if let elements = object.accessibilityElements as? [NSObject] {
        return elements.flatMap(labels)
      }
      return (object as? UIView)?.subviews.flatMap(labels) ?? []
    }
    let found = labels(object)
    return found.isEmpty ? nil : found.joined(separator: ", ")
  }

  // The bounds are the provider's: TextKit 2 skips an attachment's view
  // provider when the attachment overrides `attachmentBounds`, and draws a
  // file icon instead.
  final class DecoratorViewProvider: NSTextAttachmentViewProvider {
    override func attachmentBounds(
      for attributes: [NSAttributedString.Key: Any], location: any NSTextLocation,
      textContainer: NSTextContainer?, proposedLineFragment: CGRect, position: CGPoint
    ) -> CGRect {
      (textAttachment as? LexicalDecoratorAttachment)?.bounds(
        proposedLineFragment: proposedLineFragment, font: attributes[.font] as? UIFont) ?? .zero
    }

    override func loadView() {
      let attachment = textAttachment as? LexicalDecoratorAttachment
      view = attachment?.ruleView ?? attachment?.host.view
    }
  }

  /// A tap that reaches the decorator's view and not the text view.
  final class DecoratorTapRecognizer: UITapGestureRecognizer {
    var onTap: (() -> Void)?

    init() {
      super.init(target: nil, action: nil)
      addTarget(self, action: #selector(tapped))
      cancelsTouchesInView = false
    }

    @objc private func tapped() { onTap?() }
  }
#endif
