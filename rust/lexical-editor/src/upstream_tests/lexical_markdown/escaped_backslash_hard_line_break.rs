//! Ports of `packages/lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts`.

use super::support::markdown_test_editor;
use crate::get_root;
use crate::markdown::{
    convert_from_markdown_string, normalize_markdown, parse_markdown_hard_line_break, transformers,
};
use crate::upstream_tests::support::update_with;

const ONE: &str = "foo\\";
const TWO: &str = "foo\\\\";
const THREE: &str = "foo\\\\\\";

/// `importText(markdown)`, merging adjacent lines.
fn import_text(markdown: &str) -> String {
    let editor = markdown_test_editor();
    let markdown = markdown.to_owned();
    update_with(&editor, move || {
        convert_from_markdown_string(&markdown, &transformers(), None, false, true)?;
        Ok(get_root().get_text_content())
    })
}

fn pair(a: &str, b: &str) -> Option<(String, String)> {
    Some((a.to_owned(), b.to_owned()))
}

// upstream: lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts::escaped backslash at end of line > treats a lone trailing backslash as a hard line break
#[test]
fn treats_a_lone_trailing_backslash_as_a_hard_line_break() {
    assert_eq!(parse_markdown_hard_line_break(ONE), pair("foo", "\\"));
}

// upstream: lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts::escaped backslash at end of line > does not treat an escaped backslash as a hard line break
#[test]
fn does_not_treat_an_escaped_backslash_as_a_hard_line_break() {
    assert_eq!(parse_markdown_hard_line_break(TWO), None);
}

// upstream: lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts::escaped backslash at end of line > treats an odd run of backslashes as a hard line break
#[test]
fn treats_an_odd_run_of_backslashes_as_a_hard_line_break() {
    assert_eq!(parse_markdown_hard_line_break(THREE), pair("foo\\\\", "\\"));
}

// upstream: lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts::escaped backslash at end of line > still recognises the two-space hard line break
#[test]
fn still_recognises_the_two_space_hard_line_break() {
    assert_eq!(parse_markdown_hard_line_break("foo  "), pair("foo", "  "));
}

// upstream: lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts::escaped backslash at end of line > merges a line ending in an escaped backslash with the next line
#[test]
fn merges_a_line_ending_in_an_escaped_backslash_with_the_next_line() {
    assert_eq!(
        normalize_markdown(&format!("{TWO}\nbar"), true),
        format!("{TWO} bar")
    );
}

// upstream: lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts::escaped backslash at end of line > keeps a real hard line break unmerged
#[test]
fn keeps_a_real_hard_line_break_unmerged() {
    assert_eq!(
        normalize_markdown(&format!("{ONE}\nbar"), true),
        format!("{ONE}\nbar")
    );
}

// upstream: lexical-markdown/src/__tests__/unit/EscapedBackslashHardLineBreak.test.ts::escaped backslash at end of line > keeps the literal backslash in the imported text
#[test]
fn keeps_the_literal_backslash_in_the_imported_text() {
    // `foo\\` is an escaped backslash, so the paragraph text is `foo\ bar`.
    assert_eq!(import_text(&format!("{TWO}\nbar")), "foo\\ bar");
}
