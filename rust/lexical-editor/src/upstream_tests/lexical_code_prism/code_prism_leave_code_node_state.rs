//! Ports of `packages/lexical-code-prism/src/__tests__/unit/CodePrismLeaveCodeNodeState.test.ts`.
//!
//! `CodePrismExtension` with Prism; here with [`TestTokenizer`] (see
//! `support`).

#![cfg(all(feature = "extension", feature = "rich-text"))]

use super::super::support::update;
use super::support::{TestTokenizer, code_prism_extension};
use crate::code::{
    CodeHighlightNode, create_code_highlight_node, create_code_node, is_code_highlight_node,
};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::node_state::{RequiredNodeStateConfig, StateConfig};
use crate::nodes::text::{TextData, TextNode};
use crate::{
    InitialEditorStateType, LexicalExtension, LexicalNode, LexicalNodeReplacement, NodeBase,
    NodeClass, NodeRef, create, create_paragraph_node, create_state, define_extension, get_root,
    get_state, set_state,
};
use std::rc::Rc;

fn parse_string(value: &crate::json::Value) -> String {
    value.as_str().unwrap_or("").to_owned()
}

thread_local! {
    static NOTE_STATE: StateConfig<String> = create_state("note", parse_string);
    static FLAT_STATE: StateConfig<String> = create_state("flatNote", parse_string);
    static CREATED_STATE: StateConfig<String> = create_state("created", parse_string);
}

fn note_state() -> StateConfig<String> {
    NOTE_STATE.with(Clone::clone)
}
fn flat_state() -> StateConfig<String> {
    FLAT_STATE.with(Clone::clone)
}
fn created_state() -> StateConfig<String> {
    CREATED_STATE.with(Clone::clone)
}

/// `FlatHighlightNode`: a code highlight node with a flat state.
#[derive(Clone, Debug, Default)]
struct FlatHighlightNode {
    base: NodeBase,
    text: TextData,
}

impl LexicalNode for FlatHighlightNode {
    fn get_type(&self) -> &'static str {
        "test-flat-highlight"
    }
    fn super_types(&self) -> Vec<&'static str> {
        vec!["code-highlight", "text"]
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn text(&self) -> Option<&TextData> {
        Some(&self.text)
    }
    fn text_mut(&mut self) -> Option<&mut TextData> {
        Some(&mut self.text)
    }
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        vec![RequiredNodeStateConfig::new(&flat_state(), true)]
    }
}

/// `CreatedTextNode`: a `TextNode` replacement with a state of its own.
#[derive(Clone, Debug, Default)]
struct CreatedTextNode {
    base: NodeBase,
    text: TextData,
}

impl LexicalNode for CreatedTextNode {
    fn get_type(&self) -> &'static str {
        "test-created-text"
    }
    fn super_types(&self) -> Vec<&'static str> {
        vec!["text"]
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn text(&self) -> Option<&TextData> {
        Some(&self.text)
    }
    fn text_mut(&mut self) -> Option<&mut TextData> {
        Some(&mut self.text)
    }
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        vec![RequiredNodeStateConfig::new(&created_state(), false)]
    }
}

fn build(name: &str, nodes: Vec<NodeClass>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            code_prism_extension(TestTokenizer::default()),
        ],
        nodes,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `$getRoot().getFirstChildOrThrow().replace($createParagraphNode(), true)`.
fn replace_with_paragraph(editor: &crate::Editor) {
    update(editor, || {
        get_root()
            .get_first_child_or_throw()?
            .replace(&create_paragraph_node(), true)?;
        Ok(())
    });
}

// upstream: lexical-code-prism/src/__tests__/unit/CodePrismLeaveCodeNodeState.test.ts::code highlight nodes turned back into text when they leave a code block keep their NodeState
#[test]
fn code_highlight_nodes_leaving_a_code_block_keep_their_node_state() {
    let editor = build("leave-code-node-state", vec![]);
    update(&editor, || {
        let token = create_code_highlight_node("note", None);
        set_state(&token, &note_state(), "kept".to_owned())?;
        let code = create_code_node(None, None);
        code.append(&[token])?;
        get_root().append(&[code])
    });
    replace_with_paragraph(&editor);
    let nodes = editor
        .read(|| {
            get_root()
                .get_all_text_nodes()
                .iter()
                .map(|node| {
                    (
                        node.is_text() && !is_code_highlight_node(node),
                        node.get_text_content(),
                        get_state(node, &note_state()).unwrap(),
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(nodes, [(true, "note".to_owned(), "kept".to_owned())]);
}

// upstream: lexical-code-prism/src/__tests__/unit/CodePrismLeaveCodeNodeState.test.ts::the copy includes flat states and keeps the state of a TextNode replacement
#[test]
fn the_copy_includes_flat_states_and_keeps_the_state_of_a_text_node_replacement() {
    let editor = build(
        "leave-code-flat-node-state",
        vec![
            NodeClass::of::<FlatHighlightNode>(),
            LexicalNodeReplacement {
                replace: NodeClass::of::<CodeHighlightNode>(),
                with: Rc::new(|node: &NodeRef| {
                    let new = create::<FlatHighlightNode>();
                    new.set_text_content(&node.get_text_content());
                    Ok(new)
                }),
                with_klass: Some(NodeClass::of::<FlatHighlightNode>()),
            }
            .into(),
            NodeClass::of::<CreatedTextNode>(),
            LexicalNodeReplacement {
                replace: NodeClass::of::<TextNode>(),
                with: Rc::new(|node: &NodeRef| {
                    let new = create::<CreatedTextNode>();
                    new.set_text_content(&node.get_text_content());
                    set_state(&new, &created_state(), "yes".to_owned())
                }),
                with_klass: Some(NodeClass::of::<CreatedTextNode>()),
            }
            .into(),
        ],
    );
    update(&editor, || {
        let code = create_code_node(None, None);
        code.append(&[create_code_highlight_node("note", None)])?;
        get_root().append(&[code])
    });
    // Once highlighting has made its tokens.
    update(&editor, || {
        for token in get_root().get_all_text_nodes() {
            set_state(&token, &flat_state(), "kept".to_owned())?;
            set_state(&token, &note_state(), "kept".to_owned())?;
        }
        Ok(())
    });
    replace_with_paragraph(&editor);
    let nodes = editor
        .read(|| {
            get_root()
                .get_all_text_nodes()
                .iter()
                .map(|node| {
                    [
                        node.get_type().to_owned(),
                        get_state(node, &flat_state()).unwrap(),
                        get_state(node, &note_state()).unwrap(),
                        get_state(node, &created_state()).unwrap(),
                    ]
                })
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(
        nodes,
        [["test-created-text", "kept", "kept", "yes"].map(str::to_owned)]
    );
}
