//! `@lexical/clipboard` (the `clipboard` feature): the model half of
//! `packages/lexical-clipboard/src/clipboard.ts`, with the default import
//! stack of `ClipboardImportExtension.ts` that
//! `$insertDataTransferForRichText` runs.
//!
//! Copying serializes the selection as Lexical JSON
//! ([`generate_json_from_selected_nodes`], [`get_lexical_content`]) and
//! text ([`get_clipboard_data_from_selection`]) into a [`DataTransfer`]
//! ([`set_lexical_clipboard_data_transfer`]); pasting parses it back
//! ([`generate_nodes_from_serialized_nodes`], [`insert_generated_nodes`])
//! or inserts its text ([`insert_data_transfer_for_rich_text`],
//! [`insert_data_transfer_for_plain_text`]).
//!
//! With the `html` feature, the selection is copied as HTML too
//! ([`get_html_content`]) and pasted HTML is imported. With `extension`,
//! [`get_clipboard_data_extension`] and [`clipboard_import_extension`]
//! (and, with both, [`clipboard_dom_import_extension`]) configure the
//! stacks of each MIME type that copy and paste run.
//!
//! Not ported (rust/PARITY.md): drag and drop (`$handleRichTextDrop`,
//! `$handlePlainTextDrop`, `$writeDragSourceToDataTransfer`,
//! `caretFromPoint`), which needs a DOM, and `copyToClipboard`'s event and
//! timer plumbing.

mod clipboard_import_extension;

#[cfg(all(feature = "extension", feature = "html"))]
pub use clipboard_import_extension::clipboard_dom_import_extension;
pub use clipboard_import_extension::{
    ClipboardImportConfig, ClipboardImportOutput, ImportMimeTypeConfig, ImportMimeTypeFunction,
    ImportMimeTypeNext, ImportMimeTypePriority, default_import_mime_type,
    default_import_mime_type_priority, get_import_output,
};
#[cfg(feature = "extension")]
pub use clipboard_import_extension::{
    ClipboardImportConfigOverrides, ClipboardImportExtensionType, clipboard_import_extension,
};

use crate::commands::{InsertClipboardNodesPayload, SELECTION_INSERT_CLIPBOARD_NODES_COMMAND};
use crate::events::DataTransfer;
use crate::json::{self, Object, Value};
use crate::lexical_selection::SelectionRef;
#[cfg(feature = "rich-text")]
use crate::lexical_selection::get_selection;
use crate::lexical_utils::get_root;
use crate::node::NodeRef;
use crate::selection::slice_selected_text_node_content;
use crate::serialized_export::export_node_json;
use crate::updates::parse_serialized_node;
use crate::{
    CaretDirection, Editor, LexicalError, caret_from_point, get_caret_range, get_child_caret,
};
use std::rc::Rc;

/// The `application/x-lexical-editor` MIME type: a clipboard payload of
/// Lexical JSON.
pub const LEXICAL_MIME_TYPE: &str = "application/x-lexical-editor";

/// `LexicalClipboardData`: the serialized selection by MIME type, in the
/// order JS sets them (`text/plain` always, then the others it has).
pub type LexicalClipboardData = Vec<(String, String)>;

/// The result of `$generateJSONFromSelectedNodes`: the editor's namespace
/// and the serialized nodes (`BaseSerializedNode`s).
#[derive(Clone, Debug)]
pub struct SerializedSelectedNodes {
    pub namespace: String,
    pub nodes: Vec<Object>,
}

impl SerializedSelectedNodes {
    /// `{namespace, nodes}`, in that key order.
    pub fn to_json(&self) -> Value {
        let mut json = Object::new();
        json.insert("namespace", self.namespace.as_str().into());
        json.insert(
            "nodes",
            Value::Array(self.nodes.iter().cloned().map(Value::from).collect()),
        );
        json.into()
    }
}

