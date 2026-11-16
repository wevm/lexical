//! Ports of `packages/lexical-mark/src/__tests__/unit/MarkStateExtension.test.ts`,
//! split by `describe` group as Swift's port is: the helpers and test
//! nodes are here.
//!
//! Upstream mounts some editors on a contenteditable root to read the DOM
//! the reconciler made, or to dispatch DOM events at it. Rust's reconciler
//! renders no DOM and the renderer half of `MarkStateExtension.ts`
//! (`$decorateMarkIDs`, `isMarkIDShown`) isn't ported (rust/PARITY.md), so
//! the "DOM" and "isMarkIDShown" groups are skipped (rust/parity/skips.json).
//! The other `withRoot` editors are mounted on a no-op root element, which
//! runs the reconciler's model half. A paste event dispatched at the root
//! element dispatches `PASTE_COMMAND` outside an update, so the "paste
//! event" cases dispatch the command outside an update, as that event does
//! (Swift's port does the same).

use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, get_extension_dependency_from_editor,
};
use crate::json::Value;
use crate::lexical_utils::set_node_key;
use crate::mark::{
    MarkStateConfigOverrides, MarkStateOutput, MarkableNodeMatch, add_mark_id,
    default_markable_nodes, get_node_mark_ids, mark_extension, mark_state_extension,
    wrap_selection_in_mark_node,
};
use crate::nodes::element::ElementData;
use crate::nodes::text::TextData;
use crate::upstream_tests::support::set_test_root_element;
use crate::{
    AnyLexicalExtensionArgument, CONTROLLED_TEXT_INSERTION_COMMAND, ClipboardEvent, DataTransfer,
    Editor, InitialEditorStateType, LexicalError, LexicalExtension, LexicalNode, NodeBase,
    NodeClass, NodeClone, NodeRef, PasteCommandType, PointType, SelectionRef, UpdateOptions, Which,
    config_extension, create_paragraph_node, create_range_selection, create_text_node,
    define_extension, get_node_by_key, get_root, get_slot, set_selection,
};
use std::collections::BTreeMap;
use std::rc::Rc;

mod compatibility;
mod composition;
mod edges;
mod inserted_text;
mod output;
mod replaced_text;

pub(super) const P: &str = "Revenue grew 4% in Q3.";

/// `ChipNode`: an inline decorator whose text is `@`.
#[derive(Clone, Debug, Default)]
pub(super) struct ChipNode {
    base: NodeBase,
}

impl LexicalNode for ChipNode {
    fn get_type(&self) -> &'static str {
        "test-chip"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn is_decorator(&self) -> bool {
        true
    }
    fn is_inline(&self) -> bool {
        true
    }
    fn get_text_content(&self) -> String {
        "@".to_owned()
    }
}

/// `FigureNode`: a block decorator that hosts a `caption` slot.
#[derive(Clone, Debug, Default)]
pub(super) struct FigureNode {
    base: NodeBase,
}

impl LexicalNode for FigureNode {
    fn get_type(&self) -> &'static str {
        "test-figure"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn is_decorator(&self) -> bool {
        true
    }
    fn is_inline(&self) -> bool {
        false
    }
}

/// `TitledParagraphNode`: a paragraph with a named slot.
#[derive(Clone, Debug, Default)]
pub(super) struct TitledParagraphNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for TitledParagraphNode {
    fn static_node_config_chain(&self) -> Vec<crate::structure::NodeConfigLink> {
        crate::structure::inline_holder_chain(self.get_type())
    }
    fn get_type(&self) -> &'static str {
        "test-titled-paragraph"
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
    fn declared_slots(&self) -> &'static [&'static str] {
        &["title"]
    }
}

/// `EmptyInlineDecoratorNode`: an inline decorator without text.
#[derive(Clone, Debug, Default)]
pub(super) struct EmptyInlineDecoratorNode {
    base: NodeBase,
}

impl LexicalNode for EmptyInlineDecoratorNode {
    fn get_type(&self) -> &'static str {
        "test-empty-inline-decorator"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn is_decorator(&self) -> bool {
        true
    }
    fn is_inline(&self) -> bool {
        true
    }
}

