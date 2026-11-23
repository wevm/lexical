---
'lexical-swift': patch
---

Added table selection by touch to `LexicalUITextView`: a drag of a cell's selection handle past the cell's edge selected the cells between as a `TableSelection`, as a drag across cells does on macOS and the web. A table selection showed its own handles at its corners, which grew or shrank it, and an edit menu with Cut, Copy and Paste for the selected cells. A cell's text view also took touches in the cell's padding, so a tap there put the caret in the cell and UIKit's handle under a cell's last line could be dragged.