/// Whether nothing is selected: a collapsed range selection, or one with
/// no nodes.
fn is_nothing_selected(selection: &SelectionRef) -> Result<bool, LexicalError> {
    Ok(selection.range().is_some_and(|r| r.is_collapsed()) || selection.get_nodes()?.is_empty())
}

/// `$getLexicalContent(editor, selection)`: the Lexical JSON of the nodes
/// in `selection` as a string, relying on each node's `exportJSON`; `None`
/// when nothing is selected. JS's `selection` defaults to
/// `$getSelection()`; pass it here.
pub fn get_lexical_content(
    editor: &Editor,
    selection: Option<&SelectionRef>,
) -> Result<Option<String>, LexicalError> {
    let Some(selection) = selection else {
        return Err(LexicalError::new("Expected valid LexicalSelection"));
    };
    // If we haven't selected anything
    if is_nothing_selected(selection)? {
        return Ok(None);
    }
    let json = generate_json_from_selected_nodes(editor, Some(selection))?.to_json();
    Ok(json::stringify(&json))
}

/// `$insertDataTransferForPlainText(dataTransfer, selection)`: inserts the
/// `text/plain` data (or, when that is empty, the `text/uri-list` data) at
/// `selection` with `insertRawText`.
pub fn insert_data_transfer_for_plain_text(
    data_transfer: &DataTransfer,
    selection: &SelectionRef,
) -> Result<(), LexicalError> {
    let mut text = data_transfer.get_data("text/plain");
    if text.is_empty() {
        text = data_transfer.get_data("text/uri-list");
    }
    selection.insert_raw_text(&text)
}

/// `$insertDataTransferForRichText(dataTransfer, selection, editor)`:
/// inserts the contents of `data_transfer` at `selection` with the
/// rich-text import pipeline, `$getImportOutput().$insertDataTransfer`:
/// the stacks of the editor's `ClipboardImportExtension`, or the defaults
/// (`application/x-lexical-editor`, Lexical JSON in the editor's namespace;
/// `text/html` with the `html` feature, unless it is the plain text
/// verbatim, as iOS Safari's autocorrect pastes; `text/plain`;
/// `text/uri-list`), the first type with data whose stack takes it.
///
/// Every payload type leaves the editor's selection after the inserted
/// content, so `selection` must be a live selection this update may write
/// to. JS's `editor` is unused, and so is this one (the importers use the
/// active editor).
pub fn insert_data_transfer_for_rich_text(
    data_transfer: &DataTransfer,
    selection: &SelectionRef,
    _editor: &Editor,
) -> Result<(), LexicalError> {
    get_import_output()?
        .insert_data_transfer(data_transfer, selection)
        .map(|_| ())
}

/// `$insertGeneratedNodes(editor, nodes, selection)`: inserts `nodes` at
/// `selection`, unless a `SELECTION_INSERT_CLIPBOARD_NODES_COMMAND` handler
/// takes them, and then gives a collapsed range selection the format and
/// style of the text before it.
pub fn insert_generated_nodes(
    editor: &Editor,
    nodes: Vec<NodeRef>,
    selection: &SelectionRef,
) -> Result<(), LexicalError> {
    let payload = InsertClipboardNodesPayload {
        nodes: nodes.clone(),
        selection: selection.clone(),
    };
    if !editor.dispatch_command(&SELECTION_INSERT_CLIPBOARD_NODES_COMMAND, payload)? {
        selection.insert_nodes(nodes)?;
        update_selection_on_insert(selection)?;
    }
    Ok(())
}