/// `BracketedInlineNode`: an inline element that shows its text in
/// brackets. JS's `getTextContentSize` is then `LexicalNode`'s, the length
/// of that text.
#[derive(Clone, Debug, Default)]
pub(super) struct BracketedInlineNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for BracketedInlineNode {
    fn get_type(&self) -> &'static str {
        "test-bracketed-inline"
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
    fn is_inline(&self) -> bool {
        true
    }
    fn get_text_content(&self) -> String {
        format!(
            "[{}]",
            crate::nodes::element::get_text_content(self.as_node())
        )
    }
    fn get_text_content_size(&self) -> usize {
        crate::utf16::len(&self.get_text_content())
    }
}

/// `TestDecoratorTextNode`: a `DecoratorTextNode` whose text is `#`.
#[derive(Clone, Debug, Default)]
pub(super) struct TestDecoratorTextNode {
    inner: crate::extension::DecoratorTextNode,
}

impl LexicalNode for TestDecoratorTextNode {
    fn get_type(&self) -> &'static str {
        "test-decorator-text"
    }
    fn super_types(&self) -> Vec<&'static str> {
        vec!["decorator-text"]
    }
    fn base(&self) -> &NodeBase {
        self.inner.base()
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        self.inner.base_mut()
    }
    fn is_decorator(&self) -> bool {
        true
    }
    fn is_inline(&self) -> bool {
        true
    }
    fn state_configs(&self) -> Vec<crate::RequiredNodeStateConfig> {
        self.inner.state_configs()
    }
    fn is_inline_formattable(&self) -> bool {
        true
    }
    fn get_text_content(&self) -> String {
        "#".to_owned()
    }
}

/// `MentionNode`: a token TextNode that a transform creates from text.
#[derive(Clone, Debug, Default)]
pub(super) struct MentionNode {
    base: NodeBase,
    text: TextData,
}

impl LexicalNode for MentionNode {
    fn get_type(&self) -> &'static str {
        "test-mention"
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
}

/// `ReplacementTextNode`.
#[derive(Clone, Debug, Default)]
pub(super) struct ReplacementTextNode {
    base: NodeBase,
    text: TextData,
}

impl LexicalNode for ReplacementTextNode {
    fn get_type(&self) -> &'static str {
        "test-replacement-text"
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
}

/// `new ChipNode()`.
pub(super) fn create_chip_node() -> NodeRef {
    set_node_key(Box::new(ChipNode::default()))
}

/// `$createMentionNode(text)`.
pub(super) fn create_mention_node(text: &str) -> NodeRef {
    let node = crate::create::<MentionNode>();
    node.set_text_content(text);
    node.set_mode("token").unwrap();
    node
}

/// `JSON.stringify(text)`.
pub(super) fn quoted(text: &str) -> String {
    Value::from(text).to_string()
}

/// `$describeNode(node)`.
pub(super) fn describe_node(node: &NodeRef) -> String {
    let ids = get_node_mark_ids(node).unwrap();
    let tag = if ids.is_empty() {
        String::new()
    } else {
        format!("[{}]", ids.join(","))
    };
    let children = |node: &NodeRef| {
        node.get_children()
            .iter()
            .map(describe_node)
            .collect::<Vec<_>>()
            .join(" ")
    };
    if node.is_text() {
        return quoted(&node.get_text_content()) + &tag;
    }
    if node.get_type() == "linebreak" {
        return format!("br{tag}");
    }
    if crate::link::is_link_node(node) {
        return format!("link({})", children(node));
    }
    if let Some(mark_ids) = node.get_ids() {
        return format!("mark[{}]({})", mark_ids.join(","), children(node));
    }
    if node.is_element() {
        return format!("{}({})", node.get_type(), children(node));
    }
    if node.get_latest().is::<FigureNode>() {
        let caption = get_slot(node, "caption");
        return format!(
            "{}{tag}{{{}}}",
            node.get_type(),
            caption.as_ref().map(describe_node).unwrap_or_default()
        );
    }
    format!("{}{tag}", node.get_type())
}

