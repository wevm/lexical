//! Ports of `packages/lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts`.
//!
//! Upstream compares the mounted editor's `innerHTML`; as in the other
//! ports, the committed tree it is reconciled from is compared instead
//! ([`describe`]), each expectation being the tree that renders the upstream
//! HTML quoted above it.

#![cfg(all(
    feature = "html",
    feature = "clipboard",
    feature = "list",
    feature = "code"
))]

use super::super::support::{initialize_unit_test, range_selection, update};
use crate::clipboard::insert_data_transfer_for_rich_text;
use crate::code::{CodeHighlightNode, CodeNode};
use crate::list::ListType;
use crate::{
    CreateEditorArgs, DataTransfer, Editor, LexicalError, NodeClass, NodeRef,
    create_paragraph_node, get_root,
};

/// The committed tree: `<type attrs>...</type>` for an element, with its
/// list type (`list:bullet`), check state (`checked`/`unchecked`), alignment
/// (`@right`) and direction (`dir=ltr`) when it has them; `[text]` for a text
/// node, with `|format` and `{style}` when set; `<br>` for a line break.
fn describe(node: &NodeRef) -> String {
    if node.is_element() {
        let mut tag = node.get_type().to_owned();
        if let Some(list_type) = node.get_list_type() {
            tag.push_str(match list_type {
                ListType::Bullet => ":bullet",
                ListType::Number => ":number",
                ListType::Check => ":check",
            });
        }
        if let Some(checked) = node.get_checked() {
            tag.push_str(if checked { " checked" } else { " unchecked" });
        }
        let data = node.element_data();
        if !data.get_format_type().is_empty() {
            tag.push_str(&format!(" @{}", data.get_format_type()));
        }
        if let Some(dir) = data.get_direction() {
            tag.push_str(&format!(" dir={dir:?}"));
        }
        let children: String = node.get_children().iter().map(describe).collect();
        format!("<{tag}>{children}</{}>", node.get_type())
    } else if node.is_text() {
        let data = node.text_data();
        let mut out = format!("[{}", node.get_text_content());
        if data.get_format() != 0.0 {
            out.push_str(&format!("|{}", data.get_format()));
        }
        if !data.get_style().is_empty() {
            out.push_str(&format!("{{{}}}", data.get_style()));
        }
        out.push(']');
        out
    } else if node.get_type() == "linebreak" {
        "<br>".to_owned()
    } else {
        format!("<{}/>", node.get_type())
    }
}

/// `initializeUnitTest` with `{namespace: 'test', theme: ...}` and the
/// `beforeEach`; the editor, its paragraph selected.
fn set_up() -> Editor {
    let editor = initialize_unit_test(CreateEditorArgs {
        namespace: "test".to_owned(),
        nodes: vec![
            NodeClass::of::<CodeNode>(),
            NodeClass::of::<CodeHighlightNode>(),
        ],
        ..CreateEditorArgs::default()
    });
    update(&editor, || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.select(None, None)?;
        Ok(())
    });
    editor
}

fn tree(editor: &Editor) -> String {
    editor
        .read(|| get_root().get_children().iter().map(describe).collect())
        .unwrap()
}

fn paste_into(
    editor: &Editor,
    data_transfer: DataTransfer,
    before: impl FnOnce() -> Result<(), LexicalError> + 'static,
    plain_text_insert: Option<&'static str>,
) {
    let e = editor.clone();
    update(editor, move || {
        before()?;
        insert_data_transfer_for_rich_text(&data_transfer, &range_selection(), &e)?;
        if let Some(text) = plain_text_insert {
            range_selection().insert_text(text)?;
        }
        Ok(())
    });
}

/// One case of `HTML_COPY_PASTING_TESTS`.
fn paste(html: &'static str, plain_text_insert: Option<&'static str>) -> String {
    let editor = set_up();
    let data_transfer = DataTransfer::new();
    data_transfer.set_data("text/html", html);
    paste_into(&editor, data_transfer, || Ok(()), plain_text_insert);
    tree(&editor)
}