/// `$updateSelectionOnInsert(selection)`: for a collapsed range selection,
/// the format and style of the text node before it (the anchor's, or the
/// last one before it in its block).
fn update_selection_on_insert(selection: &SelectionRef) -> Result<(), LexicalError> {
    let Some(range) = selection.range().filter(|r| r.is_collapsed()) else {
        return Ok(());
    };
    let mut node_to_inspect: Option<NodeRef> = None;
    let anchor_caret = caret_from_point(&range.anchor, CaretDirection::Previous)?;
    if anchor_caret.is_text_point_caret() {
        node_to_inspect = Some(anchor_caret.origin().clone());
    } else {
        let root_end = get_child_caret(&get_root(), CaretDirection::Next)
            .expect("the root is an element")
            .get_flipped()?;
        for caret in get_caret_range(anchor_caret, root_end)?.iter() {
            let origin = caret.origin();
            if origin.is_text() {
                node_to_inspect = Some(origin.clone());
                break;
            } else if origin.is_element() && !origin.is_inline() {
                break;
            }
        }
    }
    if let Some(text) = node_to_inspect.filter(NodeRef::is_text) {
        let data = text.text_data();
        selection.with_range(|r| {
            if r.format != data.format || r.style != data.style {
                r.format = data.format;
                r.style = data.style.clone();
                r.dirty = true;
            }
        });
    }
    Ok(())
}

/// `$appendNodesToJSON(editor, selection, currentNode, targetArray)`:
/// pushes the JSON of `current_node` to `target_array` if it is included
/// (selected, or taken by a child, `extractWithChild`) and not excluded
/// (`excludeFromCopy("clone")`), with the selected text of a text node at
/// the selection's edge; otherwise its children's JSON. Returns whether it
/// is included. An included host carries its slots whole (`$slots`), as
/// a range never selects part of one. (JS's `editor` argument is unused,
/// and left out.)
fn append_nodes_to_json(
    selection: Option<&SelectionRef>,
    current_node: &NodeRef,
    target_array: &mut Vec<Object>,
) -> Result<bool, LexicalError> {
    let mut should_include = match selection {
        Some(selection) => current_node.is_selected(Some(selection))?,
        None => true,
    };
    // 'clone', not 'html': this builds the internal
    // `application/x-lexical-editor` payload.
    let should_exclude = current_node.is_element() && current_node.exclude_from_copy("clone");
    let target = match selection {
        Some(selection) if current_node.is_text() => {
            slice_selected_text_node_content(selection, current_node)?
        }
        _ => current_node.get_latest(),
    };
    // Route through the shared export so a selection honors the same form
    // as `editorState.toJSON()`.
    let mut serialized_node = export_node_json(&*target);
    let children = if current_node.is_element() {
        current_node.get_children()
    } else {
        Vec::new()
    };
    // JS pushes into the serialized node's own children array.
    let mut child_target: Vec<Object> = serialized_node
        .get("children")
        .and_then(Value::as_array)
        .map(|children| {
            children
                .iter()
                .filter_map(|child| child.as_object().cloned())
                .collect()
        })
        .unwrap_or_default();
    if let Some(text) = target.text()
        && text.text.is_empty()
    {
        // If an uncollapsed selection ends or starts at the end of a line of
        // specialized text nodes, such as code tokens, we get a 'blank' text
        // node here. We don't want it.
        should_include = false;
    }
    // An element host in a node selection recurses into its children with
    // no selection, so the whole subtree serializes. Only a whole-host node
    // selection promotes; a partial range selection keeps slicing.
    let child_selection = if should_include
        && selection.is_some_and(SelectionRef::is_node)
        && current_node.is_element()
    {
        None
    } else {
        selection
    };
    for child_node in &children {
        let should_include_child =
            append_nodes_to_json(child_selection, child_node, &mut child_target)?;
        if !should_include
            && current_node.is_element()
            && should_include_child
            && current_node.extract_with_child(child_node, selection, "clone")?
        {
            should_include = true;
        }
    }
    if should_include && !should_exclude {
        let slot_names = crate::slot::get_slot_names(current_node);
        if !slot_names.is_empty() {
            let mut serialized_slots = Object::new();
            for name in slot_names {
                let Some(slot_node) = crate::slot::get_slot(current_node, &name) else {
                    return Err(LexicalError::new(format!(
                        "LexicalNode: Node {} has slot \"{name}\" but it resolved to no node during export.",
                        current_node.get_type()
                    )));
                };
                let mut slot_array = Vec::new();
                append_nodes_to_json(None, &slot_node, &mut slot_array)?;
                // A whole-slot export must give exactly the slot value node.
                if slot_array.len() != 1
                    || (slot_node.is_element() && slot_node.exclude_from_copy("clone"))
                {
                    return Err(LexicalError::new(format!(
                        "LexicalNode: slot \"{name}\" on {} did not serialize to exactly the slot value node (got {} nodes); a slot value must not be excluded from copy.",
                        current_node.get_type(),
                        slot_array.len()
                    )));
                }
                serialized_slots.insert(&name, Value::Object(slot_array.remove(0)));
            }
            serialized_node.insert("$slots", Value::Object(serialized_slots));
        }
    }
    if should_include && !should_exclude {
        if serialized_node.contains_key("children") {
            serialized_node.insert(
                "children",
                Value::Array(child_target.into_iter().map(Value::from).collect()),
            );
        }
        target_array.push(serialized_node);
    } else {
        // Splice up whatever the recursion collected in this node's place.
        target_array.extend(child_target);
    }
    Ok(should_include)
}

