//! `@lexical/react`'s `DecoratorBlockNode` (the `decorator-block` feature),
//! from `packages/lexical-react/src/LexicalDecoratorBlockNode.ts`: the base
//! of block embeds (videos, tweets), a block decorator that keeps an element
//! alignment (`format`) and is not indentable.
//!
//! JS's class is abstract, and an embed extends it. Here an embed is a node
//! whose [`LexicalNode::decorator_block`] returns its [`DecoratorBlockData`],
//! as a node that extends `ElementNode` returns its `ElementData`: the
//! trait's defaults then make it a decorator ([`LexicalNode::is_decorator`])
//! that is not inline and can't be indented, and [`export_decorator_block`]
//! and [`update_decorator_block_from_json`] are its schema's JSON
//! (`decoratorBlockNodeSchema`).
//!
//! With the `extension` feature, [`block_with_alignable_contents_extension`]
//! is the alignment and selection that React's `BlockWithAlignableContents`
//! (`LexicalBlockWithAlignableContents.tsx`) registers for each block, as
//! Swift's `BlockWithAlignableContentsExtension` has them.
//!
//! Not ported: `createDOM` and `updateDOM`, which come with `html`, and the
//! component's rendering (rust/PARITY.md).

use crate::json::{Object, Value};
use crate::lexical_utils::with_writable;
use crate::node::{LexicalNode, NodeRef};
use crate::schema::{NodeSchema, enum_value, node_schema, with_accessors};
use std::rc::Rc;

/// The formats `decoratorBlockNodeSchema`'s `enumValue` takes, `''` (the
/// default) first.
const FORMATS: [&str; 7] = ["", "left", "start", "center", "right", "end", "justify"];

/// The fields `DecoratorBlockNode` adds: `__format`, an
/// `ElementFormatType`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DecoratorBlockData {
    format: &'static str,
}

impl DecoratorBlockData {
    /// `new DecoratorBlockNode(format)`'s fields: `format`, or `''` when it
    /// names no format.
    pub fn new(format: &str) -> Self {
        DecoratorBlockData {
            format: parse_format(format),
        }
    }

    /// `__format`.
    pub fn format(&self) -> &'static str {
        self.format
    }
}

/// `enumValue([...])`'s parse: the format, or `''` for anything else.
fn parse_format(format: &str) -> &'static str {
    FORMATS.into_iter().find(|f| *f == format).unwrap_or("")
}

thread_local! {
    static DECORATOR_BLOCK_NODE_SCHEMA: NodeSchema = node_schema(vec![(
        "format",
        with_accessors(
            enum_value(FORMATS.map(Value::from).to_vec(), None).expect("the formats"),
            Some(Rc::new(|node| {
                node.decorator_block()
                    .expect("a DecoratorBlockNode")
                    .format
                    .into()
            })),
            Some(Rc::new(|node, value| {
                node.decorator_block_mut()
                    .expect("a DecoratorBlockNode")
                    .format = value.as_str().map_or("", parse_format);
            })),
        )
        .expect("a field"),
    )])
    .expect("DecoratorBlockNode's schema");
}

/// `decoratorBlockNodeSchema`: the `format` field, which every node that
/// extends `DecoratorBlockNode` composes with its own
/// ([`LexicalNode::json_schema_chain`]'s default for one).
pub fn decorator_block_node_schema() -> NodeSchema {
    DECORATOR_BLOCK_NODE_SCHEMA.with(Clone::clone)
}

/// `exportJSON(compact)` of a node that extends `DecoratorBlockNode` and
/// adds nothing to its JSON: `LexicalNode`'s (`type`, `version`) with
/// `format`, which the compact form leaves out when it is `''`
/// (`GENERATED_DECORATORBLOCK`). An embed that adds properties inserts them
/// into the result.
pub fn export_decorator_block(node: &dyn LexicalNode, compact: bool) -> Object {
    let format = node.decorator_block().expect("a DecoratorBlockNode").format;
    let mut json = Object::new();
    if compact {
        json.insert("type", node.get_type().into());
        if !format.is_empty() {
            json.insert("format", format.into());
        }
    } else {
        json.insert("format", format.into());
        json.insert("type", node.get_type().into());
        json.insert("version", 1.0.into());
    }
    json
}

/// `updateFromJSON(json)`'s part for `DecoratorBlockNode`: `format`, `''`
/// when it is missing or names no format.
pub fn update_decorator_block_from_json(data: &mut DecoratorBlockData, json: &Object) {
    data.format = json
        .get("format")
        .and_then(Value::as_str)
        .map_or("", parse_format);
}

/// `$isDecoratorBlockNode(node)`: whether the node extends
/// `DecoratorBlockNode`.
pub fn is_decorator_block_node(node: &NodeRef) -> bool {
    node.get_latest().decorator_block().is_some()
}

