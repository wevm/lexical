//! Ports of `packages/lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts`.

use super::support::{export, markdown_test_editor};
use crate::code::is_code_node;
use crate::get_root;
use crate::markdown::{convert_from_markdown_string, transformers};
use crate::upstream_tests::support::{update, update_with};

/// `roundTrip(markdown)`.
pub(super) fn round_trip(markdown: &str) -> String {
    let editor = markdown_test_editor();
    let markdown = markdown.to_owned();
    update(&editor, move || {
        convert_from_markdown_string(&markdown, &transformers(), None, false, false)
    });
    export(&editor, &transformers())
}

/// `importCodeBlock(markdown)`: the language (`None` for `undefined`, or
/// when the first block isn't code) and the text.
fn import_code_block(markdown: &str) -> (Option<String>, Option<String>) {
    let editor = markdown_test_editor();
    let markdown = markdown.to_owned();
    update_with(&editor, move || {
        convert_from_markdown_string(&markdown, &transformers(), None, false, false)?;
        Ok(match get_root().get_first_child() {
            Some(first) if is_code_node(&first) => {
                (first.get_language(), Some(first.get_text_content()))
            }
            _ => (None, None),
        })
    })
}

fn expected(language: Option<&str>, text: &str) -> (Option<String>, Option<String>) {
    (language.map(str::to_owned), Some(text.to_owned()))
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > does not put a title attribute into the code content
#[test]
fn does_not_put_a_title_attribute_into_the_code_content() {
    assert_eq!(
        import_code_block("```js title=\"x\"\ncode\n```"),
        expected(Some("js"), "code")
    );
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > does not put a line-highlight range into the code content
#[test]
fn does_not_put_a_line_highlight_range_into_the_code_content() {
    assert_eq!(
        import_code_block("```ts {1,3}\ncode\n```"),
        expected(Some("ts"), "code")
    );
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > does not put a bare info-string word into the code content
#[test]
fn does_not_put_a_bare_info_string_word_into_the_code_content() {
    assert_eq!(
        import_code_block("```js showLineNumbers\na\nb\n```"),
        expected(Some("js"), "a\nb")
    );
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > keeps content that follows a fence with no language
#[test]
fn keeps_content_that_follows_a_fence_with_no_language() {
    assert_eq!(
        import_code_block("``` code\nmore\n```"),
        expected(None, "code\nmore")
    );
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > leaves a plain language fence unchanged
#[test]
fn leaves_a_plain_language_fence_unchanged() {
    assert_eq!(
        import_code_block("```javascript\nCode\n```"),
        expected(Some("javascript"), "Code")
    );
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > round trips the info string of a closed fence
#[test]
fn round_trips_the_info_string_of_a_closed_fence() {
    assert_eq!(
        round_trip("```js title=\"x\"\ncode\n```"),
        "```js title=\"x\"\ncode\n```"
    );
    assert_eq!(
        round_trip("```ts {1,3}\ncode\n```"),
        "```ts {1,3}\ncode\n```"
    );
    assert_eq!(
        round_trip("```js showLineNumbers\na\nb\n```"),
        "```js showLineNumbers\na\nb\n```"
    );
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > adds nothing to a fence that has no info string tail
#[test]
fn adds_nothing_to_a_fence_that_has_no_info_string_tail() {
    assert_eq!(round_trip("```js\ncode\n```"), "```js\ncode\n```");
    assert_eq!(round_trip("```\ncode\n```"), "```\ncode\n```");
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockInfoString.test.ts::fenced code block info string > leaves an unterminated fence unchanged
#[test]
fn leaves_an_unterminated_fence_unchanged() {
    assert_eq!(
        import_code_block("```javascript Incomplete tag"),
        expected(Some("javascript"), "Incomplete tag")
    );
}
