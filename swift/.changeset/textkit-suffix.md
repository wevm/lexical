---
'lexical-swift': patch
---

`TextKitInputController.setSuffix(_:)` shows a `TextKitSuffix` after a text node in the TextKit views, outside the editor state, as JS appends an unmanaged `<span>` to a text node's DOM: it is part of the text layout, so the text after it moves aside and it wraps, but the model, its selection, the history and copying never see it, a click in it lands at its node's end and the arrow keys cross it in one step. Typing into its node drops it. The theme has an `autocomplete` type (`#ccc`) for it. When the model ends an IME composition itself (`setCompositionKey(nil)`), the views now end the input method's marked text with it (`TextKitUpdate.endsComposition`), and on macOS a key the input method commits its marked text with and passes on (Tab, an arrow) reaches `KEY_DOWN_COMMAND` after the commit.
