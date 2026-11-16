//! Ports of `packages/lexical/src/__tests__/unit/CodeBlock.test.ts`.
//!
//! Upstream compares the mounted editor's `innerHTML`; as in the other
//! ports, the committed tree it is reconciled from is compared instead
//! ([`describe`]: `<type>...</type>` for an element, `[text]` or
//! `[text|format]` for a text node (with its format flags), a heading by
//! its tag, `<br>` for a
//! line break). Each expectation is the tree that renders the upstream HTML,
//! which is quoted above it.

#![cfg(all(feature = "html", feature = "clipboard", feature = "code"))]

use super::super::support::{initialize_unit_test, range_selection, update};
use crate::clipboard::insert_data_transfer_for_rich_text;
use crate::code::{CodeHighlightNode, CodeNode};
use crate::{CreateEditorArgs, DataTransfer, NodeClass, NodeRef, create_paragraph_node, get_root};

fn describe(node: &NodeRef) -> String {
    if node.is_element() {
        let children: String = node.get_children().iter().map(describe).collect();
        // A heading by its tag, as the HTML names it.
        #[cfg(feature = "rich-text")]
        if let Some(tag) = node.get_heading_tag() {
            return format!("<{t}>{children}</{t}>", t = tag.as_str());
        }
        format!("<{t}>{children}</{t}>", t = node.get_type())
    } else if node.is_text() {
        let format = node.text_data().get_format();
        if format == 0.0 {
            format!("[{}]", node.get_text_content())
        } else {
            format!("[{}|{format}]", node.get_text_content())
        }
    } else if node.get_type() == "linebreak" {
        "<br>".to_owned()
    } else {
        format!("<{}/>", node.get_type())
    }
}

/// `initializeUnitTest` with `{namespace: 'test', theme: {text: {...}}}` and
/// the `beforeEach`, then the paste of `html`; the root's children.
fn paste(html: &'static str) -> String {
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
    let e = editor.clone();
    update(&editor, move || {
        let data_transfer = DataTransfer::new();
        data_transfer.set_data("text/html", html);
        insert_data_transfer_for_rich_text(&data_transfer, &range_selection(), &e)
    });
    editor
        .read(|| get_root().get_children().iter().map(describe).collect())
        .unwrap()
}

// <code spellcheck="false" dir="auto"><span data-lexical-text="true">function run() {</span><br><span data-lexical-text="true">  return [null, undefined, 2, ""];</span><br><span data-lexical-text="true">}</span></code>
// upstream: lexical/src/__tests__/unit/CodeBlock.test.ts::CodeBlock tests > Code block html paste: VS Code
#[test]
fn code_block_html_paste_vs_code() {
    assert_eq!(
        paste(
            r#"<meta charset='utf-8'><div style="color: #d4d4d4;background-color: #1e1e1e;font-family: Menlo, Monaco, 'Courier New', monospace;font-weight: normal;font-size: 12px;line-height: 18px;white-space: pre;"><div><span style="color: #569cd6;">function</span><span style="color: #d4d4d4;"> </span><span style="color: #dcdcaa;">run</span><span style="color: #d4d4d4;">() {</span></div><div><span style="color: #d4d4d4;">  </span><span style="color: #c586c0;">return</span><span style="color: #d4d4d4;"> [</span><span style="color: #569cd6;">null</span><span style="color: #d4d4d4;">, </span><span style="color: #569cd6;">undefined</span><span style="color: #d4d4d4;">, </span><span style="color: #b5cea8;">2</span><span style="color: #d4d4d4;">, </span><span style="color: #ce9178;">""</span><span style="color: #d4d4d4;">];</span></div><div><span style="color: #d4d4d4;">}</span></div></div>"#
        ),
        "<code>[function run() {]<br>[  return [null, undefined, 2, \"\"];]<br>[}]</code>"
    );
}

// <code spellcheck="false" dir="auto"><span data-lexical-text="true">function run() {</span><br><span data-lexical-text="true">  return [null, undefined, 2, ""];</span><br><span data-lexical-text="true">}</span></code>
// upstream: lexical/src/__tests__/unit/CodeBlock.test.ts::CodeBlock tests > Code block html paste: Quip
#[test]
fn code_block_html_paste_quip() {
    assert_eq!(
        paste(
            r#"<meta charset='utf-8'><pre>function run() {<br>  return [null, undefined, 2, ""];<br>}</pre>"#
        ),
        "<code>[function run() {]<br>[  return [null, undefined, 2, \"\"];]<br>[}]</code>"
    );
}