/// `$tree()`.
pub(super) fn tree_string() -> String {
    get_root()
        .get_children()
        .iter()
        .map(describe_node)
        .collect::<Vec<_>>()
        .join(" / ")
}

/// `$paragraph(index)`.
pub(super) fn paragraph_at(index: usize) -> NodeRef {
    get_root().get_child_at_index(index).unwrap()
}

/// `$paragraph()`.
pub(super) fn paragraph() -> NodeRef {
    paragraph_at(0)
}

/// `$locate(paragraph, offset, preferNext)`: the text node and offset at a
/// character offset of a paragraph.
pub(super) fn locate(paragraph: &NodeRef, offset: usize, prefer_next: bool) -> (NodeRef, usize) {
    let mut acc = 0;
    let texts = paragraph.get_all_text_nodes();
    for (i, node) in texts.iter().enumerate() {
        let size = node.get_text_content_size();
        if offset < acc + size || (offset == acc + size && (!prefer_next || i == texts.len() - 1)) {
            return (node.clone(), offset - acc);
        }
        acc += size;
    }
    panic!("offset {offset} out of range");
}

/// `$select(start, end, paragraph)`.
pub(super) fn select_in(start: usize, end: usize, paragraph: &NodeRef) -> SelectionRef {
    let (start_node, start_offset) = locate(paragraph, start, start != end);
    let (end_node, end_offset) = locate(paragraph, end, false);
    let selection = create_range_selection();
    selection
        .point_set(
            Which::Anchor,
            start_node.get_key(),
            start_offset,
            PointType::Text,
        )
        .unwrap();
    selection
        .point_set(
            Which::Focus,
            end_node.get_key(),
            end_offset,
            PointType::Text,
        )
        .unwrap();
    set_selection(Some(selection.clone()));
    selection
}

/// `$select(start, end)`.
pub(super) fn select(start: usize, end: usize) -> SelectionRef {
    select_in(start, end, &paragraph())
}

/// `$select(at)`.
pub(super) fn select_at(at: usize) -> SelectionRef {
    select(at, at)
}

/// `$init(text)`.
pub(super) fn init_with(text: &str) -> Result<(), LexicalError> {
    let root = get_root();
    root.clear()?;
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node(text)])?;
    root.append(&[paragraph])
}

/// `$init()`.
pub(super) fn init() -> Result<(), LexicalError> {
    init_with(P)
}

/// The options of `createTestEditor(options)` (`isMarkIDShown` isn't
/// ported).
#[derive(Default)]
pub(super) struct Options {
    pub disabled: bool,
    pub export_ids_to_html: bool,
    pub markable_nodes: Option<Vec<MarkableNodeMatch>>,
    pub with_root: bool,
}

/// `createTestEditor(options)`.
pub(super) fn create_test_editor_with(options: Options) -> LexicalEditorWithDispose {
    let theme = crate::json::parse(r#"{"mark":"mark","markOverlap":"mark-overlap"}"#)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: vec![
            crate::rich_text::rich_text_extension().into(),
            crate::link::link_extension().into(),
            mark_extension().into(),
            crate::html::import::core_import_extension().into(),
            config_extension(
                &mark_state_extension(),
                [MarkStateConfigOverrides {
                    disabled: Some(options.disabled),
                    export_ids_to_html: Some(options.export_ids_to_html),
                    markable_nodes: Some(
                        options
                            .markable_nodes
                            .unwrap_or_else(default_markable_nodes),
                    ),
                }],
            ),
        ],
        nodes: vec![
            NodeClass::of::<ChipNode>(),
            NodeClass::of::<FigureNode>(),
            NodeClass::of::<BracketedInlineNode>(),
            NodeClass::of::<EmptyInlineDecoratorNode>(),
            NodeClass::of::<TestDecoratorTextNode>(),
            NodeClass::of::<TitledParagraphNode>(),
        ],
        theme: Some(theme),
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
    .unwrap();
    if options.with_root {
        set_test_root_element(&editor);
    }
    editor
}

