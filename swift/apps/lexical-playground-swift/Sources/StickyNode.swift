import Foundation
import Lexical
import LexicalExtension
import LexicalHistory
import LexicalPlainText
import LexicalSwiftUI
import LexicalTextKit
import LexicalUtils
import SwiftUI

#if os(macOS)
  import AppKit
#else
  import UIKit
#endif

// Ports of packages/lexical-playground/src/nodes/StickyNode.tsx and
// StickyComponent.tsx: a sticky note with its own plain-text editor,
// floating over the editor at an offset from its top left corner. Upstream
// portals the note to the document's body; here `StickyNotesHost` puts the
// notes in the editor's text view, and the node itself draws nothing in
// the text, as upstream's `display: contents` element takes no room.

/// `StickyEditorExtension`: plain text sharing its parent's history, 24
/// point handwriting (`.StickyNode__contentEditable`) with the placeholder
/// in #999.
let StickyEditorExtension = defineExtension(
  name: "@lexical/playground/StickyEditor",
  dependencies: [SharedHistoryExtension, PlainTextExtension, NestedEditorExtension],
  editorConfig: InitialEditorConfig(namespace: "@lexical/playground/StickyEditor"),
  register: { editor, _, _ in
    editor.registerRootListener { root, _ in
      nonisolated(unsafe) let root = root
      MainActor.assumeIsolated {
        if let controller = textController(root) {
          controller.theme["root"]?.fontSize = .points(24)
          controller.theme["root"]?.fontFamily = [stickyFontFamily]
          controller.theme["root"]?.foregroundColor = TextKitColor(5, 5, 5)
          controller.theme["placeholder"] = TextKitStyle(
            fontFamily: [stickyFontFamily], fontSize: .points(24),
            foregroundColor: TextKitColor(153, 153, 153))
        }
        // The note draws the background.
        #if os(macOS)
          (root as? NSTextView)?.drawsBackground = false
          DispatchQueue.main.async {
            (root as? NSTextView)?.enclosingScrollView?.hasVerticalScroller = false
          }
        #else
          (root as? UIView)?.backgroundColor = .clear
        #endif
      }
      return nil
    }
  })

/// Upstream's 'Reenie Beanie' is a web font; the Apple platforms' own
/// handwriting face stands in for it.
let stickyFontFamily = "Noteworthy"

/// `StickyNoteColor`.
enum StickyNoteColor: String {
  case yellow, pink
}

/// `StickyNode`.
final class StickyNode: DecoratorNode {
  var __x: Double = 0
  var __y: Double = 0
  var __color = "yellow"
  /// Built when first read, so that clones, which share their original's,
  /// don't each build one.
  fileprivate var _caption: Editor?

  override class func config() throws -> NodeConfig? {
    try NodeConfig(
      StickyNode.self, type: "sticky", extends: DecoratorNode.self,
      json: [
        SchemaField(
          "caption", rawValue(), of: StickyNode.self,
          get: { .object(try $0.getSerializedCaption()) },
          set: { try $0.setCaption($1?.objectValue) }),
        SchemaField("color", enumValue(["yellow", "pink"]), field: \StickyNode.__color),
        SchemaField("xOffset", numberValue(), field: \StickyNode.__x),
        SchemaField("yOffset", numberValue(), field: \StickyNode.__y),
      ])
  }

  override func afterCloneFrom(_ prevNode: LexicalNode) throws {
    try super.afterCloneFrom(prevNode)
    _caption = (prevNode as! StickyNode).caption
  }

  /// `__caption`.
  var caption: Editor {
    if let _caption { return _caption }
    // The extension is the playground's own and always builds.
    let editor = try! buildEditorFromExtensions(StickyEditorExtension)
    _caption = editor
    return editor
  }

  /// The nested caption editor's own serialized state.
  func getSerializedCaption() throws -> JSONObject { try getLatest().caption.toJSON() }

