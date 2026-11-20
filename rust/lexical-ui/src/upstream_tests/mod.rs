//! Ports of upstream's unit tests, one module per upstream test file, under
//! a directory per package. Each test carries `// upstream: <id>` markers
//! naming the upstream tests it ports (rust/AGENTS.md, "Parity ledger").
//! Shared helpers live in `support`.

mod lexical;
#[cfg(all(feature = "a11y", feature = "rich-text"))]
mod lexical_a11y;
#[cfg(all(feature = "clipboard", feature = "extension", feature = "rich-text"))]
mod lexical_clipboard;
#[cfg(feature = "code")]
mod lexical_code_core;
#[cfg(feature = "code-highlight")]
mod lexical_code_prism;
#[cfg(feature = "extension")]
mod lexical_extension;
#[cfg(feature = "hashtag")]
mod lexical_hashtag;
mod lexical_headless;
#[cfg(feature = "history")]
mod lexical_history;
#[cfg(feature = "html")]
mod lexical_html;
#[cfg(feature = "link")]
mod lexical_link;
#[cfg(all(feature = "list", feature = "link", feature = "extension"))]
mod lexical_list;
#[cfg(feature = "mark")]
mod lexical_mark;
#[cfg(all(feature = "markdown", feature = "extension"))]
mod lexical_markdown;
mod lexical_react;
#[cfg(feature = "rich-text")]
mod lexical_rich_text;
#[cfg(feature = "selection")]
mod lexical_selection;
#[cfg(all(
    feature = "table",
    feature = "extension",
    feature = "html",
    feature = "rich-text"
))]
mod lexical_table;
#[cfg(all(feature = "text", feature = "extension", feature = "markdown"))]
mod lexical_text;
#[cfg(feature = "utils")]
mod lexical_utils;
mod support;