/// `$generateJSONFromSelectedNodes(editor, selection)`: the Lexical JSON
/// of the nodes inside `selection` (of every node for `None`), with the
/// editor's namespace. The walk starts at the selection's slot frame, the
/// root outside slots: a selection in a slot never holds its host.
pub fn generate_json_from_selected_nodes(
    editor: &Editor,
    selection: Option<&SelectionRef>,
) -> Result<SerializedSelectedNodes, LexicalError> {
    let mut nodes = Vec::new();
    let slot_frame = crate::slot::get_selection_slot_frame(selection)?;
    let walk_root = slot_frame
        .filter(NodeRef::is_element)
        .unwrap_or_else(get_root);
    for top_level_node in walk_root.get_children() {
        append_nodes_to_json(selection, &top_level_node, &mut nodes)?;
    }
    Ok(SerializedSelectedNodes {
        namespace: editor.get_namespace().to_owned(),
        nodes,
    })
}

/// `$generateNodesFromSerializedNodes(serializedNodes)`: the nodes of a
/// clipboard payload, of the classes registered on the active editor.
pub fn generate_nodes_from_serialized_nodes(
    serialized_nodes: &[Value],
) -> Result<Vec<NodeRef>, LexicalError> {
    serialized_nodes.iter().map(parse_serialized_node).collect()
}

/// `$getHtmlContent(editor, selection)`: the HTML of the nodes in
/// `selection` (`$generateHtmlFromNodes`, relying on each node's
/// `exportDOM`); `""` when nothing is selected. JS's `selection` defaults
/// to `$getSelection()`; pass it here.
#[cfg(feature = "html")]
pub fn get_html_content(
    editor: &Editor,
    selection: Option<&SelectionRef>,
) -> Result<String, LexicalError> {
    let Some(selection) = selection else {
        return Err(LexicalError::new("Expected valid LexicalSelection"));
    };
    // If we haven't selected anything
    if is_nothing_selected(selection)? {
        return Ok(String::new());
    }
    crate::html::generate_html_from_nodes(editor, Some(selection))
}

/// The `next` of an [`ExportMimeTypeFunction`]: the handler below it in
/// the stack (`None` below the bottom).
pub type ExportMimeTypeNext<'a> = &'a dyn Fn() -> Result<Option<String>, LexicalError>;

