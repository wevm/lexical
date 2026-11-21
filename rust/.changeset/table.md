---
'lexical-ui': patch
---

Added `@lexical/table` behind the new `table` feature: `TableNode`, `TableRowNode` and `TableCellNode` (with merged cells and background colors), `TableSelection` as `Selection::Table`, the table utilities (insert, delete, merge and unmerge rows, columns and cells, `$computeTableMap`), `INSERT_TABLE_COMMAND`, `register_table_plugin`, `TableExtension`, `TableImportRules` and the HTML export, and Tab, Shift+Tab, arrow and Shift+arrow navigation between cells. With `devtools`, `generate_content` also prints a table selection. Apps that don't enable `table` see no change.
