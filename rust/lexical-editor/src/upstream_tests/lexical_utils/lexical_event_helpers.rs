//! Ports of
//! `packages/lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx`.
//!
//! Upstream renders a React `RichTextPlugin` editor (with `AutoFocusPlugin`)
//! into jsdom, dispatches a `paste` whose clipboard has only `text/html`,
//! and reads the container's `innerHTML`. The port registers rich text on
//! an editor with upstream's nodes (but the table's, which the pasted HTML
//! never makes) and theme, mounted, with the selection where
//! `AutoFocusPlugin` puts it (the end of the composer's empty paragraph),
//! dispatches `PASTE_COMMAND` with a `ClipboardEvent` carrying the HTML, and
//! renders the committed document as the reconciler does
//! ([`rendered_html`]). The cases are generated from upstream's suites.

#![cfg(all(
    feature = "html",
    feature = "rich-text",
    feature = "list",
    feature = "link",
    feature = "code",
    feature = "hashtag",
    feature = "overflow"
))]

use crate::upstream_tests::lexical_selection::support::rendered_html;
use crate::upstream_tests::support::{create_test_editor, set_test_root_element, update};
use crate::{
    ClipboardEvent, CreateEditorArgs, DataTransfer, Editor, NodeClass, PASTE_COMMAND,
    PasteCommandType, create_paragraph_node, get_root,
};
use std::rc::Rc;

const ROOT_OPEN: &str = "<div contenteditable=\"true\" style=\"user-select: text; white-space: pre-wrap; word-break: break-word;\" data-lexical-editor=\"true\">";

const THEME: &str = r#"{"code":"editor-code","heading":{"h1":"editor-heading-h1","h2":"editor-heading-h2","h3":"editor-heading-h3","h4":"editor-heading-h4","h5":"editor-heading-h5","h6":"editor-heading-h6"},"image":"editor-image","list":{"listitem":"editor-listitem","olDepth":["editor-list-ol"],"ulDepth":["editor-list-ul"]},"paragraph":"editor-paragraph","placeholder":"editor-placeholder","quote":"editor-quote","text":{"bold":"editor-text-bold","code":"editor-text-code","hashtag":"editor-text-hashtag","italic":"editor-text-italic","link":"editor-text-link","strikethrough":"editor-text-strikethrough","underline":"editor-text-underline","underlineStrikethrough":"editor-text-underlineStrikethrough"}}"#;

/// The editor of upstream's `TestBase`, and what `RichTextPlugin`
/// registered.
fn init() -> (Editor, crate::Unregister) {
    let editor = create_test_editor(CreateEditorArgs {
        nodes: vec![
            NodeClass::of::<crate::code::CodeNode>(),
            NodeClass::of::<crate::hashtag::HashtagNode>(),
            NodeClass::of::<crate::code::CodeHighlightNode>(),
            NodeClass::of::<crate::overflow::OverflowNode>(),
        ],
        theme: crate::json::parse(THEME)
            .unwrap()
            .as_object()
            .unwrap()
            .clone(),
        ..CreateEditorArgs::default()
    });
    let unregister = crate::rich_text::register_rich_text(&editor);
    set_test_root_element(&editor);
    // `LexicalComposer`'s initial paragraph; `AutoFocusPlugin` selects its
    // end.
    update(&editor, || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.select_end()?;
        Ok(())
    });
    (editor, unregister)
}

/// `container.innerHTML`.
fn container_html(editor: &Editor) -> String {
    editor.read(|| ()).unwrap();
    format!("{ROOT_OPEN}{}</div>", rendered_html(editor))
}