/// `createTestEditor()`.
pub(super) fn create_test_editor() -> LexicalEditorWithDispose {
    create_test_editor_with(Options::default())
}

/// An editor of `[RichTextExtension, HashtagExtension?, MarkExtension,
/// MarkStateExtension?]` (MarkStateExtension unless `with_mark_node`), with
/// `nodes`.
pub(super) fn editor_with(
    with_mark_node: bool,
    hashtag: bool,
    nodes: Vec<NodeClass>,
) -> LexicalEditorWithDispose {
    let mut dependencies: Vec<AnyLexicalExtensionArgument> =
        vec![crate::rich_text::rich_text_extension().into()];
    if hashtag {
        dependencies.push(crate::hashtag::hashtag_extension().into());
    }
    dependencies.push(mark_extension().into());
    if !with_mark_node {
        dependencies.push(mark_state_extension().into());
    }
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies,
        nodes,
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
    .unwrap()
}

/// Marks `selection` with `c1`: a MarkNode (`$wrapSelectionInMarkNode`) or
/// MarkState ids (`$addMarkID`).
pub(super) fn mark_selection(selection: &SelectionRef, with_mark_node: bool) {
    if with_mark_node {
        wrap_selection_in_mark_node(selection, false, "c1", None).unwrap();
    } else {
        add_mark_id("c1", selection).unwrap();
    }
}

