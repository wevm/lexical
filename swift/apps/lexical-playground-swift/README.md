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
- **Typeahead menus:** "/" opens the component picker
  (`ComponentPickerPlugin`): paragraph, headings 1–3, the lists, quote,
  code, divider and the alignments, filtered by what follows the slash,
  chosen with the arrows and Enter or Tab, a click or a tap. The table,
  embed, equation, poll and other options insert nodes the port does not
  have yet. "@" and a name (`MentionsPlugin`) list up to five people from
  the web's dummy lookup, which answers after 500 ms, and insert a
  `MentionNode`. ":" and a keyword (`EmojiPickerPlugin`) list up to ten
  emoji; `Sources/emoji-list.json` is upstream's `utils/emoji-list.ts`
  (the gemoji list) with only the emoji, aliases and tags.
- **Floating format toolbar** (`FloatingTextFormatToolbarPlugin`): while
  text is selected, bold, italic, underline, strikethrough, subscript,
  superscript, the three cases, code and link. On macOS it floats above the
  selection, as on the web; on iOS the same actions are in the edit menu
  above the selection, since UIKit's menu appears where the popup would
  (the web leaves the popup out of narrow viewports).
- **Autocomplete** (off by default; Settings → Autocomplete): at the end of
  a word the rest of it shows in grey after the caret
  (`AutocompleteExtension`), accepted with Tab or the right arrow, or a
  swipe right on iOS. Only the English wordlist
  (`Sources/autocomplete-english.txt`) is bundled; the Korean one and the
  suggestions during an IME composition are not ported yet.
- **Images** (`ImagesExtension`, `ImageNode`): Insert → Image, or "/"
  and Image, opens the insert dialog: the sample image, a URL, or a file,
  with its alt text. Dropped or pasted image files are inserted too
  (`DragDropPasteExtension`). An image is drawn at its size, scaled down to
  the line's width and to its maximum width (500 by default), and a click
  or tap selects it as a whole with the blue outline and the resize
  handles (`ImageResizer`): an edge handle changes the width or height, a
  corner keeps the aspect, and the image takes its new size when the
  handle is let go. Its JSON and HTML are the web's. A caption shows as its
  text; editing it in its nested editor and dragging an image to move it
  are not ported yet, and HTML imported from the Import menu does not
  restore a `<figure>`'s caption, which upstream imports through the
  `DOMImportExtension` rules.
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
  settings (rich or plain text, the debug view, editable, autocomplete,
  the welcome document).

The Xcode project is generated with [XcodeGen](https://github.com/yonaskolb/XcodeGen)
and not checked in:

```sh
cd swift/apps/lexical-playground-swift
xcodegen generate
open LexicalPlayground.xcodeproj
```

## Tests

`UITests/` has smoke tests for the iOS Simulator: the tree view shows the
welcome document, the Bold button formats the selection, JSON export
shows the document, the typeahead menus insert blocks, mentions and emoji,
autocomplete, the edit menu's formats, and inserting an image. `Tests/`
has unit tests comparing `ImageNode`'s JSON and HTML with the web's.

```sh
xcodebuild test -project LexicalPlayground.xcodeproj -scheme LexicalPlayground \
  -destination 'platform=iOS Simulator,name=iPhone 18 Pro' \
  -skipMacroValidation -skipPackagePluginValidation
```

On a device, pass `-allowProvisioningUpdates DEVELOPMENT_TEAM=<team>`.
