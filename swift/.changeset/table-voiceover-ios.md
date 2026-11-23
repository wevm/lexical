---
'lexical-swift': patch
---

Made VoiceOver read an iOS table cell by cell: the grid in `LexicalUITextView` became a `UIAccessibilityContainerDataTable` whose elements are the cells' text views at their rows and columns (each `LexicalUITextView` a `UIAccessibilityContainerDataTableCell`), with a header row's or column's cells as the headers of the cells in their column or row. The table's character read only the table's size, where it had read every cell's text in one label.
