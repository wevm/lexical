//! Helpers for group b of the lexical-table test ports.

#![allow(dead_code)]

use crate::dom::{DomNode, parse_from_string};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::json::Object;
use crate::table::{TableConfigOverrides, compute_table_map_skip_cell_check, table_extension};
use crate::upstream_tests::support::{panic_message, set_test_root_element};
use crate::{
    AnyLexicalExtensionArgument, Editor, EditorConfig, LexicalExtension, NodeRef, config_extension,
    define_extension,
};

/// A theme from its JSON.
pub(crate) fn theme(theme: &str) -> Object {
    crate::json::parse(theme)
        .unwrap()
        .as_object()
        .unwrap()
        .clone()
}

/// An `EditorConfig` with the theme `theme`.
pub(crate) fn theme_config(theme_json: &str) -> EditorConfig {
    EditorConfig {
        theme: theme(theme_json),
    }
}

/// `node.createDOM(config).outerHTML`.
pub(crate) fn outer_html(node: &NodeRef, config: &EditorConfig, editor: &Editor) -> String {
    node.get_latest()
        .create_dom(node, config, editor)
        .unwrap()
        .outer_html()
}

/// `buildEditorFromExtensions(defineExtension({dependencies:
/// [configExtension(TableExtension, {hasHorizontalScroll}), ...extra], name,
/// theme}))`, on a root element.
pub(crate) fn build_table_editor(
    name: &'static str,
    has_horizontal_scroll: bool,
    theme_json: &str,
    extra: Vec<AnyLexicalExtensionArgument>,
) -> LexicalEditorWithDispose {
    let mut dependencies = vec![config_extension(
        &table_extension(),
        [TableConfigOverrides {
            has_horizontal_scroll: Some(has_horizontal_scroll),
            ..TableConfigOverrides::default()
        }],
    )];
    dependencies.extend(extra);
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        theme: Some(theme(theme_json)),
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    editor
}

/// `table.getCellNodeFromCords(x, y, DOMTable)`, read off the table map in
/// place of the rendered table (the DOM coordinates aren't ported).
pub(crate) fn cell_at(table: &NodeRef, x: usize, y: usize) -> Option<NodeRef> {
    let (map, _, _) = compute_table_map_skip_cell_check(table, None, None).ok()?;
    map.get(y)?.get(x)?.as_ref().map(|v| v.cell.clone())
}

/// What `expectHtmlToBeEqual` compares: prettier's formatting of both
/// sides, where the whitespace between tags and around text doesn't count,
/// attributes are sorted (`prettier-plugin-organize-attributes`), and a
/// `style` attribute is formatted as CSS (one `name: value;` per
/// declaration).
pub(crate) fn canonical_html(html: &str) -> String {
    let doc = parse_from_string(&format!("<body>{}</body>", html.replace('\n', "")));
    let body = doc.body().expect("a body");
    let mut out = String::new();
    for child in body.child_nodes() {
        write_node(&child, &mut out);
    }
    out
}

fn group(name: &str) -> u8 {
    match name {
        "class" => 0,
        "id" | "name" => 1,
        _ if name.starts_with("aria-") => 3,
        _ if name.starts_with("data-") => 4,
        _ => 2,
    }
}

