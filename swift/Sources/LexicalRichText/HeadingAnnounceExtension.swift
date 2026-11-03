import Lexical
import LexicalA11y
import LexicalExtension

// Port of packages/lexical-rich-text/src/HeadingAnnounceExtension.ts.

/// `HeadingAnnounceExtensionConfig`.
public struct HeadingAnnounceExtensionConfig {
  /// Announced when a block becomes a heading. `%s` is replaced with the
  /// level (1-6).
  public var created: String
  /// Announced when a heading stops being a heading. `%s` is replaced with
  /// the level it was.
  public var destroyed: String
  /// When `true`, headings are not announced.
  public var disabled: Bool

  public init(
    created: String = "Heading level %s", destroyed: String = "Heading level %s removed",
    disabled: Bool = false
  ) {
    self.created = created
    self.destroyed = destroyed
    self.disabled = disabled
  }
}

/// `$readHeadingTag`.
private func readHeadingTag(_ key: NodeKey) throws -> HeadingTagType? {
  try (getNodeByKey(key) as? HeadingNode)?.getTag()
}

/// JS `String.prototype.replace` with a string pattern: the first match.
private func replacingFirst(_ text: String, _ pattern: String, _ with: String) -> String {
  guard let range = text.firstRange(of: pattern) else { return text }
  var result = text
  result.replaceSubrange(range, with: with)
  return result
}

/// `HeadingAnnounceExtension`: announces a block becoming a heading, and a
/// heading ceasing to be one, through the `AriaLiveRegionExtension` sink.
/// Typing inside a heading stays silent. A destroyed heading's level is read
/// from the previous state.
public let HeadingAnnounceExtension = defineExtension(
  name: "@lexical/rich-text/HeadingAnnounce", config: safeCast(HeadingAnnounceExtensionConfig()),
  dependencies: [AriaLiveRegionExtension],
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let output = state.getOutput()
    let announcer = try state.getDependency(AriaLiveRegionExtension).output
    return try effect { () -> (() -> Void)? in
      if output.disabled.value {
        return nil
      }
      return try editor.registerMutationListener(
        HeadingNode.self,
        { nodes, payload in
          // A level change fires both a removal and a creation; prefer the
          // creation, and announce a removal only when nothing replaced it.
          var createdTag: HeadingTagType?
          var destroyedTag: HeadingTagType?
          for (key, mutation) in nodes {
            if mutation == "created" && createdTag == nil {
              createdTag = try editor.read(.latest) { try readHeadingTag(key) }
            } else if mutation == "destroyed" && destroyedTag == nil {
              destroyedTag = try payload.prevEditorState.read { try readHeadingTag(key) }
            }
          }
          if let createdTag {
            announcer.announce(
              replacingFirst(output.created.peek(), "%s", String(createdTag.dropFirst())))
          } else if let destroyedTag {
            announcer.announce(
              replacingFirst(
                output.destroyed.peek(), "%s", String(destroyedTag.dropFirst())))
          }
        }, skipInitialization: true)
    }
  })
