//! Ports of `packages/lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts`.
//!
//! "GitHub raw-file-view table still wins when TableExtension is present"
//! is skipped: it needs `@lexical/table` (rust/parity/skips.json).

#![cfg(all(feature = "html", feature = "extension"))]

use super::super::support::update;
use crate::code::{code_extension, is_code_node};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::import::generate_nodes_from_dom_via_extension;
use crate::{Editor, LexicalExtension, NodeRef, define_extension, get_root};

/// `buildEditor()`: an editor built with `CodeExtension` alone.
fn build_editor() -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies: vec![code_extension().into()],
        ..LexicalExtension::<(), (), ()>::new("code-host", ())
    })
    .into()])
    .unwrap()
}

/// `importInto(editor, html)`: `$getRoot().clear().splice(0, 0,
/// $generate(html))`, with the DOMImportExtension's output.
fn import_into(editor: &Editor, html: &str) {
    let html = format!("<!doctype html><html><body>{html}</body></html>");
    update(editor, move || {
        let dom = crate::dom::parse_from_string(&html);
        let nodes = generate_nodes_from_dom_via_extension(&dom, None)?;
        let root = get_root();
        root.clear()?;
        root.splice(0, 0, &nodes)
    });
}

/// `$rootCode()`.
fn root_code() -> NodeRef {
    let node = get_root()
        .get_first_child()
        .expect("expected CodeNode at root");
    assert!(is_code_node(&node), "expected CodeNode at root");
    node
}

/// `$getRoot().getChildren().filter($isCodeNode)`.
fn code_nodes() -> Vec<NodeRef> {
    get_root()
        .get_children()
        .into_iter()
        .filter(is_code_node)
        .collect()
}

fn read<R>(editor: &Editor, f: impl FnOnce() -> R) -> R {
    editor.read(f).unwrap()
}

// upstream: lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > <pre> imports as CodeNode
#[test]
fn pre_imports_as_code_node() {
    let editor = build_editor();
    import_into(&editor, "<pre>const x = 1;</pre>");
    read(&editor, || {
        assert!(root_code().get_text_content().contains("const x = 1;"));
    });
}

