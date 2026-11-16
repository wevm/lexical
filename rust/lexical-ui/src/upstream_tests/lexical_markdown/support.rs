//! Helpers shared by the ports of `packages/lexical-markdown`'s unit tests:
//! `MarkdownTestExtension` (`__tests__/utils.ts`) and the table-driven
//! tests' check of their markers.

#![allow(dead_code)]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::markdown::{
    Transformer, convert_from_markdown_string, convert_to_markdown_string, transformers,
};
use crate::upstream_tests::support::{panic_message, update};
use crate::{AnyLexicalExtensionArgument, Editor, LexicalExtension, define_extension};

/// `MarkdownTestExtension`: the nodes the default `TRANSFORMERS` need, as
/// extensions (rich text, list, link and code). Nodes only.
pub(crate) fn markdown_test_extension() -> AnyLexicalExtensionArgument {
    define_extension(LexicalExtension {
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            crate::list::list_extension().into(),
            crate::link::link_extension().into(),
            crate::code::code_extension().into(),
        ],
        ..LexicalExtension::<(), (), ()>::new("@lexical/markdown/test", ())
    })
    .into()
}

/// An editor built from extensions that disposes itself when dropped, as
/// JS's `using editor = buildEditorFromExtensions(...)` does.
pub(crate) struct Using(pub(crate) LexicalEditorWithDispose);

impl Drop for Using {
    fn drop(&mut self) {
        self.0.dispose();
    }
}

impl std::ops::Deref for Using {
    type Target = Editor;

    fn deref(&self) -> &Editor {
        &self.0
    }
}

/// `using editor = buildEditorFromExtensions(extensions)`.
pub(crate) fn build(extensions: Vec<AnyLexicalExtensionArgument>) -> Using {
    Using(build_editor_from_extensions(extensions).unwrap())
}

/// `using editor = buildEditorFromExtensions([MarkdownTestExtension])`.
pub(crate) fn markdown_test_editor() -> Using {
    build(vec![markdown_test_extension()])
}

/// `editor.read(() => $convertToMarkdownString(transformers))`.
pub(crate) fn export(editor: &Editor, transformers: &[Transformer]) -> String {
    let transformers = transformers.to_vec();
    editor
        .read(move || convert_to_markdown_string(&transformers, None, false))
        .unwrap()
        .unwrap()
}

/// `editor.update(() => $convertFromMarkdownString(markdown, transformers),
/// {discrete: true})`.
pub(crate) fn import(editor: &Editor, markdown: &str, transformers: &[Transformer]) {
    let markdown = markdown.to_owned();
    let transformers = transformers.to_vec();
    update(editor, move || {
        convert_from_markdown_string(&markdown, &transformers, None, false, false)
    });
}

/// `TRANSFORMERS`.
pub(crate) fn all() -> Vec<Transformer> {
    transformers()
}

/// The ids marked above the test named `test_fn` in `source` (that file's
/// `include_str!`), as the ledger reads them.
pub(crate) fn markers_of(source: &str, test_fn: &str) -> Vec<String> {
    let lines: Vec<&str> = source.lines().collect();
    let at = lines
        .iter()
        .position(|l| *l == format!("fn {test_fn}() {{"))
        .expect("the test function");
    let mut start = at;
    while start > 0 && !lines[start - 1].is_empty() {
        start -= 1;
    }
    lines[start..at]
        .iter()
        .filter_map(|l| l.strip_prefix("// upstream: "))
        .map(|id| {
            if id.starts_with('"') {
                crate::json::parse(id)
                    .expect("a JSON string literal")
                    .as_str()
                    .expect("a string")
                    .to_owned()
            } else {
                id.to_owned()
            }
        })
        .collect()
}

/// A table-driven test's cases, each an upstream test: its id (after
/// `<file>::`) and its body.
pub(crate) type Cases = Vec<(String, Box<dyn FnOnce()>)>;

/// Runs each case, fails listing the cases that failed, and checks the ids
/// the cases stand for (`file::name`) are exactly the markers of `test_fn`
/// in `source`, in order.
pub(crate) fn run_cases(source: &str, file: &str, test_fn: &str, cases: Cases) {
    let mut ids = Vec::new();
    let mut failed = Vec::new();
    for (name, case) in cases {
        if let Err(payload) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(case)) {
            failed.push(format!("{name}: {}", panic_message(&*payload)));
        }
        ids.push(format!("{file}::{name}"));
    }
    assert!(
        failed.is_empty(),
        "{} of {} failed:\n{}",
        failed.len(),
        ids.len(),
        failed.join("\n")
    );
    assert_eq!(markers_of(source, test_fn), ids);
}

/// Vitest's `%j` of a string: `JSON.stringify`.
pub(crate) fn j(s: &str) -> String {
    crate::json::Value::String(s.to_owned()).to_string()
}
