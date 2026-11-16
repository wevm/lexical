//! Ports of `packages/lexical-markdown/src/__tests__/unit/CodeBlockLeadingBlankLine.test.ts`.

use super::code_block_fence_indent::import_code_text;
use super::code_block_info_string::round_trip;

fn code(s: &str) -> Option<String> {
    Some(s.to_owned())
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockLeadingBlankLine.test.ts::code block leading blank lines > keeps a blank first line of a fenced code block
#[test]
fn keeps_a_blank_first_line_of_a_fenced_code_block() {
    assert_eq!(import_code_text("```js\n\ncode\n```"), code("\ncode"));
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockLeadingBlankLine.test.ts::code block leading blank lines > keeps several blank first lines
#[test]
fn keeps_several_blank_first_lines() {
    assert_eq!(import_code_text("```js\n\n\na\nb\n```"), code("\n\na\nb"));
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockLeadingBlankLine.test.ts::code block leading blank lines > round trips a code block that starts with a blank line
#[test]
fn round_trips_a_code_block_that_starts_with_a_blank_line() {
    let markdown = "```js\n\ncode\n```";
    assert_eq!(round_trip(markdown), markdown);
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockLeadingBlankLine.test.ts::code block leading blank lines > still drops nothing from a code block without leading blank lines
#[test]
fn still_drops_nothing_from_a_code_block_without_leading_blank_lines() {
    assert_eq!(import_code_text("```js\ncode\n```"), code("code"));
    assert_eq!(round_trip("```js\ncode\n```"), "```js\ncode\n```");
}

// upstream: lexical-markdown/src/__tests__/unit/CodeBlockLeadingBlankLine.test.ts::code block leading blank lines > still treats text after the opening fence as content
#[test]
fn still_treats_text_after_the_opening_fence_as_content() {
    assert_eq!(import_code_text("``` code\nmore\n```"), code("code\nmore"));
    assert_eq!(
        import_code_text("```javascript Incomplete tag"),
        code("Incomplete tag")
    );
}