// upstream: lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > <pre data-language="ts"> sets the language
#[test]
fn pre_data_language_sets_the_language() {
    let editor = build_editor();
    import_into(&editor, r#"<pre data-language="ts">x</pre>"#);
    read(&editor, || {
        assert_eq!(root_code().get_language().as_deref(), Some("ts"));
    });
}

// upstream: lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > <pre data-theme="poimandres"> restores the theme
#[test]
fn pre_data_theme_restores_the_theme() {
    let editor = build_editor();
    import_into(
        &editor,
        r#"<pre data-language="ts" data-theme="poimandres">x</pre>"#,
    );
    read(&editor, || {
        let node = root_code();
        assert_eq!(node.get_language().as_deref(), Some("ts"));
        assert_eq!(node.get_theme().as_deref(), Some("poimandres"));
    });
}

// upstream: lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > multi-line <code data-theme> restores the theme
#[test]
fn multi_line_code_data_theme_restores_the_theme() {
    let editor = build_editor();
    import_into(
        &editor,
        "<code data-language=\"ts\" data-theme=\"poimandres\">a\nb</code>",
    );
    read(&editor, || {
        let node = root_code();
        assert_eq!(node.get_language().as_deref(), Some("ts"));
        assert_eq!(node.get_theme().as_deref(), Some("poimandres"));
    });
}

// upstream: lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > multi-line <code> imports as CodeNode (not inline)
#[test]
fn multi_line_code_imports_as_code_node() {
    let editor = build_editor();
    import_into(&editor, "<code>line1\nline2</code>");
    read(&editor, || {
        let text = root_code().get_text_content();
        assert!(text.contains("line1"));
        assert!(text.contains("line2"));
    });
}

// upstream: lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > single-line <code> defers to inline-format (no CodeNode)
#[test]
fn single_line_code_defers_to_inline_format() {
    let editor = build_editor();
    import_into(&editor, "<p><code>inline</code></p>");
    read(&editor, || {
        let para = get_root().get_first_child().expect("expected paragraph");
        assert_eq!(para.get_type(), "paragraph", "expected paragraph");
        let text = para
            .get_all_text_nodes()
            .into_iter()
            .next()
            .expect("expected text node");
        assert_eq!(text.get_text_content(), "inline");
        assert!(text.has_format("code"));
    });
}

// upstream: lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > <div style="font-family: monospace"> imports as CodeNode
#[test]
fn div_monospace_imports_as_code_node() {
    let editor = build_editor();
    import_into(
        &editor,
        "<div style=\"font-family: Menlo, monospace\">a\nb</div>",
    );
    read(&editor, || {
        let text = root_code().get_text_content();
        assert!(text.contains('a'));
        assert!(text.contains('b'));
    });
}

// upstream: lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > GitHub raw-file-view table imports as CodeNode
#[test]
fn github_raw_file_view_table_imports_as_code_node() {
    let editor = build_editor();
    import_into(
        &editor,
        &[
            r#"<table class="js-file-line-container">"#,
            r#"<tr><td class="js-file-line">line 1</td></tr>"#,
            r#"<tr><td class="js-file-line">line 2</td></tr>"#,
            "</table>",
        ]
        .join(""),
    );
    read(&editor, || {
        let text = root_code().get_text_content();
        assert!(text.contains("line 1"));
        assert!(text.contains("line 2"));
    });
}

// upstream: lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension > plain <table> falls through (no CodeNode)
#[test]
fn plain_table_falls_through() {
    let editor = build_editor();
    import_into(&editor, "<table><tr><td>a</td></tr></table>");
    read(&editor, || {
        assert!(
            !get_root()
                .get_first_child()
                .is_some_and(|n| is_code_node(&n)),
            "plain table should not become a CodeNode"
        );
    });
}

const VSCODE_EXPECTED: &str = "## 📱 iOS alpha four

Biggest alpha update!

- Animate on sending message and delete
- Set online/offline
- Show all users when @ is typed

[Get it from TestFlight.](https://testflight.apple.com/join/yKKmzbhD)

## Smaller improvements";

const VSCODE_CHROME_HTML: &str = r##"<meta charset='utf-8'><div style="color: #cccccc;background-color: #1f1f1f;font-family: Menlo, Monaco, 'Courier New', monospace;font-weight: normal;font-size: 12px;line-height: 18px;white-space: pre;"><div><span style="color: #569cd6;font-weight: bold;">## 📱 iOS alpha four</span></div><br><div><span style="color: #cccccc;">Biggest alpha update!</span></div><br><div><span style="color: #6796e6;">-</span><span style="color: #cccccc;"> Animate on sending message and delete</span></div><div><span style="color: #6796e6;">-</span><span style="color: #cccccc;"> Set online/offline</span></div><div><span style="color: #6796e6;">-</span><span style="color: #cccccc;"> Show all users when @ is typed</span></div><br><div><span style="color: #cccccc;">[</span><span style="color: #ce9178;">Get it from TestFlight.</span><span style="color: #cccccc;">](</span><span style="color: #cccccc;text-decoration: underline;">https://testflight.apple.com/join/yKKmzbhD</span><span style="color: #cccccc;">)</span></div><br><div><span style="color: #569cd6;font-weight: bold;">## Smaller improvements</span></div></div>"##;

/// `VSCODE_SAFARI_HTML`, with `SAFARI_LINE_STYLE` on each line.
fn vscode_safari_html() -> String {
    let s = r#"font-family: Menlo, Monaco, "Courier New", monospace; white-space: pre;"#;
    [
        format!(
            r#"<div style='{s}'><span style="color: rgb(86, 156, 214); font-weight: bold;">## 📱 iOS alpha four</span></div>"#
        ),
        format!("<br style='{s}'>"),
        format!(
            r#"<div style='{s}'><span style="color: rgb(204, 204, 204);">Biggest alpha update!</span></div>"#
        ),
        format!("<br style='{s}'>"),
        format!(
            "<div style='{s}'><span>-</span><span> Animate on sending message and delete</span></div>"
        ),
        format!("<div style='{s}'><span>-</span><span> Set online/offline</span></div>"),
        format!(
            "<div style='{s}'><span>-</span><span> Show all users when @ is typed</span></div>"
        ),
        format!("<br style='{s}'>"),
        format!(
            r#"<div style='{s}'><span>[</span><span>Get it from TestFlight.</span><span>](</span><span style="text-decoration: underline;">https://testflight.apple.com/join/yKKmzbhD</span><span>)</span></div>"#
        ),
        format!("<br style='{s}'>"),
        format!(
            r#"<div style='{s}'><span style="color: rgb(86, 156, 214); font-weight: bold;">## Smaller improvements</span></div>"#
        ),
    ]
    .join("")
}

fn expect_one_code_node(editor: &Editor, text: &'static str) {
    read(editor, || {
        let code_nodes = code_nodes();
        assert_eq!(code_nodes.len(), 1);
        assert_eq!(code_nodes[0].get_text_content(), text);
    });
}

// upstream: lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension — VS Code paste > Chrome (single outer monospace wrapper) → one CodeNode
#[test]
fn vscode_chrome_paste_is_one_code_node() {
    let editor = build_editor();
    import_into(&editor, VSCODE_CHROME_HTML);
    expect_one_code_node(&editor, VSCODE_EXPECTED);
}

// upstream: lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension — VS Code paste > Safari (flat sibling monospace divs / brs) → one CodeNode
#[test]
fn vscode_safari_paste_is_one_code_node() {
    let editor = build_editor();
    import_into(&editor, &vscode_safari_html());
    expect_one_code_node(&editor, VSCODE_EXPECTED);
}

// upstream: lexical-code-core/src/__tests__/unit/CodeImportExtension.test.ts::CodeImportExtension — VS Code paste > paste without the VS Code structural signal does not install the overlay
#[test]
fn paste_without_the_vscode_signal_does_not_install_the_overlay() {
    let editor = build_editor();
    import_into(
        &editor,
        r#"<div style="font-family: monospace; white-space: pre;">just one line</div>"#,
    );
    expect_one_code_node(&editor, "just one line");
}