  /// Applies a serialized nested caption editor; an empty state is ignored
  /// so it does not clobber the caption the node was created with.
  @discardableResult
  func setCaption(_ caption: JSONObject?) throws -> Self {
    let writable = try getWritable()
    if let state = caption?["editorState"]?.objectValue {
      let nested = writable.caption
      let editorState = try nested.parseEditorState(state)
      if !editorState.isEmpty() {
        try nested.setEditorState(editorState)
      }
    }
    return writable
  }

  func getColor() throws -> StickyNoteColor {
    StickyNoteColor(rawValue: try getLatest().__color) ?? .yellow
  }

  @discardableResult
  func setXOffset(_ xOffset: Double) throws -> Self {
    let writable = try getWritable()
    writable.__x = xOffset
    return writable
  }

  @discardableResult
  func setYOffset(_ yOffset: Double) throws -> Self {
    let writable = try getWritable()
    writable.__y = yOffset
    return writable
  }

  @discardableResult
  func setColor(_ color: StickyNoteColor) throws -> Self {
    let writable = try getWritable()
    writable.__color = color.rawValue
    return writable
  }

  /// Moves the note, and clears the selection, as upstream does.
  @discardableResult
  func setPosition(_ x: Double, _ y: Double) throws -> Self {
    let writable = try getWritable()
    writable.__x = x
    writable.__y = y
    try setSelection(nil)
    return writable
  }

  @discardableResult
  func toggleColor() throws -> Self {
    let writable = try getWritable()
    writable.__color = writable.__color == "pink" ? "yellow" : "pink"
    return writable
  }

  override func createDOM(_ config: EditorConfig, _ editor: Editor? = nil) throws -> DOMElement {
    let div = getDocument().createElement("div")
    div.style.setProperty("display", "contents")
    return div
  }

  override func updateDOM(
    _ prevNode: LexicalNode, _ dom: DOMElement, _ config: EditorConfig
  ) throws -> Bool { false }

  /// Nothing in the text: `StickyNotesHost` shows the note.
  override func decorate(_ editor: Editor) throws -> Any? {
    MainActor.assumeIsolated { EmptyView() }
  }

  override func isIsolated() -> Bool { true }
}

/// `$createStickyNode(xOffset, yOffset)`.
func createStickyNode(_ xOffset: Double, _ yOffset: Double) throws -> StickyNode {
  let node = try StickyNode()
  node.__x = xOffset
  node.__y = yOffset
  // Upstream builds the caption editor with the node.
  node._caption = node.caption
  return node
}

func isStickyNode(_ node: LexicalNode?) -> Bool { node is StickyNode }

/// What a note shows of its node.
struct StickyNote {
  let key: NodeKey
  let x: Double
  let y: Double
  let color: StickyNoteColor
  let caption: Editor
}

#if os(macOS)
  private typealias StickyHostingView = NSHostingView<StickyComponent>
#else
  private typealias StickyHostingView = UIView
#endif

/// Keeps the editor's sticky notes over its text view, each at its offset
/// from the top left corner of the text, as upstream portals each note to
/// the document's body at the root element's rect, so a note scrolls with
/// the text as upstream's does with the page. On macOS the notes are in the
/// text view; on iOS they are in the window, over the text view and
/// clipped to it, following its scrolling, as the typeahead menu is: the
/// text view's gestures would take the touches of its own subviews, and a
/// SwiftUI view over the editor gets none.
@MainActor
final class StickyNotesHost {
  private weak var editor: Editor?
  #if os(macOS)
    private weak var textView: NSView?
  #else
    private weak var textView: UIView?
  #endif
  private var notes: [NodeKey: (host: AnyObject, view: StickyHostingView, unregister: () -> Void)] =
    [:]
  private var unregisterMutations: (() -> Void)?
  #if os(iOS)
    private var offsetObservation: NSKeyValueObservation?
    /// The text view's frame in the window, which the notes are clipped to.
    private let clipView = PassThroughView()
    /// The last offset each note was put at, to follow scrolling.
    private var positions: [NodeKey: (note: StickyNote, translation: CGSize)] = [:]
  #endif

