#if os(macOS)
  import AppKit
  import Lexical
  import LexicalTextKit
  import SwiftUI

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
    /// Called with the key of a decorator that was clicked.
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
      attachment.hostingView.textContent =
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
  final class HorizontalRuleView: NSView {
    var rule: TextKitRule? {
      didSet { needsDisplay = true }
    }
    var isSelected = false {
      didSet { needsDisplay = true }
    }

    override var isFlipped: Bool { true }

    override func draw(_ dirtyRect: NSRect) {
      guard let rule else { return }
      // The box is inset by the outline, which CSS draws outside it, so
      // the view's edges do not clip it.
      let inset = rule.selectedOutlineWidth
      let box = NSRect(
        x: inset, y: rule.margin, width: bounds.width - 2 * inset,
        height: 2 * rule.padding + rule.thickness)
      rule.color.nsColor.setFill()
      box.insetBy(dx: rule.padding, dy: rule.padding).fill()
      if isSelected {
        let width = rule.selectedOutlineWidth
        let outline = NSBezierPath(rect: box.insetBy(dx: -width / 2, dy: -width / 2))
        outline.lineWidth = width
        rule.selectedOutlineColor.nsColor.setStroke()
        outline.stroke()
      }
    }
  }

  /// A decorator's attachment: a hosting view for its SwiftUI view.
  final class LexicalDecoratorAttachment: NSTextAttachment {
    let key: NodeKey
    let hostingView: DecoratorHostingView
    /// Read by layout, which TextKit runs on the main thread.
    nonisolated(unsafe) private var size = CGSize.zero
    nonisolated(unsafe) private var isInline = true
    /// A horizontal rule's view, which TextKit shows in place of the
    /// hosting view; its line is only as tall as its box.
    nonisolated(unsafe) private(set) var ruleView: HorizontalRuleView?
    nonisolated(unsafe) var isRule: Bool { ruleView != nil }
    private let onClick: () -> Void

    @MainActor
    init(key: NodeKey, onClick: @escaping (NodeKey) -> Void) {
      self.key = key
      self.onClick = { onClick(key) }
      hostingView = DecoratorHostingView(rootView: AnyView(EmptyView()))
      super.init(data: nil, ofType: nil)
      allowsTextAttachmentView = true
      // What TextKit draws where it does not show the view, in place of
      // its file icon.
      image = NSImage(size: NSSize(width: 1, height: 1))
      let click = DecoratorClickRecognizer(target: nil, action: nil)
      click.onClick = self.onClick
      hostingView.addGestureRecognizer(click)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    @MainActor
    func update(_ view: AnyView, isInline: Bool) {
      hostingView.rootView = view
      size = hostingView.fittingSize
      self.isInline = isInline
    }

    /// Shows `rule` in place of a SwiftUI view.
    @MainActor
    func update(_ rule: TextKitRule, isSelected: Bool) {
      let view = ruleView ?? HorizontalRuleView()
      if ruleView == nil {
        let click = DecoratorClickRecognizer(target: nil, action: nil)
        click.onClick = onClick
        view.addGestureRecognizer(click)
        ruleView = view
      }
      view.rule = rule
      view.isSelected = isSelected
      size = CGSize(width: 1, height: rule.height)
      isInline = false
    }

    override func viewProvider(
      for parentView: NSView?, location: any NSTextLocation, textContainer: NSTextContainer?
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
    func bounds(proposedLineFragment: CGRect, font: NSFont?) -> CGRect {
      if isInline {
        // No wider than the line, keeping its aspect, as an image's
        // `max-width: 100%`.
        let scale =
          proposedLineFragment.width > 0 && size.width > proposedLineFragment.width
          ? proposedLineFragment.width / size.width : 1
        return CGRect(
          origin: .zero, size: CGSize(width: size.width * scale, height: size.height * scale))
      }
      return CGRect(
        x: 0, y: isRule ? font?.descender ?? 0 : 0, width: max(proposedLineFragment.width, 1),
        height: size.height)
    }
  }

  /// The hosting view of a decorator's SwiftUI view. VoiceOver's label for
  /// it is the labels of the SwiftUI view's accessibility elements, which
  /// SwiftUI builds only while an assistive technology is running, or else
  /// the decorator's text content.
  final class DecoratorHostingView: NSHostingView<AnyView> {
    /// The decorator's `getTextContent()`.
    var textContent = ""

    override func accessibilityLabel() -> String? {
      func labels(_ element: Any) -> [String] {
        guard let element = element as? NSAccessibilityProtocol & NSObject else { return [] }
        if let label = element.accessibilityLabel(), !label.isEmpty { return [label] }
        return (element.accessibilityChildren() ?? []).flatMap(labels)
      }
      if let own = super.accessibilityLabel(), !own.isEmpty { return own }
      let found = (accessibilityChildren() ?? []).flatMap(labels)
      if !found.isEmpty { return found.joined(separator: ", ") }
      return textContent.isEmpty ? nil : textContent
    }
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
        proposedLineFragment: proposedLineFragment, font: attributes[.font] as? NSFont) ?? .zero
    }

    override func loadView() {
      guard let attachment = textAttachment as? LexicalDecoratorAttachment else { return }
      view = DecoratorContainerView(content: attachment.ruleView ?? attachment.hostingView)
    }
  }

  /// A view provider's view, holding the attachment's one view. TextKit
  /// makes a new provider each time it lays the decorator's line out again
  /// and removes the old provider's view only after it shows the new one,
  /// so the attachment's view, shared by both, would leave with the old
  /// one. Each provider has its own container instead, and the one shown
  /// last holds the view.
  final class DecoratorContainerView: NSView {
    let content: NSView

    init(content: NSView) {
      self.content = content
      super.init(frame: content.frame)
      hold()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override func viewDidMoveToWindow() {
      super.viewDidMoveToWindow()
      if window != nil { hold() }
    }

    private func hold() {
      guard content.superview !== self else { return }
      content.frame = bounds
      content.autoresizingMask = [.width, .height]
      addSubview(content)
    }
  }

  /// A click that reaches the decorator's view and not the text view.
  final class DecoratorClickRecognizer: NSClickGestureRecognizer {
    var onClick: (() -> Void)?

    override init(target: Any?, action: Selector?) {
      super.init(target: nil, action: nil)
      self.target = self
      self.action = #selector(clicked)
      delaysPrimaryMouseButtonEvents = false
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    @objc private func clicked() { onClick?() }
  }
#endif