/// `ExportMimeTypeFunction`: serializes the selection (`None` when there is
/// none) for one MIME type, or returns `None` to leave the type out.
pub type ExportMimeTypeFunction = Rc<
    dyn Fn(Option<&SelectionRef>, ExportMimeTypeNext<'_>) -> Result<Option<String>, LexicalError>,
>;

/// `ExportMimeTypeConfig`: a stack per MIME type, in the order the types
/// were first added; the last function of a stack runs first.
pub type ExportMimeTypeConfig = Vec<(String, Vec<ExportMimeTypeFunction>)>;

/// `DEFAULT_EXPORT_MIME_TYPE`: [`get_lexical_content`],
/// [`get_html_content`] (with the `html` feature) and the selection's text.
pub fn default_export_mime_type() -> ExportMimeTypeConfig {
    fn active_editor() -> Result<Editor, LexicalError> {
        crate::updates::get_active_editor()
            .ok_or_else(|| LexicalError::new("Unable to find an active editor."))
    }
    let mut config: ExportMimeTypeConfig = vec![(
        LEXICAL_MIME_TYPE.to_owned(),
        vec![Rc::new(
            |sel: Option<&SelectionRef>, next: ExportMimeTypeNext<'_>| match sel {
                Some(sel) => get_lexical_content(&active_editor()?, Some(sel)),
                None => next(),
            },
        )],
    )];
    #[cfg(feature = "html")]
    config.push((
        "text/html".to_owned(),
        vec![Rc::new(
            |sel: Option<&SelectionRef>, next: ExportMimeTypeNext<'_>| match sel {
                Some(sel) => get_html_content(&active_editor()?, Some(sel)).map(Some),
                None => next(),
            },
        )],
    ));
    config.push((
        "text/plain".to_owned(),
        vec![Rc::new(
            |sel: Option<&SelectionRef>, next: ExportMimeTypeNext<'_>| match sel {
                Some(sel) => sel.get_text_content().map(Some),
                None => next(),
            },
        )],
    ));
    config
}

/// `$getExportConfig(editor)`: the editor's `GetClipboardDataExtension`
/// output, or the defaults without it.
fn get_export_config(editor: &Editor) -> Result<Rc<ExportMimeTypeConfig>, LexicalError> {
    #[cfg(feature = "extension")]
    if let Some(output) = crate::extension::get_peer_dependency_from_editor::<
        GetClipboardDataConfig,
        ExportMimeTypeConfig,
        (),
    >(
        editor,
        get_clipboard_data::GET_CLIPBOARD_DATA_EXTENSION_NAME,
    )?
    .and_then(|dep| dep.output)
    {
        return Ok(output);
    }
    let _ = editor;
    Ok(Rc::new(default_export_mime_type()))
}

/// `callExportMimeTypeFunctionStack(fns, selection)`: the last function
/// first, each `next` calling the one below it.
fn call_export_mime_type_function_stack(
    fns: &[ExportMimeTypeFunction],
    selection: Option<&SelectionRef>,
) -> Result<Option<String>, LexicalError> {
    fn call_at(
        fns: &[ExportMimeTypeFunction],
        i: usize,
        selection: Option<&SelectionRef>,
    ) -> Result<Option<String>, LexicalError> {
        if i == 0 {
            return Ok(None);
        }
        (fns[i - 1])(selection, &|| call_at(fns, i - 1, selection))
    }
    call_at(fns, fns.len(), selection)
}

/// `$getClipboardDataFromSelection(selection)`: `selection` serialized for
/// each MIME type of the editor's export stacks
/// (`GetClipboardDataExtension`'s, or the defaults): `text/plain` first
/// (`""` without a selection), then each type in the stacks' order whose
/// stack gives a value. JS's `selection` defaults to `$getSelection()`;
/// pass it here.
pub fn get_clipboard_data_from_selection(
    selection: Option<&SelectionRef>,
) -> Result<LexicalClipboardData, LexicalError> {
    let editor = crate::updates::get_active_editor()
        .ok_or_else(|| LexicalError::new("Unable to find an active editor."))?;
    let mut clipboard_data: LexicalClipboardData = vec![("text/plain".to_owned(), String::new())];
    for (k, fns) in get_export_config(&editor)?.iter() {
        if let Some(v) = call_export_mime_type_function_stack(fns, selection)? {
            match clipboard_data.iter_mut().find(|(key, _)| key == k) {
                Some(entry) => entry.1 = v,
                None => clipboard_data.push((k.clone(), v)),
            }
        }
    }
    Ok(clipboard_data)
}

