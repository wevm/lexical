import Lexical
import LexicalA11y
import LexicalExtension

// Port of packages/lexical-link/src/AutoLinkAnnounceExtension.ts.

/// `AutoLinkAnnounceExtensionConfig`. In the plural messages `%s` is the
/// count.
public struct AutoLinkAnnounceExtensionConfig {
  public var created: String
  public var createdMany: String
  public var destroyed: String
  public var destroyedMany: String
  public var disabled: Bool

  public init(
    created: String = "Link", createdMany: String = "%s links",
    destroyed: String = "Link removed", destroyedMany: String = "%s links removed",
    disabled: Bool = false
  ) {
    self.created = created
    self.createdMany = createdMany
    self.destroyed = destroyed
    self.destroyedMany = destroyedMany
    self.disabled = disabled
  }
}

/// JS `String.prototype.replace` with a string pattern: the first match.
private func replacingFirst(_ text: String, _ pattern: String, _ with: String) -> String {
  guard let range = text.firstRange(of: pattern) else { return text }
  var result = text
  result.replaceSubrange(range, with: with)
  return result
}

/// `AutoLinkAnnounceExtension`: announces autolinks being created or
/// removed, through the `AriaLiveRegionExtension` sink. An update that both
/// creates and removes autolinks (an edit that re-links) stays silent.
public let AutoLinkAnnounceExtension = defineExtension(
  name: "@lexical/link/AutoLinkAnnounce", config: AutoLinkAnnounceExtensionConfig(),
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
        AutoLinkNode.self,
        { nodes, _ in
          var createdCount = 0
          var destroyedCount = 0
          for (_, mutation) in nodes {
            if mutation == "created" {
              createdCount += 1
            } else if mutation == "destroyed" {
              destroyedCount += 1
            }
          }
          if createdCount > 0 && destroyedCount == 0 {
            announcer.announce(
              createdCount == 1
                ? output.created.peek()
                : replacingFirst(output.createdMany.peek(), "%s", String(createdCount)))
          } else if destroyedCount > 0 && createdCount == 0 {
            announcer.announce(
              destroyedCount == 1
                ? output.destroyed.peek()
                : replacingFirst(output.destroyedMany.peek(), "%s", String(destroyedCount)))
          }
        }, skipInitialization: true)
    }
  })
