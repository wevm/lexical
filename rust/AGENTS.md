# Rust port

Guidance for `rust/`. The root AGENTS.md describes upstream Lexical; where
they differ, this file wins for the Rust port. Read [DESIGN.md](DESIGN.md)
before changing architecture, and update [PARITY.md](PARITY.md) with what
you port.

## Rules

- **JS is the reference.** A difference from upstream is a Rust bug, unless
  it is an exception in PARITY.md. Scope is Tier 1 (model and headless
  packages); no editing UI.
- **Port, don't invent.** Modules mirror upstream files and functions keep
  their JS names in snake case without the `$`; doc comments name the JS
  function in backticks.
- **Offsets are UTF-16 code units**, as in JS; convert at the point of use.
- **JSON is byte-exact** in both the legacy and compact forms. Use the
  crate's `json` module, not `serde_json`.
- **No third-party dependencies** in the default build; put one behind an
  opt-in feature.
- **The crate name is provisional.** Other crates depend on it as `lexical`;
  don't spread `lexical_editor` further than DESIGN.md lists. Never publish.
- Vectors come from `packages/lexical-conformance` (JS). Never edit one by
  hand or write one from Rust; see `swift/AGENTS.md`, "Vectors".

## Checks

Before opening or updating a pull request, in `rust/`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo run -p lexical-conformance
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked
```

Pull requests follow `swift/AGENTS.md` (conventional-commit titles such as
`feat(rust): ...`, squash-merged, merged once CI is green).