fn normalize_style(style: &str) -> String {
    style
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(|d| match d.split_once(':') {
            Some((name, value)) => format!("{}: {};", name.trim(), value.trim()),
            None => format!("{d};"),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn write_node(node: &DomNode, out: &mut String) {
    if node.is_text() {
        let text = node.data();
        let text = text.trim();
        if !text.is_empty() {
            let container = crate::dom::get_document().create_element("div");
            container.append_text(text);
            out.push_str(&container.inner_html());
        }
        return;
    }
    if !node.is_element() {
        return;
    }
    let tag = node.tag_name().to_lowercase();
    let mut attributes = node.attributes();
    attributes.sort_by(|(a, _), (b, _)| (group(a), a).cmp(&(group(b), b)));
    out.push('<');
    out.push_str(&tag);
    for (name, value) in attributes {
        let value = if name == "style" {
            normalize_style(&value)
        } else {
            value
        };
        out.push_str(&format!(" {name}=\"{value}\""));
    }
    out.push('>');
    for child in node.child_nodes() {
        write_node(&child, out);
    }
    if !crate::dom::is_void_html_element(&tag) {
        out.push_str(&format!("</{tag}>"));
    }
}

/// `expectHtmlToBeEqual(actual, expected)`.
#[track_caller]
pub(crate) fn expect_html_to_be_equal(actual: &str, expected: &str) {
    assert_eq!(canonical_html(actual), canonical_html(expected));
}

/// `wrapTableHtml(expected)`: each `<table>` (with its `dir`) in the
/// scroll wrapper.
pub(crate) fn wrap_table_html(expected: &str) -> String {
    let mut out = String::new();
    let mut rest = expected;
    while let Some(i) = rest.find("<table") {
        out.push_str(&rest[..i]);
        rest = &rest[i + "<table".len()..];
        let mut dir = "";
        for d in [r#" dir="auto""#, r#" dir="ltr""#, r#" dir="rtl""#] {
            if rest.starts_with(d) {
                dir = d;
                rest = &rest[d.len()..];
                break;
            }
        }
        out.push_str(&format!(
            r#"<div{dir} class="table-scrollable-wrapper"><table"#
        ));
    }
    out.push_str(rest);
    out.replace("</table>", "</table></div>")
}

/// A generator of `emptyTableContentsHtml`'s cells.
#[derive(Clone, Copy)]
pub(crate) enum TableHtmlGenerator {
    /// `PlainTableHTMLGenerator`.
    Plain,
    /// `HeadingTableHTMLGenerator`: `th` in the first row and column.
    Heading,
}

/// `emptyTableContentsHtml(rows, cols, gen)`.
pub(crate) fn empty_table_contents_html(
    rows: usize,
    cols: usize,
    generator: TableHtmlGenerator,
) -> String {
    let mut out = String::new();
    for row in 0..rows {
        out.push_str(r#"<tr dir="auto">"#);
        for col in 0..cols {
            let tag = match generator {
                TableHtmlGenerator::Heading if col == 0 || row == 0 => "th",
                _ => "td",
            };
            out.push_str(&format!(
                r#"<{tag} dir="auto"><p dir="auto"><br data-lexical-managed-linebreak="true" /></p></{tag}>"#
            ));
        }
        out.push_str("</tr>");
    }
    out
}

/// The DOM the reconciler mounts for `node`: its `createDOM`, with the
/// attributes `$createNode` adds (`data-lexical-text`; `dir`, `auto` on a
/// block in a root or shadow root with no direction; the indent and the
/// format's `text-align`), its children in its DOM slot (a table's
/// `<table>` inside its scroll wrapper), and the managed `<br>` of a block
/// that is empty or ends in a line break or an inline decorator.
pub(crate) fn render_dom(node: &NodeRef, editor: &Editor) -> DomNode {
    let dom = node
        .get_latest()
        .create_dom(node, editor.config(), editor)
        .unwrap();
    if node.is_text() {
        dom.set_attribute("data-lexical-text", "true");
    } else if node.get_latest().is_decorator() {
        dom.set_attribute("data-lexical-decorator", "true");
        dom.set_attribute("contenteditable", "false");
    }
    if !node.is_element() {
        return dom;
    }
    if let Some(direction) = node.element_data().get_direction() {
        dom.set_attribute("dir", direction.as_str());
    } else if let Some(parent) = node.get_parent()
        && crate::is_root_or_shadow_root(&parent)
        && parent.element_data().get_direction().is_none()
    {
        dom.set_attribute("dir", "auto");
    }
    let indent = node.element_data().get_indent();
    if indent != 0.0 {
        dom.style().set_property(
            "padding-inline-start",
            &format!(
                "calc({} * var(--lexical-indent-base-value, 40px))",
                crate::json::number_to_string(indent)
            ),
            "",
        );
    }
    // `TableNode.getDOMSlot`: the `<table>` inside the wrapper.
    let slot = if crate::table::is_table_node(node) && dom.local_name() != "table" {
        dom.descendant_elements()
            .into_iter()
            .find(|e| e.local_name() == "table")
            .expect("a table")
    } else {
        dom.clone()
    };
    let children = node.get_children();
    for child in &children {
        slot.append_child(&render_dom(child, editor));
    }
    let format = node.element_data().get_format_type();
    if !format.is_empty() {
        dom.style().set_property("text-align", format, "");
    }
    if !node.is_inline() {
        let managed = match children.last() {
            None => true,
            Some(last) => {
                last.get_type() == "linebreak"
                    || (last.get_latest().is_decorator() && last.is_inline())
            }
        };
        if managed {
            let br = crate::lexical_dom::get_document().create_element("br");
            br.set_attribute("data-lexical-managed-linebreak", "true");
            slot.append_child(&br);
        }
    }
    dom
}

/// `testEnv.innerHTML`: the root's children as the reconciler mounts them,
/// in the committed state.
pub(crate) fn rendered_html(editor: &Editor) -> String {
    let e = editor.clone();
    editor
        .read_with_mode(crate::EditorReadMode::Latest, move || {
            let container = crate::lexical_dom::get_document().create_element("div");
            for child in crate::get_root().get_children() {
                container.append_child(&render_dom(&child, &e));
            }
            container.inner_html()
        })
        .unwrap()
}

/// The mounted DOM of each of the root's children, as `outerHTML`.
pub(crate) fn rendered_children(editor: &Editor) -> Vec<String> {
    let e = editor.clone();
    editor
        .read_with_mode(crate::EditorReadMode::Latest, move || {
            crate::get_root()
                .get_children()
                .iter()
                .map(|c| render_dom(c, &e).outer_html())
                .collect()
        })
        .unwrap()
}

/// The message of `f`'s panic, if it panics.
pub(crate) fn panic_of<R>(f: impl FnOnce() -> R) -> Option<String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .err()
        .map(|p| panic_message(p.as_ref()))
}

/// `TestDecoratorNode` (`test_decorator`) with its `importDOM` and
/// `exportDOM`: a `<test-decorator>` element.
#[derive(Clone, Debug, Default)]
pub(crate) struct HtmlTestDecoratorNode {
    base: crate::NodeBase,
}

impl crate::LexicalNode for HtmlTestDecoratorNode {
    fn get_type(&self) -> &'static str {
        "test_decorator"
    }
    fn base(&self) -> &crate::NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut crate::NodeBase {
        &mut self.base
    }
    fn is_decorator(&self) -> bool {
        true
    }
    fn is_inline(&self) -> bool {
        true
    }
    fn get_text_content(&self) -> String {
        "Hello world".to_owned()
    }
    fn decorate(&self, _editor: &Editor) -> Option<std::rc::Rc<dyn std::any::Any>> {
        Some(std::rc::Rc::new("Hello world"))
    }
    fn create_dom(
        &self,
        _this: &NodeRef,
        _config: &EditorConfig,
        _editor: &Editor,
    ) -> Result<DomNode, crate::LexicalError> {
        Ok(crate::lexical_dom::get_document().create_element("span"))
    }
    fn export_dom(
        &self,
        _this: &NodeRef,
        _editor: &Editor,
    ) -> Result<crate::lexical_dom::DomExportOutput, crate::LexicalError> {
        Ok(crate::lexical_dom::DomExportOutput::element(
            crate::lexical_dom::get_document().create_element("test-decorator"),
        ))
    }
    fn import_dom(&self) -> Option<crate::lexical_dom::DomConversionMap> {
        Some(vec![(
            "test-decorator".to_owned(),
            crate::lexical_dom::importer(0, |_| {
                Ok(Some(crate::lexical_dom::DomConversionOutput::node(Some(
                    crate::lexical_utils::set_node_key(Box::new(HtmlTestDecoratorNode::default())),
                ))))
            }),
        )])
    }
}

/// `initializeUnitTest`'s editor with the table nodes, the test decorator
/// with its HTML import and export, and `theme`, on a root element.
pub(crate) fn table_unit_test_editor(theme_json: &str) -> Editor {
    crate::reset_random_key();
    let editor = crate::create_editor(crate::CreateEditorArgs {
        namespace: "test".to_owned(),
        nodes: {
            let mut nodes = crate::table::table_nodes();
            nodes.push(crate::NodeClass::of::<HtmlTestDecoratorNode>());
            nodes
        },
        theme: theme(theme_json),
        ..crate::CreateEditorArgs::default()
    });
    set_test_root_element(&editor);
    editor
}