  /// Moves the notes into `root`, the editor's new root element.
  func attach(to root: AnyObject?, editor: Editor) {
    unregisterMutations?()
    unregisterMutations = nil
    #if os(iOS)
      offsetObservation = nil
      clipView.removeFromSuperview()
      clipView.clipsToBounds = true
    #endif
    for key in Array(notes.keys) { remove(key) }
    self.editor = editor
    #if os(macOS)
      textView = root as? NSView
    #else
      textView = root as? UIView
    #endif
    guard let textView else { return }
    #if os(iOS)
      offsetObservation = (textView as? UIScrollView)?.observe(\.contentOffset) {
        [weak self] _, _ in
        MainActor.assumeIsolated {
          guard let self else { return }
          for (key, position) in self.positions {
            self.move(key, position.note, position.translation, animated: false)
          }
        }
      }
    #endif
    unregisterMutations = try? editor.registerMutationListener(
      StickyNode.self,
      { [weak self] mutations, _ in
        MainActor.assumeIsolated {
          for (key, mutation) in mutations {
            if mutation == "destroyed" { self?.remove(key) } else { self?.update(key) }
          }
        }
      }, skipInitialization: false)
  }

  private func remove(_ key: NodeKey) {
    #if os(iOS)
      positions[key] = nil
    #endif
    guard let note = notes.removeValue(forKey: key) else { return }
    note.unregister()
    note.view.removeFromSuperview()
  }

  /// Shows note `key` as its node now is, moving it to its offset.
  private func update(_ key: NodeKey) {
    guard let editor, let textView,
      let note = try? editor.read(
        .latest,
        { () -> StickyNote? in
          guard let node = try getNodeByKey(key) as? StickyNode else { return nil }
          return StickyNote(
            key: key, x: node.__x, y: node.__y, color: try node.getColor(), caption: node.caption)
        })
    else { return remove(key) }
    let component = StickyComponent(
      editor: editor, note: note,
      onDrag: { [weak self] translation, ended in self?.drag(key, translation, ended: ended) })
    if let existing = notes[key] {
      #if os(macOS)
        existing.view.rootView = component
      #else
        (existing.host as? UIHostingController<StickyComponent>)?.rootView = component
      #endif
      move(key, note, .zero, animated: true)
      return
    }
    #if os(macOS)
      let view = NSHostingView(rootView: component)
      let host: AnyObject = view
    #else
      let host = UIHostingController(rootView: component)
      host.sizingOptions = []
      host.view.backgroundColor = .clear
      let view: UIView = host.view
    #endif
    #if os(macOS)
      textView.addSubview(view)
    #else
      clipView.addSubview(view)
    #endif
    // Dragging the paper moves the note, as `onPointerDown` on the note
    // (not its editor) starts a drag. On iOS a pan of the view's own, which
    // takes the touch only once it moves: a SwiftUI drag gesture in the note
    // keeps its buttons from firing. On macOS the hosting view takes the
    // mouse before a recognizer on it would, so an AppKit view behind the
    // note's content drags it (`stickyDrag`).
    #if os(iOS)
      let drag = StickyDrag(key: key, host: self)
      drag.attach(to: view)
    #endif
    // The caption grows the note as it is typed in.
    let unregister = note.caption.registerUpdateListener { [weak self] _ in
      MainActor.assumeIsolated {
        DispatchQueue.main.async { self?.move(key, note, .zero, animated: false) }
      }
    }
    notes[key] = (
      host, view,
      {
        unregister()
        #if os(iOS)
          _ = drag
        #endif
      }
    )
    move(key, note, .zero, animated: false)
  }

