import Foundation
import Lexical
import SwiftUI

#if os(macOS)
  import AppKit
  typealias PlatformImage = NSImage
#else
  import UIKit
  typealias PlatformImage = UIImage
#endif

// Port of packages/lexical-playground/src/nodes/ImageComponent.tsx: the
// image at its own size, or the size the node gives it, no wider than its
// `maxWidth` (`style={{height, maxWidth, width}}`) or the line
// (`.editor-image img { max-width: 100% }`, which the views' decorator
// attachments apply to an inline decorator wider than the line), with
// the selection outline and `ImageResizer`'s handles while it is in a node
// selection, and its caption below it. Editing the caption is not ported
// yet.

/// The images loaded so far, by `src`. A decorator's view is sized when it
/// is attached, so loading one marks its nodes dirty, which attaches them
/// again at their size; the views observe the cache, since a node that
/// decorates the same view again does not draw it again.
@MainActor
@Observable
final class ImageCache {
  static let shared = ImageCache()
  private var images: [String: PlatformImage] = [:]
  private var failed: Set<String> = []
  @ObservationIgnored private var loading: [String: [() -> Void]] = [:]

  func image(_ src: String) -> PlatformImage? { images[src] }

  func hasFailed(_ src: String) -> Bool { failed.contains(src) }

  /// Loads `src` once; `done` runs when it has loaded or failed.
  func load(_ src: String, _ done: @escaping () -> Void) {
    if images[src] != nil || failed.contains(src) { return }
    if loading[src] != nil {
      loading[src]!.append(done)
      return
    }
    loading[src] = [done]
    let url: URL? =
      URL(string: src).flatMap { $0.scheme == nil ? nil : $0 }
      // A source without a scheme names one of the playground's own
      // images, as the web's bundled `src/images` are.
      ?? Bundle.main.url(
        forResource: (src as NSString).deletingPathExtension,
        withExtension: (src as NSString).pathExtension)
    guard let url else { return finish(src, nil) }
    URLSession.shared.dataTask(with: url) { data, _, _ in
      let bytes = data
      DispatchQueue.main.async {
        MainActor.assumeIsolated {
          ImageCache.shared.finish(src, bytes.flatMap { PlatformImage(data: $0) })
        }
      }
    }.resume()
  }

  private func finish(_ src: String, _ image: PlatformImage?) {
    if let image { images[src] = image } else { failed.insert(src) }
    for done in loading.removeValue(forKey: src) ?? [] { done() }
  }
}

/// Whether a node is in the editor's node selection
/// (`useLexicalNodeSelection`).
@MainActor
@Observable
final class NodeSelectionState {
  private(set) var isSelected = false
  @ObservationIgnored private var unregister: (() -> Void)?

  func observe(_ editor: Editor, _ key: NodeKey) {
    guard unregister == nil else { return }
    let read = { [weak self] in
      let selected =
        (try? editor.read(.latest) { (try getSelection() as? NodeSelection)?.has(key) ?? false })
        ?? false
      MainActor.assumeIsolated { if self?.isSelected != selected { self?.isSelected = selected } }
    }
    read()
    unregister = editor.registerUpdateListener { _ in read() }
  }

  isolated deinit { unregister?() }
}

/// `ImageComponent`.
struct ImageComponent: View {
  let editor: Editor
  let nodeKey: NodeKey
  let src: String
  let altText: String
  let width: ImageDimension
  let height: ImageDimension
  let maxWidth: Double
  let showCaption: Bool
  let caption: String
  @State private var selection = NodeSelectionState()
  /// The size while a handle is dragged, which the node takes when it is
  /// let go (`onResizeEnd`).
  @State private var resizing: CGSize?

  /// The image's size: the node's, or else the image's own, scaled down
  /// to `maxWidth`.
  private func size(_ image: PlatformImage?) -> CGSize {
    let natural = image?.size ?? CGSize(width: 200, height: 200)
    var w = width.points ?? natural.width
    var h =
      height.points
      ?? (width.points.map { $0 * natural.height / max(natural.width, 1) } ?? natural.height)
    if w > maxWidth {
      h *= maxWidth / w
      w = maxWidth
    }
    return CGSize(width: w, height: h)
  }

  private func click() {
    #if os(macOS)
      let shiftKey = NSEvent.modifierFlags.contains(.shift)
    #else
      let shiftKey = false
    #endif
    _ = try? editor.dispatchCommand(
      CLICK_COMMAND, MouseEvent("click", shiftKey: shiftKey, targetKey: nodeKey))
  }

