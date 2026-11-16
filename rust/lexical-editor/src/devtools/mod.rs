//! `@lexical/devtools-core` (the `devtools` feature), from
//! `packages/lexical-devtools-core/src`: [`generate_content`], the text of
//! the playground's tree view of an editor's state, its selection and the
//! editor's settings. Its React parts (`TreeView.tsx`,
//! `useLexicalCommandsLog.ts`) aren't ported; a log of commands is the
//! caller's to keep ([`LexicalCommandLogEntry`]).

mod generate_content;

pub use generate_content::{CustomPrintNodeFn, LexicalCommandLogEntry, generate_content};
