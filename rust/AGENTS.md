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
  don't spread `lexical_editor` further than DESIGN.md lists. Never publish
  by hand: `rust-release.yml` publishes (see Releases).
- Vectors come from `packages/lexical-conformance` (JS). Never edit one by
  hand or write one from Rust; see `swift/AGENTS.md`, "Vectors".

## Parity ledger

Every upstream unit test of a package in `parity/packages.json` is either
ported, with a `// upstream: <id>` line directly above its `#[test]` (ids
are in `../swift/parity/upstream-tests.json`, the list both ports share;
one id per line, written as it is, or as a JSON string literal when it has
leading or trailing whitespace; an id is marked once), or listed in
`parity/skips.json` with a reason, `dom-only`, `react-only`,
`out-of-scope-package`, `covered-by-vector` or `deviation`, and a note
saying why for Rust. Mark only a test that checks what the upstream test
checks. Copy a Swift skip only where its reason holds for Rust: the `html`
feature has a DOM, Rust's deviations are PARITY.md's, and Rust's
`covered-by-vector` names what its runner checks. A package is marked
`"enforced"` once each of its tests is ported or skipped; CI then fails on
anything neither. An `out-of-scope-package` skip lists the directory names
of the packages it needs in `packages`; once all of them are enforced, the
ledger fails until the test is ported. `pnpm --filter lexical-conformance
run check-ledger` checks both ports and prints the counts (DESIGN.md,
Parity ledger).

## Checks

Before opening or updating a pull request, in `rust/`:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo run -p lexical-conformance
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --locked
pnpm --filter lexical-conformance run check-ledger
```

Pull requests follow `swift/AGENTS.md` (conventional-commit titles such as
`feat(rust): ...`, squash-merged, merged once CI is green).

## Changesets

A user-facing change to the `lexical-editor` crate (API, behavior, types,
features, minimum Rust version) adds a changeset in `rust/.changeset/` in
the same pull request; its README has the format. Write its body in the
past tense ("Added …", "Changed …", "Fixed …"). A break needs a `minor`
changeset below 1.0. Tests, the conformance runner, the playgrounds and CI
need none. Don't edit `rust/CHANGELOG.md` or the crate's version by hand.

## Releases

While changesets are pending, the Rust Release workflow keeps a
`chore(rust): version packages` pull request open, made by
`rust/scripts/version-packages.sh`, and starts the Rust workflow on it.
Merging it (squash, once that run is green) is the release: the workflow
publishes the mirror (wevm/lexical-rust), the tag, the GitHub release from
that version's `CHANGELOG.md` section, and the crate on crates.io. The
`rust-release` environment only limits deployments to `main`; no one
approves them. DESIGN.md, Releases, has the one-time steps tmm takes before
the first release.