  var body: some View {
    let image = ImageCache.shared.image(src)
    let size = resizing ?? size(image)
    VStack(alignment: .leading, spacing: 0) {
      Group {
        if let image {
          #if os(macOS)
            Image(nsImage: image).resizable()
          #else
            Image(uiImage: image).resizable()
          #endif
        } else {
          // `BrokenImage` once loading failed; nothing while it loads.
          Rectangle().fill(Color.gray.opacity(ImageCache.shared.hasFailed(src) ? 0.2 : 0.08))
            .overlay {
              if ImageCache.shared.hasFailed(src) {
                Image(systemName: "photo").font(.largeTitle).foregroundStyle(.secondary)
              }
            }
        }
      }
      // Its size, or smaller where the view gives it less (a line
      // narrower than the image), keeping the aspect.
      .aspectRatio(size.width / max(size.height, 1), contentMode: .fit)
      .frame(
        idealWidth: size.width, maxWidth: size.width, idealHeight: size.height,
        maxHeight: size.height
      )
      // `.editor-image img.focused { outline: 2px solid rgb(60, 132, 244) }`.
      .overlay {
        if selection.isSelected {
          Rectangle().strokeBorder(
            Color(red: 60 / 255, green: 132 / 255, blue: 244 / 255), lineWidth: 2)
        }
      }
      // ImageComponent's `onClick`, as `CLICK_COMMAND` with the image's
      // key: SwiftUI's hosting view takes the click from the attachment's
      // own recognizer.
      .contentShape(Rectangle())
      .onTapGesture { click() }
      .accessibilityElement()
      .accessibilityLabel(altText)
      .accessibilityAction { click() }
      .accessibilityAddTraits(.isImage)
      .accessibilityAddTraits(selection.isSelected ? .isSelected : [])
      .accessibilityIdentifier("editor-image")
      .overlay {
        if selection.isSelected || resizing != nil, editor.isEditable() {
          ImageResizer(size: size, maxWidth: maxWidth, resizing: $resizing) { size in
            let key = nodeKey
            try? editor.update {
              try (getNodeByKey(key) as? ImageNode)?.setWidthAndHeight(
                .points(size.width), .points(size.height))
            }
          }
        }
      }
      if showCaption, !caption.isEmpty {
        // `.image-caption-container`.
        Text(caption)
          .font(.system(size: 14))
          .foregroundStyle(.black)
          .frame(maxWidth: size.width, alignment: .leading)
          .padding(.vertical, 4)
          .background(Color.white.opacity(0.9))
      }
    }
    .onAppear {
      selection.observe(editor, nodeKey)
      if image == nil {
        let key = nodeKey
        nonisolated(unsafe) let editor = editor
        ImageCache.shared.load(src) {
          // Attach the image again at its loaded size.
          try? editor.update(tag: HISTORY_MERGE_TAG) { try getNodeByKey(key)?.markDirty() }
        }
      }
    }
  }
}

/// `ImageResizer`: the eight handles of `.image-resizer`, which resize the
/// image from its edges (keeping the other side) or its corners (keeping
/// its aspect), between 100 points and `maxWidth` wide. Upstream bounds the
/// height by the editor's root element, which the native views do not
/// have; it is bounded by 10 000 points.
private struct ImageResizer: View {
  let size: CGSize
  let maxWidth: Double
  @Binding var resizing: CGSize?
  let onResizeEnd: (CGSize) -> Void
  @State private var start: CGSize?

  /// `Direction`.
  private struct Direction: OptionSet {
    let rawValue: Int
    static let east = Direction(rawValue: 1 << 0)
    static let north = Direction(rawValue: 1 << 3)
    static let south = Direction(rawValue: 1 << 1)
    static let west = Direction(rawValue: 1 << 2)
  }

  private static let minWidth = 100.0
  private static let minHeight = 100.0
  private static let maxHeight = 10_000.0

  private func clamp(_ value: Double, _ low: Double, _ high: Double) -> Double {
    min(max(value, low), high)
  }

  /// `handlePointerMove`: the size for a drag by `translation`.
  private func resized(_ start: CGSize, _ direction: Direction, _ translation: CGSize) -> CGSize {
    let isHorizontal = !direction.isDisjoint(with: [.east, .west])
    let isVertical = !direction.isDisjoint(with: [.north, .south])
    let maxWidth = max(maxWidth, Self.minWidth)
    // `Math.floor(startX - clientX)`, negated for the east handles.
    let diffX = (-translation.width).rounded(.down)
    let dx = direction.contains(.east) ? -diffX : diffX
    if isHorizontal && isVertical {
      let width = clamp(start.width + dx, Self.minWidth, maxWidth)
      return CGSize(width: width, height: width / max(start.width / start.height, 0.001))
    }
    if isVertical {
      let diffY = (-translation.height).rounded(.down)
      let dy = direction.contains(.south) ? -diffY : diffY
      return CGSize(
        width: start.width, height: clamp(start.height + dy, Self.minHeight, Self.maxHeight))
    }
    return CGSize(width: clamp(start.width + dx, Self.minWidth, maxWidth), height: start.height)
  }

  private func handle(_ direction: Direction, _ alignment: Alignment) -> some View {
    // 7 points with a 1-point white border, as in the CSS, but inside the
    // image's edges rather than 6 points outside them: the image's hosting
    // view is its size, and neither view system hits a point outside it.
    return Rectangle()
      .fill(Color(red: 60 / 255, green: 132 / 255, blue: 244 / 255))
      .frame(width: 7, height: 7)
      .border(Color.white, width: 1)
      .frame(width: 9, height: 9)
      // A larger target for a finger, as the drawn handle is small.
      .padding(8)
      .contentShape(Rectangle())
      .gesture(
        DragGesture(minimumDistance: 1, coordinateSpace: .global)
          .onChanged { value in
            let from = start ?? size
            start = from
            resizing = resized(from, direction, value.translation)
          }
          .onEnded { _ in
            let final = resizing
            start = nil
            if let final { onResizeEnd(final) }
            resizing = nil
          }
      )
      .accessibilityHidden(true)
      .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: alignment)
  }

  var body: some View {
    ZStack {
      handle(.north, .top)
      handle([.north, .east], .topTrailing)
      handle(.east, .trailing)
      handle([.south, .east], .bottomTrailing)
      handle(.south, .bottom)
      handle([.south, .west], .bottomLeading)
      handle(.west, .leading)
      handle([.north, .west], .topLeading)
    }
    // The padding around each handle reaches past the image.
    .padding(-8)
  }
}