/// `$exportMimeTypeFromSelection(mimeType, selection)`: `selection`
/// serialized for one MIME type with the editor's export stack, or `None`
/// when it has none for the type or every handler gave `None`. JS's
/// `selection` defaults to `$getSelection()`; pass it here.
pub fn export_mime_type_from_selection(
    mime_type: &str,
    selection: Option<&SelectionRef>,
) -> Result<Option<String>, LexicalError> {
    let editor = crate::updates::get_active_editor()
        .ok_or_else(|| LexicalError::new("Unable to find an active editor."))?;
    let config = get_export_config(&editor)?;
    let fns = config
        .iter()
        .find(|(k, _)| k == mime_type)
        .map(|(_, fns)| &fns[..])
        .unwrap_or_default();
    call_export_mime_type_function_stack(fns, selection)
}

/// The merge of `$exportMimeType` and `$importMimeType`: each of
/// `partial`'s functions go on top of its type's stack, and a new type is
/// added last.
#[cfg(feature = "extension")]
pub(crate) fn append_stacks<F: Clone>(
    config: &mut Vec<(String, Vec<F>)>,
    partial: &[(String, Vec<F>)],
) {
    for (k, v) in partial {
        match config.iter_mut().find(|(key, _)| key == k) {
            Some(entry) => entry.1.extend(v.iter().cloned()),
            None => config.push((k.clone(), v.clone())),
        }
    }
}

#[cfg(feature = "extension")]
pub use get_clipboard_data::*;

#[cfg(feature = "extension")]
mod get_clipboard_data {
    use super::{ExportMimeTypeConfig, append_stacks, default_export_mime_type};
    use crate::Editor;
    use crate::extension_core::{
        ExtensionBuildState, ExtensionConfigBase, LexicalExtension, define_extension,
    };
    use std::rc::Rc;

    pub(super) const GET_CLIPBOARD_DATA_EXTENSION_NAME: &str =
        "@lexical/clipboard/GetClipboardData";

    /// `GetClipboardDataConfig`.
    #[derive(Clone)]
    pub struct GetClipboardDataConfig {
        /// `$exportMimeType`: a contribution's functions go on top of each
        /// type's stack (`[...prev, ...override]`).
        pub export_mime_type: ExportMimeTypeConfig,
    }

    /// `Partial<GetClipboardDataConfig>`.
    #[derive(Clone, Default)]
    pub struct GetClipboardDataConfigOverrides {
        pub export_mime_type: Option<ExportMimeTypeConfig>,
    }

    impl ExtensionConfigBase for GetClipboardDataConfig {
        type Partial = GetClipboardDataConfigOverrides;

        fn shallow_merge(&self, overrides: &GetClipboardDataConfigOverrides) -> Self {
            GetClipboardDataConfig {
                export_mime_type: overrides
                    .export_mime_type
                    .clone()
                    .unwrap_or_else(|| self.export_mime_type.clone()),
            }
        }
    }

    /// `GetClipboardDataExtension`.
    pub type GetClipboardDataExtensionType =
        LexicalExtension<GetClipboardDataConfig, ExportMimeTypeConfig, ()>;

    fn make() -> Rc<GetClipboardDataExtensionType> {
        define_extension(LexicalExtension {
            build: Some(Rc::new(
                |_: &Editor, config: &GetClipboardDataConfig, _: &ExtensionBuildState<()>| {
                    Ok(config.export_mime_type.clone())
                },
            )),
            merge_config: Some(Rc::new(
                |config: &GetClipboardDataConfig, partial: &GetClipboardDataConfigOverrides| {
                    let mut merged = config.shallow_merge(partial);
                    if let Some(export_mime_type) = &partial.export_mime_type {
                        merged.export_mime_type = config.export_mime_type.clone();
                        append_stacks(&mut merged.export_mime_type, export_mime_type);
                    }
                    merged
                },
            )),
            ..LexicalExtension::new(
                GET_CLIPBOARD_DATA_EXTENSION_NAME,
                GetClipboardDataConfig {
                    export_mime_type: default_export_mime_type(),
                },
            )
        })
    }

