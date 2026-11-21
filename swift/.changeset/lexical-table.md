---
'lexical-swift': patch
---

Added the `LexicalTable` library product, a port of `@lexical/table`: `TableNode`, `TableRowNode` and `TableCellNode` (including spans, background colors, column widths, row striping and frozen rows and columns), `TableSelection`, the table utilities and commands (inserting, deleting, moving and merging rows, columns and cells), `TableExtension` with Tab and arrow-key navigation, deletion, formatting and paste over a table selection, and `TableImportExtension` for HTML import and export. Where upstream measures the DOM, `TableConfig.layout` takes a `TableLayout` that a view supplies.
