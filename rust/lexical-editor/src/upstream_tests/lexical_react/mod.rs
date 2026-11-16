//! Ports of `packages/lexical-react`'s unit tests whose assertions the
//! React-free counterparts reach: the menus (`menu`), auto-embed
//! (`auto-embed`), the character limit (`overflow`) and the root text
//! helpers (`text`). The rest render React components and are skipped as
//! `react-only` (rust/parity/skips.json).

mod lexical_auto_embed_plugin;
mod lexical_character_limit_plugin;
mod lexical_menu;
mod use_lexical_character_limit;
mod use_lexical_is_text_content_empty;
