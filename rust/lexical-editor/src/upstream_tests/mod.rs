//! Ports of upstream's unit tests, one module per upstream test file, under
//! a directory per package. Each test carries `// upstream: <id>` markers
//! naming the upstream tests it ports (rust/AGENTS.md, "Parity ledger").
//! Shared helpers live in `support`.

mod lexical;
#[cfg(feature = "extension")]
mod lexical_extension;
#[cfg(feature = "hashtag")]
mod lexical_hashtag;
mod lexical_headless;
#[cfg(feature = "history")]
mod lexical_history;
#[cfg(feature = "mark")]
mod lexical_mark;
#[cfg(feature = "selection")]
mod lexical_selection;
mod support;
