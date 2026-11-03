import Lexical
import LexicalExtension

// Port of packages/lexical-a11y/src/index.ts: the announcement extensions.
// The focus trap, roving tabindex and focus manager move DOM focus between
// HTML elements; native views get that behavior from the platform, so they
// are not ported (DESIGN.md, "Accessibility").

/// `AriaPoliteness`: how urgently a screen reader speaks an announcement.
public enum AriaPoliteness: String, Equatable, Sendable {
  case polite
  case assertive
}

/// The editor's live region, the `role="status"` element JS mounts with
/// `aria-live` and `aria-atomic="true"`. A `LiveRegionHost` shows it: an
/// Apple view speaks each new `textContent` as an accessibility
/// announcement, at a priority from `politeness`.
public final class LiveRegion {
  public let role = "status"
  public let atomic = true
  public internal(set) var politeness: AriaPoliteness = .polite {
    didSet { host?.liveRegionDidUpdate(self) }
  }
  /// The last announcement. A repeat of the same text ends in U+200B, as in
  /// JS, so the change still registers; hosts drop it before speaking.
  public var textContent = "" {
    didSet { host?.liveRegionDidUpdate(self) }
  }
  public private(set) weak var host: (any LiveRegionHost)?

  init(host: any LiveRegionHost) {
    self.host = host
  }

  func remove() {
    host?.unmountLiveRegion(self)
    host = nil
  }
}

/// What JS's live region is appended to: `owner`, or the root element's
/// document body. A root element that conforms hosts the editor's region;
/// any other root has none, and announcements are dropped.
public protocol LiveRegionHost: AnyObject {
  /// The region was created on this host.
  func mountLiveRegion(_ region: LiveRegion)
  /// The region was removed (the root or `owner` changed, or the editor
  /// was disposed).
  func unmountLiveRegion(_ region: LiveRegion)
  /// The region's `textContent` or `politeness` changed.
  func liveRegionDidUpdate(_ region: LiveRegion)
}

extension LiveRegionHost {
  public func mountLiveRegion(_ region: LiveRegion) {}
  public func unmountLiveRegion(_ region: LiveRegion) {}
  public func liveRegionDidUpdate(_ region: LiveRegion) {}
}

/// `AriaLiveRegionExtensionConfig`.
public struct AriaLiveRegionExtensionConfig {
  /// The politeness of the mounted region.
  public var politeness: AriaPoliteness
  /// The host to mount the region on. `nil` mounts it on the root element,
  /// when that is a `LiveRegionHost`.
  public var owner: (any LiveRegionHost)?

  public init(politeness: AriaPoliteness = .polite, owner: (any LiveRegionHost)? = nil) {
    self.politeness = politeness
    self.owner = owner
  }
}

/// `AriaLiveRegion`, the output of `AriaLiveRegionExtension`: the
/// `politeness` and `owner` signals, and a stable `announce` sink.
public final class AriaLiveRegion {
  public let politeness: Signal<AriaPoliteness>
  public let owner: Signal<(any LiveRegionHost)?>
  private let message: Signal<String>

  init(_ config: NamedSignals<AriaLiveRegionExtensionConfig>, message: Signal<String>) {
    self.politeness = config.politeness
    self.owner = config.owner
    self.message = message
  }

  /// Writes a message into the editor's live region. Announcing the same
  /// string back to back appends a zero-width space, so the region still
  /// changes. Does nothing until a region is mounted.
  public func announce(_ text: String) {
    // Toggle a trailing zero-width space on repeats so re-announcing the
    // same text still registers as a textContent change downstream.
    message.value = text == message.peek() ? text + "\u{200B}" : text
  }
}

