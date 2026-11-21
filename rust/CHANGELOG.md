# Changelog

Each release names the upstream Lexical version it conforms to. Entries come
from the changesets in `.changeset/` (see its README), and the release
workflow publishes the section headed `## <version>`, the version in
`lexical-ui/Cargo.toml`.

## 0.1.3

Conforms to upstream Lexical **0.53.0** (`lexical_ui::UPSTREAM_VERSION`).

### Patch Changes

- Added `@lexical/table` behind the new `table` feature: `TableNode`, `TableRowNode` and `TableCellNode` (with merged cells and background colors), `TableSelection` as `Selection::Table`, the table utilities (insert, delete, merge and unmerge rows, columns and cells, `$computeTableMap`), `INSERT_TABLE_COMMAND`, `register_table_plugin`, `TableExtension`, `TableImportRules` and the HTML export, and Tab, Shift+Tab, arrow and Shift+arrow navigation between cells. With `devtools`, `generate_content` also prints a table selection. Apps that don't enable `table` see no change.

## 0.1.2

Conforms to upstream Lexical **0.53.0** (`lexical_ui::UPSTREAM_VERSION`).

### Patch Changes

- Added `EmbedConfig::parse_url_async` and `ParseUrlDone`, so an embed config can answer `parseUrl` later, as a JS `parseUrl` that returns a promise does: the auto-embed menu opens when the config resolves, unless a newer paste was checked meanwhile or the link was removed or given another URL, pending edits included. `EmbedConfig::parse_url` now has a default (`None`), so a config implements one of the two; existing configs are unchanged.
- Fixed HTML import ignoring the `font` shorthand: the inline style declaration now expands `font` into its longhands as jsdom's does, so `<span style="font: bold 12px serif">` and a `<style>` rule's `font: italic 10px a` import as bold and italic. Added pseudo-class selectors to `DomNode::query_selector_all` and `DomNode::matches` (`:first-child` and the other `:nth-` positions, `:not()`, `:is()`, `:where()`, `:has()`, `:link`, `:lang()`, `:empty`, `:root`, `:checked`, `:disabled`, `:enabled`), so `<style>` rules that use them, such as Word's `a:link` lists, now apply on paste; user-action pseudo-classes such as `:hover` and pseudo-elements match nothing.
- Fixed word and line deletion crossing a slot boundary: when an editor's `SelectionMovement` lands in another slot frame, `delete_word`, `delete_line` and `delete_character` now stop at the boundary in the caret's frame, as JS's `'deletion'` mode of `$setPointAtSlotBoundary` does, instead of deleting into the other frame.
- Added `watch_editable_extension()`, the port of `WatchEditableExtension`: its output is a signal of `Editor::is_editable` that follows `Editor::set_editable` while something watches it.

## 0.1.1

Conforms to upstream Lexical **0.53.0** (`lexical_ui::UPSTREAM_VERSION`).

### Patch Changes

- Added `code::register_code_gutter`, the port of `registerCodeGutter`: it gives the app each code block's line numbers (what JS writes to `data-gutter`) when the block is rendered and whenever its line count changes.
- Added `on_selection_change`, the model half of JS's `onSelectionChange`: an app passes the points its view selected, and the editor selects them, gives a caret the format and style of the text it is in (and a range the format all its text shares), and dispatches `SELECTION_CHANGE_COMMAND`.

## 0.1.0

Conforms to upstream Lexical **0.53.0** (`lexical_ui::UPSTREAM_VERSION`).

### Minor Changes

- Added `lexical-ui`, a Rust port of Lexical's model and headless packages at parity with Lexical JS: the core (editor, editor state, nodes, updates, transforms, commands, listeners, selection, NodeState and byte-exact JSON in the legacy and compact forms), with one opt-in feature per ported package (`extension`, `selection`, `utils`, `rich-text`, `plain-text`, `history`, `clipboard`, `list`, `link`, `autolink`, `code`, `code-highlight`, `mark`, `overflow`, `text`, `hashtag`, `file`, `devtools`, `html`, `a11y`, `decorator-block`, `menu`, `auto-embed` and `markdown`). The default build has no third-party dependencies. `rust/PARITY.md` lists what is ported and the documented exceptions.
