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

    @MainActor
    init(key: NodeKey, onClick: @escaping (NodeKey) -> Void) {
      self.key = key
      host = UIHostingController(rootView: AnyView(EmptyView()))
      host.view.backgroundColor = .clear
      super.init(data: nil, ofType: nil)
      allowsTextAttachmentView = true
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

    override func attachmentBounds(
      for attributes: [NSAttributedString.Key: Any], location: any NSTextLocation,
      textContainer: NSTextContainer?, proposedLineFragment: CGRect, position: CGPoint
    ) -> CGRect {
      if isInline {
        return CGRect(origin: .zero, size: size)
      }
      return CGRect(x: 0, y: 0, width: max(proposedLineFragment.width, 1), height: size.height)
    }
  }

  final class DecoratorViewProvider: NSTextAttachmentViewProvider {
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
