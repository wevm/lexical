---
'lexical-swift': patch
---

On iOS, a long press on a decorator in `LexicalUITextView` no longer opens UIKit's text attachment menu, which offered Copy Image and Save to Camera Roll over a blank preview. That menu could also take the long press that starts dragging a selected decorator, so the drag didn't start.