/// `editor.update(fn, {discrete: true})`.
pub(super) fn update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    editor
        .update(
            f,
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

/// `update(editor, fn)` for a closure that returns nothing.
pub(super) fn run(editor: &Editor, f: impl FnOnce() + 'static) {
    update(editor, move || {
        f();
        Ok(())
    });
}

pub(super) fn read<R>(editor: &Editor, f: impl FnOnce() -> R) -> R {
    editor.read(f).unwrap()
}

/// `tree(editor)`.
pub(super) fn tree(editor: &Editor) -> String {
    read(editor, tree_string)
}

/// `mark(editor, start, end, id)`.
pub(super) fn mark(editor: &Editor, start: usize, end: usize, id: &str) {
    let id = id.to_owned();
    run(editor, move || {
        add_mark_id(&id, &select(start, end)).unwrap();
    });
}

pub(super) fn output(editor: &Editor) -> Rc<MarkStateOutput> {
    get_extension_dependency_from_editor(editor, &mark_state_extension())
        .unwrap()
        .output
        .unwrap()
}

/// `markIDNodeKeys(editor).size`.
pub(super) fn mark_id_node_keys_size(editor: &Editor) -> usize {
    output(editor).mark_id_node_keys.value().len()
}

/// `registryText(editor)`: id -> text of the nodes that carry it, from the
/// markIDNodeKeys output, sorted by UTF-16 code unit.
pub(super) fn registry_text(editor: &Editor) -> BTreeMap<String, Vec<String>> {
    let out = output(editor);
    read(editor, move || {
        let map = out.mark_id_node_keys.value();
        map.iter()
            .map(|(id, keys)| {
                let mut texts: Vec<String> = keys
                    .iter()
                    .map(|key| {
                        get_node_by_key(key)
                            .map_or_else(|| "<missing>".to_owned(), |n| n.get_text_content())
                    })
                    .collect();
                texts.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
                (id.to_owned(), texts)
            })
            .collect()
    })
}

/// `{c1: [...], ...}` for [`registry_text`].
pub(super) fn registry(entries: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
    entries
        .iter()
        .map(|(id, texts)| {
            (
                (*id).to_owned(),
                texts.iter().map(|t| (*t).to_owned()).collect(),
            )
        })
        .collect()
}

/// `typeText(editor, text)`.
pub(super) fn type_text(editor: &Editor, text: &str) {
    let ed = editor.clone();
    let text = text.to_owned();
    update(editor, move || {
        ed.dispatch_command(&CONTROLLED_TEXT_INSERTION_COMMAND, text)?;
        Ok(())
    });
}

/// `dataTransfer(data)`: clipboard or drag data by MIME type.
pub(super) fn data_transfer(data: &[(&str, &str)]) -> Rc<DataTransfer> {
    let transfer = DataTransfer::new();
    for (mime, value) in data {
        transfer.set_data(mime, value);
    }
    Rc::new(transfer)
}

/// `pasteEvent(text, lexical, html)`.
pub(super) fn paste_event(
    text: &str,
    lexical: Option<&str>,
    html: Option<&str>,
) -> PasteCommandType {
    let mut data = vec![("text/plain", text)];
    if let Some(lexical) = lexical {
        data.push(("application/x-lexical-editor", lexical));
    }
    if let Some(html) = html {
        data.push(("text/html", html));
    }
    PasteCommandType::Clipboard(Rc::new(ClipboardEvent::new(
        "paste",
        Some(data_transfer(&data)),
    )))
}

/// `$getNodeMarkIDs` of the first text node whose text includes `text`.
pub(super) fn node_mark_ids_of_text(text: &str) -> Vec<String> {
    let node = get_root()
        .get_all_text_nodes()
        .into_iter()
        .find(|n| n.get_text_content().contains(text))
        .unwrap_or_else(|| panic!("no text node with {text:?}"));
    get_node_mark_ids(&node).unwrap()
}

/// The `$` record of each child of the first paragraph in the JSON.
pub(super) fn paragraph_child_states(editor: &Editor) -> Vec<Option<Value>> {
    let json = editor.get_editor_state().to_json(false);
    json.get("root")
        .and_then(|r| r.get("children"))
        .and_then(|c| c.as_array())
        .and_then(|c| c.first())
        .and_then(|p| p.get("children"))
        .and_then(|c| c.as_array())
        .map(|children| children.iter().map(|c| c.get("$").cloned()).collect())
        .unwrap_or_default()
}

/// `{markIds: ids}`.
pub(super) fn mark_ids_state(ids: &[&str]) -> Option<Value> {
    Some(
        crate::json::parse(&format!(
            r#"{{"markIds":{}}}"#,
            Value::from(ids.iter().map(|id| Value::from(*id)).collect::<Vec<_>>())
        ))
        .unwrap(),
    )
}

/// Dispatches `COMPOSITION_START_COMMAND`, inside an update, then does what
/// JS's `$handleCompositionStart` does on jsdom and the port's core leaves
/// out (its empty space heuristic, rust/PARITY.md, the core's event
/// handlers): where the composition can't write into the caret's TextNode
/// as it is (an element point, a selection, a toggled format or style, a
/// token or segmented node, an edge that takes no text), it inserts
/// `COMPOSITION_START_CHAR` (jsdom's, a zero-width space) through
/// `CONTROLLED_TEXT_INSERTION_COMMAND` and composes in the node that holds
/// it. As Swift's port does.
pub(super) fn dispatch_composition_start(editor: &Editor) -> Result<(), LexicalError> {
    let was_composing = editor.is_composing();
    editor.dispatch_command(
        &crate::COMPOSITION_START_COMMAND,
        Rc::new(crate::CompositionEvent::new("compositionstart", "")),
    )?;
    if was_composing {
        return Ok(());
    }
    let Some(range) = crate::get_selection().and_then(|s| s.range()) else {
        return Ok(());
    };
    let anchor = &range.anchor;
    let mut prepare = anchor.point_type == PointType::Element || !range.is_collapsed();
    if !prepare {
        let node = anchor.get_node()?;
        if node.is_text() {
            let text = node.text_data();
            prepare = text.get_format() != range.format
                || text.get_style() != range.style
                || node.is_token()
                || node.is_segmented()
                || (anchor.offset == 0 && !node.can_insert_text_before())
                || (anchor.offset == node.get_text_content_size() && !node.can_insert_text_after());
        }
    }
    if prepare {
        editor.dispatch_command(&CONTROLLED_TEXT_INSERTION_COMMAND, "\u{200B}".to_owned())?;
        if let Some(updated) = crate::get_selection().and_then(|s| s.range()) {
            crate::set_composition_key(Some(&updated.anchor.key));
        }
    }
    Ok(())
}
