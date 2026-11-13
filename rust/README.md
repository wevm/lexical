# Lexical for Rust

A Rust port of [Lexical](https://lexical.dev)'s model and headless packages,
at parity with the JS implementation: the same documents give the same
editor state and the same JSON, byte for byte. Early work; nothing is
published, and the crate name (`lexical-editor`) may change.

```rust
use lexical_editor::{create_editor, get_root, CreateEditorArgs};

let editor = create_editor(CreateEditorArgs::default());
let state = editor.parse_editor_state(json)?;
let text = state.read(|| get_root().get_text_content());
let compact = state.to_json(true).to_string();
```

- [DESIGN.md](DESIGN.md): decisions, features per package, milestones.
- [PARITY.md](PARITY.md): what is ported, from which JS files, checked by
  which vectors, and the documented exceptions.
- `cargo run -p lexical-conformance` replays the conformance vectors that
  `packages/lexical-conformance` records from the JS packages.

MIT licensed, like Lexical.
