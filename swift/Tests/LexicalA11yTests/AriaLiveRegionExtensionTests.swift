import Lexical
import LexicalA11y
import LexicalExtension
import LexicalRichText
import LexicalTesting
import Testing

// Port of packages/lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts.
// The region is a `LiveRegion` on the root element (or `owner`) instead of
// a `div[aria-live]` in the root's document.

private func buildEditor(
  _ region: any AnyLexicalExtensionArgument = AriaLiveRegionExtension
) throws -> Editor {
  try buildEditorFromExtensions(
    defineExtension(name: "[root]", dependencies: [region, RichTextExtension]))
}

@Suite struct AriaLiveRegionExtensionTests {
  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > mounts a polite live region on the root document by default"
    ))
  func mountsPoliteRegionByDefault() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    let root = try mountRoot(editor)
    #expect(root.regions.count == 1)
    #expect(root.regions[0].politeness == .polite)
    #expect(root.regions[0].atomic)
    #expect(root.regions[0].role == "status")
  }

  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > disposes the live region when the editor is disposed"
    ))
  func disposesRegionWithEditor() throws {
    let editor = try buildEditor()
    let root = try mountRoot(editor)
    #expect(root.regions.count == 1)
    editor.dispose()
    #expect(root.regions.isEmpty)
  }

  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > respects assertive politeness from config"
    ))
  func respectsAssertiveConfig() throws {
    let editor = try buildEditor(
      configExtension(AriaLiveRegionExtension) { $0.politeness = .assertive })
    defer { editor.dispose() }
    let root = try mountRoot(editor)
    #expect(root.regions.first?.politeness == .assertive)
  }

  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > reflects politeness changes from the output signal at runtime"
    ))
  func reflectsPolitenessSignal() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    let root = try mountRoot(editor)
    let region = try #require(root.regions.first)
    #expect(region.politeness == .polite)
    let output = try getExtensionDependencyFromEditor(editor, AriaLiveRegionExtension).output
    output.politeness.value = .assertive
    #expect(region.politeness == .assertive)
  }

  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > mounts onto a custom owner element regardless of the root"
    ))
  func mountsOntoCustomOwner() throws {
    let owner = LiveRegionOwner()
    let editor = try buildEditor(
      configExtension(AriaLiveRegionExtension) { $0.owner = owner })
    defer { editor.dispose() }
    #expect(owner.regions.count == 1)
  }

  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > exposes a stable announce sink via dependency output"
    ))
  func exposesAnnounceSink() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    let root = try mountRoot(editor)
    let output = try getExtensionDependencyFromEditor(editor, AriaLiveRegionExtension).output
    output.announce("hello")
    #expect(root.regions.first?.textContent == "hello")
  }

  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > does not replay the last message into a region re-created on remount"
    ))
  func doesNotReplayOnRemount() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    let output = try getExtensionDependencyFromEditor(editor, AriaLiveRegionExtension).output
    let root1 = try mountRoot(editor)
    output.announce("Saved")
    #expect(root1.regions.first?.textContent == "Saved")

    try editor.setRootElement(nil)
    #expect(root1.regions.isEmpty)
    let root2 = try mountRoot(editor)
    let region = try #require(root2.regions.first)
    #expect(region.textContent == "")

    output.announce("Loaded")
    #expect(region.textContent == "Loaded")
  }

  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > announcing before a region is mounted is a no-op, not buffered for mount"
    ))
  func announcingBeforeMountIsDropped() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    let output = try getExtensionDependencyFromEditor(editor, AriaLiveRegionExtension).output
    output.announce("Early")

    let root = try mountRoot(editor)
    let region = try #require(root.regions.first)
    #expect(region.textContent == "")

    output.announce("Ready")
    #expect(region.textContent == "Ready")
  }

  @Test(
    .upstream(
      "lexical-a11y/src/__tests__/unit/AriaLiveRegionExtension.test.ts::AriaLiveRegionExtension > keeps a custom-owner region across editor root changes (no churn)"
    ))
  func keepsCustomOwnerRegionAcrossRoots() throws {
    let owner = LiveRegionOwner()
    let editor = try buildEditor(
      configExtension(AriaLiveRegionExtension) { $0.owner = owner })
    defer { editor.dispose() }
    let region = try #require(owner.regions.first)

    // The root hosts regions too, but the owner wins and never changes.
    let root = try mountRoot(editor)
    try editor.setRootElement(nil)

    #expect(owner.regions.count == 1)
    #expect(owner.regions.first === region)
    #expect(root.regions.isEmpty)
  }

  @Test func repeatedAnnouncementsStillChangeTheRegion() throws {
    let editor = try buildEditor()
    defer { editor.dispose() }
    let root = try mountRoot(editor)
    let output = try getExtensionDependencyFromEditor(editor, AriaLiveRegionExtension).output
    output.announce("Saved")
    output.announce("Saved")
    #expect(root.regions.first?.textContent == "Saved\u{200B}")
    output.announce("Saved")
    #expect(root.regions.first?.textContent == "Saved")
  }
}
