# Lexical playground on Rune

The Lexical playground as a native desktop app: a document in
[`lexical-editor`](../../lexical-editor), the Rust port of Lexical, drawn and
edited with the [Rune](https://github.com/anomalyco/rune) UI framework.
macOS first; it also builds and renders on Linux.

- **The editor.** Lexical owns the document and the selection. Rune's
  `text_input` draws a mirror of them and does the layout, hit-testing,
  caret movement and input-method composition. Every change goes through
  Lexical:
  - Typing, Return (`insertParagraph`), Shift-Return (`insertLineBreak`),
    Backspace and Delete (with Option for a word, Command for a line), Tab,
    paste (`insertRawText`), ⌘A, ⌘B, ⌘I and ⌘U run as Lexical updates
    before the field sees them.
  - A change the field still makes itself, such as an input method's
    commit or a cut, is replayed into Lexical as the text it replaced.
  - After each edit the mirror is rebuilt from Lexical's state.
  - The shared binding, [`lexical-playground-core`](../lexical-playground-core),
    also backs the GPUI playground.
- **The toolbar.** Bold, italic, underline, strikethrough, code, highlight,
  subscript and superscript follow the selection's format, as in the web
  playground. Select all, and Reset.
- **The side panel.**
  - **Tree:** the node tree with keys.
  - **JSON:** the editor state's JSON.
  - **Parity:** **Replay vectors** runs the conformance vectors JS recorded
    (`packages/lexical-conformance/vectors`) in the app. **Export session**
    writes this session as a vector file (`$TMPDIR/lexical-playground-session.json`,
    or `$LEXICAL_PLAYGROUND_EXPORT`), which JS Lexical replays and compares
    at every step:

    ```sh
    cd packages/lexical-conformance
    LEXICAL_REPLAY=/path/to/lexical-playground-session.json pnpm exec vitest run src/replay.test.ts
    ```

Undo, headings, lists, links, code blocks, HTML and Markdown arrive as the
port gets the packages ([DESIGN.md](../../DESIGN.md), milestones).

## Running it

```sh
cd rust/apps/lexical-playground-rune
cargo run --release
```

Rune is pre-release and pinned to a rev of its `dev` branch (the one
wevm/zero's zbugs-rune uses). It needs Rust 1.90 or later, so the app is a
Cargo workspace of its own with Rune's toolchain (`rust-toolchain.toml`),
apart from `rust/`, which builds on 1.88. rustup installs that toolchain on
the first build.

On macOS, `cargo run` runs the app as an app: `bundle.sh`, Cargo's runner
there (`.cargo/config.toml`), makes `Lexical Playground (Rune).app` beside the
executable and runs the app from inside it, so it shows in the Dock with its
icon. The bundle stays, to open from Finder.

The icon, `AppIcon.icon`, is an Icon Composer document, as wevm/zero's
zbugs-rune has: the Lexical mark of the Swift playground's icon, with the
Rune mark in the bottom right corner, in light and dark. `bundle.sh`
compiles it with Xcode's `actool` when it changes. Open it in Icon Composer
to edit it.

Like every Rune app, it renders headless too:
`cargo run -- --snap playground.png --size 1100x720 --scene typed` (and
`--tree`, `--script`; see Rune's README). The scenes are `typed`, `json` and `parity` (Replay vectors, run).
