import Lexical
import LexicalCodeCore
import LexicalExtension
import LexicalLink
import LexicalList
import LexicalMarkdown
import LexicalRichText

// Shared helpers for the ports of packages/lexical-markdown/src/__tests__.

/// `MarkdownTestExtension` (`__tests__/utils.ts`): the nodes the default
/// `TRANSFORMERS` need, as extensions. Nodes only: a test that needs the
/// shortcut listener registers it from an extension of its own.
nonisolated(unsafe) let MarkdownTestExtension = defineExtension(
  name: "@lexical/markdown/test",
  dependencies: [RichTextExtension, ListExtension, LinkExtension, CodeExtension])

/// LexicalMarkdown.test.ts's `createTestEditor(config)`: a headless editor
/// with the heading, list, quote, code and link nodes (or `nodes`).
func createMarkdownTestEditor(nodes: [LexicalNodeConfig]? = nil) throws -> Editor {
  try createEditor(
    CreateEditorArgs(
      nodes: nodes
        ?? [
          .klass(HeadingNode.self), .klass(ListNode.self), .klass(ListItemNode.self),
          .klass(QuoteNode.self), .klass(CodeNode.self), .klass(LinkNode.self),
        ], onError: { throw $0 }, headless: true))
}