  /// A drag of note `key` moved by `translation`, or `ended` there.
  fileprivate func drag(_ key: NodeKey, _ translation: CGSize, ended: Bool) {
    guard let editor,
      let note = try? editor.read(
        .latest,
        { () -> StickyNote? in
          guard let node = try getNodeByKey(key) as? StickyNode else { return nil }
          return StickyNote(
            key: key, x: node.__x, y: node.__y, color: try node.getColor(), caption: node.caption)
        })
    else { return }
    move(key, note, translation, animated: false)
    if ended {
      let (x, y) = (note.x + translation.width, note.y + translation.height)
      try? editor.update { try (getNodeByKey(key) as? StickyNode)?.setPosition(x, y) }
    }
  }

  /// Puts note `key` at its offset plus `translation`, sized to fit.
  private func move(_ key: NodeKey, _ note: StickyNote, _ translation: CGSize, animated: Bool) {
    guard let entry = notes[key] else { return }
    #if os(macOS)
      let size = entry.view.fittingSize
    #else
      let size =
        (entry.host as? UIHostingController<StickyComponent>)?.sizeThatFits(
          in: CGSize(width: 10_000, height: 10_000)) ?? .zero
    #endif
    var frame = CGRect(
      x: note.x + translation.width, y: note.y + translation.height, width: size.width,
      height: size.height)
    #if os(iOS)
      positions[key] = (note, translation)
      // From the text's top left corner to the clip view's, which is
      // where the text view is, in the window just above the view that
      // holds the text view: under the sheets the playground presents. (In
      // SwiftUI's root view it would get no touches either.)
      if let textView, let window = textView.window {
        var top: UIView = textView
        while let superview = top.superview, superview !== window { top = superview }
        if clipView.superview !== window { window.insertSubview(clipView, aboveSubview: top) }
        clipView.frame = textView.convert(textView.bounds, to: window)
        frame = textView.convert(frame, to: clipView)
      }
    #endif
    // `transition: top 0.3s ease 0s, left 0.3s ease 0s`, but not while
    // dragging (`.dragging`).
    #if os(macOS)
      if animated, entry.view.frame.size == frame.size {
        NSAnimationContext.runAnimationGroup { context in
          context.duration = 0.3
          entry.view.animator().frame = frame
        }
      } else {
        entry.view.frame = frame
      }
    #else
      if animated, entry.view.frame.size == frame.size {
        UIView.animate(withDuration: 0.3) { entry.view.frame = frame }
      } else {
        entry.view.frame = frame
      }
    #endif
  }
}

#if os(iOS)
  /// A view that takes no touches itself, only its subviews do.
  private final class PassThroughView: UIView {
    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
      let view = super.hitTest(point, with: event)
      return view === self ? nil : view
    }
  }
#endif

