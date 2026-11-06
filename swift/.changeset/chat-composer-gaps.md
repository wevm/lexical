---
'lexical-swift': patch
---

`LexicalEditor` (SwiftUI) gains `theme(_:)`, which draws the text with a `TextKitTheme` such as the default with styles for an app's own nodes, and `editorBackground(_:)`, which makes the text view transparent so a message bubble or a composer's own background shows through. Its macOS scroll view now shows the scroller only while the content overflows. `LexicalNSTextView` and `LexicalUITextView` now keep a collapsed caret in view after each commit while focused, as upstream's `updateDOMSelection` does with `scrollIntoViewIfNeeded`, unless the update has the `skip-scroll-into-view` tag. Before, typing past the bottom of a scrolled or height-capped editor left the caret out of sight.
