---
'lexical-swift': minor
---

`LexicalEditor.configure { textView in ... }` sets up the text view (`LexicalUITextView` on iOS, `LexicalNSTextView` on macOS, named `LexicalEditor.TextView`) each time SwiftUI updates the editor, after the editor's own settings: the text container's inset and padding, exclusion paths, the accessibility label, the macOS insertion point color. It runs on every update, so it can read SwiftUI state and must set the same values each time; register commands and listeners on the editor instead. With `sizedToContent`, `LexicalEditor` now measures its content at the width SwiftUI proposes without resizing the text view it is measuring.
