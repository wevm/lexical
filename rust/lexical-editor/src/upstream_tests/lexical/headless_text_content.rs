//! Ports of `packages/lexical/src/__tests__/unit/HeadlessTextContent.test.ts`.
//!
//! Upstream spies on `RootNode.prototype.getTextContent`; here, as in
//! Swift's port, the paragraphs are a replacement class that counts how
//! often their text is read, which every read of the document text goes
//! through (`ElementNode.getTextContent` reads each child's).

use crate::nodes::element::ElementData;
use crate::nodes::paragraph::ParagraphNode;
use crate::upstream_tests::support::update;
use crate::{
    CreateEditorArgs, Editor, LexicalError, LexicalNode, LexicalNodeReplacement, NodeBase,
    NodeClass, NodeRef, create, create_editor, create_paragraph_node, create_text_node, get_root,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

thread_local! {
    static TEXT_READS: Cell<usize> = const { Cell::new(0) };
}

/// A `ParagraphNode` subclass that counts the reads of its text.
#[derive(Clone, Debug, Default)]
struct CountingParagraph {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for CountingParagraph {
    fn get_type(&self) -> &'static str {
        "counting-paragraph"
    }
    fn super_types(&self) -> Vec<&'static str> {
        vec!["paragraph"]
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.element)
    }
    fn get_text_content(&self) -> String {
        TEXT_READS.with(|reads| reads.set(reads.get() + 1));
        crate::nodes::element::get_text_content(self)
    }
}

/// `createHeadlessEditor(paragraphs)`.
fn create_headless_editor(paragraphs: usize) -> Editor {
    let editor = create_editor(CreateEditorArgs {
        nodes: vec![
            NodeClass::of::<CountingParagraph>(),
            LexicalNodeReplacement {
                replace: NodeClass::of::<ParagraphNode>(),
                with: Rc::new(|_: &NodeRef| Ok(create::<CountingParagraph>())),
                with_klass: Some(NodeClass::of::<CountingParagraph>()),
            }
            .into(),
        ],
        ..CreateEditorArgs::default()
    });
    update(&editor, move || {
        for i in 0..paragraphs {
            let paragraph = create_paragraph_node();
            paragraph.append(&[create_text_node(&format!("para {i}"))])?;
            get_root().append(&[paragraph])?;
        }
        Ok(())
    });
    editor
}

/// `$firstText(paragraph)`.
fn first_text(paragraph: Option<NodeRef>) -> Result<NodeRef, LexicalError> {
    paragraph
        .filter(NodeRef::is_element)
        .and_then(|p| p.get_first_child())
        .filter(NodeRef::is_text)
        .ok_or_else(|| LexicalError::new("expected a paragraph with text"))
}

/// `edit(editor, index)`.
fn edit(editor: &Editor, index: usize) {
    update(editor, move || {
        let text = first_text(get_root().get_child_at_index(index))?;
        text.set_text_content(&format!("x{}", text.get_text_content()));
        Ok(())
    });
}

// upstream: lexical/src/__tests__/unit/HeadlessTextContent.test.ts::text content listeners in a headless editor > a commit computes no document text while nothing listens
#[test]
fn a_commit_computes_no_document_text_while_nothing_listens() {
    let editor = create_headless_editor(100);
    assert!(get_root_is_counting(&editor));
    TEXT_READS.with(|reads| reads.set(0));
    for i in 0..20 {
        edit(&editor, i);
    }
    assert_eq!(TEXT_READS.with(Cell::get), 0);
}

// upstream: lexical/src/__tests__/unit/HeadlessTextContent.test.ts::text content listeners in a headless editor > a listener still gets each change of the text, and only those
#[test]
fn a_listener_still_gets_each_change_of_the_text_and_only_those() {
    let editor = create_headless_editor(3);
    let texts = Rc::new(RefCell::new(Vec::<String>::new()));
    let seen = texts.clone();
    let unregister =
        editor.register_text_content_listener(move |text| seen.borrow_mut().push(text.to_owned()));
    edit(&editor, 1);
    assert_eq!(*texts.borrow(), ["para 0\n\nxpara 1\n\npara 2"]);
    // A change that leaves the text as it was
    update(&editor, || {
        first_text(get_root().get_first_child())?.toggle_format("bold");
        Ok(())
    });
    assert_eq!(texts.borrow().len(), 1);
    unregister();
    TEXT_READS.with(|reads| reads.set(0));
    edit(&editor, 2);
    assert_eq!(texts.borrow().len(), 1);
    assert_eq!(TEXT_READS.with(Cell::get), 0);
}

/// The paragraphs are the counting replacement, so the count stands for
/// the spy.
fn get_root_is_counting(editor: &Editor) -> bool {
    editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .all(|p| p.get_latest().is::<CountingParagraph>())
        })
        .unwrap()
}