impl NodeRef {
    /// `DecoratorBlockNode.getFormat()`; `None` for any other node.
    pub fn get_decorator_block_format(&self) -> Option<&'static str> {
        self.get_latest().decorator_block().map(|d| d.format)
    }

    /// `DecoratorBlockNode.setFormat(format)`: `''` for a string that names
    /// no format. Does nothing to any other node.
    pub fn set_decorator_block_format(&self, format: &str) {
        if is_decorator_block_node(self) {
            let format = parse_format(format);
            with_writable(self.get_key(), |n| {
                n.decorator_block_mut()
                    .expect("a DecoratorBlockNode")
                    .format = format;
            });
        }
    }
}

#[cfg(feature = "extension")]
pub use block_with_alignable_contents::{
    BlockWithAlignableContentsExtensionType, block_with_alignable_contents_extension,
};

/// The commands of `LexicalBlockWithAlignableContents.tsx` as an extension
/// (the `extension` feature, as Swift's `BlockWithAlignableContentsExtension`).
#[cfg(feature = "extension")]
mod block_with_alignable_contents {
    use super::is_decorator_block_node;
    use crate::extension_core::{LexicalExtension, define_extension};
    use crate::node::NodeRef;
    use crate::utils::get_nearest_block_element_ancestor_or_throw;
    use crate::{
        CLICK_COMMAND, COMMAND_PRIORITY_LOW, FORMAT_ELEMENT_COMMAND, create_node_selection,
        get_node_by_key, get_selection, merge_register, set_selection,
    };
    use std::rc::Rc;

    /// `BlockWithAlignableContentsExtension`.
    pub type BlockWithAlignableContentsExtensionType = LexicalExtension;

    fn make() -> Rc<BlockWithAlignableContentsExtensionType> {
        define_extension(LexicalExtension {
            register: Some(Rc::new(|editor, _, _| {
                Ok(merge_register(vec![
                    editor.register_command(
                        &FORMAT_ELEMENT_COMMAND,
                        |format, _| {
                            let Some(selection) = get_selection() else {
                                return Ok(false);
                            };
                            let nodes = selection.get_nodes()?;
                            let mut selected = Vec::new();
                            for node in nodes.iter().filter(|n| is_decorator_block_node(n)) {
                                if node.is_selected(None)? {
                                    selected.push(node.clone());
                                }
                            }
                            if selected.is_empty() {
                                return Ok(false);
                            }
                            if selection.is_node() {
                                for block in &selected {
                                    block.set_decorator_block_format(format);
                                }
                            } else if selection.is_range() {
                                for node in &nodes {
                                    if is_decorator_block_node(node) {
                                        node.set_decorator_block_format(format);
                                    } else {
                                        get_nearest_block_element_ancestor_or_throw(node)?
                                            .set_element_format(format);
                                    }
                                }
                            }
                            Ok(true)
                        },
                        COMMAND_PRIORITY_LOW,
                    ),
                    editor.register_command(
                        &CLICK_COMMAND,
                        |event, _| {
                            let Some(block) = event
                                .target_key
                                .as_deref()
                                .and_then(get_node_by_key)
                                .filter(is_decorator_block_node)
                            else {
                                return Ok(false);
                            };
                            event.prevent_default();
                            // `clearSelection(); setSelected(!isSelected)`.
                            let is_selected = block.is_selected(None)?;
                            let selection = match get_selection().filter(|s| s.is_node()) {
                                Some(current) => {
                                    if !event.shift_key {
                                        current.clear();
                                    }
                                    current
                                }
                                None => {
                                    // A range that covers the block selects
                                    // it too, so even its deselection
                                    // replaces the range with a node
                                    // selection (`useLexicalNodeSelection`).
                                    let selection = create_node_selection();
                                    set_selection(Some(selection.clone()));
                                    selection
                                }
                            };
                            let key = NodeRef::get_key(&block);
                            if is_selected {
                                selection.delete(key);
                            } else {
                                selection.add(key);
                            }
                            Ok(true)
                        },
                        COMMAND_PRIORITY_LOW,
                    ),
                ]))
            })),
            ..LexicalExtension::new("@lexical/react/BlockWithAlignableContents", ())
        })
    }

    thread_local! {
        static BLOCK_WITH_ALIGNABLE_CONTENTS_EXTENSION:
            Rc<BlockWithAlignableContentsExtensionType> = make();
    }

