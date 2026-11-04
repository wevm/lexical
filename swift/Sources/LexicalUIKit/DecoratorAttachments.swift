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
    /// Called with the key of a decorator that was tapped.
    var onClick: ((NodeKey) -> Void)?

    func attachment(for span: TextKitDecoratorSpan, editor: Editor) -> NSTextAttachment? {
      guard let decorated = editor.getDecorators()[span.key], let view = decorated as? any View
      else {
        attachments[span.key] = nil
        return nil
      }
      let attachment =
        attachments[span.key]
        ?? LexicalDecoratorAttachment(key: span.key) { [weak self] key in self?.onClick?(key) }
      attachment.update(AnyView(view), isInline: span.isInline)
      attachment.textContent =
        (try? editor.getEditorState().read { try getNodeByKey(span.key)?.getTextContent() }) ?? ""
      attachments[span.key] = attachment
      return attachment
    }

    /// Forgets the attachments of decorators that are gone.
    func retain(_ keys: Set<NodeKey>) {
      attachments = attachments.filter { keys.contains($0.key) }
    }
  }

  /// A decorator's attachment: a hosting view for its SwiftUI view.
  final class LexicalDecoratorAttachment: NSTextAttachment {
    let key: NodeKey
    nonisolated(unsafe) let host: UIHostingController<AnyView>
    /// Read by layout, which TextKit runs on the main thread.
    nonisolated(unsafe) private var size = CGSize.zero
    nonisolated(unsafe) private var isInline = true
    /// The decorator's `getTextContent()`.
    nonisolated(unsafe) var textContent = ""

    @MainActor
    init(key: NodeKey, onClick: @escaping (NodeKey) -> Void) {
      self.key = key
      host = UIHostingController(rootView: AnyView(EmptyView()))
      host.view.backgroundColor = .clear
      super.init(data: nil, ofType: nil)
      allowsTextAttachmentView = true
      // What TextKit draws where it does not show the view, in place of
      // its file icon.
      image = UIGraphicsImageRenderer(size: CGSize(width: 1, height: 1)).image { _ in }
      let tap = DecoratorTapRecognizer()
      tap.onTap = { onClick(key) }
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
    /// view's size, a block decorator the line's width.
    func bounds(proposedLineFragment: CGRect) -> CGRect {
      if isInline {
        return CGRect(origin: .zero, size: size)
      }
      return CGRect(x: 0, y: 0, width: max(proposedLineFragment.width, 1), height: size.height)
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
        proposedLineFragment: proposedLineFragment) ?? .zero
    }

    override func loadView() {
      view = (textAttachment as? LexicalDecoratorAttachment)?.host.view
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
