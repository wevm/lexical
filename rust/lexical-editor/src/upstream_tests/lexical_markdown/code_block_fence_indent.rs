//! Ports of `packages/lexical-markdown/src/__tests__/unit/CodeBlockFenceIndent.test.ts`.

use super::support::markdown_test_editor;
use crate::code::is_code_node;
use crate::get_root;
use crate::markdown::{convert_from_markdown_string, transformers};
use crate::upstream_tests::support::update_with;

/// `importCodeText(markdown)`.
pub(super) fn import_code_text(markdown: &str) -> Option<String> {
    let editor = markdown_test_editor();
    let markdown = markdown.to_owned();
    update_with(&editor, move || {
        convert_from_markdown_string(&markdown, &transformers(), None, false, false)?;
        let first = get_root().get_first_child();
        Ok(first
            .filter(is_code_node)
            .map(|first| first.get_text_content()))
    })
}

fn code(s: &str) -> Option<String> {
    Some(s.to_owned())
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockFenceIndent.test.ts::indented code fences > removes the fence indentation from the content
#[test]
fn removes_the_fence_indentation_from_the_content() {
    assert_eq!(import_code_text("  ```js\n  code\n  ```"), code("code"));
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockFenceIndent.test.ts::indented code fences > preserves indentation relative to the fence
#[test]
fn preserves_indentation_relative_to_the_fence() {
    assert_eq!(
        import_code_text("   ```\n   a\n     b\n   ```"),
        code("a\n  b")
    );
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockFenceIndent.test.ts::indented code fences > removes at most the fence indentation
#[test]
fn removes_at_most_the_fence_indentation() {
    assert_eq!(import_code_text("  ```\ncode\n  ```"), code("code"));
    assert_eq!(import_code_text("  ```\n a\n  b\n  ```"), code("a\nb"));
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockFenceIndent.test.ts::indented code fences > handles a tab-indented fence
#[test]
fn handles_a_tab_indented_fence() {
    assert_eq!(import_code_text("\t```\n\tcode\n\t```"), code("code"));
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockFenceIndent.test.ts::indented code fences > keeps content indentation when the fence is not indented
#[test]
fn keeps_content_indentation_when_the_fence_is_not_indented() {
    assert_eq!(import_code_text("```js\n  code\n```"), code("  code"));
    assert_eq!(import_code_text("```js\ncode\n```"), code("code"));
}
