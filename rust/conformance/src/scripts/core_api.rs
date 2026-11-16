//! What `scripts/core-api` adds to `ops.ts`: `TestTextNode` and its
//! `selectionTransform` log, `textReplacement`, `$getters` and the
//! `console.warn` log (`withWarnings`).

use super::{Ctx, err};
use lexical::json::Value;
use lexical::nodes::text::TextNode;
use lexical::{
    LexicalError, LexicalNode, LexicalNodeReplacement, NodeBase, NodeClass, NodeRef, SelectionRef,
    TextData, Which,
};
use std::cell::RefCell;
use std::rc::{Rc, Weak};

thread_local! {
    /// `selectionTransformCtx`: the session whose `TestTextNode`s log
    /// their `selectionTransform` (the hook gets no context of the
    /// session's).
    static SELECTION_TRANSFORM_CTX: RefCell<Weak<Ctx>> = const { RefCell::new(Weak::new()) };
}

/// Makes `ctx` the session `TestTextNode`s log to.
pub(super) fn set_selection_transform_ctx(ctx: &Rc<Ctx>) {
    SELECTION_TRANSFORM_CTX.with(|c| *c.borrow_mut() = Rc::downgrade(ctx));
}

/// `rangeJson(selection)`: `[anchorKey, anchorOffset, focusKey,
/// focusOffset]` of a range selection, else `null`.
fn range_json(selection: Option<&SelectionRef>) -> Value {
    let Some(selection) = selection.filter(|s| s.is_range()) else {
        return Value::Null;
    };
    let mut out = Vec::new();
    for which in [Which::Anchor, Which::Focus] {
        let point = selection.point(which).expect("a range selection");
        out.push(point.key.as_str().into());
        out.push((point.offset as f64).into());
    }
    Value::Array(out)
}

/// `TestTextNode` (`test-text`): a `TextNode` whose `selectionTransform`
/// logs `['selectionTransform', key, prev, next]`, the selections as
/// `rangeJson`.
#[derive(Clone, Debug, Default)]
pub(crate) struct TestTextNode {
    base: NodeBase,
    text: TextData,
}

impl LexicalNode for TestTextNode {
    fn get_type(&self) -> &'static str {
        "test-text"
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
    fn selection_transform(
        &self,
        this: &NodeRef,
        prev_selection: Option<&SelectionRef>,
        next_selection: &SelectionRef,
    ) -> Result<(), LexicalError> {
        if let Some(ctx) = SELECTION_TRANSFORM_CTX.with(|c| c.borrow().upgrade()) {
            ctx.log(vec![
                "selectionTransform".into(),
                this.get_key().into(),
                range_json(prev_selection),
                range_json(Some(next_selection)),
            ]);
        }
        Ok(())
    }
}

/// `TestTextNode`'s class.
pub(crate) fn test_text_node() -> NodeClass {
    NodeClass::of::<TestTextNode>()
}

/// `textReplacement(legacy)`: `{replace: TextNode, with, withKlass:
/// TestTextNode}`, `with` making a `TestTextNode` with the node's text;
/// without `withKlass` when `legacy`.
pub(crate) fn text_replacement(legacy: bool) -> NodeClass {
    LexicalNodeReplacement {
        replace: NodeClass::of::<TextNode>(),
        with: Rc::new(|node: &NodeRef| {
            let replacement = lexical::create::<TestTextNode>();
            replacement.set_text_content(&node.get_text_content());
            Ok(replacement)
        }),
        with_klass: (!legacy).then(test_text_node),
    }
    .into()
}

/// `attempt(f)`: `f()`, or `'<throws>'` when it fails.
fn attempt(f: impl FnOnce() -> Result<Value, LexicalError>) -> Value {
    f().unwrap_or_else(|_| "<throws>".into())
}

fn key_of(node: NodeRef) -> Value {
    node.get_key().into()
}

fn keys_json(keys: Vec<String>) -> Value {
    Value::Array(keys.into_iter().map(Value::from).collect())
}

fn entry(name: &str, value: Value) -> Value {
    Value::Array(vec![name.into(), value])
}

/// `$getters(node)`: the node's getters as `[name, value]` entries, as
/// `ops.ts` lists them.
pub(super) fn getters(node: &NodeRef) -> Value {
    let mut entries = vec![
        entry(
            "topLevelElement",
            attempt(|| Ok(node.get_top_level_element()?.map_or(Value::Null, key_of))),
        ),
        entry(
            "topLevelElementOrThrow",
            attempt(|| node.get_top_level_element_or_throw().map(key_of)),
        ),
        entry("parentKeys", keys_json(node.get_parent_keys())),
    ];
    if node.is_element() {
        entries.extend([
            entry("childrenKeys", keys_json(node.get_children_keys())),
            entry("isLastChild", attempt(|| Ok(node.is_last_child()?.into()))),
            entry(
                "firstChildOrThrow",
                attempt(|| node.get_first_child_or_throw().map(key_of)),
            ),
            entry(
                "lastChildOrThrow",
                attempt(|| node.get_last_child_or_throw().map(key_of)),
            ),
            entry("hasTextFormat", node.has_text_format("bold").into()),
            entry(
                "formatFlags",
                (node.get_format_flags("bold", None) as f64).into(),
            ),
            entry("canReplaceWith", node.can_replace_with(node).into()),
            entry("canInsertAfter", node.can_insert_after(node).into()),
        ]);
    }
    if node.is_text() {
        entries.extend([
            entry(
                "formatFlags",
                (node.get_format_flags("bold", None) as f64).into(),
            ),
            entry(
                "alignedFormatFlags",
                (node.get_format_flags("italic", Some(2)) as f64).into(),
            ),
            entry("isDirectionless", node.is_directionless().into()),
        ]);
    }
    Value::Array(entries)
}

/// `withWarnings(ctx, on, f)`: `f`, with each warning logged as `['warn',
/// message]` when `on`.
pub(super) fn with_warnings<R>(ctx: &Rc<Ctx>, on: bool, f: impl FnOnce() -> R) -> R {
    if !on {
        return f();
    }
    let log = ctx.clone();
    lexical::with_warning_handler(
        move |message| log.log(vec!["warn".into(), message.into()]),
        f,
    )
}

/// `KNOWN_CLASSES`: the types `logHasNodes` may ask about.
pub(super) fn known_class(node_type: &str) -> Result<&str, LexicalError> {
    match node_type {
        "heading" | "link" | "paragraph" | "quote" | "test-text" | "text" => Ok(node_type),
        other => Err(err(format!("unknown class {other}"))),
    }
}
