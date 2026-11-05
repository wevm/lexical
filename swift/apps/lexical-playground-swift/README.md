# Lexical playground for Swift

The Swift counterpart of [the Lexical playground](../../../packages/lexical-playground),
for iOS 26 and macOS 26. It builds the package from `../..`.

- **Toolbar:** undo and redo, the block type (normal, headings 1–6,
  bulleted, numbered and check lists, quote, code block), list buttons,
  the font family, the font size stepper (8–72), text and background
  color pickers (inline styles through `patchStyleText`), an Insert menu
  with a horizontal rule, bold, italic, underline, strikethrough, code,
  subscript, superscript and highlight, a link button, clear formatting,
  alignment, and indent and outdent. The buttons follow the selection.
- **Editing:** Markdown shortcuts (`# `, `- `, `1. `, `[ ] `, `> `,
  ` ``` `, `**bold**`, …), autolinked URLs and email addresses, and check
  boxes that toggle on a click or tap. Links open on a click while the
  editor is read-only; while editing, a Command-click opens one, and a
  caret in a link shows the floating link editor below it
  (`FloatingLinkEditorPlugin`): the URL, which opens the link, with edit
  and remove. The toolbar's link button links the selection and edits its
  URL there.
- **Keyboard shortcuts:** the web playground's (`ShortcutsExtension`), from
  the editor and the Format menu: ⌥⌘0–3 for normal and headings 1–3,
  ⇧⌘7/8/9 for numbered, bulleted and check lists, ⌥⌘C code block, ⌃⇧Q
  quote, ⌘> and ⌘< font size, ⇧⌘C inline code, ⇧⌘X strikethrough, ⌃⇧1/2/3
  lowercase, uppercase and capitalize, ⇧⌘E/J/L/R alignment, ⌘, and ⌘.
  subscript and superscript, ⌘] and ⌘[ indent and outdent, ⌘\ clear
  formatting and ⌘K link. The Keyboard Shortcuts panel opens from the
  bottom bar and the Help menu. They work with a hardware keyboard on iOS
  too. Differences from the web: Add comment (⌥⌘M) is left out, as comments
  aren't ported; ⌘, is subscript as on the web, as the playground has no
  Settings menu item (its settings are a sheet); and the menu items' key
  equivalents only fire while the editor isn't focused, since the editor
  handles the same keys first.
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