/// `applySelectionInputs([pasteHTML(html)])`, then the HTML upstream
/// compares: the container's (`baseline`, `Google Docs`) or the root
/// element's (`W3 spacing`), whose expectations leave out the root.
fn check(html: &str, expected_html: &str) {
    let (editor, _unregister) = init();
    let clipboard_data = DataTransfer::new();
    clipboard_data.set_data("text/html", html);
    editor
        .dispatch_command(
            &PASTE_COMMAND,
            PasteCommandType::Clipboard(Rc::new(ClipboardEvent::new(
                "paste",
                Some(Rc::new(clipboard_data)),
            ))),
        )
        .unwrap();
    let actual = if expected_html.starts_with(ROOT_OPEN) {
        container_html(&editor)
    } else {
        editor.read(|| ()).unwrap();
        rendered_html(&editor)
    };
    assert_eq!(actual, expected_html);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > Expect initial output to be a block with no text
#[test]
fn expect_initial_output_to_be_a_block_with_no_text() {
    let (editor, _unregister) = init();
    assert_eq!(
        container_html(&editor),
        format!(
            "{ROOT_OPEN}<p class=\"editor-paragraph\" dir=\"auto\"><br data-lexical-managed-linebreak=\"true\"></p></div>"
        )
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML h1 element (#1)
#[test]
fn baseline_a_pasted_html_h1_element_1() {
    check(
        r#"<meta charset='utf-8'><h1>Hello</h1>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><h1 class="editor-heading-h1" dir="auto"><span data-lexical-text="true">Hello</span></h1></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML h2 element (#2)
#[test]
fn baseline_a_pasted_html_h2_element_2() {
    check(
        r#"<meta charset='utf-8'><h2>From</h2>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><h2 class="editor-heading-h2" dir="auto"><span data-lexical-text="true">From</span></h2></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML h3 element (#3)
#[test]
fn baseline_a_pasted_html_h3_element_3() {
    check(
        r#"<meta charset='utf-8'><h3>The</h3>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><h3 class="editor-heading-h3" dir="auto"><span data-lexical-text="true">The</span></h3></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML ul element (#4)
#[test]
fn baseline_a_pasted_html_ul_element_4() {
    check(
        r#"<meta charset='utf-8'><ul><li>Other side</li><li>I must have called</li></ul>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><ul class="editor-list-ul" dir="auto"><li value="1" class="editor-listitem"><span data-lexical-text="true">Other side</span></li><li value="2" class="editor-listitem"><span data-lexical-text="true">I must have called</span></li></ul></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from pasted HTML ol element (#5)
#[test]
fn baseline_pasted_html_ol_element_5() {
    check(
        r#"<meta charset='utf-8'><ol><li>To tell you</li><li>I’m sorry</li></ol>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><ol class="editor-list-ol" dir="auto"><li value="1" class="editor-listitem"><span data-lexical-text="true">To tell you</span></li><li value="2" class="editor-listitem"><span data-lexical-text="true">I’m sorry</span></li></ol></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from pasted DOM Text Node (#6)
#[test]
fn baseline_pasted_dom_text_node_6() {
    check(
        r#"<meta charset='utf-8'>A thousand times"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">A thousand times</span></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML b element (#7)
#[test]
fn baseline_a_pasted_html_b_element_7() {
    check(
        r#"<meta charset='utf-8'><b>Bold</b>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><strong class="editor-text-bold" data-lexical-text="true">Bold</strong></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML i element (#8)
#[test]
fn baseline_a_pasted_html_i_element_8() {
    check(
        r#"<meta charset='utf-8'><i>Italic</i>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><em class="editor-text-italic" data-lexical-text="true">Italic</em></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML em element (#9)
#[test]
fn baseline_a_pasted_html_em_element_9() {
    check(
        r#"<meta charset='utf-8'><em>Italic</em>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><em class="editor-text-italic" data-lexical-text="true">Italic</em></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML u element (#10)
#[test]
fn baseline_a_pasted_html_u_element_10() {
    check(
        r#"<meta charset='utf-8'><u>Underline</u>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><span class="editor-text-underline" data-lexical-text="true">Underline</span></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from pasted heading node followed by a DOM Text Node (#11)
#[test]
fn baseline_pasted_heading_node_followed_by_a_dom_text_node_11() {
    check(
        r#"<meta charset='utf-8'><h1>Lyrics to Hello by Adele</h1>A thousand times"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><h1 class="editor-heading-h1" dir="auto"><span data-lexical-text="true">Lyrics to Hello by Adele</span></h1><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">A thousand times</span></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted HTML anchor element (#12)
#[test]
fn baseline_a_pasted_html_anchor_element_12() {
    check(
        r#"<meta charset='utf-8'><a href="https://facebook.com">Facebook</a>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><a href="https://facebook.com"><span data-lexical-text="true">Facebook</span></a></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted combination of an HTML text node followed by an anchor node (#13)
#[test]
fn baseline_a_pasted_combination_of_an_html_text_node_followed_by_an_anchor_node_13() {
    check(
        r#"<meta charset='utf-8'>Welcome to<a href="https://facebook.com">Facebook!</a>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">Welcome to</span><a href="https://facebook.com"><span data-lexical-text="true">Facebook!</span></a></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should produce the correct editor state from a pasted combination of HTML anchor elements and text nodes (#14)
#[test]
fn baseline_a_pasted_combination_of_html_anchor_elements_and_text_nodes_14() {
    check(
        r#"<meta charset='utf-8'>Welcome to<a href="https://facebook.com">Facebook!</a>We hope you like it here."#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">Welcome to</span><a href="https://facebook.com"><span data-lexical-text="true">Facebook!</span></a><span data-lexical-text="true">We hope you like it here.</span></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should ignore DOM node types that do not have transformers, but still process their children. (#15)
#[test]
fn baseline_should_ignore_dom_node_types_that_do_not_have_transformers_but_still_process_their_children_15()
 {
    check(
        r#"<meta charset='utf-8'><doesnotexist><ul><li>Hello</li><li>from the other</li><li>side</li></ul></doesnotexist>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><ul class="editor-list-ul" dir="auto"><li value="1" class="editor-listitem"><span data-lexical-text="true">Hello</span></li><li value="2" class="editor-listitem"><span data-lexical-text="true">from the other</span></li><li value="3" class="editor-listitem"><span data-lexical-text="true">side</span></li></ul></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should ignore multiple levels of DOM node types that do not have transformers, but still process their children. (#16)
#[test]
fn baseline_should_ignore_multiple_levels_of_dom_node_types_that_do_not_have_transformers_but_still_process_their_children_16()
 {
    check(
        r#"<meta charset='utf-8'><doesnotexist><doesnotexist><ul><li>Hello</li><li>from the other</li><li>side</li></ul></doesnotexist></doesnotexist>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><ul class="editor-list-ul" dir="auto"><li value="1" class="editor-listitem"><span data-lexical-text="true">Hello</span></li><li value="2" class="editor-listitem"><span data-lexical-text="true">from the other</span></li><li value="3" class="editor-listitem"><span data-lexical-text="true">side</span></li></ul></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should preserve formatting from HTML tags on deeply nested text nodes. (#17)
#[test]
fn baseline_should_preserve_formatting_from_html_tags_on_deeply_nested_text_nodes_17() {
    check(
        r#"<meta charset='utf-8'>Welcome to<b><a href="https://facebook.com">Facebook!</a></b>We hope you like it here."#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">Welcome to</span><a href="https://facebook.com"><strong class="editor-text-bold" data-lexical-text="true">Facebook!</strong></a><span data-lexical-text="true">We hope you like it here.</span></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should preserve formatting from HTML tags on deeply nested and top level text nodes. (#18)
#[test]
fn baseline_should_preserve_formatting_from_html_tags_on_deeply_nested_and_top_level_text_nodes_18()
{
    check(
        r#"<meta charset='utf-8'>Welcome to<b><a href="https://facebook.com">Facebook!</a>We hope you like it here.</b>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">Welcome to</span><a href="https://facebook.com"><strong class="editor-text-bold" data-lexical-text="true">Facebook!</strong></a><strong class="editor-text-bold" data-lexical-text="true">We hope you like it here.</strong></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > baseline > should preserve multiple types of formatting on deeply nested text nodes and top level text nodes (#19)
#[test]
fn baseline_should_preserve_multiple_types_of_formatting_on_deeply_nested_text_nodes_and_top_level_text_nodes_19()
 {
    check(
        r#"<meta charset='utf-8'>Welcome to<b><i><a href="https://facebook.com">Facebook!</a>We hope you like it here.</i></b>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">Welcome to</span><a href="https://facebook.com"><strong class="editor-text-bold editor-text-italic" data-lexical-text="true">Facebook!</strong></a><strong class="editor-text-bold editor-text-italic" data-lexical-text="true">We hope you like it here.</strong></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > Google Docs > should produce the correct editor state from Normal text (#1)
#[test]
fn google_docs_normal_text_1() {
    check(
        r#"<b style="font-weight:normal;" id="docs-internal-guid-2c706577-7fff-f54a-fe65-12f480020fac"><span style="font-size:11pt;font-family:Arial;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">Get schwifty!</span></b>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">Get schwifty!</span></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > Google Docs > should produce the correct editor state from bold text (#2)
#[test]
fn google_docs_bold_text_2() {
    check(
        r#"<b style="font-weight:normal;" id="docs-internal-guid-9db03964-7fff-c26c-8b1e-9484fb3b54a4"><span style="font-size:11pt;font-family:Arial;color:#000000;background-color:transparent;font-weight:700;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">Get schwifty!</span></b>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><strong class="editor-text-bold" data-lexical-text="true">Get schwifty!</strong></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > Google Docs > should produce the correct editor state from italic text (#3)
#[test]
fn google_docs_italic_text_3() {
    check(
        r#"<b style="font-weight:normal;" id="docs-internal-guid-9db03964-7fff-c26c-8b1e-9484fb3b54a4"><span style="font-size:11pt;font-family:Arial;color:#000000;background-color:transparent;font-weight:400;font-style:italic;font-variant:normal;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">Get schwifty!</span></b>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><em class="editor-text-italic" data-lexical-text="true">Get schwifty!</em></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > Google Docs > should produce the correct editor state from strikethrough text (#4)
#[test]
fn google_docs_strikethrough_text_4() {
    check(
        r#"<b style="font-weight:normal;" id="docs-internal-guid-9db03964-7fff-c26c-8b1e-9484fb3b54a4"><span style="font-size:11pt;font-family:Arial;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:line-through;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">Get schwifty!</span></b>"#,
        r#"<div contenteditable="true" style="user-select: text; white-space: pre-wrap; word-break: break-word;" data-lexical-editor="true"><p class="editor-paragraph" dir="auto"><span class="editor-text-strikethrough" data-lexical-text="true">Get schwifty!</span></p></div>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > inline hello world (#1)
#[test]
fn w3_spacing_inline_hello_world_1() {
    check(
        r#"<span>hello world</span>"#,
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">hello world</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > inline hello world (2) (#2)
#[test]
fn w3_spacing_inline_hello_world_2_2() {
    check(
        r#"<span>    hello  </span>world  "#,
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">hello world</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > pre + inline (inline collapses with pre) (#3)
#[test]
fn w3_spacing_pre_inline_inline_collapses_with_pre_3() {
    check(
        r#" <span style="white-space: pre"> hello </span> world  "#,
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true"> hello world</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > white-space: pre (1) (no touchy) (#4)
#[test]
fn w3_spacing_white_space_pre_1_no_touchy_4() {
    check(
        "<p style=\"white-space: pre\">  a b\tc  </p>",
        "<p class=\"editor-paragraph\" dir=\"auto\"><span data-lexical-text=\"true\">  a b</span><span data-lexical-text=\"true\">\t</span><span data-lexical-text=\"true\">c  </span></p>",
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > tabs are collapsed (#5)
#[test]
fn w3_spacing_tabs_are_collapsed_5() {
    check(
        "<p>\ta\tb  <span>c\t</span>\t</p>",
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">a b c</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > remove beginning + end spaces on the block (#6)
#[test]
fn w3_spacing_remove_beginning_end_spaces_on_the_block_6() {
    check(
        "\n              <div>\n                hello\n                world\n              </div>\n            ",
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">hello world</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > remove beginning + end spaces on the block (2) (#7)
#[test]
fn w3_spacing_remove_beginning_end_spaces_on_the_block_2_7() {
    check(
        "\n              <div>\n                <strong>\n                  hello\n                  world\n                </strong>\n              </div>\n          ",
        r#"<p class="editor-paragraph" dir="auto"><strong class="editor-text-bold" data-lexical-text="true">hello world</strong></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > remove beginning + end spaces on the block + anonymous inlines collapsible rules (#8)
#[test]
fn w3_spacing_remove_beginning_end_spaces_on_the_block_anonymous_inlines_collapsible_rules_8() {
    check(
        "\n              <div>\n                a\n                <strong>b</strong>\n                c\n              </div>\n          ",
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">a </span><strong class="editor-text-bold" data-lexical-text="true">b</strong><span data-lexical-text="true"> c</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > collapsibles and neighbors (1) (#9)
#[test]
fn w3_spacing_collapsibles_and_neighbors_1_9() {
    check(
        r#"<div><strong>a </strong>b</div>"#,
        r#"<p class="editor-paragraph" dir="auto"><strong class="editor-text-bold" data-lexical-text="true">a </strong><span data-lexical-text="true">b</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > collapsibles and neighbors (2) (#10)
#[test]
fn w3_spacing_collapsibles_and_neighbors_2_10() {
    check(
        r#"<div>a<strong> b</strong></div>"#,
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">a</span><strong class="editor-text-bold" data-lexical-text="true"> b</strong></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > collapsibles and neighbors (3) (#11)
#[test]
fn w3_spacing_collapsibles_and_neighbors_3_11() {
    check(
        r#"<div><strong>a </strong><span></span>b</div>"#,
        r#"<p class="editor-paragraph" dir="auto"><strong class="editor-text-bold" data-lexical-text="true">a </strong><span data-lexical-text="true">b</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > collapsibles and neighbors (4) (#12)
#[test]
fn w3_spacing_collapsibles_and_neighbors_4_12() {
    check(
        r#"<div>a<span></span><strong> b</strong></div>"#,
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">a</span><strong class="editor-text-bold" data-lexical-text="true"> b</strong></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > empty block (#13)
#[test]
fn w3_spacing_empty_block_13() {
    check(
        "\n              <p>\n              </p>\n          ",
        r#"<p class="editor-paragraph" dir="auto"><br data-lexical-managed-linebreak="true"></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > redundant inline at start (#14)
#[test]
fn w3_spacing_redundant_inline_at_start_14() {
    check(
        r#"<span> </span><span>a</span>"#,
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">a</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > redundant inline at end (#15)
#[test]
fn w3_spacing_redundant_inline_at_end_15() {
    check(
        r#"<span>a</span><span> </span>"#,
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">a</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > collapsible spaces with nested structures (#16)
#[test]
fn w3_spacing_collapsible_spaces_with_nested_structures_16() {
    check(
        "\n            <div>\n              <p>\n                a\n              </p>\n              <p>\n                b\n              </p>\n            </div>\n            ",
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">a</span></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">b</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > collapsible spaces with nested structures (3) (#17)
#[test]
fn w3_spacing_collapsible_spaces_with_nested_structures_3_17() {
    check(
        "\n            <div>\n              <strong>\n                a\n              </strong>\n              <strong>\n                b\n              </strong>\n            </div>\n            ",
        r#"<p class="editor-paragraph" dir="auto"><strong class="editor-text-bold" data-lexical-text="true">a b</strong></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > forced line break should remain (#18)
#[test]
fn w3_spacing_forced_line_break_should_remain_18() {
    check(
        "\n            <p>\n            a\n            <br>\n            b\n            </p>\n            ",
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">a</span><br><span data-lexical-text="true">b</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > forced line break with tabs (#19)
#[test]
fn w3_spacing_forced_line_break_with_tabs_19() {
    check(
        "\n            <p>\n            a\n            \t<br>\t\n            b\n            </p>\n            ",
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">a</span><br><span data-lexical-text="true">b</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > pre (no touchy) (1) (#20)
#[test]
fn w3_spacing_pre_no_touchy_1_20() {
    check(
        "<pre>\na\r\nb\r\n</pre>",
        r#"<code class="editor-code" spellcheck="false" dir="auto"><span data-lexical-text="true">a</span><br><span data-lexical-text="true">b</span><br><br data-lexical-managed-linebreak="true"></code>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > pre (no touchy) (2) (#21)
#[test]
fn w3_spacing_pre_no_touchy_2_21() {
    check(
        "\n              <pre>\na\r\nb\r\n</pre>\n          ",
        r#"<code class="editor-code" spellcheck="false" dir="auto"><span data-lexical-text="true">a</span><br><span data-lexical-text="true">b</span><br><br data-lexical-managed-linebreak="true"></code>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > white-space: pre (no touchy) (2) (#22)
#[test]
fn w3_spacing_white_space_pre_no_touchy_2_22() {
    check(
        "<span style=\"white-space: pre\">\na\r\nb\r\n</span>",
        r#"<p class="editor-paragraph" dir="auto"><br><span data-lexical-text="true">a</span><br><span data-lexical-text="true">b</span><br><br data-lexical-managed-linebreak="true"></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > two Apple Notes paragraphs (#23)
#[test]
fn w3_spacing_two_apple_notes_paragraphs_23() {
    check(
        "\n<p class=\"p1\">paragraph1</p>\n<p class=\"p1\">paragraph2</p>\n",
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">paragraph1</span></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">paragraph2</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > two Apple Notes lines + two paragraphs separated by an empty paragraph (#24)
#[test]
fn w3_spacing_two_apple_notes_lines_two_paragraphs_separated_by_an_empty_paragraph_24() {
    check(
        "\n<p class=\"p1\">line 1<br>\nline 2</p>\n<p class=\"p2\"><br></p>\n<p class=\"p1\">paragraph 1</p>\n<p class=\"p1\">paragraph 2</p>\n",
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">line 1</span><br><span data-lexical-text="true">line 2</span></p><p class="editor-paragraph" dir="auto"><br data-lexical-managed-linebreak="true"></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">paragraph 1</span></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">paragraph 2</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > two lines + two paragraphs separated by an empty paragraph (2) (#25)
#[test]
fn w3_spacing_two_lines_two_paragraphs_separated_by_an_empty_paragraph_2_25() {
    check(
        "\n<p class=\"p1\">line 1<br>\nline 2</p>\n<p class=\"p2\">\n<br>\n</p>\n<p class=\"p1\">paragraph 1</p>\n<p class=\"p1\">paragraph 2</p>\n",
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">line 1</span><br><span data-lexical-text="true">line 2</span></p><p class="editor-paragraph" dir="auto"><br data-lexical-managed-linebreak="true"></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">paragraph 1</span></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">paragraph 2</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > two lines and br in spans (#26)
#[test]
fn w3_spacing_two_lines_and_br_in_spans_26() {
    check(
        r#"<p class="p1"><span>line 1</span><span><br></span><span>line 2</span></p>"#,
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">line 1</span><br><span data-lexical-text="true">line 2</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > lone trailing block-level br collapses to an empty paragraph (#27)
#[test]
fn w3_spacing_lone_trailing_block_level_br_collapses_to_an_empty_paragraph_27() {
    check(
        r#"<p>a</p><br>"#,
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">a</span></p><p class="editor-paragraph" dir="auto"><br data-lexical-managed-linebreak="true"></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > lone block-level br between paragraphs collapses to an empty paragraph (#28)
#[test]
fn w3_spacing_lone_block_level_br_between_paragraphs_collapses_to_an_empty_paragraph_28() {
    check(
        r#"<p>a</p><br><p>b</p>"#,
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">a</span></p><p class="editor-paragraph" dir="auto"><br data-lexical-managed-linebreak="true"></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">b</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > block-level br followed by inline content is preserved (#29)
#[test]
fn w3_spacing_block_level_br_followed_by_inline_content_is_preserved_29() {
    check(
        r#"<br>a<p>b</p>"#,
        r#"<p class="editor-paragraph" dir="auto"><br><span data-lexical-text="true">a</span></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">b</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > empty block node in li behaves like a line break (#30)
#[test]
fn w3_spacing_empty_block_node_in_li_behaves_like_a_line_break_30() {
    check(
        r#"<ol><li>1<div></div>2</li><li></li><li>3</li></ol>"#,
        r#"<ol class="editor-list-ol" dir="auto"><li value="1" class="editor-listitem"><span data-lexical-text="true">1</span><br><span data-lexical-text="true">2</span></li><li value="2" class="editor-listitem"><br data-lexical-managed-linebreak="true"></li><li value="3" class="editor-listitem"><span data-lexical-text="true">3</span></li></ol>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > empty block node in div behaves like a line break (#31)
#[test]
fn w3_spacing_empty_block_node_in_div_behaves_like_a_line_break_31() {
    check(
        r#"<div>1<div></div>2</div>"#,
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">1</span><br><span data-lexical-text="true">2</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > empty inline node does not behave like a line break (#32)
#[test]
fn w3_spacing_empty_inline_node_does_not_behave_like_a_line_break_32() {
    check(
        r#"<div>1<text></text>2</div>"#,
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">12</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > empty block node between non inline siblings does not behave like a line break (#33)
#[test]
fn w3_spacing_empty_block_node_between_non_inline_siblings_does_not_behave_like_a_line_break_33() {
    check(
        r#"<div><div>1</div><div></div><div>2</div></div>"#,
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">1</span></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">2</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > nested divs (#34)
#[test]
fn w3_spacing_nested_divs_34() {
    check(
        r#"<div>a<div>b b<div>c<div><div></div>z</div></div>d e</div>fg</div>"#,
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">a</span></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">b b</span></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">c</span></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">z</span></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">d e</span></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">fg</span></p>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > only br in a li (#35)
#[test]
fn w3_spacing_only_br_in_a_li_35() {
    check(
        r#"<ol><li>1</li><li><br /></li><li>3</li></ol>"#,
        r#"<ol class="editor-list-ol" dir="auto"><li value="1" class="editor-listitem"><span data-lexical-text="true">1</span></li><li value="2" class="editor-listitem"><br data-lexical-managed-linebreak="true"></li><li value="3" class="editor-listitem"><span data-lexical-text="true">3</span></li></ol>"#,
    );
}

// upstream: lexical-utils/src/__tests__/unit/LexicalEventHelpers.test.tsx::LexicalEventHelpers > onPasteForRichText > W3 spacing > last br in a block node is ignored (#36)
#[test]
fn w3_spacing_last_br_in_a_block_node_is_ignored_36() {
    check(
        r#"1<p>2<br /></p>3"#,
        r#"<p class="editor-paragraph" dir="auto"><span data-lexical-text="true">1</span></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">2</span></p><p class="editor-paragraph" dir="auto"><span data-lexical-text="true">3</span></p>"#,
    );
}
