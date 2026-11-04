import Lexical
import LexicalExtension
import LexicalHTML

// Port of packages/lexical-code-core/src/CodeExtension.ts.

/// `CodeExtension`: registers `CodeNode` and `CodeHighlightNode`, Enter
/// leaving a code block after two empty lines, a transform that clears the
/// format of tabs in code, and `CodeImportRules`.
public let CodeExtension = defineExtension(
  name: "@lexical/code",
  dependencies: [
    // DOMImportExtension support for the nodes registered here. Inert
    // unless the editor imports HTML through the DOMImportExtension
    // pipeline.
    CoreImportExtension,
    configExtension(DOMImportExtension) {
      $0.preprocess = [installVscodeCodePasteOverlay]
      $0.rules = CodeImportRules.map(\.entry)
    },
  ],
  nodes: { [.klass(CodeNode.self), .klass(CodeHighlightNode.self)] },
  register: { editor, _, _ in
    mergeRegister(
      editor.registerCommand(
        KEY_ENTER_COMMAND,
        { event, _ in
          guard let selection = try getSelection() as? RangeSelection,
            try exitCodeNodeOnEnter(selection) != nil
          else { return false }
          event?.preventDefault()
          return true
        }, COMMAND_PRIORITY_LOW),
      try editor.registerNodeTransform(TabNode.self) { node in
        if try node.getFormat() != 0, try node.getParent() is CodeNode {
          try node.setFormat(0)
        }
      })
  })
