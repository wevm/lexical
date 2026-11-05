---
'lexical-swift': patch
---

`LexicalNSTextView`, `LexicalUITextView` and the SwiftUI `LexicalEditor` (`LexicalEditor(editor, placeholder:)`) take a `placeholder`. It shows while the editor is empty, decided by `canShowPlaceholder` the way upstream's `ContentEditable` decides it. It sits on the first line, where typed text goes, in the theme's new `placeholder` style (the playground's `#999` root text). VoiceOver reads it as the view's placeholder value on macOS and as its value on iOS. `TextKitUpdate.canShowPlaceholder` reports the decision for each commit.
