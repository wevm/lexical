# Lexical for Rust

A native Rust port of [Lexical](https://lexical.dev)'s model and headless
packages. Your app builds an editor from extensions, as in JS, and runs
updates, commands and listeners on it; the app draws the document and
feeds it input, as a browser does for `contenteditable`.

It's a port of the JS packages and behaves the same: the same documents
give the same editor state and the same JSON, byte for byte, in the legacy
and compact forms. Conformance vectors generated from upstream JS check it,
and the playgrounds' sessions are replayed in JS.

> [!NOTE]
> Before 1.0, minor versions may change the API.

## Install

```sh
cargo add lexical-ui --features extension,rich-text,history
```

Rust 1.88 or later. The crate is `lexical-ui`, used as `lexical_ui`; the
core is always built, and each upstream package is an opt-in
[feature](#features).

## Quick start

```rust
use lexical_ui::extension::build_editor_from_extensions;
use lexical_ui::history::history_extension;
use lexical_ui::rich_text::rich_text_extension;
use lexical_ui::{UpdateOptions, create_paragraph_node, create_text_node, get_root};

// Rich text and undo, as JS's `buildEditorFromExtensions`.
let editor =
    build_editor_from_extensions([rich_text_extension().into(), history_extension().into()])?;

// `$` functions drop the `$`, and work inside `update` and `read`.
editor.update(
    || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("Hello, Lexical")])?;
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph])
    },
    UpdateOptions { discrete: true, ..UpdateOptions::default() },
)?;
let text = editor.read(|| get_root().get_text_content())?;

// The same JSON as JS, byte for byte (`true` for the compact form).
let json = editor.get_editor_state().to_json(false).to_string();
editor.set_editor_state(&editor.parse_editor_state(&json)?, None)?;
```

## API at a glance

The [API reference](https://docs.rs/lexical-ui) documents every item, and
names the JS function each one ports.

| Concept | API | Notes |
| --- | --- | --- |
| Editor | `extension::build_editor_from_extensions(...)?`, `define_extension`, `config_extension` | `create_editor(CreateEditorArgs)` without extensions |
| Updates and reads | `editor.update(\|\| ..., options)?`, `editor.read(\|\| ...)?`, `get_root()`, `get_selection()` | `$getRoot()` is `get_root()` |
| Nodes | `NodeRef`: `append`, `remove`, `split_text`, `toggle_format`, ... | Resolves the latest version on every call |
| Commands | `create_command`, `register_command`, `dispatch_command` | JS's `COMMAND_PRIORITY_*` |
| Listeners | `register_update_listener`, `register_text_content_listener`, `register_mutation_listener` | Return an `Unregister` |
| Transforms | `register_node_transform` | |
| JSON | `get_editor_state().to_json(compact)`, `parse_editor_state(&str)?` | The crate's own `json`, not serde |
| Custom nodes | `impl LexicalNode`, registered in the editor's `nodes` | `ElementData`, `TextData` for the base classes |
| Node state | `create_state`, `get_state`, `set_state` | |
| The app's view | `set_root_element`, `on_selection_change`, `on_key_down`, `on_before_input` | The app lays out and hit-tests |

## Features

The default build is the core, with no dependencies. Each feature ports the
upstream package of the same name, and enables the features it needs:

| Feature | Adds |
| --- | --- |
| `extension` | The extension API and its model-level extensions, with each other feature's extensions |
| `rich-text`, `plain-text` | `register_rich_text`, `register_plain_text`, headings and quotes |
| `history`, `selection`, `utils`, `clipboard`, `text` | The upstream packages of the same name |
| `list`, `link`, `mark`, `hashtag`, `overflow`, `decorator-block` | Their nodes and commands |
| `autolink` | Auto-linking, on the `regex` crate: the one feature with a dependency |
| `code`, `code-highlight` | Code blocks, and highlighting over a `Tokenizer` the app supplies |
| `markdown` | Import, export and shortcuts |
| `html` | A DOM of its own, HTML export and import |
| `a11y`, `menu`, `auto-embed`, `file`, `devtools` | Announcements, typeahead menus, auto-embed, `.lexical` files, the tree view |

## Differences from JS

- **No `$` prefix**, and snake case: `$getRoot()` is `get_root()`. JS's
  invariants return a `LexicalError` with the same message.
- **One thread.** An `Editor` is neither `Send` nor `Sync`.
- **No editing UI.** The app draws the document, lays it out and moves the
  caret, and sends Lexical its input as commands; the playgrounds show how.
  With the `html` feature, the crate has a DOM of its own.
- **Code highlighting** runs over the app's `Tokenizer`; Prism's grammars
  aren't ported.
- **Offsets are UTF-16 code units**, as in JS.

[PARITY.md](https://github.com/wevm/lexical-rust/blob/main/PARITY.md) maps
each part of the JS packages to its Rust name, and lists what isn't ported
and the documented exceptions.

## Compatibility

| lexical-ui | Conforms to upstream Lexical |
| --- | --- |
| 0.1.0+ | 0.53.0 |

Each release's section in
[CHANGELOG.md](https://github.com/wevm/lexical-rust/blob/main/CHANGELOG.md)
names the upstream version, as does `lexical_ui::UPSTREAM_VERSION`.

## Documentation

- [Lexical's docs](https://lexical.dev/docs/intro)
- [API reference](https://docs.rs/lexical-ui) on docs.rs
- [Coming from JS](https://github.com/wevm/lexical-rust/blob/main/PARITY.md)
- [The design](https://github.com/wevm/lexical-rust/blob/main/DESIGN.md) and
  [the changelog](https://github.com/wevm/lexical-rust/blob/main/CHANGELOG.md)
- Desktop playgrounds on one shared core
  ([`apps/lexical-playground-core`](https://github.com/wevm/lexical-rust/tree/main/apps/lexical-playground-core)):
  [`lexical-playground-gpui`](https://github.com/wevm/lexical-rust/tree/main/apps/lexical-playground-gpui)
  and
  [`lexical-playground-rune`](https://github.com/wevm/lexical-rust/tree/main/apps/lexical-playground-rune)

## Contributing

This directory is published from [wevm/lexical](https://github.com/wevm/lexical)
(`rust/`), a fork of [meta/lexical](https://github.com/meta/lexical); send
changes there. From `rust/` in that repository:

```sh
cargo test --workspace --all-features   # unit tests
cargo run -p lexical-conformance        # every replayed vector matches JS, file by file
```

## License

MIT, like Lexical.