// <code spellcheck="false" dir="auto"><span data-lexical-text="true">function run() {</span><br><span data-lexical-text="true">  return [null, undefined, 2, ""];</span><br><span data-lexical-text="true">}</span></code>
// upstream: lexical/src/__tests__/unit/CodeBlock.test.ts::CodeBlock tests > Code block html paste: WebStorm / Idea
#[test]
fn code_block_html_paste_webstorm_idea() {
    assert_eq!(
        paste(
            r#"<html><head><meta http-equiv="content-type" content="text/html; charset=UTF-8"></head><body><pre style="background-color:#2b2b2b;color:#a9b7c6;font-family:'JetBrains Mono',monospace;font-size:9.8pt;"><span style="color:#cc7832;">function&#32;</span><span style="color:#ffc66d;">run</span>()&#32;{<br>&#32;&#32;<span style="color:#cc7832;">return&#32;</span>[<span style="color:#cc7832;">null,&#32;undefined,&#32;</span><span style="color:#6897bb;">2</span><span style="color:#cc7832;">,&#32;</span><span style="color:#6a8759;">""</span>]<span style="color:#cc7832;">;<br></span>}</pre></body></html>"#
        ),
        "<code>[function run() {]<br>[  return [null, undefined, 2, \"\"];]<br>[}]</code>"
    );
}

// <code spellcheck="false" dir="auto"><strong class="editor-text-bold" data-lexical-text="true">function</strong><span data-lexical-text="true"> run() {</span><br><span data-lexical-text="true">  </span><strong class="editor-text-bold" data-lexical-text="true">return</strong><span data-lexical-text="true"> [</span><strong class="editor-text-bold" data-lexical-text="true">null</strong><span data-lexical-text="true">, </span><strong class="editor-text-bold" data-lexical-text="true">undefined</strong><span data-lexical-text="true">, 2, ""];</span><br><span data-lexical-text="true">}</span></code>
// upstream: lexical/src/__tests__/unit/CodeBlock.test.ts::CodeBlock tests > Code block html paste: Postman IDE
#[test]
fn code_block_html_paste_postman_ide() {
    assert_eq!(
        paste(
            r#"<meta charset='utf-8'><div style="color: #000000;background-color: #fffffe;font-family: Menlo, Monaco, 'Courier New', monospace;font-weight: normal;font-size: 12px;line-height: 18px;white-space: pre;"><div><span style="color: #800555;font-weight: bold;">function</span><span style="color: #000000;"> run() {</span></div><div><span style="color: #000000;">  </span><span style="color: #800555;font-weight: bold;">return</span><span style="color: #000000;"> [</span><span style="color: #800555;font-weight: bold;">null</span><span style="color: #000000;">, </span><span style="color: #800555;font-weight: bold;">undefined</span><span style="color: #000000;">, </span><span style="color: #ff00aa;">2</span><span style="color: #000000;">, </span><span style="color: #2a00ff;">""</span><span style="color: #000000;">];</span></div><div><span style="color: #000000;">}</span></div></div>"#
        ),
        "<code>[function|1][ run() {]<br>[  ][return|1][ [][null|1][, ][undefined|1][, 2, \"\"];]<br>[}]</code>"
    );
}

// <code spellcheck="false" dir="auto"><span data-lexical-text="true">function run() {</span><br><span data-lexical-text="true">  return [null, undefined, 2, ""];</span><br><span data-lexical-text="true">}</span></code>
// upstream: lexical/src/__tests__/unit/CodeBlock.test.ts::CodeBlock tests > Code block html paste: Slack message
#[test]
fn code_block_html_paste_slack_message() {
    assert_eq!(
        paste(
            r#"<meta charset='utf-8'><pre class="c-mrkdwn__pre" data-stringify-type="pre" style="box-sizing: inherit; margin: 4px 0px; padding: 8px; --saf-0:rgba(var(--sk_foreground_low,29,28,29),0.13); font-size: 12px; line-height: 1.50001; font-variant-ligatures: none; white-space: pre-wrap; word-break: break-word; word-break: normal; tab-size: 4; font-family: Monaco, Menlo, Consolas, &quot;Courier New&quot;, monospace !important; border: 1px solid var(--saf-0); border-radius: 4px; background: rgba(var(--sk_foreground_min,29,28,29),0.04); counter-reset: list-0 0 list-1 0 list-2 0 list-3 0 list-4 0 list-5 0 list-6 0 list-7 0 list-8 0 list-9 0; color: rgb(29, 28, 29); font-style: normal; font-variant-caps: normal; font-weight: 400; letter-spacing: normal; orphans: 2; text-align: left; text-indent: 0px; text-transform: none; widows: 2; word-spacing: 0px; -webkit-text-stroke-width: 0px; text-decoration-thickness: initial; text-decoration-style: initial; text-decoration-color: initial;">function run() {
  return [null, undefined, 2, ""];
}</pre>"#
        ),
        "<code>[function run() {]<br>[  return [null, undefined, 2, \"\"];]<br>[}]</code>"
    );
}

