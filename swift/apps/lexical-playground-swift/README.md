# Lexical playground for Swift

The Swift counterpart of [the Lexical playground](../../../packages/lexical-playground),
for iOS 26 and macOS 26. It builds the package from `../..`.

- **Toolbar:** undo and redo, the block type (normal, headings 1–6,
  bulleted, numbered and check lists, quote, code block), list buttons,
  bold, italic, underline, strikethrough, code, subscript, superscript and
  highlight, a link editor (insert, edit, remove), clear formatting,
  alignment, and indent and outdent. The buttons follow the selection.
- **Editing:** Markdown shortcuts (`# `, `- `, `1. `, `[ ] `, `> `,
  ` ``` `, `**bold**`, …), autolinked URLs and email addresses, and check
  boxes that toggle on a click or tap. Links open on a click while the
  editor is read-only.
- **Debug view:** the tree view of `@lexical/devtools-core`
  (`LexicalDevtoolsCore.generateContent`): the node tree with the
  selection, the last commands and the editor's settings, or the exported
  HTML. It sits beside the editor in a regular-width window and below it
  otherwise.
- **Actions:** import and export as JSON, HTML or Markdown, clear, read-only, and
  settings (rich or plain text, the debug view, editable, the welcome
  document).

The Xcode project is generated with [XcodeGen](https://github.com/yonaskolb/XcodeGen)
and not checked in:

```sh
cd swift/apps/lexical-playground-swift
xcodegen generate
open LexicalPlayground.xcodeproj
```

## Tests

`UITests/` has smoke tests for the iOS Simulator: the tree view shows the
welcome document, the Bold button formats the selection, and JSON export
shows the document.

```sh
xcodebuild test -project LexicalPlayground.xcodeproj -scheme LexicalPlayground \
  -destination 'platform=iOS Simulator,name=iPhone 18 Pro' \
  -skipMacroValidation -skipPackagePluginValidation
```

On a device, pass `-allowProvisioningUpdates DEVELOPMENT_TEAM=<team>`.
