import Lexical
import LexicalA11y

/// Stands in for upstream's `document.body` with a root mounted in it: a
/// root element that hosts the editor's live region, so
/// `document.body.querySelectorAll('[aria-live]')` is `regions`.
final class LiveRegionHostRoot: LexicalRootElement, LiveRegionHost {
  var isConnected: Bool { true }
  private(set) var regions: [LiveRegion] = []

  func mountLiveRegion(_ region: LiveRegion) { regions.append(region) }
  func unmountLiveRegion(_ region: LiveRegion) { regions.removeAll { $0 === region } }
}

/// An explicit `owner` that is not a root element.
final class LiveRegionOwner: LiveRegionHost {
  private(set) var regions: [LiveRegion] = []

  func mountLiveRegion(_ region: LiveRegion) { regions.append(region) }
  func unmountLiveRegion(_ region: LiveRegion) { regions.removeAll { $0 === region } }
}

/// `mountRoot(editor)`: mounts a fresh root that hosts the live region.
@discardableResult
func mountRoot(_ editor: Editor) throws -> LiveRegionHostRoot {
  let root = LiveRegionHostRoot()
  try editor.setRootElement(root)
  return root
}

/// `readLiveRegion()`: the region's text, with the repeat marker stripped.
func readLiveRegion(_ root: LiveRegionHostRoot) -> String {
  String((root.regions.first?.textContent ?? "").filter { $0 != "\u{200B}" })
}
