#if canImport(UIKit)
  import LexicalA11y
  import UIKit

  // The editor's live region (`AriaLiveRegionExtension`): each new text is
  // spoken as a VoiceOver announcement, where JS's `aria-live` region is
  // read by the screen reader.
  extension LexicalUITextView: @MainActor LiveRegionHost {
    public func mountLiveRegion(_ region: LiveRegion) { liveRegion = region }

    public func unmountLiveRegion(_ region: LiveRegion) {
      if liveRegion === region { liveRegion = nil }
    }

    public func liveRegionDidUpdate(_ region: LiveRegion) {
      let text = region.textContent.announcementText
      guard !text.isEmpty else { return }
      let priority: UIAccessibilityPriority = region.politeness == .assertive ? .high : .default
      UIAccessibility.post(
        notification: .announcement,
        argument: NSAttributedString(
          string: text, attributes: [.accessibilitySpeechAnnouncementPriority: priority]))
    }
  }

  extension String {
    /// The text to speak: without the U+200B that marks a repeat.
    var announcementText: String {
      hasSuffix("\u{200B}") ? String(dropLast()) : self
    }
  }
#endif