// <code spellcheck="false" dir="auto"><span data-lexical-text="true">const Lexical = requireCond('gk', 'runtime_is_dev', {</span><br><span data-lexical-text="true">  true: 'Lexical.dev',</span><br><span data-lexical-text="true">  false: 'Lexical.prod',</span><br><span data-lexical-text="true">});</span></code>
// upstream: lexical/src/__tests__/unit/CodeBlock.test.ts::CodeBlock tests > Code block html paste: CodeHub
#[test]
fn code_block_html_paste_codehub() {
    assert_eq!(
        paste(
            r#"<meta charset='utf-8'><div style="color: #000000;background-color: #fffffe;font-family: 'monaco,monospace', Menlo, Monaco, 'Courier New', monospace;font-weight: normal;font-size: 13px;line-height: 20px;white-space: pre;"><div><span style="color: #ff0000;">const</span><span style="color: #000000;"> </span><span style="color: #800000;">Lexical</span><span style="color: #000000;"> = </span><span style="color: #383838;">requireCond</span><span style="color: #000000;">(</span><span style="color: #863b00;">'gk'</span><span style="color: #000000;">, </span><span style="color: #863b00;">'runtime_is_dev'</span><span style="color: #000000;">, {</span></div><div><span style="color: #000000;">  </span><span style="color: #863b00;">true</span><span style="color: #000000;">: </span><span style="color: #863b00;">'Lexical.dev'</span><span style="color: #000000;">,</span></div><div><span style="color: #000000;">  </span><span style="color: #863b00;">false</span><span style="color: #000000;">: </span><span style="color: #863b00;">'Lexical.prod'</span><span style="color: #000000;">,</span></div><div><span style="color: #000000;">});</span></div></div>"#
        ),
        "<code>[const Lexical = requireCond('gk', 'runtime_is_dev', {]<br>[  true: 'Lexical.dev',]<br>[  false: 'Lexical.prod',]<br>[});]</code>"
    );
}

