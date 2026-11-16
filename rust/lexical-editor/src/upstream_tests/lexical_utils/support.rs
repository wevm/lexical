//! Helpers shared by the ports of `@lexical/utils`'s tests.

use crate::upstream_tests::support::create_test_editor;
use crate::{CreateEditorArgs, Editor, NodeRef, create_paragraph_node, create_text_node};

/// `createTestEditor()` with `editor._headless = true`: an editor without
/// a root element, which never reconciles either.
pub(super) fn create_headless_test_editor() -> Editor {
    create_test_editor(CreateEditorArgs::default())
}

/// `$createTextAndParagraphWithDepth(depth)`.
pub(super) fn create_text_and_paragraph_with_depth(depth: usize) -> Vec<NodeRef> {
    if depth == 0 {
        return vec![create_text_node(&format!("<{depth} />"))];
    }
    let paragraph = create_paragraph_node();
    paragraph
        .append(&create_text_and_paragraph_with_depth(depth - 1))
        .unwrap();
    vec![
        create_text_node(&format!("<{depth}>")),
        paragraph,
        create_text_node(&format!("</{depth}>")),
    ]
}

/// `textContentForDepth(i)`.
pub(super) fn text_content_for_depth(i: usize) -> String {
    if i > 0 {
        format!("<{i}>{}</{i}>", text_content_for_depth(i - 1))
    } else {
        format!("<{i} />")
    }
}

/// `TestDecoratorNode` (`test_decorator`) as upstream's test utilities
/// define it, with its `importDOM` and `exportDOM` (`<test-decorator>`) and
/// `setIsInline`, which the shared test node lacks.
#[cfg(feature = "html")]
#[derive(Clone, Debug, Default)]
pub(super) struct TestDecoratorNode {
    base: crate::NodeBase,
    block: bool,
}

#[cfg(feature = "html")]
impl crate::LexicalNode for TestDecoratorNode {
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
        !self.block
    }
    fn get_text_content(&self) -> String {
        "Hello world".to_owned()
    }
    fn decorate(&self, _editor: &Editor) -> Option<std::rc::Rc<dyn std::any::Any>> {
        Some(std::rc::Rc::new("Hello world"))
    }
    fn import_dom(&self) -> Option<crate::lexical_dom::DomConversionMap> {
        Some(vec![(
            "test-decorator".to_owned(),
            crate::lexical_dom::importer(0, |_| {
                Ok(Some(crate::lexical_dom::DomConversionOutput::node(Some(
                    create_test_decorator_node(),
                ))))
            }),
        )])
    }
    fn export_dom(
        &self,
        _this: &NodeRef,
        _editor: &Editor,
    ) -> Result<crate::lexical_dom::DomExportOutput, crate::LexicalError> {
        Ok(crate::lexical_dom::DomExportOutput {
            element: Some(crate::lexical_dom::get_document().create_element("test-decorator")),
            ..Default::default()
        })
    }
    fn create_dom(
        &self,
        _this: &NodeRef,
        _config: &crate::EditorConfig,
        _editor: &Editor,
    ) -> Result<crate::dom::DomNode, crate::LexicalError> {
        Ok(
            crate::lexical_dom::get_document().create_element(if self.block {
                "div"
            } else {
                "span"
            }),
        )
    }
}

/// `$createTestDecoratorNode()`.
#[cfg(feature = "html")]
pub(super) fn create_test_decorator_node() -> NodeRef {
    crate::create::<TestDecoratorNode>()
}

/// `testDecorator.setIsInline(inline)`.
#[cfg(feature = "html")]
pub(super) fn set_is_inline(node: &NodeRef, inline: bool) -> NodeRef {
    crate::lexical_utils::with_writable(node.get_key(), |n| {
        n.as_any_mut()
            .downcast_mut::<TestDecoratorNode>()
            .expect("a TestDecoratorNode")
            .block = !inline;
    });
    node.clone()
}

/// `createTestEditor()` (with `editor._headless = true`), whose
/// `TestDecoratorNode` is [`TestDecoratorNode`]: `DEFAULT_NODES` as
/// [`create_test_editor`] has them, with this decorator in place of the
/// shared one.
#[cfg(feature = "html")]
pub(super) fn create_headless_test_editor_with_dom_decorator() -> Editor {
    use crate::upstream_tests::support::{
        TestElementNode, TestInlineElementNode, TestSegmentedNode,
    };
    #[allow(unused_mut)]
    let mut nodes = vec![
        crate::NodeClass::of::<TestElementNode>(),
        crate::NodeClass::of::<TestSegmentedNode>(),
        crate::NodeClass::of::<TestDecoratorNode>(),
        crate::NodeClass::of::<TestInlineElementNode>(),
    ];
    #[cfg(feature = "rich-text")]
    nodes.extend([
        crate::NodeClass::of::<crate::rich_text::HeadingNode>(),
        crate::NodeClass::of::<crate::rich_text::QuoteNode>(),
    ]);
    #[cfg(feature = "list")]
    nodes.extend([
        crate::NodeClass::of::<crate::list::ListNode>(),
        crate::NodeClass::of::<crate::list::ListItemNode>(),
    ]);
    #[cfg(feature = "link")]
    nodes.extend([
        crate::NodeClass::of::<crate::link::LinkNode>(),
        crate::NodeClass::of::<crate::link::AutoLinkNode>(),
    ]);
    crate::create_editor(CreateEditorArgs {
        nodes,
        ..CreateEditorArgs::default()
    })
}

/// Drops each ` value="<digits>"`, as upstream's
/// `.replace(/\svalue="\d{1,}"/g, '')`.
#[cfg(feature = "html")]
pub(super) fn strip_values(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(i) = rest.find(" value=\"") {
        let after = &rest[i + 8..];
        let digits = after.chars().take_while(char::is_ascii_digit).count();
        if digits > 0 && after[digits..].starts_with('"') {
            out.push_str(&rest[..i]);
            rest = &after[digits + 1..];
        } else {
            out.push_str(&rest[..i + 8]);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}