// <p dir="auto"><span data-lexical-text="true">Hello!</span></p>
// upstream: lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: plain DOM text node
#[test]
fn html_copy_paste_plain_dom_text_node() {
    assert_eq!(paste(r#"Hello!"#, None), "<paragraph>[Hello!]</paragraph>");
}

// <p dir="auto"><span data-lexical-text="true">Hello!</span></p><p dir="auto"><br data-lexical-managed-linebreak="true"></p>
// upstream: lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: a paragraph element
#[test]
fn html_copy_paste_a_paragraph_element() {
    assert_eq!(
        paste(r#"<p>Hello!<p>"#, None),
        "<paragraph>[Hello!]</paragraph><paragraph></paragraph>"
    );
}

// <p dir="auto"><span data-lexical-text="true">123</span></p><p dir="auto"><span data-lexical-text="true">456</span></p>
// upstream: lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: a single div
#[test]
fn html_copy_paste_a_single_div() {
    assert_eq!(
        paste(
            r#"123
            <div>
              456
            </div>"#,
            None
        ),
        "<paragraph>[123]</paragraph><paragraph>[456]</paragraph>"
    );
}

// <p dir="auto"><span data-lexical-text="true">a b c d e</span></p><p dir="auto"><span data-lexical-text="true">f g h</span></p>
// upstream: lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: multiple nested spans and divs
#[test]
fn html_copy_paste_multiple_nested_spans_and_divs() {
    assert_eq!(
        paste(
            r#"<div>
            a b
            <span>
              c d
              <span>e</span>
            </span>
            <div>
              f
              <span>g h</span>
            </div>
          </div>"#,
            None
        ),
        "<paragraph>[a b c d e]</paragraph><paragraph>[f g h]</paragraph>"
    );
}

// <p dir="auto"><span data-lexical-text="true">123</span></p><p dir="auto"><span data-lexical-text="true">456</span></p>
// upstream: lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: nested span in a div
#[test]
fn html_copy_paste_nested_span_in_a_div() {
    assert_eq!(
        paste(
            r#"<div>
            <span>
              123
              <div>456</div>
            </span>
          </div>"#,
            None
        ),
        "<paragraph>[123]</paragraph><paragraph>[456]</paragraph>"
    );
}

// <p dir="auto"><span data-lexical-text="true">123</span></p><p dir="auto"><span data-lexical-text="true">456</span></p>
// upstream: lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: nested div in a span
#[test]
fn html_copy_paste_nested_div_in_a_span() {
    assert_eq!(
        paste(r#" <span>123<div>456</div></span>"#, None),
        "<paragraph>[123]</paragraph><paragraph>[456]</paragraph>"
    );
}

// <ul dir="auto"><li value="1" dir="ltr" style="text-align: right;"><span data-lexical-text="true">Hello</span></li><li value="2" dir="ltr" style="text-align: right;"><span data-lexical-text="true">World</span></li></ul>
// upstream: lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: google doc list with right alignment
#[test]
fn html_copy_paste_google_doc_list_with_right_alignment() {
    assert_eq!(
        paste(
            r#"<meta charset='utf-8'><meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-61c25daf-7fff-957d-d4b5-8e006d20766d"><ul style="margin-top:0;margin-bottom:0;padding-inline-start:48px;"><li dir="ltr" style="list-style-type:disc;font-size:11pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;" aria-level="1"><p dir="ltr" style="line-height:1.38;text-align: right;margin-top:0pt;margin-bottom:0pt;" role="presentation"><span style="font-size:11pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">Hello</span></p></li><li dir="ltr" style="list-style-type:disc;font-size:11pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;" aria-level="1"><p dir="ltr" style="line-height:1.38;text-align: right;margin-top:0pt;margin-bottom:0pt;" role="presentation"><span style="font-size:11pt;font-family:Arial,sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">World</span></p></li></ul></b>"#,
            None
        ),
        "<list:bullet><listitem @right dir=Ltr>[Hello]</listitem><listitem @right dir=Ltr>[World]</listitem></list>"
    );
}

// <ul dir="auto"><li role="checkbox" tabindex="-1" aria-checked="true" value="1" dir="ltr"><span data-lexical-text="true">done</span></li><li role="checkbox" tabindex="-1" aria-checked="false" value="2" dir="ltr"><span data-lexical-text="true">todo</span></li><li value="3"><ul><li role="checkbox" tabindex="-1" aria-checked="true" value="1" dir="ltr"><span data-lexical-text="true">done</span></li><li role="checkbox" tabindex="-1" aria-checked="false" value="2" dir="ltr"><span data-lexical-text="true">todo</span></li></ul></li><li role="checkbox" tabindex="-1" aria-checked="false" value="3" dir="ltr"><span data-lexical-text="true">todo</span></li></ul>
// upstream: lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: google doc checklist
#[test]
fn html_copy_paste_google_doc_checklist() {
    assert_eq!(
        paste(
            r#"<meta charset='utf-8'><meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-1980f960-7fff-f4df-4ba3-26c6e1508542"><ul style="margin-top:0;margin-bottom:0;padding-inline-start:28px;"><li dir="ltr" role="checkbox" aria-checked="true" style="list-style-type:none;font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:line-through;-webkit-text-decoration-skip:none;text-decoration-skip-ink:none;vertical-align:baseline;white-space:pre;" aria-level="1"><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEgAAABICAYAAABV7bNHAAABbElEQVR4Ae3bsU4CYRDEcRsxodZE8Q0BbS258l5MwESJNL6HOfrPKdhyxeBcwk5mkn9F98sGIOSuPM/zPM/zPI+xG/SEtuiAWpEOaIOWaDIWziP6RK14OzSjX44ITvTBvqRn1MRaMIHeBIE2TKBBEGhgArWkKmtJBjKQgQxkIANd/Aw0NVC+O7RHvYFynHasN1COE/UGynGiXgOIjxOtdIH4OGJAfBwxID6OGBAfRwiIjyMARMCpCjRF5+72Dzhd5R+rHfpC92NeTlWgLl5PkQg4RYBynBSJgFMGKMNJkQg4lYFeUDuFRMCpBXQOEgGnDtA/kPg4xT7m2y/tCd9zKgOdviTC5RQEIiAFjh4QASlw9IAISIEjCURAWvmf1UDKcQwUSDmOgWLdMcxA7BnIQAYykIEM5EcRvplAW0GgNRNoKQg0ZwJN0E4I5x1dI+pmgSSA84BG2QQt0LrYG/eAXtGccjme53me53me9wPjPWZWjhktAQAAAABJRU5ErkJggg==" width="18.4px" height="18.4px" alt="checked" aria-roledescription="checkbox" style="margin-right:3px;" /><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;display:inline-block;vertical-align:top;margin-top:0;" role="presentation"><span style="font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:line-through;-webkit-text-decoration-skip:none;text-decoration-skip-ink:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">done</span></p></li><li dir="ltr" role="checkbox" aria-checked="false" style="list-style-type:none;font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;" aria-level="1"><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEgAAABICAYAAABV7bNHAAAA1ElEQVR4Ae3bMQ4BURSFYY2xBuwQ7BIkTGxFRj9Oo9RdkXn5TvL3L19u+2ZmZmZmZhVbpH26pFcaJ9IrndMudb/CWadHGiden1bll9MIzqd79SUd0thY20qga4NA50qgoUGgoRJo/NL/V/N+QIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIEyFeEZyXQpUGgUyXQrkGgTSVQl/qGcG5pnkq3Sn0jOMv0k3Vpm05pmNjfsGPalFyOmZmZmdkbSS9cKbtzhxMAAAAASUVORK5CYII=" width="18.4px" height="18.4px" alt="unchecked" aria-roledescription="checkbox" style="margin-right:3px;" /><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;display:inline-block;vertical-align:top;margin-top:0;" role="presentation"><span style="font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">todo</span></p></li><ul style="margin-top:0;margin-bottom:0;padding-inline-start:28px;"><li dir="ltr" role="checkbox" aria-checked="true" style="list-style-type:none;font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:line-through;-webkit-text-decoration-skip:none;text-decoration-skip-ink:none;vertical-align:baseline;white-space:pre;" aria-level="2"><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEgAAABICAYAAABV7bNHAAABbElEQVR4Ae3bsU4CYRDEcRsxodZE8Q0BbS258l5MwESJNL6HOfrPKdhyxeBcwk5mkn9F98sGIOSuPM/zPM/zPI+xG/SEtuiAWpEOaIOWaDIWziP6RK14OzSjX44ITvTBvqRn1MRaMIHeBIE2TKBBEGhgArWkKmtJBjKQgQxkIANd/Aw0NVC+O7RHvYFynHasN1COE/UGynGiXgOIjxOtdIH4OGJAfBwxID6OGBAfRwiIjyMARMCpCjRF5+72Dzhd5R+rHfpC92NeTlWgLl5PkQg4RYBynBSJgFMGKMNJkQg4lYFeUDuFRMCpBXQOEgGnDtA/kPg4xT7m2y/tCd9zKgOdviTC5RQEIiAFjh4QASlw9IAISIEjCURAWvmf1UDKcQwUSDmOgWLdMcxA7BnIQAYykIEM5EcRvplAW0GgNRNoKQg0ZwJN0E4I5x1dI+pmgSSA84BG2QQt0LrYG/eAXtGccjme53me53me9wPjPWZWjhktAQAAAABJRU5ErkJggg==" width="18.4px" height="18.4px" alt="checked" aria-roledescription="checkbox" style="margin-right:3px;" /><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;display:inline-block;vertical-align:top;margin-top:0;" role="presentation"><span style="font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:line-through;-webkit-text-decoration-skip:none;text-decoration-skip-ink:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">done</span></p></li><li dir="ltr" role="checkbox" aria-checked="false" style="list-style-type:none;font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;" aria-level="2"><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEgAAABICAYAAABV7bNHAAAA1ElEQVR4Ae3bMQ4BURSFYY2xBuwQ7BIkTGxFRj9Oo9RdkXn5TvL3L19u+2ZmZmZmZhVbpH26pFcaJ9IrndMudb/CWadHGiden1bll9MIzqd79SUd0thY20qga4NA50qgoUGgoRJo/NL/V/N+QIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIEyFeEZyXQpUGgUyXQrkGgTSVQl/qGcG5pnkq3Sn0jOMv0k3Vpm05pmNjfsGPalFyOmZmZmdkbSS9cKbtzhxMAAAAASUVORK5CYII=" width="18.4px" height="18.4px" alt="unchecked" aria-roledescription="checkbox" style="margin-right:3px;" /><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;display:inline-block;vertical-align:top;margin-top:0;" role="presentation"><span style="font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">todo</span></p></li></ul><li dir="ltr" role="checkbox" aria-checked="false" style="list-style-type:none;font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;" aria-level="1"><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEgAAABICAYAAABV7bNHAAAA1ElEQVR4Ae3bMQ4BURSFYY2xBuwQ7BIkTGxFRj9Oo9RdkXn5TvL3L19u+2ZmZmZmZhVbpH26pFcaJ9IrndMudb/CWadHGiden1bll9MIzqd79SUd0thY20qga4NA50qgoUGgoRJo/NL/V/N+QIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIECBAgQIAAAQIEyFeEZyXQpUGgUyXQrkGgTSVQl/qGcG5pnkq3Sn0jOMv0k3Vpm05pmNjfsGPalFyOmZmZmdkbSS9cKbtzhxMAAAAASUVORK5CYII=" width="18.4px" height="18.4px" alt="unchecked" aria-roledescription="checkbox" style="margin-right:3px;" /><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:0pt;display:inline-block;vertical-align:top;margin-top:0;" role="presentation"><span style="font-size:11.5pt;font-family:'Optimistic Text',sans-serif;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">todo</span></p></li></ul></b>"#,
            None
        ),
        "<list:check><listitem checked dir=Ltr>[done|4]</listitem><listitem unchecked dir=Ltr>[todo]</listitem><listitem unchecked><list:check><listitem checked dir=Ltr>[done|4]</listitem><listitem unchecked dir=Ltr>[todo]</listitem></list></listitem><listitem unchecked dir=Ltr>[todo]</listitem></list>"
    );
}

// <p dir="auto" style="text-align: start;"><span data-lexical-text="true">checklist</span></p><ul dir="auto"><li role="checkbox" tabindex="-1" aria-checked="true" value="1" style="text-align: start;"><span data-lexical-text="true">done</span></li><li role="checkbox" tabindex="-1" aria-checked="false" value="2" style="text-align: start;"><span data-lexical-text="true">todo</span></li></ul>
// upstream: lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: github checklist
#[test]
fn html_copy_paste_github_checklist() {
    assert_eq!(
        paste(
            r#"<meta charset='utf-8'><p dir="auto" style="box-sizing: border-box; margin-top: 0px !important; margin-bottom: 16px; color: rgb(31, 35, 40); font-family: -apple-system, &quot;system-ui&quot;, &quot;Segoe UI&quot;, &quot;Noto Sans&quot;, Helvetica, Arial, sans-serif, &quot;Apple Color Emoji&quot;, &quot;Segoe UI Emoji&quot;; font-size: 14px; font-style: normal; font-variant-ligatures: normal; font-variant-caps: normal; font-weight: 400; letter-spacing: normal; orphans: 2; text-align: start; text-indent: 0px; text-transform: none; widows: 2; word-spacing: 0px; -webkit-text-stroke-width: 0px; white-space: normal; background-color: rgb(255, 255, 255); text-decoration-thickness: initial; text-decoration-style: initial; text-decoration-color: initial;">checklist</p><ul class="contains-task-list" style="box-sizing: border-box; padding: 0px; margin-top: 0px; margin-bottom: 0px !important; position: relative; color: rgb(31, 35, 40); font-family: -apple-system, &quot;system-ui&quot;, &quot;Segoe UI&quot;, &quot;Noto Sans&quot;, Helvetica, Arial, sans-serif, &quot;Apple Color Emoji&quot;, &quot;Segoe UI Emoji&quot;; font-size: 14px; font-style: normal; font-variant-ligatures: normal; font-variant-caps: normal; font-weight: 400; letter-spacing: normal; orphans: 2; text-align: start; text-indent: 0px; text-transform: none; widows: 2; word-spacing: 0px; -webkit-text-stroke-width: 0px; white-space: normal; background-color: rgb(255, 255, 255); text-decoration-thickness: initial; text-decoration-style: initial; text-decoration-color: initial;"><li class="task-list-item enabled" style="box-sizing: border-box; list-style-type: none; padding: 2px 15px 2px 42px; margin-right: -15px; margin-left: -15px; line-height: 1.5; border: 0px;"><span class="handle" style="box-sizing: border-box; display: block; float: left; width: 20px; padding: 2px 0px 0px 2px; margin-left: -43px; opacity: 0;"><svg class="drag-handle" aria-hidden="true" width="16" height="16"><path d="M10 13a1 1 0 100-2 1 1 0 000 2zm-4 0a1 1 0 100-2 1 1 0 000 2zm1-5a1 1 0 11-2 0 1 1 0 012 0zm3 1a1 1 0 100-2 1 1 0 000 2zm1-5a1 1 0 11-2 0 1 1 0 012 0zM6 5a1 1 0 100-2 1 1 0 000 2z"></path></svg></span><input type="checkbox" id="" class="task-list-item-checkbox" checked="" style="box-sizing: border-box; font: inherit; margin: 0px 0.2em 0.25em -1.4em; overflow: visible; padding: 0px; vertical-align: middle;"><span></span>done</li><li class="task-list-item enabled" style="box-sizing: border-box; list-style-type: none; margin-top: 0px; padding: 2px 15px 2px 42px; margin-right: -15px; margin-left: -15px; line-height: 1.5; border: 0px;"><span class="handle" style="box-sizing: border-box; display: block; float: left; width: 20px; padding: 2px 0px 0px 2px; margin-left: -43px; opacity: 0;"><svg class="drag-handle" aria-hidden="true" width="16" height="16"><path d="M10 13a1 1 0 100-2 1 1 0 000 2zm-4 0a1 1 0 100-2 1 1 0 000 2zm1-5a1 1 0 11-2 0 1 1 0 012 0zm3 1a1 1 0 100-2 1 1 0 000 2zm1-5a1 1 0 11-2 0 1 1 0 012 0zM6 5a1 1 0 100-2 1 1 0 000 2z"></path></svg></span><input type="checkbox" id="" class="task-list-item-checkbox" style="box-sizing: border-box; font: inherit; margin: 0px 0.2em 0.25em -1.4em; overflow: visible; padding: 0px; vertical-align: middle;"><span></span>todo</li></ul>"#,
            None
        ),
        "<paragraph @start>[checklist]</paragraph><list:check><listitem checked @start>[done]</listitem><listitem unchecked @start>[todo]</listitem></list>"
    );
}

// <ul dir="auto"><li role="checkbox" tabindex="-1" aria-checked="true" value="1"><span data-lexical-text="true">Milk</span></li><li role="checkbox" tabindex="-1" aria-checked="false" value="2"><span data-lexical-text="true">Eggs</span></li><li role="checkbox" tabindex="-1" aria-checked="true" value="3"><span data-lexical-text="true">Beer</span></li></ul>
// upstream: lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: joplin checklist
#[test]
fn html_copy_paste_joplin_checklist() {
    assert_eq!(
        paste(
            r#"<ul data-is-checklist="1"><li class="md-checkbox joplin-checkbox"><div class="checkbox-wrapper"><input type="checkbox" id="md-checkbox-0" checked="checked"><label id="cb-label-md-checkbox-0" for="md-checkbox-0" class="checkbox-label-checked">Milk</label></div></li><li class="md-checkbox joplin-checkbox"><div class="checkbox-wrapper"><input type="checkbox" id="md-checkbox-1"><label id="cb-label-md-checkbox-1" for="md-checkbox-1" class="checkbox-label-unchecked">Eggs</label></div></li><li class="md-checkbox joplin-checkbox"><div class="checkbox-wrapper"><input type="checkbox" id="md-checkbox-2" checked="checked"><label id="cb-label-md-checkbox-2" for="md-checkbox-2" class="checkbox-label-checked">Beer</label></div></li></ul>"#,
            None
        ),
        "<list:check><listitem checked>[Milk]</listitem><listitem unchecked>[Eggs]</listitem><listitem checked>[Beer]</listitem></list>"
    );
}

// <p dir="auto"><strong class="editor-text-bold" data-lexical-text="true">hello world</strong></p>
// upstream: lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: pasting inheritance
#[test]
fn html_copy_paste_pasting_inheritance() {
    assert_eq!(
        paste(r#"<strong>hello</strong>"#, Some(" world")),
        "<paragraph>[hello world|1]</paragraph>"
    );
}

// <p dir="auto"><span data-lexical-text="true">hello world</span></p>
// upstream: lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: div with display:inline should not insert linebreaks
#[test]
fn html_copy_paste_div_with_display_inline_should_not_insert_linebreaks() {
    assert_eq!(
        paste(
            r#"<div><div style="display: inline;">hello</div> <div style="display: inline;">world</div></div>"#,
            None
        ),
        "<paragraph>[hello world]</paragraph>"
    );
}

// <code spellcheck="false" dir="auto"><span data-lexical-text="true">❯ cd ~/src/lexical</span></code>
// upstream: lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > HTML copy paste: Ghostty terminal paste with inline divs
#[test]
fn html_copy_paste_ghostty_terminal_paste_with_inline_divs() {
    assert_eq!(
        paste(
            r#"<div style="font-family: monospace; white-space: pre;"><div style="display: inline;color: rgb(181, 189, 104);font-weight: bold;">&#10095;</div> cd ~/src/lexical</div>"#,
            None
        ),
        "<code>[❯ cd ~/src/lexical]</code>"
    );
}

// <p dir="auto"><span style="background-color: rgb(255, 170, 45);" data-lexical-text="true">Prediction</span></p>
// upstream: lexical/src/__tests__/unit/HTMLCopyAndPaste.test.ts::HTMLCopyAndPaste tests > iOS fix: Word predictions should be handled as plain text to maintain selection formatting
#[cfg(feature = "selection")]
#[test]
fn ios_fix_word_predictions_should_be_handled_as_plain_text() {
    let editor = set_up();
    let data_transfer = DataTransfer::new();
    data_transfer.set_data("text/plain", "Prediction");
    data_transfer.set_data("text/html", "Prediction");
    paste_into(
        &editor,
        data_transfer,
        || {
            crate::selection::patch_style_text(
                &range_selection(),
                &[("background-color", "rgb(255,170,45)".into())],
            )
        },
        None,
    );
    // The model's style is the patch as written; the DOM's CSSOM serializes it
    // with spaces.
    assert_eq!(
        tree(&editor),
        "<paragraph>[Prediction{background-color: rgb(255,170,45);}]</paragraph>"
    );
}