#if os(iOS)
  /// A note's drag on iOS: a pan that starts on the note but not in its
  /// caption, moves the note while it lasts and writes its position when it
  /// ends.
  @MainActor
  private final class StickyDrag: NSObject, UIGestureRecognizerDelegate {
    let key: NodeKey
    weak var host: StickyNotesHost?

    init(key: NodeKey, host: StickyNotesHost) {
      self.key = key
      self.host = host
    }

    func attach(to view: UIView) {
      let pan = UIPanGestureRecognizer(target: self, action: #selector(panned))
      pan.delegate = self
      view.addGestureRecognizer(pan)
    }

    @objc private func panned(_ pan: UIPanGestureRecognizer) {
      let t = pan.translation(in: pan.view?.superview)
      let translation = CGSize(width: t.x, height: t.y)
      switch pan.state {
      case .changed: host?.drag(key, translation, ended: false)
      case .ended: host?.drag(key, translation, ended: true)
      case .cancelled, .failed: host?.drag(key, .zero, ended: false)
      default: break
      }
    }

    /// Not in the caption, whose text view takes its own touches.
    func gestureRecognizer(
      _ gestureRecognizer: UIGestureRecognizer, shouldReceive touch: UITouch
    ) -> Bool {
      var view = touch.view
      while let current = view, current !== gestureRecognizer.view {
        if current is UITextView { return false }
        view = current.superview
      }
      return true
    }
  }
#endif

/// `StickyComponent`: the note, with the look of `.sticky-note`. The
/// buttons delete it and toggle its color; dragging its paper moves it.
struct StickyComponent: View {
  /// The buttons at the top right, with their padding.
  static let buttonsSize = CGSize(width: 2 * 22 + 2 + 4, height: 1 + 22)

  let editor: Editor
  let note: StickyNote
  /// A drag of the paper by a translation, and whether it ended there.
  let onDrag: (CGSize, Bool) -> Void
  /// `.sticky-note`'s 120 pixels, as its 24-point text grows with the
  /// reader's text size, up to most of a phone's width.
  @ScaledMetric private var width: CGFloat = 120

  var body: some View {
    let isPink = note.color == .pink
    let base = isPink ? Color(hex: 0xF7CBE8) : Color(hex: 0xFFFF88)
    let corner = isPink ? Color(hex: 0xE7BFE1) : Color(hex: 0xFFFFC6)
    LexicalEditor(note.caption, placeholder: "What's up?")
      .sizedToContent(minHeight: 30, maxHeight: 400)
      .frame(width: min(width, 280))
      .padding(.vertical, 20).padding(.horizontal, 10)
      .background {
        // The 135° gradient, which reaches its corner color only in the
        // last fifth, and the curled bottom right corner.
        StickyPaper()
          .fill(
            LinearGradient(
              stops: [.init(color: base, location: 0.82), .init(color: corner, location: 1)],
              startPoint: .topLeading, endPoint: .bottomTrailing)
          )
          .overlay(
            StickyPaper().stroke(Color(white: 0.91))
          )
          .shadow(color: .black.opacity(0.3), radius: 5, x: 2, y: 12)
          .contentShape(Rectangle())
          .stickyDrag(onDrag)
      }
      .overlay(alignment: .topTrailing) {
        // Targets that fire in a hosted view on each platform
        // (`stickyNoteTap`).
        HStack(spacing: 2) {
          Image(systemName: "drop.fill").scaledFont(size: 11, relativeTo: .caption2)
            .stickyNoteButton("Change sticky note color", id: "sticky-color", action: toggleColor)
          Text("X").scaledFont(size: 10, relativeTo: .caption2)
            .stickyNoteButton("Delete sticky note", id: "sticky-delete", action: delete)
        }
        .foregroundStyle(.black)
        .opacity(0.5)
        .padding(.top, 1).padding(.trailing, 4)
        // Their targets keep their 22 points (the macOS drag area leaves
        // them out by that size), so their glyphs grow only as a bar's do.
        .barDynamicTypeSize()
      }
      .environment(\.colorScheme, .light)
      // `.sticky-note`'s margin, with room for its shadow.
      .padding(25)
      .accessibilityElement(children: .contain)
      .accessibilityIdentifier("sticky-note")
  }

  private func delete() {
    let key = note.key
    try? editor.update { try getNodeByKey(key)?.remove() }
  }

  private func toggleColor() {
    let key = note.key
    try? editor.update { try (getNodeByKey(key) as? StickyNode)?.toggleColor() }
  }
}

extension View {
  /// The paper's drag on macOS: an AppKit view behind the note's content,
  /// which takes the mouse only where the caption and the buttons do not.
  /// (A SwiftUI drag gesture in the note keeps its buttons from firing.) On iOS a pan on the hosting view (`StickyDrag`) moves the
  /// note instead.
  @ViewBuilder
  fileprivate func stickyDrag(_ onDrag: @escaping (CGSize, Bool) -> Void) -> some View {
    #if os(macOS)
      overlay(StickyDragArea(onDrag: onDrag))
    #else
      self
    #endif
  }

  /// A 22 point target that runs `action`: a plain `Button` on macOS, where
  /// a tap gesture in the note's hosting view does not fire, and a tap
  /// gesture on iOS, where a `Button` in a view hosted outside SwiftUI's own
  /// hierarchy does not.
  @ViewBuilder
  fileprivate func stickyNoteTap(_ action: @escaping () -> Void) -> some View {
    #if os(macOS)
      Button(action: action) { frame(width: 22, height: 22).contentShape(Rectangle()) }
        .buttonStyle(.plain)
    #else
      frame(width: 22, height: 22).contentShape(Rectangle()).onTapGesture(perform: action)
    #endif
  }

  /// One of the note's buttons: a 22 point target that runs `action`.
  fileprivate func stickyNoteButton(
    _ label: String, id: String, action: @escaping () -> Void
  ) -> some View {
    stickyNoteTap(action)
      .accessibilityElement(children: .ignore)
      .accessibilityLabel(label)
      .accessibilityAddTraits(.isButton)
      .accessibilityAction { action() }
      .accessibilityIdentifier(id)
  }
}

#if os(macOS)
  /// The view that drags the note on macOS, reporting the mouse's movement
  /// since it went down, downward positive as in the text view.
  private struct StickyDragArea: NSViewRepresentable {
    let onDrag: (CGSize, Bool) -> Void

    func makeNSView(context: Context) -> DragView { DragView() }

    func updateNSView(_ view: DragView, context: Context) { view.onDrag = onDrag }

    final class DragView: NSView {
      var onDrag: ((CGSize, Bool) -> Void)?
      private var start: NSPoint?

      override var isFlipped: Bool { true }

      /// Not under the buttons at the top right: the hosting view gives an
      /// AppKit view in it the mouse even under SwiftUI's views above it.
      override func hitTest(_ point: NSPoint) -> NSView? {
        let local = convert(point, from: superview)
        let buttons = CGRect(
          x: bounds.maxX - StickyComponent.buttonsSize.width, y: bounds.minY,
          width: StickyComponent.buttonsSize.width, height: StickyComponent.buttonsSize.height)
        return buttons.contains(local) ? nil : super.hitTest(point)
      }

      override func mouseDown(with event: NSEvent) { start = event.locationInWindow }

      override func mouseDragged(with event: NSEvent) {
        guard let translation = translation(to: event) else { return }
        onDrag?(translation, false)
      }

      override func mouseUp(with event: NSEvent) {
        if let translation = translation(to: event), translation != .zero {
          onDrag?(translation, true)
        }
        start = nil
      }

      private func translation(to event: NSEvent) -> CGSize? {
        guard let start else { return nil }
        return CGSize(
          width: event.locationInWindow.x - start.x, height: start.y - event.locationInWindow.y)
      }
    }
  }
#endif

/// `.sticky-note`'s box: `border-bottom-right-radius: 60px 5px`, an
/// elliptical corner 60 points wide and 5 tall.
private struct StickyPaper: Shape {
  func path(in rect: CGRect) -> Path {
    Path { path in
      path.move(to: CGPoint(x: rect.minX, y: rect.minY))
      path.addLine(to: CGPoint(x: rect.maxX, y: rect.minY))
      path.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY - 5))
      path.addQuadCurve(
        to: CGPoint(x: rect.maxX - 60, y: rect.maxY), control: CGPoint(x: rect.maxX, y: rect.maxY))
      path.addLine(to: CGPoint(x: rect.minX, y: rect.maxY))
      path.closeSubpath()
    }
  }
}

extension Color {
  /// A color from its `0xRRGGBB` hex.
  fileprivate init(hex: UInt32) {
    self.init(
      red: Double((hex >> 16) & 0xFF) / 255, green: Double((hex >> 8) & 0xFF) / 255,
      blue: Double(hex & 0xFF) / 255)
  }
}
