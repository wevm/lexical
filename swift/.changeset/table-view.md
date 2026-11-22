---
'lexical-swift': patch
---

`LexicalNSTextView` and `LexicalUITextView` showed tables: each table renders as a grid of its cells' own text views over the same editor, laid out as the playground's CSS lays it out (column widths, padding, borders, header and striped backgrounds), scrolling sideways when it is wider than the line. Typing, IME and the pasteboard worked in every cell; Tab and the arrow keys, up and down included, moved between cells; on macOS a drag or Shift-click across cells made a `TableSelection`, drawn over the selected cells; the focus stayed in the editor as it moved between cells. `LexicalDevtoolsCore` printed a table selection, as upstream's tree view does, and the playground inserted tables from its Insert menu and "/" (including "/3x4").
