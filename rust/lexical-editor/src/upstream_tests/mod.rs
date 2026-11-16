//! Ports of upstream's unit tests, one module per upstream test file, under
//! a directory per package. Each test carries `// upstream: <id>` markers
//! naming the upstream tests it ports (rust/AGENTS.md, "Parity ledger").
//! Shared helpers live in `support`.

mod lexical;
mod support;
