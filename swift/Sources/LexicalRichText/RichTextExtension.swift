import Lexical
import LexicalExtension

// Port of packages/lexical-rich-text/src/LexicalRichTextExtension.ts.

/// `RichTextConfig`.
public struct RichTextConfig {
  public var escapeFormatTriggers: EscapeFormatTriggerConfig
  public var shouldHandlePasteAsFiles: ShouldHandlePasteAsFiles

  public init(
    escapeFormatTriggers: EscapeFormatTriggerConfig = DEFAULT_ESCAPE_FORMAT_TRIGGERS,
    shouldHandlePasteAsFiles: @escaping ShouldHandlePasteAsFiles = defaultShouldHandlePasteAsFiles
  ) {
    self.escapeFormatTriggers = escapeFormatTriggers
    self.shouldHandlePasteAsFiles = shouldHandlePasteAsFiles
  }
}

/// `mergeEscapeFormatTriggers`: each format's triggers merge key by key; a
/// format set to `.some(nil)` (JS `null`) replaces the default outright.
func mergeEscapeFormatTriggers(
  _ config: EscapeFormatTriggerConfig, _ overrides: EscapeFormatTriggerConfig
) -> EscapeFormatTriggerConfig {
  var merged = config
  for (format, override) in overrides {
    let base = config[format] ?? nil
    if let base, let override {
      merged.updateValue(base.merged(with: override), forKey: format)
    } else {
      merged.updateValue(override, forKey: format)
    }
  }
  return merged
}

/// `RichTextExtension`: headings, quotes and the rich text commands.
///
/// Of upstream's dependencies, only NormalizeInlineElementsExtension is
/// ported. HeadingAnnounce needs @lexical/a11y; Dragon and
/// NormalizeTripleClickSelection handle browser events; the
/// DOMImportExtension rules arrive with that pipeline (DESIGN.md).
public let RichTextExtension = defineExtension(
  name: "@lexical/rich-text", config: RichTextConfig(), conflictsWith: ["@lexical/plain-text"],
  dependencies: [NormalizeInlineElementsExtension],
  mergeConfig: { config, overrides in
    var merged = overrides.applied(to: config)
    if let triggers = overrides.escapeFormatTriggers {
      merged.escapeFormatTriggers = mergeEscapeFormatTriggers(config.escapeFormatTriggers, triggers)
    }
    return merged
  },
  nodes: { [.klass(HeadingNode.self), .klass(QuoteNode.self)] },
  build: { _, config, _ in namedSignals(config) },
  register: { editor, _, state in
    let output = state.getOutput()
    return try effect { () -> (() -> Void)? in
      registerRichText(
        editor, output.escapeFormatTriggers, output.shouldHandlePasteAsFiles)
    }
  })
