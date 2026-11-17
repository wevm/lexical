# Changelog

Each release names the upstream Lexical version it conforms to. Entries come
from the changesets in `.changeset/` (see its README), and the release
workflow publishes the section headed `## <version>`, the version in
`lexical-ui/Cargo.toml`.

## 0.1.1

Conforms to upstream Lexical **0.53.0** (`lexical_ui::UPSTREAM_VERSION`).

### Patch Changes

- Added `code::register_code_gutter`, the port of `registerCodeGutter`: it gives the app each code block's line numbers (what JS writes to `data-gutter`) when the block is rendered and whenever its line count changes.
- Added `on_selection_change`, the model half of JS's `onSelectionChange`: an app passes the points its view selected, and the editor selects them, gives a caret the format and style of the text it is in (and a range the format all its text shares), and dispatches `SELECTION_CHANGE_COMMAND`.

## 0.1.0

Conforms to upstream Lexical **0.53.0** (`lexical_ui::UPSTREAM_VERSION`).

### Minor Changes

- Added `lexical-ui`, a Rust port of Lexical's model and headless packages at parity with Lexical JS: the core (editor, editor state, nodes, updates, transforms, commands, listeners, selection, NodeState and byte-exact JSON in the legacy and compact forms), with one opt-in feature per ported package (`extension`, `selection`, `utils`, `rich-text`, `plain-text`, `history`, `clipboard`, `list`, `link`, `autolink`, `code`, `code-highlight`, `mark`, `overflow`, `text`, `hashtag`, `file`, `devtools`, `html`, `a11y`, `decorator-block`, `menu`, `auto-embed` and `markdown`). The default build has no third-party dependencies. `rust/PARITY.md` lists what is ported and the documented exceptions.
