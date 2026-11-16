---
'lexical-swift': patch
---

Fixed the core's `KEY_DOWN_COMMAND` handler returning early for a key event whose `key` is empty, so such an event with a modifier held now dispatches `KEY_MODIFIER_COMMAND`, as in JS. The handler also no longer records the last key itself: the views' `keyDown` did already, as JS's `onKeyDown` does, so a `KEY_DOWN_COMMAND` dispatched directly no longer changes which key ends a text replacement.

Fixed the core's `BEFORE_INPUT_COMMAND` handler deleting a character on `deleteContentBackward` when the editor had no selection: as in JS, it now only restores the previous update's selection and leaves the event to the platform.