    /// `BlockWithAlignableContentsExtension`: what React's
    /// `BlockWithAlignableContents` registers for each block it draws, for
    /// every `DecoratorBlockNode`, as Swift has it. While a block is
    /// selected (`isSelected()`, which a range over it is too),
    /// [`FORMAT_ELEMENT_COMMAND`] aligns it: under a node selection the
    /// selected blocks, under a range each block in it and the nearest block
    /// element of every other node. A [`CLICK_COMMAND`] whose
    /// [`target_key`](crate::MouseEvent::target_key) is a block (JS: a click
    /// on the component's `<div>`) selects only it, or deselects it when it
    /// was selected; with Shift it adds it to the node selection or takes it
    /// out. Both at [`COMMAND_PRIORITY_LOW`].
    ///
    /// React registers one handler per block, so of several blocks in a
    /// node selection only the one whose handler runs first is aligned;
    /// this aligns them all, as Swift does (rust/PARITY.md).
    pub fn block_with_alignable_contents_extension() -> Rc<BlockWithAlignableContentsExtensionType>
    {
        BLOCK_WITH_ALIGNABLE_CONTENTS_EXTENSION.with(Rc::clone)
    }
}

#[cfg(all(test, feature = "extension", feature = "rich-text"))]
mod tests {
    use super::*;
    use crate::extension::build_editor_from_extensions;
    use crate::extension_core::{LexicalExtension, define_extension};
    use crate::node::NodeBase;
    use crate::{
        CLICK_COMMAND, Editor, FORMAT_ELEMENT_COMMAND, MouseEvent, NodeClass, PointType,
        UpdateOptions, Which, create, create_node_selection, create_paragraph_node,
        create_range_selection, create_text_node, get_root, get_selection, set_selection,
    };

    /// A block embed that adds nothing.
    #[derive(Clone, Debug, Default)]
    struct EmbedNode {
        base: NodeBase,
        block: DecoratorBlockData,
    }

