# Changelog

Each release names the upstream Lexical version it conforms to. Entries come
from the changesets in `.changeset/` (see its README), and the release
workflow publishes the section headed `## <swift/VERSION>`.

## 0.1.5

Conforms to upstream Lexical **0.53.0** (`UPSTREAM_VERSION`).

### Patch Changes

- The HTML serializer (`generateHtmlFromNodes`, `outerHTML` and `innerHTML` of the port's DOM) writes `<` and `>` in an attribute value as `&lt;` and `&gt;`, as Chromium, WebKit and the current HTML standard do, so exported HTML matches the browser's byte for byte where an attribute holds them. Text and the other escapes are unchanged.
- `LexicalDecoratorBlockNode` ports `@lexical/react`'s `DecoratorBlockNode`, the base of block embeds (an alignment `format`, not indentable, a `<div>`), and `BlockWithAlignableContentsExtension`, the commands of `BlockWithAlignableContents`: `FORMAT_ELEMENT_COMMAND` aligns a selected block, and a click selects or deselects it. `LexicalAutoEmbed` ports the logic of `LexicalAutoEmbedPlugin`: `registerAutoEmbed` offers a node menu at a link that a paste of one token created when an `EmbedConfig` parses its URL, with `URL_MATCHER`, `INSERT_EMBED_COMMAND` and `AutoEmbedOption`.
- `LexicalNSTextView` and `LexicalUITextView` gain `blockRect(for:)`: the rect of a rendered block (a code block's box, padding included), as JS reads the block element's `getBoundingClientRect()`, for floating UI anchored at a block.
- `LexicalEditor` (SwiftUI) gains `theme(_:)`, which draws the text with a `TextKitTheme` such as the default with styles for an app's own nodes, and `editorBackground(_:)`, which makes the text view transparent so a message bubble or a composer's own background shows through. Its macOS scroll view now shows the scroller only while the content overflows. `LexicalNSTextView` and `LexicalUITextView` now keep a collapsed caret in view after each commit while focused, as upstream's `updateDOMSelection` does with `scrollIntoViewIfNeeded`, unless the update has the `skip-scroll-into-view` tag. Before, typing past the bottom of a scrolled or height-capped editor left the caret out of sight.
- A line break in the TextKit views takes its block's text look (font, size and color), as the web's `<br>` takes its block's font. Before, it took the editor's base look, so in a code block an empty line after Enter got a body-size caret drawn off its line, and the line above it drew its text lower once it ended in a break. Every code line, empty or not, now has the code style's line box and caret on macOS and iOS.
- `LexicalCodeCore` gains pluggable syntax highlighting: the `CodeTokenizer` protocol (Prism-style `CodeToken`s, `tokenize(codeNode:)` for `$tokenize`, and `languageState`/`loadLanguage` for grammars that load later), `CodeHighlighterExtension` with the tokenizer in its config, and `registerCodeHighlighting`. It ports the highlighting loop of `@lexical/code-prism`: the code-node transform, the token diff that replaces only changed `CodeHighlightNode`s, and caret retention. `ComposedCodeTokenizer` falls back from one tokenizer to another per language, and `PlainCodeTokenizer` highlights nothing.
- `LexicalCodeTreeSitter` is a new `CodeTokenizer` built on the tree-sitter C core (v0.27.1, vendored, called through a thin wrapper), with one opt-in product per grammar: `LexicalCodeTreeSitterJavaScript`, `TypeScript`, `Swift`, `Python`, `JSON`, `HTML`, `CSS`, `Rust`, `Go` and `Bash`. It parses incrementally over UTF-16, so tokens map straight to Lexical offsets, and maps each grammar's highlight captures to Prism's token names. `LexicalTextKit` now styles a `CodeHighlightNode` with the theme's `codeHighlight.<type>` key. `CodeTokenizer.tokenize(codeNode:)` returns `[LexicalNode]?`, where `nil`, the default, means the highlighter tokenizes the block's text, and the highlighter now builds nodes only for the tokens that changed.
- [feature] `LexicalNSTextView.contextMenuItems` adds an app's own items, built for the click event, before `NSTextView`'s items in the context menu that a right or Control click opens.
- A selected decorator can be dragged to move it, as a selected image's `draggable` element is on the web. On macOS a drag of its view and on iOS a long press and drag dispatch `DRAGSTART_COMMAND` from the decorator, and the drop dispatches `DROP_COMMAND` where it lands, so the handlers an app registers (an image extension's) place it. `LexicalNSTextView` takes a drop of a Lexical drag payload from itself as a move, which `NSTextView` refused. `View.lexicalDecoratorDragDisabled(_:)` keeps a decorator's view from being dragged, for example while it resizes.
- A text field in a decorator's SwiftUI view keeps editing when TextKit lays the decorator's line out again, in the AppKit and UIKit views. The view moved through a container outside the window, which ended the editing; on iOS the keyboard change that followed resized the text view during its own layout, and TextKit 2 raised (`textViewportElementsInRect: cannot be called during layout`).
- [feature] `LexicalUITextView` has `willPresentEditMenu` and `willDismissEditMenu`, called as UIKit presents and dismisses the edit menu (the view is its own `UITextViewDelegate`), so an app can move its floating views out of the menu's way.
- On iOS, a selected decorator (a node selection, such as a tapped image or horizontal rule) shows only its own outline, as on the web: `LexicalUITextView` turns UIKit's selection display (caret, highlight and handles) off while the selection is a node selection, and on again for text.
- `LexicalNSTextView` registers for the Lexical pasteboard types as drag types, so a drag that carries only a Lexical payload reaches its `DRAGOVER_COMMAND` and `DROP_COMMAND` handlers: `TextKitPasteboard.types` gains `application/x-lexical-drag-block` (`dev.lexical.drag-block`), the payload of a block dragged by its handle, and the view takes it from itself as a move.
- Add `LexicalFile`, a port of `@lexical/file` in its own library product: `SerializedDocument`, `serializedDocumentFromEditorState` and `editorStateFromSerializedDocument` write and read `.lexical` documents byte for byte as JS does, with the same `lastSaved`, `source` and `version` handling. `importFile` and `exportFile` take the app's file picker as a hook (`ReadTextFileFromSystem`, `ExportBlob`) in place of upstream's DOM file input and download link.
- Add `LexicalMark`, a port of `@lexical/mark` in its own library product: `MarkNode`, `wrapSelectionInMarkNode`, `unwrapMarkNode`, `getMarkIDs`, `MarkExtension` and the mark state extension. The TextKit views draw marks with the theme's `mark` and `markOverlap` backgrounds. `TextKitInputController.highlights` paints named model ranges behind the text without changing it, as a page's CSS custom highlights do, for things such as a selected comment's marks or search matches.
- `LexicalMenu`'s `MenuState` gains `onCloseAsync`, upstream's `onClose` that returns a promise: a closed menu keeps its resolution and its keys until it returns or throws, then clears, unless a newer query reopened the menu meanwhile, which it keeps.
- Parsing a document no longer traps on a `format`, `detail`, `textFormat` or `indent` number outside `Int`'s range: the flags read as JS's bitwise operators read them (`ToInt32`) and indents are clamped. Enter in an empty code-highlight node now throws and leaves the editor unchanged, as in JS, instead of trapping.
- Add `registerTableOfContents` to `LexicalRichText`, a port of `@lexical/react`'s `TableOfContentsPlugin` logic: it reports the document's headings (`TableOfContentsEntry`: key, text and tag), in order, at once and after each change to them, for a view to list.
- [fix] A line with an inline decorator taller than the line height, such as an inline image, draws its text on the decorator's baseline, as CSS puts an inline image's line's text, in `LexicalNSTextView` and `LexicalUITextView`. Its text was drawn centered in the tall line instead, overlapping the next line's text, which looked like that line twice; a list item's marker follows the text.
  
  Verify: an inline image in a paragraph or list item, with text before and after it and text that wraps below it.
- `TextKitInputController.setSuffix(_:)` shows a `TextKitSuffix` after a text node in the TextKit views, outside the editor state, as JS appends an unmanaged `<span>` to a text node's DOM: it is part of the text layout, so the text after it moves aside and it wraps, but the model, its selection, the history and copying never see it, a click in it lands at its node's end and the arrow keys cross it in one step. Typing into its node drops it. The theme has an `autocomplete` type (`#ccc`) for it. When the model ends an IME composition itself (`setCompositionKey(nil)`), the views now end the input method's marked text with it (`TextKitUpdate.endsComposition`), and on macOS a key the input method commits its marked text with and passes on (Tab, an arrow) reaches `KEY_DOWN_COMMAND` after the commit.
- `LexicalCodeTreeSitter` has eight more opt-in grammar products: `LexicalCodeTreeSitterC`, `CPP`, `Java`, `Markdown`, `ObjC`, `PowerShell`, `SQL` and `XML`, for the code languages `c`, `cpp`/`c++`, `java`, `markdown`/`md`, `objc`/`objectivec`, `powershell`/`ps1`, `sql` and `xml`. C++ and Objective-C highlight with C's query as well as their own, as their grammars extend it. Markdown uses the block grammar, so headings, lists, quotes and fences highlight, not emphasis or links. Each grammar's MIT license is vendored with it.
- `TreeSitterGrammar.tokenize` keeps the tokens of its last call outside the lines an edit re-types and builds new ones only for those lines, so a keystroke in a 200-line code block takes about 4.6 ms p50 rather than 7.1 ms on Linux. The tokens are the same as a fresh parse's.

## 0.1.4

Conforms to upstream Lexical **0.53.0** (`UPSTREAM_VERSION`).

### Patch Changes

- Decorator views in the TextKit views (images, horizontal rules, inline decorators such as the playground's date pill) no longer stop drawing after an edit lays their line out again. TextKit makes a new view provider for the line and releases the old one, which took the decorator's view out of the text after the new provider had shown it; each provider now shows the view in its own container. Fixed on macOS and iOS.
- `LexicalMenu`: `MenuState.drawsEmptyMenu` tells the menu that the app's view draws it even with no options, as a JS `menuRenderFn` can. Such a menu keeps the arrow, Enter, Tab and Escape keys instead of letting them through to the editor, and an arrow key leaves nothing highlighted, as upstream does.

## 0.1.3

Conforms to upstream Lexical **0.53.0** (`UPSTREAM_VERSION`).

### Patch Changes

- A quote's bar and a code block's box and gutter cover the block's last line when it is empty, as after a trailing line break or in an empty block, the way a trailing `<br>` gives the web's block an empty line box. On macOS, a block that changes at the end of the text, such as an empty paragraph that becomes a code block, lays its empty line out again with the block's spacing straight away instead of once something is typed.
- Add `registerCharacterLimit(_:maxCharacters:strlen:remainingCharacters:)` to `LexicalOverflow`, a port of `@lexical/react`'s `useCharacterLimit`: it wraps the text past the limit in `OverflowNode`s and reports the characters left. The TextKit views lay out an element inline when its `createDOM` makes an inline-level HTML element, as a browser does, so the text past the limit stays on its line, with the theme's `characterLimit` background.
  
  Verify: an element node that is a block in the model but makes a `<span>`, `<a>` or another inline-level element in `createDOM` now shows inline in the views.
- A key pressed with Command reports the `key` Safari does: Option applies and Shift doesn't (⇧⌘7 is `7`, ⌥⌘1 is `¡`, ⇧⌘. is `.`), on macOS and with a hardware keyboard on iOS. Shortcuts bound to Shift+Command and a digit or symbol, such as ⇧⌘7 for a numbered list, now match. With Control alone, Shift still applies, as before.
- The TextKit views attach a decorator again when its node changes, so its view shows the node's new properties, as React renders a decorator again. An inline decorator wider than its line is scaled down to the line's width, keeping its aspect, as an image's `max-width: 100%` scales it.
- `LexicalUITextView.editMenuElements` adds an app's menu elements to the edit menu UIKit shows for a selection, before the system's actions.
- The SwiftUI `LexicalEditor` can size itself to its content with `.sizedToContent(minHeight:maxHeight:)`: it grows with its text up to `maxHeight` and scrolls past it, as a chat composer does. Without the modifier it still fills the height it's offered. `LexicalNSTextView` and `LexicalUITextView` take the same range as `heightRange`, which sets their intrinsic height to their `contentHeight` clamped to that range. On iOS, an empty last line now has the last block's height and look, as on macOS, so an empty editor and its caret are as tall as a line of text.
- `LexicalNSTextView`, `LexicalUITextView` and the SwiftUI `LexicalEditor` (`LexicalEditor(editor, placeholder:)`) take a `placeholder`. It shows while the editor is empty, decided by `canShowPlaceholder` the way upstream's `ContentEditable` decides it. It sits on the first line, where typed text goes, in the theme's new `placeholder` style (the playground's `#999` root text). VoiceOver reads it as the view's placeholder value on macOS and as its value on iOS. `TextKitUpdate.canShowPlaceholder` reports the decision for each commit.
- Add `LexicalHashtag`, a port of `@lexical/hashtag`: `HashtagNode`, `createHashtagNode(_:)`, `isHashtagNode(_:)`, `HashtagExtension`, `HashtagConfig` and `registerLexicalHashtag(_:_:)`, with JS's matcher. The TextKit views style a text node whose type is not `text` with the theme style of its type, and the default theme gives hashtags the web playground's background.
- Add `LexicalText`, a port of `@lexical/text`: `registerLexicalTextEntity(_:_:_:_:)`, which turns the text a matcher finds into entity nodes (such as hashtags) and back, `EntityMatch`, `rootTextContent()`, `isRootTextContentEmpty(_:trim:)`, `canShowPlaceholder(_:)`, their curried forms, and `findTextIntersectionFromCharacters(_:_:)`.
- On macOS, a link is underlined while the pointer is over it, as the web playground's `__link:hover { text-decoration: underline }` does, and only that link. It is view state: no editor update or history entry. `TextKitStyle` gains `hoverUnderline`, which the default theme's `link` style sets.
- Add `LexicalOverflow`, a port of `@lexical/overflow`: `OverflowNode`, which holds the text past a character limit, `createOverflowNode()`, `isOverflowNode(_:)` and `OverflowExtension`.
- On macOS, the selection reaches the editor at each step of a click or a drag, as a browser's `selectionchange` does, rather than once the mouse button is released: update listeners (and a toolbar that shows the block type) and the caret's size follow the selection from the press on. Selection-only updates still add no history.
- `LexicalMenu` ports the platform-neutral half of `@lexical/react`'s typeahead and node menus: `basicTypeaheadTriggerMatch` (`useBasicTypeaheadTriggerMatch`), `MenuOption`, `registerTypeaheadMenu` and `NodeMenu`, which find the query before the caret, keep the options and the highlighted one, and handle the arrow keys, Enter, Tab and Escape at the menu's priority while it is open. The app draws the menu. `LexicalNSTextView` and `LexicalUITextView` gain `rect(from:to:)` and `screenRect(from:to:)`, the rect of a model range, to position floating UI at. On iOS the software keyboard's Return is a `keydown` of Enter, as in Safari, so an open menu takes it.

## 0.1.2

Conforms to upstream Lexical **0.53.0** (`UPSTREAM_VERSION`).

### Patch Changes

- The TextKit reconciler applies an update's edits to its rendered text in one pass instead of splicing the text once per edit, so restyling a whole document (select all and bold, on 5,000 blocks) is about 40% faster. The edits sent to the text storage are unchanged.
- Code blocks look like the web playground's: the default theme's `code` style draws a background, padding and a line-number gutter with a right border, in light and dark mode, and sets a tab size of 2. `TextKitStyle` gains `paddingTop`, `paddingBottom`, `gutter` (`TextKitGutter`) and `tabSize` for themes that want the same for their own blocks.
- Horizontal rules show on macOS and iOS: they rendered nothing, because `HorizontalRuleNode` has no view of its own. The views now draw one from the theme's new `horizontalRule` (`TextKitRule`), which the default theme sets to the web playground's 2-point #ccc line with 1em margins, outlined in blue while the rule is selected, without the selection highlight or iOS's handles.
- On macOS the pointer is a pointing hand over links, as the playground theme's `cursor: pointer` shows it on the web. `TextKitStyle` and `TextKitTextAttributes` gain `cursor` (`TextKitCursor`) for themes that want it for other text.

## 0.1.1

Conforms to upstream Lexical **0.53.0** (`UPSTREAM_VERSION`).

### Patch Changes

- The caret at the end of the text is as tall as the font on macOS and iOS, as it is everywhere else, instead of as tall as the whole line.
- On macOS the caret on an empty last block moves as soon as the block's indent or list marker changes (a paragraph that becomes a list item, a list item that becomes a paragraph), instead of once something is typed.
- A theme's `lineHeightMultiple` lays lines out as CSS `line-height` does on macOS and iOS: each line is that many times the font size, with its text centered in it rather than at the line's bottom, and the caret is as tall as the font instead of the line. The playground's text, quotes and list markers so sit as they do on the web.
- Node, editor and editor-state fields, the `JSMap` storage and the per-thread context skip Swift's dynamic exclusivity checks, which every node read and write paid for. Updates and loads are faster; behavior is unchanged.
- `JSMap`, `JSSet` and `JSONObject` hash string keys with a fast hash of their UTF-8 bytes instead of SipHash, which made node-map lookups a large share of every update and load. String keys now compare by code units, as a JS `Map` or object does: two keys that are canonically equivalent but encoded differently (`"\u{E9}"` and `"e\u{301}"`) are distinct, where they used to collapse into one.
- `getWritable()` on a node already marked dirty in the current pass returns it without marking it and its ancestors again. Setting several properties of one node, as importing JSON does, is faster; the dirty nodes an update reports are unchanged.
- `JSONValue.parse` (and so `parseEditorState`) is about five times faster: it reads the text's UTF-8 instead of copying it to UTF-16, decodes strings without escapes in one step, and keeps a `JSONObject`'s entries in one ordered array, which it searches linearly up to 16 keys. Parsed values, key order and error messages are unchanged; error positions are still UTF-16 offsets.
- On iOS, quote bars and list markers stay beside their text while the editor is pulled down past its top.
- The TextKit reconciler skips the unchanged children around an inserted, removed or moved block instead of walking and reconciling every child of its parent. Enter and joining paragraphs in a 5,000-block document now take about 4 ms (release), under the 8 ms target.
- Restyling a block's text after a commit walks only that block, not every text run in the document. Enter and joining paragraphs in a 5,000-block document went from about 23 ms to 12 ms (release).
- The selection highlight covers only the selected text on macOS and iOS, as in a browser: no longer the list markers, indents and quote bars before it, the rest of each line after it, or the line-height's spacing above and below.
- A TextKit render resolves each element's style chain and each distinct text and paragraph look once per update instead of once per run and block, and a full render no longer restyles the runs its one edit already styles. Loading a large document renders about a third faster; rendered attributes are unchanged.
- `LexicalNSTextView` no longer flashes a triple click's selection across the line's whole width: the paragraph selection it shows while the mouse is down stops before the newline, as `UITextView`'s does.

## 0.1.0

The first release. It conforms to upstream Lexical **0.53.0**: upstream
main just after the v0.53.0 tag, at 2b388b92d, the merge base of this
fork's main with facebook/lexical main (`UPSTREAM_VERSION` is `0.53.0`).

- **Core (`Lexical`)**: nodes, updates and reads, transforms, commands,
  listeners, selection, NodeState, carets, and legacy and compact JSON
  that match JS byte for byte, with the `@LexicalNode` macro for custom
  nodes. Offsets are UTF-16 code units, as in JS.
- **Packages**: `LexicalExtension`, `LexicalSelection`, `LexicalUtils`,
  `LexicalHTML`, `LexicalRichText`, `LexicalPlainText`, `LexicalClipboard`,
  `LexicalHistory`, `LexicalHeadless`, `LexicalA11y`, `LexicalList`,
  `LexicalLink`, `LexicalCodeCore`, `LexicalMarkdown` and
  `LexicalDevtoolsCore`, each a port of the upstream package of the same
  name. Every upstream unit test of these packages is ported or skipped
  with a reason in the parity ledger (`parity/skips.json`), except
  devtools-core's, which is checked by its tree-view vectors instead.
  Lists, links and code blocks serialize byte for byte as in JS, and
  Markdown import and export match JS on all 149 cases of upstream's
  import/export table. Code blocks have no highlighter or gutter yet.
- **Views**: `LexicalUITextView` (UIKit), `LexicalNSTextView` (AppKit) and
  `LexicalEditor` (SwiftUI) on TextKit 2, with an incremental reconciler,
  native input and IME, the pasteboard and drag and drop, SwiftUI
  decorators, list markers and check boxes, VoiceOver announcements,
  Dynamic Type and Dark Mode.
- **Conformance**: the vectors and scenarios in `packages/lexical-conformance`
  are generated by running the upstream JS, and the Swift tests replay
  them. See the `JSParity` article in the documentation.
- **Platforms**: iOS 26 and macOS 26 or later (CI tests the iOS 26.5 and
  iOS 27 simulators), with Swift 6.4. The non-UI modules also build on
  Linux.
- **Known deviations**: where the native views behave differently from a
  browser (13 cases, in input, selection and the clipboard), the
  difference is listed in `parity/deviations.json` and in the `JSParity`
  article. Collaboration (`@lexical/yjs`) and the packages not listed above
  (table, mark, hashtag, overflow, mdast and others) are not ported yet.
- **Deferred benchmarks**: typing, deleting and caret moves meet the 8 ms
  p95 target at 5,000 paragraphs (7.64 ms on an iPhone 15 Pro in release).
  Enter (11.6 ms p95), joining paragraphs (15.3 ms) and loading a 3 MB
  document (1.0 s against a 300 ms target) don't yet; the structural fixes
  are planned in `DESIGN.md` and deferred past this release.

The changes that led up to it, newest first:

- New `LexicalList`, a port of `@lexical/list`: `ListNode`,
  `ListItemNode`, `ListExtension`, `insertList`, `removeList`,
  `handleIndent`, `handleOutdent`, `handleListInsertParagraph`,
  `registerList`, `registerListStrictIndentTransform`, the list commands,
  `CheckListExtension` with `INSERT_CHECK_LIST_COMMAND`, `ListImportRules`
  (which `ListExtension` contributes to `DOMImportExtension`), `ListSchema`
  and the opt-in `WordListImportExtension` for lists pasted from MS Word.
  Lists serialize byte for byte as in JS.
- New `LexicalLink`, a port of `@lexical/link`: `LinkNode`,
  `AutoLinkNode`, `toggleLink`, `formatUrl`, `TOGGLE_LINK_COMMAND`,
  `LinkExtension` (with `validateUrl`, default `attributes` and URL paste),
  `LinkImportRules`, `AutoLinkExtension` with `autoLinkUrlMatcher`,
  `autoLinkEmailMatcher` and `createLinkMatcherWithRegExp`,
  `AutoLinkAnnounceExtension`, and `ClickableLinkExtension`, which opens a
  clicked link through a `LinkWindow` the app supplies. Links serialize byte
  for byte as in JS.
- New `LexicalCodeCore`, a port of `@lexical/code-core`: `CodeNode`,
  `CodeHighlightNode`, `CodeExtension` (Enter out of a code block, plain
  tabs), `CodeImportRules` with the VS Code paste overlay,
  `CodeIndentExtension` and `registerCodeIndentation`, and the line helpers
  (`getFirstCodeNodeOfLine`, `getStartOfCodeInLine`, `plainifyCodeContent`,
  `outdentLeadingSpaces`, …). Code blocks serialize byte for byte as in JS.
  There is no highlighter yet, and no gutter.
- New `LexicalMarkdown`, a port of `@lexical/markdown`:
  `convertFromMarkdownString`, `convertToMarkdownString`,
  `convertSelectionToMarkdownString`, `generateNodesFromMarkdownString`,
  `registerMarkdownShortcuts`, the built-in transformers (`HEADING`,
  `QUOTE`, `CODE`, the three lists, `LINK`, the text formats, `TRANSFORMERS`
  and its groups), the transformer types for custom ones, and
  `isTableRowDivider`. Transformers keep their JS regular expressions as
  `JSRegExp`, which runs JS regex source on ICU with UTF-16 indices. Import
  and export match JS byte for byte on all 149 cases of upstream's
  import/export table.
- The views draw list markers as the playground theme does: bullets by
  depth (•, ◦, ▪), numbers by depth (1., A., a., I., i.), and check boxes,
  with a checked item struck through. A click (AppKit) or a tap (UIKit) on
  a check box toggles the item without moving the selection. New
  `TextKitInputController.listMarkers`, `toggleChecked(_:)` and
  `checkboxItem(at:location:lineWidth:)`, `TextKitListMarker`,
  `TextKitListMarkerLayout` and `TextKitParagraphAttributes.listMarker`.
- `TextKitUpdate.restyled` lists text a commit leaves in place whose block
  changed what it passes down (a check list item checked, a list's type),
  so the views restyle it; until now such text kept its old look.
- An autolink takes the theme's `link` style, as `AutoLinkNode.createDOM`
  takes `theme.link`; it was drawn as plain text.
- `DataTransfer` reads and writes the `text` format as `text/plain` and
  `url` as `text/uri-list`, as browsers do.
- `LexicalTesting.withLargeStack` runs a test body on an 8 MB stack.
- `ElementNode.append(contentsOf:)` is `open`, so a subclass can override
  how it takes children, as `ListItemNode` does. Added
  `DOMNode.previousElementSibling` and `DOMNode.nextElementSibling`.
- `LexicalTesting.TestDecoratorNode` is `open`, so a test can subclass it
  as upstream's tests do.

- Dynamic Type on iOS: `LexicalUITextView` scales the theme's fonts with
  the content size category, as body text, and renders again when it
  changes.
- VoiceOver reads a decorator as its SwiftUI view's accessibility labels,
  or else its `getTextContent()`, on iOS and macOS.
- Dark Mode: `TextKitColor` can carry a dark variant (`withDark`), which
  the views resolve as a dynamic system color, and `TextKitTheme.default`
  has dark values. Before, every editor showed black text on a dark
  background in Dark Mode.
- `apps/lexical-playground-swift`: the Swift playground for iOS and macOS,
  with a toolbar (undo and redo, block type, text formats, clear
  formatting, alignment, indent), the tree view, JSON and HTML import and
  export, and settings (rich or plain text, debug view, editable).
- New `LexicalDevtoolsCore`: `generateContent`, the text of the playground's
  tree view (the tree with its selection, the commands and the editor's
  settings), or the state's HTML, indented. It matches
  `@lexical/devtools-core` on 38 documents and selections
  (`vectors/devtools/tree.json`). Link, mark and table nodes print as
  other nodes until those packages are ported.
- `CaretRange.iterNodeCarets` is lazy, as in JS: it returns a
  `StepwiseIterator` (now in `Lexical`, as `makeStepwiseIterator` is in
  `lexical`) instead of an array, so a loop that stops early no longer
  walks to the end of the range first, and a loop that changes the tree
  sees the changes. Use `next()`, `forEach` or `toArray()`. Backspace at
  the start of a paragraph walked to the start of the document; at 5,000
  blocks it went from 23 ms to 10 ms p50 (release).
- `getChildren()` and the other sibling walks read the node map once
  instead of resolving each node's latest version twice. Typing at 5,000
  blocks went from 2.8 ms to 1.7 ms p50, and Enter from 13.4 ms to 11.9 ms
  (release).
- Serializing an editor state to JSON is linear in its size: each string
  was appended through `String.UnicodeScalarView.append(contentsOf:)`,
  which copied the whole output so far. Stringifying a 3 MB document went
  from about 34 s to well under a second (release).
- Deleting a character no longer renders the whole document again to find
  where the caret moves: a caret move made before the update changes any
  node reuses the view's rendering. At 5,000 blocks a delete went from 81
  ms to 14 ms p95 (release). A character move now also walks only the
  line it crosses rather than every grapheme of the document, which takes
  the delete to 3.3 ms p95.
- The package supports iOS 26 and macOS 26 (it required iOS 27 and macOS
  27). CI also tests on the iOS 26 simulator.
- `LexicalUITextView` reports a triple (or later) tap as `CLICK_COMMAND`
  with the tap count as `detail`, after its paragraph selection, so
  `NormalizeTripleClickSelectionExtension` works on iOS.
- `IMEExtension`: `compositionKey` and `composingTextNode` signals for the
  IME composition in progress, set from the views' marked text.
- `NormalizeTripleClickSelectionExtension`: a triple (or later) click's
  block selection ends inside the clicked block instead of at the start of
  the next one, so formatting it no longer reaches the next block. It acts
  on `CLICK_COMMAND` with the click count as `detail`, which the views
  report after syncing their selection.
- `LexicalNSTextView` and `LexicalUITextView` host the editor's live
  region (`LiveRegionHost`): the `LexicalA11y` announcers' messages are
  spoken by VoiceOver as accessibility announcements, at high priority when
  the region is assertive. The view's `liveRegion` is the mounted region.
- `LexicalUITextView` works with the iOS software keyboard: IME
  composition (Japanese, Chinese, Korean) goes through the model instead
  of committing after each key, Return inserts a paragraph, typed keys
  merge into one undo step, and the system's undo (Command-Z, shake,
  three-finger swipe) undoes through the editor's history.
- `TextKitInputController.setSelectedRange` keeps the selection's anchor
  when the new range still ends at it, so a selection extended backward
  (Shift-Left) is backward in the model, as in a browser.
- DecoratorNodes' SwiftUI views now show in the views (they rendered as
  file icons), and a view can be created for an editor that already has
  content, such as `LexicalEditor` for a loaded document (it crashed).
  `TextKitInputController.renderAll(replacing:)` takes the length of the
  text the view shows.
- New `LexicalA11y` module: `AriaLiveRegionExtension` (a `LiveRegion` on a
  `LiveRegionHost` root or `owner`), `HistoryAnnounceExtension` and
  `EditorModeAnnounceExtension`. `RichTextExtension` now includes
  `HeadingAnnounceExtension`, which announces a block becoming a heading or
  ceasing to be one. `RootElementExtension` is added to `LexicalExtension`.
- The editor views are driven by the editor's extensions (`RichTextExtension`,
  `PlainTextExtension`, `HistoryExtension`, ...) instead of their own
  editing commands: key presses dispatch `KEY_DOWN_COMMAND`, text input and
  NSResponder selectors `BEFORE_INPUT_COMMAND`, IME marked text
  `COMPOSITION_START_COMMAND` / `COMPOSITION_END_COMMAND`, and the
  clipboard and drag and drop `COPY_COMMAND`, `CUT_COMMAND`,
  `PASTE_COMMAND`, `DRAGSTART_COMMAND` and `DROP_COMMAND` through
  `NSPasteboard` / `UIPasteboard`. `registerTextKitSliceEditing` is
  removed. The views are the editor's root element and implement
  `focus()`, `blur()`, `hasFocus`, `isConnected` and `caretFromPoint(_:_:)`.
- Editors handle `BEFORE_INPUT_COMMAND`, `COMPOSITION_START_COMMAND` and
  `COMPOSITION_END_COMMAND` by default, as in JS (in controlled mode).
- The views render with `TextKitTheme` (fonts, colors, paragraph styles,
  quote bars) and show DecoratorNodes' SwiftUI views as attachments.
  `TextKitInputController.onRender` delivers a `TextKitUpdate`: the styled
  edits, the paragraphs and decorators to style again, the selection and
  the tags.
- Each update copies the node map without rehashing it, so a keystroke in a
  5,000-paragraph document takes about a third of the time (release build on
  Linux: 11.5 ms to 3.9 ms p95).
- `LexicalTextKit` reconciles incrementally: `TextKitReconciler` turns each
  commit into `TextKitEdit` replacements for a `TextKitStorage`, and
  `TextKitInputController.onRender` now delivers those edits (one full edit
  for the first render and `setEditorState`) instead of the whole document.
- The editor views moved out of `LexicalTextKit`: `LexicalNSTextView` is in
  `LexicalAppKit` and `LexicalUITextView` in `LexicalUIKit`. Added
  `LexicalSwiftUI` with `LexicalEditor`, a SwiftUI view for an editor.
- Added `TextKitTheme` to `LexicalTextKit`: the native look of each
  `EditorThemeClasses` key (the playground's by default), resolving a text
  node's format, inline style and blocks into `TextKitTextAttributes` and
  `TextKitParagraphAttributes`.
- Added `LexicalClipboard`: copy and paste in the Lexical, HTML and plain
  text formats (`copyToClipboard`, `insertDataTransferForRichText`,
  `insertDataTransferForPlainText`, `getClipboardDataFromSelection`,
  `GetClipboardDataExtension`, `ClipboardImportExtension`), and drag and
  drop (`handleRichTextDrop`, `handlePlainTextDrop`,
  `writeDragSourceToDataTransfer`).
- `RichTextExtension` now handles copy, cut, paste, drop, dragstart and
  dragover.
- Added `LexicalPlainText`: `registerPlainText` and `PlainTextExtension`.
- `LexicalRootElement` gained `caretFromPoint(_:_:)`, which a view
  implements to resolve drop points; it returns `nil` by default.
- Editors built with `buildEditorFromExtensions` now rethrow errors from
  updates when no `onError` is configured, as in JS; they were swallowed.
- Added to `LexicalExtension`: `NodeStructureExtension`,
  `NodeSelectionExtension`, `EditorStateExtension`, `HorizontalRuleExtension`
  (`INSERT_HORIZONTAL_RULE_COMMAND`), `SelectBlockExtension`,
  `PreventSelectAllExtension`, `ClickAfterLastBlockExtension`,
  `DecoratorTextExtension` (`DecoratorTextNode`), `TabIndentationExtension`
  (`registerTabIndentation`) and `AutoFocusExtension`.
- Added node structure to core (`LexicalStructure.swift`): `NodeConfig`'s
  `children:` and `mergeAdjacent:`, `getDeclaredNodeStructure`, and a real
  `INTERNAL_isNodeStructureEnforced`. Paragraphs, headings and quotes
  declare `children: .inline`. Added `Editor.focus`.
- Added `LexicalHistory`: `registerHistory`, `createEmptyHistoryState`,
  `HistoryState`, `HistoryExtension` and `SharedHistoryExtension`.
- `Editor.setEditorState` called during an update now applies the state in
  a nested update, as in JS, instead of inline in the current one.
- Added `LexicalHeadless`: `createHeadlessEditor` and `HeadlessExtension`,
  which mark the editor headless and refuse a root element.
- `generateHtmlFromNodes` now works inside a legacy
  `editor.getEditorState().read { }` with no active editor, as in JS.
- The legacy `generateNodesFromDOM` now inlines `<style>` sheet rules into
  matching elements' styles before converting, as in JS.
- `RichTextExtension` now registers `CoreImportExtension` and
  `RichTextImportRules` (headings, quotes, the Google Docs title), and
  `ShadowRootQuoteRule` is available to import shadow root quotes.
- Added `DOMImportExtension` and `CoreImportExtension` to `LexicalHTML`:
  `defineImportRule`, the `sel` selector builders and `parseSelector`,
  `defineOverlayRules`, `BlockSchema`, `InlineSchema`, `NestedBlockSchema`,
  `RootSchema`, `distributeInlineWrapper`,
  `propagateTextAlignToBlockChildren`, `CoreImportRules`, the import
  context states (`ImportSource`, `ImportTextFormat`, `ImportTextStyle`,
  `ImportWhitespaceConfig`, `ImportOverlays`, `createImportState`,
  `getImportContextValue`, `withImportContext`), preprocessors with
  `inlineStylesFromStyleSheets`, and `generateNodesFromDOMViaExtension`.
  `LexicalExtension` gains `HorizontalRuleNode`.
- Added `DOMRenderExtension` to `LexicalHTML`: `domOverride`,
  `DOMRenderMatchConfig`, `DOMOverrideOptions`, the render context
  (`createRenderState`, `RenderContextRoot`, `RenderContextExport`,
  `getRenderContextValue`, `setRenderContextValue`,
  `updateRenderContextValue`, `withRenderContext`) and
  `getSessionDOMRenderConfig`. HTML export runs with the session's render
  config. `LexicalBuilder` now merges each extension's `html` config, as
  upstream does. `LexicalTesting` gains `prettifyHTML`.
- Added the `LexicalRichText` module: `HeadingNode`, `QuoteNode` (with
  `quoteShadowRootState`), their HTML importers and exporters,
  `registerRichText`, `RichTextExtension`, the escape-format triggers and
  `DRAG_DROP_PASTE`. Clipboard and drag handlers come with
  `LexicalClipboard`. `Editor.blur()` and `LexicalRootElement.blur()` are
  new; `SelectionGranularity` gains `.line`, and `SET_TEXT_FORMAT_COMMAND`
  now carries ordered `(format, value)` pairs.
- The upstream test utilities (the test editor, the test nodes and the
  model-shape helpers) are public in `LexicalTesting`.
- Added HTML import: `LexicalHTML.generateNodesFromDOM`, `importDOM` on
  nodes (`DOMConversionMap`, `DOMConversion`, `DOMConversionOutput`), the
  core nodes' importers, `HTMLConfig.import`, and the helpers
  `setNodeIndentFromDOM`, `setDirectionFromDOM`, `setFormatFromDOM`,
  `isInlineDomNode`, `isOnlyChildInBlockNode`, `isLastChildInBlockNode` and
  `findParentPreDOMNode`. It matches `@lexical/html` on the `html/import`
  vectors. Inlining `<style>` sheets is not ported yet.
- Added `DOMParser`, a pure-Swift HTML parser (WHATWG tokenizer, named
  and numeric character references, and the tree builder's head, body,
  table, list and formatting rules). It builds the same documents as
  jsdom's `DOMParser` for the `html/parse` vectors.
- Added the `LexicalHTML` module's export half: `generateHtmlFromNodes`,
  `generateDOMFromNodes`, `generateDOMFromRoot` and `appendNodeToHTML`,
  which match `@lexical/html` byte for byte (`html/export` vectors).
  `DOMExportOutput` gains `append` and `getChildNodes`; core gains
  `isBlockDomNode`.
- Added `createDOM`, `updateDOM`, `getDOMSlot` and `exportDOM` to nodes,
  with the core nodes' implementations (theme classes, inline styles, the
  `<b>`/`<i>`/`<s>`/`<u>` export wrappers), `EditorDOMRenderConfig`,
  `HTMLConfig` export overrides, `getDocument`, `DOMSlot`,
  `addClassNamesToElement`, `removeClassNamesFromElement`,
  `setDOMStyleFromCSS` and `setDOMStyleObject`.
- Added a small DOM to core (`DOMDocument`, `DOMElement`, `DOMText`,
  `DOMComment`, `DOMDocumentFragment`, `DOMTokenList`,
  `CSSStyleDeclaration`) with `innerHTML`/`outerHTML` serialization. Inline
  styles normalize values as jsdom does (`css/values` vectors), so HTML
  export can match upstream byte for byte.
- Added the `LexicalUtils` module: `dfs`, `reverseDfs`, the slot-aware
  traversals, `firstToLastIterator`/`lastToFirstIterator` (lazy, throwing
  `StepwiseIterator`s), `insertNodeToNearestRoot`, `insertNodeIntoLeaf`,
  `wrapNodeInElement`, `unwrapAndFilterDescendants`, `descendantsMatching`,
  `registerNestedElementResolver`, `restoreEditorState`, `eventFiles`,
  `handleIndentAndOutdent`, `onEscapeUp`/`onEscapeDown`, `makeStateWrapper`
  and `dedupeSelectionRects`. `LexicalSelection` gains `SelectionRect` and
  `filterSelectionRects`, the rect filter of `createRectsFromDOMRange`.
- Added the `LexicalSelection` module: `patchStyleText`, `setBlocksType`,
  `wrapNodes`, `getSelectionStyleValueForProperty`,
  `sliceSelectedTextNodeContent`, `trimTextContentFromAnchor`,
  `shouldOverrideDefaultCharacterSelection`, `moveCharacter` and the other
  `@lexical/selection` helpers. A root element reports computed styles
  (direction, writing mode) by conforming to `ComputedStyleProvider`.
- Added `getStyleObjectFromCSS`, which returns an ordered `StyleObject`, and
  `INTERNAL_isInlineContent` to core.
- Added keyboard and DOM event types (`KeyboardEvent`, `ClipboardEvent`,
  `InputEvent`, `DataTransfer`, …), the `KEY_*`, clipboard, drag and focus
  commands, `isExactShortcutMatch`, `compileKeyboardShortcuts` and a default
  `KEY_DOWN_COMMAND` handler that dispatches the same commands as JS on
  Apple and non-Apple platforms.
- Added `ClearEditorExtension`, `NestedEditorExtension`,
  `NormalizeInlineElementsExtension` and `KeyboardShortcutsExtension`. Extension `init` now takes the editor config
  `inout`, as JS mutates it.
- Added the `LexicalExtension` module: `buildEditorFromExtensions`,
  `LexicalBuilder`, `InitialStateExtension`, signals (a port of
  `@preact/signals-core`: `signal`, `computed`, `effect`, `batch`,
  `untracked`, `namedSignals`, `watchedSignal`) and the dependency helpers
  (`getExtensionDependencyFromEditor`, `getPeerDependencyFromEditor`, …).
  Core gains `defineExtension`, `configExtension`, `declarePeerDependency`,
  `Partial<Config>`, `shallowMergeConfig`, `safeCast` and `Editor.dispose()`.
- With a root element attached, commits now run the reconciler's model
  half: mutation listeners receive created, updated and destroyed nodes,
  `RootNode` caches its text, and elements take `textFormat`/`textStyle`
  from their first text node, as in JS.
- `editor.read()` warns when it is called from a commit listener while
  another update is pending, as JS does in development.
- Fixed: a mutation listener whose teardown was discarded could be replaced
  by the next one registered.
- Update, editable, decorator and text content listeners may return a
  `ListenerCleanup`, called before their next call and when they are
  unregistered, as in JS (`{ _ -> ListenerCleanup? in … }`).
- Added `createRefCountedRegistry(_:)`: registrations reference counted by
  key, activated on the first and torn down after the last release.
- Added `LexicalWarning`, an `Error` with a namespaced `code` and optional
  `data`, for `onWarn`.
- Added `mergeRegister(_:)`, which `lexical` exports: one clean-up that runs
  the given clean-ups in reverse order, once.
- Added `Editor.setRootElement(_:)` and `registerRootListener(_:)`. The root
  is the native view; attaching or detaching it resets the editor and
  commits the pending state again, keeping queued updates and their tags.
  A root conforming to `LexicalRootElement` reports `isConnected`; range
  selection changes notify `SELECTION_CHANGE_COMMAND` only while it is.
- `LexicalNSTextView` now builds its TextKit 2 stack itself; it had no text
  system, so it showed nothing.
- A committed state's node map is frozen as in JS's dev build: writing to it
  (for example after an `editor.read()` committed mid-update) throws.
  `drainMicrotasks()` also runs `setTimeout(0)` work, so the update-cascade
  guard fires as in JS. Errors from a queued commit go to
  `withUncaughtErrorHandler(_:_:)` (standard error by default) instead of
  `onError`, and an update started from a read-only context warns.
- An update that `onError` starts while handling a failed update is now
  queued until the failed update is cleaned up, as in JS, instead of running
  at once.
- `getRegisteredSubtypeMap(_:)`, and `withWarningHandler(_:_:)` to capture
  the warnings Lexical writes to standard error, such as the one
  `createEditor` now gives for a replacement without `withKlass`.
- `LexicalTextKit`: a first typeable editor, `LexicalNSTextView` and
  `LexicalUITextView` on TextKit 2, which turn typing, deletion, Enter and
  selection changes into commands and show what the model renders.
  `Editor.selectionMovement` lets a view supply the layout step that
  `Selection.modify` gives JS, so deletion within text works.
- `@LexicalNode(type:extends:…)` and `@Field(schema)`: a node class's
  `config()`, with one schema field per `@Field` property and a
  `getX`/`setX` pair for each. Adds a dependency on swift-syntax.
- Core model: nodes, editor, updates and transforms, commands and listeners,
  selection, NodeState, slots, carets, and byte-exact legacy and compact JSON.
- Serialization schemas: `arrayValue`, `arraySetValue`, `objectValue`,
  `unionValue` and `transformValue`, with `accepts` and union member
  selection as in JS. Schema declarations JS refuses now throw, so
  `config()` is `throws`.
- Node accessors a schema property reads or writes (`getStyle`, `setIndent`,
  `setTextContent` and the rest) are `open`, and importing JSON goes through
  them, so a subclass that overrides one owns its property as in JS.
- RangeSelection editing: `insertText`, `insertRawText`, `removeText`,
  `insertNodes`, `insertParagraph`, `insertLineBreak`, `extract` and
  `formatText`, with `insertNodes`, `formatText`, `setTextFormat`,
  `getCharacterOffsets`, `tokenizeRawText` and `generateNodesFromRawText`.
  `deleteCharacter`, `deleteWord`, `deleteLine` and `modify` handle the
  moves across decorators and blocks; moving by a character, word or line
  within text waits for the TextKit views, as JS waits for the DOM.
- Caret editing: `removeTextFromCaretRange`, `splitAtPointCaretNext`,
  `insertNodeToNearestRootAtCaret`, `splitTextPointCaret`,
  `splitTextPointCaretSlice`, `getAdjacentSiblingOrParentSiblingCaret` and
  `getAdjacentNodes`.
- `SchemaField(_:_:field:)` is `withField`: a property stored in a key-path
  field, carried across a clone with no `afterCloneFrom`.
  `getComposedSchemaFields(_:)` returns a class's composed schema.
- `setState` stores an `arraySetValue` state in its canonical form, and
  `createState(_:schema:)` takes an `unparse`. A text node without NodeState
  no longer merges with a neighbor that has state.
- Added block ids to `LexicalExtension` (`@experimental`):
  `BlockIdExtension` (`BlockIdConfig`, `BlockIdOutput`), `blockIdState`,
  `getBlockId`, `getBlockById`, `getBlockHandle`, `getBlockHandles`,
  `resolveBlockHandle` (`BlockHandleOptions`, `BlockHandleResult`),
  `MIN_BLOCK_HANDLE_LENGTH`, `isRootChildBlock`, `defaultIsIdentifiedBlock`
  and `generateBlockId`. Added `BlockIdHTMLExtension` (`BlockIdHTMLConfig`)
  and `BLOCK_ID_ATTRIBUTE`, `BLOCK_HANDLE_ATTRIBUTE` and
  `BLOCK_WRAPPER_ID_ATTRIBUTE` to `LexicalHTML`.