    thread_local! {
        static GET_CLIPBOARD_DATA_EXTENSION: Rc<GetClipboardDataExtensionType> = make();
    }

    /// `GetClipboardDataExtension`: how the selection is serialized into
    /// each MIME type when copying out of the editor. Its output is the
    /// merged [`ExportMimeTypeConfig`], which
    /// [`get_clipboard_data_from_selection`](super::get_clipboard_data_from_selection)
    /// and [`export_mime_type_from_selection`](super::export_mime_type_from_selection)
    /// read.
    pub fn get_clipboard_data_extension() -> Rc<GetClipboardDataExtensionType> {
        GET_CLIPBOARD_DATA_EXTENSION.with(Rc::clone)
    }
}

/// `setLexicalClipboardDataTransfer(clipboardData, data)`: `setData` on
/// `clipboard_data` for each MIME type in `data`, after clearing
/// `text/html` and `application/x-lexical-editor` where `data` has none.
pub fn set_lexical_clipboard_data_transfer(
    clipboard_data: &DataTransfer,
    data: &LexicalClipboardData,
) {
    for k in ["text/html", LEXICAL_MIME_TYPE] {
        if !data.iter().any(|(type_, _)| type_ == k) {
            clipboard_data.set_data(k, "");
        }
    }
    for (k, v) in data {
        clipboard_data.set_data(k, v);
    }
}

/// `$copyToClipboardEvent(editor, event)` without `data`: the selection's
/// clipboard data written to the event's `clipboardData`, unless nothing is
/// selected. JS also declines when the DOM selection is missing or outside
/// the editor; without a DOM the caller is the editor's own platform,
/// which sends only the editor's copy (as in Swift). Returns whether it
/// wrote. (Only `registerRichText` copies through it.)
#[cfg(feature = "rich-text")]
pub(crate) fn copy_to_clipboard_event(clipboard_data: &DataTransfer) -> Result<bool, LexicalError> {
    let Some(selection) = get_selection() else {
        return Ok(false);
    };
    if selection.range().is_some_and(|r| r.is_collapsed()) {
        return Ok(false);
    }
    let data = get_clipboard_data_from_selection(Some(&selection))?;
    set_lexical_clipboard_data_transfer(clipboard_data, &data);
    Ok(true)
}

/// `copyToClipboard(editor, event)` as `registerRichText`'s copy and cut
/// handlers call it: with the event's `clipboardData`, an update that
/// writes the selection to it (`$copyToClipboardEvent`). Without one, JS
/// fires a `copy` event at the DOM to get one, which a headless editor
/// can't; nothing is written. (The promise JS returns, and its guard
/// against a copy in flight, have no counterpart.)
#[cfg(feature = "rich-text")]
pub(crate) fn copy_to_clipboard(
    editor: &Editor,
    clipboard_data: Option<std::rc::Rc<DataTransfer>>,
) -> Result<(), LexicalError> {
    let Some(clipboard_data) = clipboard_data else {
        return Ok(());
    };
    editor.update(
        move || copy_to_clipboard_event(&clipboard_data).map(|_| ()),
        crate::UpdateOptions::default(),
    )
}

#[cfg(all(test, feature = "rich-text", feature = "plain-text"))]
mod tests {
    use crate::{
        COPY_COMMAND, CUT_COMMAND, CreateEditorArgs, DataTransfer, Editor, PASTE_COMMAND,
        UpdateOptions, create_editor, create_paragraph_node, create_text_node, get_root,
    };
    use std::rc::Rc;