    impl LexicalNode for EmbedNode {
        fn get_type(&self) -> &'static str {
            "embed"
        }
        fn base(&self) -> &NodeBase {
            &self.base
        }
        fn base_mut(&mut self) -> &mut NodeBase {
            &mut self.base
        }
        fn decorator_block(&self) -> Option<&DecoratorBlockData> {
            Some(&self.block)
        }
        fn decorator_block_mut(&mut self) -> Option<&mut DecoratorBlockData> {
            Some(&mut self.block)
        }
        fn generated_export_json(&self, compact: bool) -> Option<Object> {
            Some(export_decorator_block(self, compact))
        }
        fn generated_update_from_json(&mut self, json: &Object) -> bool {
            update_decorator_block_from_json(&mut self.block, json);
            true
        }
    }

    /// The blocks' keys: `[p1, e1, p2, e2, p1's text, p2's text]`.
    type Keys = Vec<String>;

    fn editor() -> crate::extension::LexicalEditorWithDispose {
        build_editor_from_extensions([
            crate::rich_text::rich_text_extension().into(),
            block_with_alignable_contents_extension().into(),
            define_extension(LexicalExtension {
                nodes: vec![NodeClass::of::<EmbedNode>()],
                ..LexicalExtension::<(), (), ()>::new("embed", ())
            })
            .into(),
        ])
        .unwrap()
    }

    fn update(editor: &Editor, f: impl FnOnce() + 'static) {
        editor
            .update(
                move || {
                    f();
                    Ok(())
                },
                UpdateOptions {
                    discrete: true,
                    ..UpdateOptions::default()
                },
            )
            .unwrap();
    }

    /// The document `p1 e1 p2 e2` (`ab`, an embed, `cd`, an embed), then
    /// `select` with its keys.
    fn reset(editor: &Editor, select: impl FnOnce(&Keys) + 'static) -> Keys {
        let keys = Rc::new(std::cell::RefCell::new(Vec::new()));
        let out = keys.clone();
        update(editor, move || {
            let (e1, e2) = (create::<EmbedNode>(), create::<EmbedNode>());
            let (t1, t2) = (create_text_node("ab"), create_text_node("cd"));
            let p1 = create_paragraph_node();
            p1.append(std::slice::from_ref(&t1)).unwrap();
            let p2 = create_paragraph_node();
            p2.append(std::slice::from_ref(&t2)).unwrap();
            let root = get_root();
            root.clear().unwrap();
            root.append(&[p1.clone(), e1.clone(), p2.clone(), e2.clone()])
                .unwrap();
            *out.borrow_mut() = [&p1, &e1, &p2, &e2, &t1, &t2]
                .iter()
                .map(|n| n.get_key().to_owned())
                .collect();
        });
        let keys = keys.borrow().clone();
        let selected = keys.clone();
        update(editor, move || select(&selected));
        keys
    }

    fn select_range(keys: &Keys, anchor: (usize, usize), focus: (usize, usize)) {
        let selection = create_range_selection();
        for (which, (i, offset)) in [(Which::Anchor, anchor), (Which::Focus, focus)] {
            selection
                .point_set(which, &keys[i], offset, PointType::Text)
                .unwrap();
        }
        set_selection(Some(selection));
    }

    /// The four blocks' formats, and the selection: `node` and the indexes
    /// of its blocks, `range`, or `none`.
    fn snapshot(editor: &Editor, keys: &Keys) -> (Vec<&'static str>, String) {
        let keys = keys.clone();
        editor
            .read(move || {
                let formats = get_root()
                    .get_children()
                    .iter()
                    .map(|n| {
                        n.get_decorator_block_format().unwrap_or_else(|| {
                            n.get_latest().element().expect("a block").get_format_type()
                        })
                    })
                    .collect();
                let selection = match get_selection() {
                    None => "none".to_owned(),
                    Some(s) if s.is_node() => {
                        let nodes = s.get_nodes().unwrap();
                        let names: Vec<String> = nodes
                            .iter()
                            .map(|n| {
                                let i = keys.iter().position(|k| k == n.get_key()).unwrap();
                                ["p1", "e1", "p2", "e2"][i].to_owned()
                            })
                            .collect();
                        format!("node {}", names.join(","))
                    }
                    Some(_) => "range".to_owned(),
                };
                (formats, selection)
            })
            .unwrap()
    }

    fn format(editor: &Editor, format: &str) -> bool {
        editor
            .dispatch_command(&FORMAT_ELEMENT_COMMAND, format.to_owned())
            .unwrap()
    }

    fn click(editor: &Editor, key: &str, shift_key: bool) {
        let event = MouseEvent {
            target_key: Some(key.to_owned()),
            shift_key,
            ..MouseEvent::default()
        };
        editor
            .dispatch_command(&CLICK_COMMAND, Rc::new(event))
            .unwrap();
    }

    // What JS records for a `DecoratorBlockNode` that renders
    // `BlockWithAlignableContents` in a `LexicalComposer` with
    // `RichTextPlugin` (jsdom, React 19), a click being one on the
    // component's `<div>`: the formats of `p1 e1 p2 e2` and the selection
    // after each step.
    #[test]
    fn format_element_command_aligns_selected_blocks() {
        let editor = editor();
        // A node selection of e1: only e1 is aligned.
        let keys = reset(&editor, |keys| {
            let selection = create_node_selection();
            selection.add(&keys[1]);
            set_selection(Some(selection));
        });
        assert!(format(&editor, "center"));
        assert_eq!(
            snapshot(&editor, &keys),
            (vec!["", "center", "", ""], "node e1".to_owned())
        );
        // A range from p1 to p2 covers e1: each block in it is aligned.
        let keys = reset(&editor, |keys| select_range(keys, (4, 1), (5, 1)));
        assert!(format(&editor, "right"));
        assert_eq!(
            snapshot(&editor, &keys),
            (vec!["right", "right", "right", ""], "range".to_owned())
        );
        // A range in p1 covers no block: rich text's handler aligns p1.
        let keys = reset(&editor, |keys| select_range(keys, (4, 0), (4, 2)));
        assert!(format(&editor, "justify"));
        assert_eq!(
            snapshot(&editor, &keys),
            (vec!["justify", "", "", ""], "range".to_owned())
        );
    }

    #[test]
    fn click_command_toggles_a_block() {
        let editor = editor();
        let keys = reset(&editor, |_| set_selection(None));
        let (e1, e2) = (keys[1].clone(), keys[3].clone());
        let steps: [(&str, bool, &str); 6] = [
            (&e1, false, "node e1"),
            (&e2, true, "node e1,e2"),
            (&e1, true, "node e2"),
            // Deselecting the last block leaves an empty node selection,
            // which the commit makes no selection.
            (&e2, false, "none"),
            (&e1, false, "node e1"),
            (&e2, false, "node e2"),
        ];
        for (key, shift, expected) in steps {
            click(&editor, key, shift);
            assert_eq!(snapshot(&editor, &keys).1, expected, "click {key} {shift}");
        }
        // A click on a block a range covers deselects it, leaving none.
        let keys = reset(&editor, |keys| select_range(keys, (4, 1), (5, 1)));
        click(&editor, &keys[1], false);
        assert_eq!(snapshot(&editor, &keys).1, "none");
        // A caret elsewhere: the click selects the block.
        let keys = reset(&editor, |keys| select_range(keys, (4, 1), (4, 1)));
        click(&editor, &keys[1], false);
        assert_eq!(snapshot(&editor, &keys).1, "node e1");
        // Shift with a covering range deselects it too.
        let keys = reset(&editor, |keys| select_range(keys, (4, 1), (5, 1)));
        click(&editor, &keys[1], true);
        assert_eq!(snapshot(&editor, &keys).1, "none");
    }
}