// <code spellcheck="false" dir="auto"><span data-lexical-text="true">function run() {</span><br><span data-lexical-text="true">  return [null, undefined, 2, ""];</span><br><span data-lexical-text="true">}</span></code>
// upstream: lexical/src/__tests__/unit/CodeBlock.test.ts::CodeBlock tests > Code block html paste: GitHub / Gist
#[test]
fn code_block_html_paste_github_gist() {
    assert_eq!(
        paste(
            r#"<meta charset='utf-8'><table class="highlight tab-size js-file-line-container js-code-nav-container js-tagsearch-file" data-tab-size="8" data-paste-markdown-skip="" data-tagsearch-lang="JavaScript" data-tagsearch-path="example.js" style="box-sizing: border-box; border-spacing: 0px; border-collapse: collapse; tab-size: 8; color: rgb(36, 41, 47); font-family: -apple-system, &quot;system-ui&quot;, &quot;Segoe UI&quot;, Helvetica, Arial, sans-serif, &quot;Apple Color Emoji&quot;, &quot;Segoe UI Emoji&quot;; font-size: 14px; font-style: normal; font-variant-ligatures: normal; font-variant-caps: normal; font-weight: 400; letter-spacing: normal; orphans: 2; text-align: start; text-transform: none; white-space: normal; widows: 2; word-spacing: 0px; -webkit-text-stroke-width: 0px; background-color: rgb(255, 255, 255); text-decoration-thickness: initial; text-decoration-style: initial; text-decoration-color: initial;"><tbody style="box-sizing: border-box;"><tr style="box-sizing: border-box;"><td id="file-example-js-LC1" class="blob-code blob-code-inner js-file-line" style="box-sizing: border-box; padding: 0px 10px; position: relative; line-height: 20px; vertical-align: top; overflow: visible; font-family: ui-monospace, SFMono-Regular, &quot;SF Mono&quot;, Menlo, Consolas, &quot;Liberation Mono&quot;, monospace; font-size: 12px; color: var(--color-fg-default); overflow-wrap: normal; white-space: pre;"><span class="pl-k" style="box-sizing: border-box; color: var(--color-prettylights-syntax-keyword);">function</span> <span class="pl-en" style="box-sizing: border-box; color: var(--color-prettylights-syntax-entity);">run</span><span class="pl-kos" style="box-sizing: border-box;">(</span><span class="pl-kos" style="box-sizing: border-box;">)</span> <span class="pl-kos" style="box-sizing: border-box;">{</span></td></tr><tr style="box-sizing: border-box; background-color: transparent;"><td id="file-example-js-L2" class="blob-num js-line-number js-code-nav-line-number" data-line-number="2" style="box-sizing: border-box; padding: 0px 10px; width: 50px; min-width: 50px; font-family: ui-monospace, SFMono-Regular, &quot;SF Mono&quot;, Menlo, Consolas, &quot;Liberation Mono&quot;, monospace; font-size: 12px; line-height: 20px; color: var(--color-fg-subtle); text-align: right; white-space: nowrap; vertical-align: top; cursor: pointer; user-select: none;"></td><td id="file-example-js-LC2" class="blob-code blob-code-inner js-file-line" style="box-sizing: border-box; padding: 0px 10px; position: relative; line-height: 20px; vertical-align: top; overflow: visible; font-family: ui-monospace, SFMono-Regular, &quot;SF Mono&quot;, Menlo, Consolas, &quot;Liberation Mono&quot;, monospace; font-size: 12px; color: var(--color-fg-default); overflow-wrap: normal; white-space: pre;">  <span class="pl-k" style="box-sizing: border-box; color: var(--color-prettylights-syntax-keyword);">return</span> <span class="pl-kos" style="box-sizing: border-box;">[</span><span class="pl-c1" style="box-sizing: border-box; color: var(--color-prettylights-syntax-constant);">null</span><span class="pl-kos" style="box-sizing: border-box;">,</span> <span class="pl-c1" style="box-sizing: border-box; color: var(--color-prettylights-syntax-constant);">undefined</span><span class="pl-kos" style="box-sizing: border-box;">,</span> <span class="pl-c1" style="box-sizing: border-box; color: var(--color-prettylights-syntax-constant);">2</span><span class="pl-kos" style="box-sizing: border-box;">,</span> <span class="pl-s" style="box-sizing: border-box; color: var(--color-prettylights-syntax-string);">""</span><span class="pl-kos" style="box-sizing: border-box;">]</span><span class="pl-kos" style="box-sizing: border-box;">;</span></td></tr><tr style="box-sizing: border-box;"><td id="file-example-js-L3" class="blob-num js-line-number js-code-nav-line-number" data-line-number="3" style="box-sizing: border-box; padding: 0px 10px; width: 50px; min-width: 50px; font-family: ui-monospace, SFMono-Regular, &quot;SF Mono&quot;, Menlo, Consolas, &quot;Liberation Mono&quot;, monospace; font-size: 12px; line-height: 20px; color: var(--color-fg-subtle); text-align: right; white-space: nowrap; vertical-align: top; cursor: pointer; user-select: none;"></td><td id="file-example-js-LC3" class="blob-code blob-code-inner js-file-line" style="box-sizing: border-box; padding: 0px 10px; position: relative; line-height: 20px; vertical-align: top; overflow: visible; font-family: ui-monospace, SFMono-Regular, &quot;SF Mono&quot;, Menlo, Consolas, &quot;Liberation Mono&quot;, monospace; font-size: 12px; color: var(--color-fg-default); overflow-wrap: normal; white-space: pre;"><span class="pl-kos" style="box-sizing: border-box;">}</span></td></tr></tbody></table>"#
        ),
        "<code>[function run() {]<br>[  return [null, undefined, 2, \"\"];]<br>[}]</code>"
    );
}

