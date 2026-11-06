# Chat composer

A small macOS and iOS SwiftUI app with a message list and a composer, built
on the library's public products only, the way an app outside this repository
would use them:

- The composer sizes itself to 1 to about 8 lines, then scrolls
  (`LexicalEditor.sizedToContent`), with the placeholder "Message…".
- Enter sends and Shift+Enter inserts a line break, through a
  `KEY_ENTER_COMMAND` handler (`ChatModel`). Enter in a list or a code block
  keeps editing it, and an open menu takes Enter first. The send button is
  disabled while there is nothing to send, and the composer clears after a
  send (`CLEAR_EDITOR_COMMAND`, `CLEAR_HISTORY_COMMAND`).
- "@" mentions people and "/" opens a command menu, both through
  `LexicalMenu`'s `registerTypeaheadMenu`, drawn as a list above the composer
  (`ComposerMenus.swift`).
- Bold, italic, code and links from the format bar or Markdown shortcuts
  (`registerMarkdownShortcuts`), and pasted URLs become links (`AutoLinkExtension`).
- The paperclip and files dropped or pasted into the composer
  (`DRAG_DROP_PASTE`) add attachment chips above it. These are app state, not
  nodes.
- Sent messages are read-only editors (`editable: false`) with the same
  theme, drawn without their own background (`editorBackground(false)`).
- A fake reply streams in word by word with the composer read-only
  (`setEditable(false)`).

Generate the project with `xcodegen generate` (it is not checked in), then open
`ChatComposer.xcodeproj`, or build it from the command line:

```sh
xcodebuild build -project ChatComposer.xcodeproj -scheme ChatComposer \
  -destination 'platform=macOS' CODE_SIGNING_ALLOWED=NO
```

Launch arguments: `-light` or `-dark` force an appearance, `-draft <markdown>`
starts the composer with a draft, and `-attach <name>` (repeated) starts it
with attachments.

## What building it found

Building a composer from the public products only turned up these gaps.
Fixed in the library:

- `LexicalEditor` had no way to set the TextKit theme; the app had to reach the
  text view's controller from a root listener. It now has `theme(_:)`.
- The text view painted an opaque background over a bubble or a composer's
  own field. `editorBackground(false)` makes it transparent.
- On macOS the scroll view always showed its scroller, even while sized to
  content. It now shows it only while the content overflows.
- Neither text view scrolled the caret into view after a commit, so typing past
  the bottom of a height-capped composer hid the caret. Both now do, as
  upstream's `updateDOMSelection` does, unless the update has
  `SKIP_SCROLL_INTO_VIEW_TAG`.

Still open:

- `LexicalMenu` keeps a menu's state and keys but has no default renderer or
  positioning, where upstream's `LexicalTypeaheadMenuPlugin` has a default
  `menuRenderFn`. Every app draws its own menu; the playground's floating menu
  (`TypeaheadMenuHost`) is app code.
- `registerTypeaheadMenu`'s `onQueryChange` doesn't receive the menu it
  belongs to, so setting options from it needs a variable captured before the
  menu exists. `MenuState.onChange` is a single closure, so only one view can
  follow a menu.
- A read-only message is a whole editor and text view per message. There is
  no lightweight renderer from an editor state to an `AttributedString` or a
  SwiftUI `Text` for static content such as a long message list.
- An array of `any AnyLexicalExtensionArgument` isn't `Sendable`, so a shared
  list of extensions can't be a global constant under Swift 6; it has to be a
  function.
