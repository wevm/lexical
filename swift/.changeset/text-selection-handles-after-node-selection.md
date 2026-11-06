---
'lexical-swift': patch
---

On iOS, a text selection made right after a node selection (a selected image, rule or other decorator) shows its handles again every time. `LexicalUITextView` now turns UIKit's selection display back on before it applies the new range, and asks it to redraw. Before, a slow device could leave the range highlighted with no handles to drag.