// <p dir="auto"><code spellcheck="false" data-lexical-text="true"><span>12</span></code></p>
// upstream: lexical/src/__tests__/unit/CodeBlock.test.ts::CodeBlock tests > Code block html paste: Single line <code>
#[test]
fn code_block_html_paste_single_line_code() {
    assert_eq!(
        paste(r#"<meta charset='utf-8'><code>12</code>"#),
        "<paragraph>[12|16]</paragraph>"
    );
}

// <code spellcheck="false" dir="auto"><span data-lexical-text="true">1</span><br><span data-lexical-text="true">2</span></code>
// upstream: lexical/src/__tests__/unit/CodeBlock.test.ts::CodeBlock tests > Code block html paste: Multiline <code>
#[test]
fn code_block_html_paste_multiline_code() {
    assert_eq!(
        paste(r#"<meta charset='utf-8'><code>1<br>2</code>"#),
        "<code>[1]<br>[2]</code>"
    );
}

// <code spellcheck="false" dir="auto"><span data-lexical-text="true">1</span><br><span data-lexical-text="true">2</span></code>
// upstream: lexical/src/__tests__/unit/CodeBlock.test.ts::CodeBlock tests > Code block html paste: Multiline <code> with nested <br>
#[test]
fn code_block_html_paste_multiline_code_with_nested_br() {
    assert_eq!(
        paste(r#"<meta charset='utf-8'><code><span>1<br>2</span></code>"#),
        "<code>[1]<br>[2]</code>"
    );
}

// <p dir="auto"><strong class="editor-text-bold editor-text-italic editor-text-underline" data-lexical-text="true">Hello </strong><sub data-lexical-text="true"><strong class="editor-text-bold editor-text-italic">World </strong></sub><sup data-lexical-text="true"><strong class="editor-text-bold editor-text-italic editor-text-underline">Lexical</strong></sup></p>
// upstream: lexical/src/__tests__/unit/CodeBlock.test.ts::CodeBlock tests > Code block html paste: Multiple text formats
#[test]
fn code_block_html_paste_multiple_text_formats() {
    assert_eq!(
        paste(
            r#"<strong style="font-weight: 700; font-style: italic; text-decoration: underline; color: rgb(0, 0, 0); font-size: 15px; text-align: left; text-indent: 0px; background-color: rgb(255, 255, 255);">Hello </strong><sub style="color: rgb(0, 0, 0); font-style: normal; font-weight: 400; text-align: left; text-indent: 0px; background-color: rgb(255, 255, 255);"><strong style="font-weight: 700; font-style: italic; text-decoration: line-through; font-size: 0.8em; vertical-align: sub !important;">World </strong></sub><sup style="color: rgb(0, 0, 0); font-style: normal; font-weight: 400; text-align: left; text-indent: 0px; background-color: rgb(255, 255, 255);"><strong style="font-weight: 700; font-style: italic; text-decoration: underline line-through; font-size: 0.8em; vertical-align: super;">Lexical</strong></sup>"#
        ),
        "<paragraph>[Hello |11][World |39][Lexical|79]</paragraph>"
    );
}

// <h1 dir="auto"><span data-lexical-text="true">My document</span></h1>
// upstream: lexical/src/__tests__/unit/CodeBlock.test.ts::CodeBlock tests > Code block html paste: Title from Google Docs
#[test]
fn code_block_html_paste_title_from_google_docs() {
    assert_eq!(
        paste(
            r#"<meta charset='utf-8'><meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-whatever"><span style="font-size:26pt;font-family:Arial;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">My document</span></b>"#
        ),
        "<h1>[My document]</h1>"
    );
}

// <h1 dir="auto"><span data-lexical-text="true">My document</span></h1>
// upstream: lexical/src/__tests__/unit/CodeBlock.test.ts::CodeBlock tests > Code block html paste: Title from Google Docs Wrapped in Paragraph
#[test]
fn code_block_html_paste_title_from_google_docs_wrapped_in_paragraph() {
    assert_eq!(
        paste(
            r#"<meta charset='utf-8'><meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-wjatever"><p dir="ltr" style="line-height:1.38;margin-top:0pt;margin-bottom:3pt;"><span style="font-size:26pt;font-family:Arial;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;">My document</span></p></b>"#
        ),
        "<h1>[My document]</h1>"
    );
}

// <p dir="auto"><sub data-lexical-text="true"><span>subscript</span></sub><span data-lexical-text="true"> and </span><sup data-lexical-text="true"><span>superscript</span></sup></p>
// upstream: lexical/src/__tests__/unit/CodeBlock.test.ts::CodeBlock tests > Code block html paste: Subscript and Superscript
#[test]
fn code_block_html_paste_subscript_and_superscript() {
    assert_eq!(
        paste(
            r#"<b style="font-weight:normal;" id="docs-internal-guid-374b5f9d-7fff-9120-bcb0-1f5c1b6d59fa"><span style="font-size:11pt;font-family:Arial;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;"><span style="font-size:0.6em;vertical-align:sub;">subscript</span></span><span style="font-size:11pt;font-family:Arial;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;"> and </span><span style="font-size:11pt;font-family:Arial;color:#000000;background-color:transparent;font-weight:400;font-style:normal;font-variant:normal;text-decoration:none;vertical-align:baseline;white-space:pre;white-space:pre-wrap;"><span style="font-size:0.6em;vertical-align:super;">superscript</span></span></b>"#
        ),
        "<paragraph>[subscript|32][ and ][superscript|64]</paragraph>"
    );
}
