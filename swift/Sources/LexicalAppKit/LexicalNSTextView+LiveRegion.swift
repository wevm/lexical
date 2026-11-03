#if os(macOS)
  import AppKit
  import LexicalA11y

  // The editor's live region (`AriaLiveRegionExtension`): each new text is
  // spoken as a VoiceOver announcement, where JS's `aria-live` region is
  // read by the screen reader.
  extension LexicalNSTextView: @MainActor LiveRegionHost {
    public func mountLiveRegion(_ region: LiveRegion) { liveRegion = region }

    public func unmountLiveRegion(_ region: LiveRegion) {
      if liveRegion === region { liveRegion = nil }
    }

    public func liveRegionDidUpdate(_ region: LiveRegion) {
      let text = region.textContent.announcementText
      guard !text.isEmpty else { return }
      let priority: NSAccessibilityPriorityLevel =
        region.politeness == .assertive ? .high : .medium
      NSAccessibility.post(
        element: self, notification: .announcementRequested,
        userInfo: [.announcement: text, .priority: priority.rawValue])
    }
  }

  extension String {
    /// The text to speak: without the U+200B that marks a repeat.
    var announcementText: String {
      hasSuffix("\u{200B}") ? String(dropLast()) : self
    }
  }
#endif