/// `AriaLiveRegionExtension`: owns one live region for the editor and
/// exposes the `AriaLiveRegion` sink the other announcers write to.
public let AriaLiveRegionExtension = defineExtension(
  name: "@lexical/a11y/AriaLiveRegion", config: safeCast(AriaLiveRegionExtensionConfig()),
  dependencies: [RootElementExtension],
  // Private reactive message buffer shared by `build` (writer) and
  // `register` (renderer).
  init: { _, _, _ in signal("") },
  build: { _, config, state in
    AriaLiveRegion(namedSignals(config), message: try state.getInitResult())
  },
  register: { _, _, state in
    let message = try state.getInitResult()
    let output = state.getOutput()
    let rootElement = try state.getDependency(RootElementExtension).output
    let region = signal(LiveRegion?.none)
    return mergeRegister(
      // Create / dispose the region as the root (or `owner`) changes.
      // Subscribe to the root only when no explicit `owner` is configured,
      // so a fixed-owner region isn't torn down on unrelated root churn.
      try effect { () -> (() -> Void)? in
        let ownerHost = output.owner.value
        let host = ownerHost ?? (rootElement.value as? any LiveRegionHost)
        guard let host else {
          return nil
        }
        let el = LiveRegion(host: host)
        host.mountLiveRegion(el)
        region.value = el
        return {
          el.remove()
          region.value = nil
        }
      },
      // Apply the politeness to whatever region is mounted.
      try effect {
        if let el = region.value {
          el.politeness = output.politeness.value
        }
      },
      // Mirror the current message into the mounted region. The region is
      // peeked, so a re-created region does not replay the last message.
      try effect {
        let text = message.value
        if let el = region.peek() {
          el.textContent = text
        }
      })
  })

/// `HistoryAnnounceExtensionConfig`.
public struct HistoryAnnounceExtensionConfig {
  /// Message announced after an undo.
  public var undone: String
  /// Message announced after a redo.
  public var redone: String
  /// When `true`, undo and redo are not announced.
  public var disabled: Bool

  public init(undone: String = "Undone", redone: String = "Redone", disabled: Bool = false) {
    self.undone = undone
    self.redone = redone
    self.disabled = disabled
  }
}

/// `HistoryAnnounceExtension`: announces undo and redo through the
/// `AriaLiveRegionExtension` sink.
public let HistoryAnnounceExtension = defineExtension(
  name: "@lexical/a11y/HistoryAnnounce", config: safeCast(HistoryAnnounceExtensionConfig()),
  dependencies: [AriaLiveRegionExtension],
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let output = state.getOutput()
    let announcer = try state.getDependency(AriaLiveRegionExtension).output
    // Both return false to keep the history command chain intact.
    return try effect { () -> (() -> Void)? in
      if output.disabled.value {
        return nil
      }
      return mergeRegister(
        editor.registerCommand(
          UNDO_COMMAND,
          { _, _ in
            announcer.announce(output.undone.peek())
            return false
          }, COMMAND_PRIORITY_LOW),
        editor.registerCommand(
          REDO_COMMAND,
          { _, _ in
            announcer.announce(output.redone.peek())
            return false
          }, COMMAND_PRIORITY_LOW))
    }
  })

/// `EditorModeAnnounceExtensionConfig`.
public struct EditorModeAnnounceExtensionConfig {
  /// Message announced when the editor becomes editable.
  public var editable: String
  /// Message announced when the editor becomes read-only.
  public var readOnly: String
  /// When `true`, editable and read-only transitions are not announced.
  public var disabled: Bool

  public init(
    editable: String = "Editor is editable", readOnly: String = "Editor is read-only",
    disabled: Bool = false
  ) {
    self.editable = editable
    self.readOnly = readOnly
    self.disabled = disabled
  }
}

/// `EditorModeAnnounceExtension`: announces `setEditable(_:)` transitions
/// through the `AriaLiveRegionExtension` sink.
public let EditorModeAnnounceExtension = defineExtension(
  name: "@lexical/a11y/EditorModeAnnounce",
  config: safeCast(EditorModeAnnounceExtensionConfig()),
  dependencies: [AriaLiveRegionExtension],
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let output = state.getOutput()
    let announcer = try state.getDependency(AriaLiveRegionExtension).output
    return try effect { () -> (() -> Void)? in
      if output.disabled.value {
        return nil
      }
      return editor.registerEditableListener { (isEditable: Bool) in
        announcer.announce(isEditable ? output.editable.peek() : output.readOnly.peek())
      }
    }
  })
