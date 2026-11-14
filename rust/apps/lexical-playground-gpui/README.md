# Lexical playground on GPUI

The Lexical playground as a native desktop app: a document in
[`lexical-editor`](../../lexical-editor), the Rust port of Lexical, drawn and
edited with [GPUI](https://www.gpui.rs) through GPUI Kit, as wevm/zero's
zbugs-gpui is. macOS.

- **The editor.** Lexical owns the document and the selection. The editor
  is a GPUI element of its own (`src/editor.rs`) that draws them, shapes
  and wraps the text, hit-tests the pointer and moves the caret, as a
  browser does for `contenteditable`. It is the window's text input client
  (`EntityInputHandler`), so typing, the character palette and input
  methods reach it as macOS sends them. Every change goes through Lexical:
  - Typed and committed text runs `insertText`; Return `insertParagraph`;
    Shift-Return `insertLineBreak`; Backspace and Delete delete by grapheme,
    Option by word and Command by line; paste runs `insertRawText`; ⌘A, ⌘B,
    ⌘I and ⌘U select all and format. ⌘Z and ⌘⇧Z dispatch `UNDO_COMMAND`
    and `REDO_COMMAND` to `@lexical/history` (the `history` feature), which
    merges typing within 300 ms into one step, as `HistoryExtension` does.
  - An input method's composition is drawn underlined and kept out of
    Lexical until it commits, as a browser keeps it out of the editor
    state.
  - After each edit the element draws again from Lexical's state.
  - The binding, `Surface` in
    [`lexical-playground-core`](../lexical-playground-core), maps the
    window's UTF-16 ranges to Lexical points. It also backs the Rune
    playground.
- **The toolbar.** Bold, italic, underline, strikethrough, code, highlight,
  subscript and superscript follow the selection's format. Select all, and
  Reset.
- **Light and dark.** The window follows the Mac's appearance and switches
  live when it changes, as the Rune playground does.
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

Headings, lists, links, code blocks, HTML and Markdown arrive as the
port gets the packages ([DESIGN.md](../../DESIGN.md), milestones).

## Running it

```sh
cd rust/apps/lexical-playground-gpui
cargo run --release
```

The app is a Cargo workspace of its own, on the toolchain zbugs-gpui uses
(`rust-toolchain.toml`), apart from `rust/`, which builds on 1.88. rustup
installs that toolchain on the first build.

`cargo run` runs the app as an app: `bundle.sh`, Cargo's runner on macOS
(`.cargo/config.toml`), makes `Lexical Playground (GPUI).app` beside the
executable and runs the app from inside it, so it shows in the Dock with its
icon. The bundle stays, to open from Finder.

The icon, `AppIcon.icon`, is an Icon Composer document, as wevm/zero's
zbugs-gpui has: the Lexical mark of the Swift playground's icon, with the
GPUI mark in the bottom right corner, in light and dark. `bundle.sh`
compiles it with Xcode's `actool` when it changes. Open it in Icon Composer
to edit it.

## End to end

Built with the `e2e` feature, the app drives its own window once it's up:
it types, formats, composes with an input method, types an emoji and
deletes it, adds a paragraph and a line break, and undoes and redoes, through the input
handler and the key bindings, checking the document after each step. It
writes the session to `$LEXICAL_PLAYGROUND_E2E` and exits 0, or 1 with the
step that failed. CI (`.github/workflows/rust-playground.yml`) then replays
the session in JS.

```sh
cargo build --features e2e
LEXICAL_PLAYGROUND_E2E=/tmp/session.json ./target/debug/lexical-playground-gpui
```