    /// An editor holding `hello world`, with `world` selected.
    fn editor(register: fn(&Editor) -> crate::Unregister) -> Editor {
        let editor = create_editor(CreateEditorArgs {
            namespace: "test".to_owned(),
            ..CreateEditorArgs::default()
        });
        let _unregister = register(&editor);
        editor
            .update(
                || {
                    let text = create_text_node("hello world");
                    let paragraph = create_paragraph_node();
                    paragraph.append(std::slice::from_ref(&text))?;
                    get_root().append(&[paragraph])?;
                    text.select(Some(6), Some(11)).map(|_| ())
                },
                UpdateOptions {
                    discrete: true,
                    ..UpdateOptions::default()
                },
            )
            .unwrap();
        editor
    }

    fn text(editor: &Editor) -> String {
        editor.read(|| get_root().get_text_content()).unwrap()
    }

    /// `registerRichText`'s copy, cut and paste handlers with a clipboard
    /// event's data: the copy writes what `scripts/clipboard`'s `copy`
    /// records for the selection, the cut removes it after copying, and the
    /// paste inserts the Lexical JSON.
    #[test]
    fn rich_text_copy_cut_and_paste() {
        let editor = editor(crate::rich_text::register_rich_text);
        let clipboard = Rc::new(DataTransfer::new());
        assert!(
            editor
                .dispatch_command(&COPY_COMMAND, Some(clipboard.clone()))
                .unwrap()
        );
        assert_eq!(text(&editor), "hello world");
        #[cfg(not(feature = "html"))]
        assert_eq!(
            clipboard.types(),
            ["text/html", "text/plain", "application/x-lexical-editor"]
        );
        #[cfg(feature = "html")]
        {
            assert_eq!(
                clipboard.types(),
                ["text/plain", "application/x-lexical-editor", "text/html"]
            );
            assert_eq!(
                clipboard.get_data("text/html"),
                r#"<span style="white-space: pre-wrap;">world</span>"#
            );
        }
        assert_eq!(clipboard.get_data("text/plain"), "world");
        assert_eq!(
            clipboard.get_data("application/x-lexical-editor"),
            r#"{"namespace":"test","nodes":[{"detail":0,"format":0,"mode":"normal","style":"","text":"world","type":"text","version":1}]}"#
        );

        let cut = Rc::new(DataTransfer::new());
        assert!(
            editor
                .dispatch_command(&CUT_COMMAND, Some(cut.clone()))
                .unwrap()
        );
        assert_eq!(text(&editor), "hello ");
        assert_eq!(cut.get_data("text/plain"), "world");

        assert!(editor.dispatch_command(&PASTE_COMMAND, Some(cut)).unwrap());
        assert!(editor.dispatch_command(&PASTE_COMMAND, None).unwrap());
        assert_eq!(text(&editor), "hello world");
    }

    /// `registerPlainText`'s: the copy writes only `text/plain` (and
    /// `text/html`), and the paste takes only the text, not the Lexical
    /// JSON.
    #[test]
    fn plain_text_copy_cut_and_paste() {
        let editor = editor(crate::plain_text::register_plain_text);
        let clipboard = Rc::new(DataTransfer::new());
        assert!(
            editor
                .dispatch_command(&CUT_COMMAND, Some(clipboard.clone()))
                .unwrap()
        );
        assert_eq!(text(&editor), "hello ");
        #[cfg(not(feature = "html"))]
        assert_eq!(clipboard.types(), ["text/plain"]);
        #[cfg(feature = "html")]
        {
            assert_eq!(clipboard.types(), ["text/html", "text/plain"]);
            assert_eq!(
                clipboard.get_data("text/html"),
                r#"<span style="white-space: pre-wrap;">world</span>"#
            );
        }
        clipboard.set_data("application/x-lexical-editor", "{}");
        clipboard.set_data("text/plain", "there\nyou");
        assert!(
            editor
                .dispatch_command(&PASTE_COMMAND, Some(clipboard))
                .unwrap()
        );
        assert_eq!(text(&editor), "hello there\nyou");
    }
}
