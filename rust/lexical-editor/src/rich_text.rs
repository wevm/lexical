//! `@lexical/rich-text` (the `rich-text` feature): `HeadingNode` and
//! `QuoteNode`, from `packages/lexical-rich-text/src/index.ts`, with their
//! generated JSON (`LexicalRichTextGeneratedJSON.ts`).
//!
//! `registerRichText`'s command handlers ([`register_rich_text`]): those
//! that act on the model, those of the keys the core's keydown handler
//! dispatches (arrows, Backspace, Delete, Enter, Escape, Space, Tab,
//! `MOVE_TO_START` and `MOVE_TO_END`), of a click, of copy, cut and paste,
//! which read the `DataTransfer` of the platform's clipboard event, and of
//! drag and drop, whose position the root element resolves
//! ([`RootElement::caret_from_point`](crate::RootElement::caret_from_point)),
//! with its `escapeFormatTriggers` and `shouldHandlePasteAsFiles`
//! ([`RichTextConfig`], [`DRAG_DROP_PASTE`]). The DOM parts
//! (`createDOM`, `exportDOM`, `importDOM`) come with `html`. With the
//! `extension` feature, [`rich_text_extension`] is `RichTextExtension`, and
//! with `a11y`, `heading_announce_extension` is `HeadingAnnounceExtension`
//! (`HeadingAnnounceExtension.ts`). [`register_table_of_contents`] is
//! `@lexical/react`'s `TableOfContentsPlugin`
//! (`LexicalTableOfContentsPlugin.tsx`).

use crate::caret::{
    CaretDirection, PointCaret, compare_point_caret_next, extend_caret_to_range, get_caret_range,
    get_child_caret, get_collapsed_caret_range, get_sibling_caret,
};
use crate::caret_utils::{
    caret_from_point, get_caret_range_in_direction, normalize_caret, rewind_sibling_caret,
    set_selection_from_caret_range,
};
use crate::clipboard::{
    caret_from_event, copy_to_clipboard, get_clipboard_data_from_selection, handle_rich_text_drop,
    insert_data_transfer_for_rich_text, set_lexical_clipboard_data_transfer,
    write_drag_source_to_data_transfer,
};
use crate::commands::{
    CLICK_COMMAND, CONTROLLED_TEXT_INSERTION_COMMAND, COPY_COMMAND, CUT_COMMAND,
    DELETE_CHARACTER_COMMAND, DELETE_LINE_COMMAND, DELETE_WORD_COMMAND, DRAGOVER_COMMAND,
    DRAGSTART_COMMAND, DROP_COMMAND, FORMAT_ELEMENT_COMMAND, FORMAT_TEXT_COMMAND,
    INDENT_CONTENT_COMMAND, INSERT_LINE_BREAK_COMMAND, INSERT_PARAGRAPH_COMMAND,
    INSERT_TAB_COMMAND, KEY_ARROW_DOWN_COMMAND, KEY_ARROW_LEFT_COMMAND, KEY_ARROW_RIGHT_COMMAND,
    KEY_ARROW_UP_COMMAND, KEY_BACKSPACE_COMMAND, KEY_DELETE_COMMAND, KEY_ENTER_COMMAND,
    KEY_ESCAPE_COMMAND, KEY_SPACE_COMMAND, KEY_TAB_COMMAND, MOVE_TO_END, MOVE_TO_START,
    OUTDENT_CONTENT_COMMAND, PASTE_COMMAND, REMOVE_TEXT_COMMAND, SELECT_ALL_COMMAND,
    SET_TEXT_FORMAT_COMMAND,
};
use crate::editor::{COMMAND_PRIORITY_EDITOR, LexicalCommand};
use crate::events::{ClipboardOrKeyboardEvent, DataTransferFile, KeyboardEvent, PasteCommandType};
use crate::generated_json::{
    export_compact_element_node, export_element_node, update_element_node,
};
use crate::json::{Object, Value};
use crate::lexical_selection::SelectionRef;
use crate::lexical_selection::get_selection;
use crate::lexical_selection::{
    PointType, RangeSelection, Which, create_node_selection, has_ancestor, set_selection,
};
use crate::lexical_utils::{
    get_node_by_key, get_root, is_root_or_shadow_root, is_shadow_root_node,
};
use crate::lexical_utils::{set_node_key, with_writable};
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::node_state::{
    RequiredNodeStateConfig, StateConfig, create_state, get_state_direct, set_state,
};
use crate::nodes::element::ElementData;
use crate::nodes::paragraph::create_paragraph_node;
use crate::nodes::tab::create_tab_node;
use crate::schema::{NodeSchema, SchemaValue, enum_value, node_schema, with_field};
use crate::selection::{
    is_parent_rtl, move_character, should_override_default_character_selection,
};
use crate::selection_deletion::{SelectionGranularity, needs_block_cursor_beside};
use crate::selection_insert_nodes::normalize_selection;
use crate::selection_insert_nodes::{insert_nodes, select_all};
use crate::utils::{FilesEvent, event_files, get_nearest_block_element_ancestor_or_throw};
use crate::utils::{find_matching_parent, handle_indent_and_outdent};
use crate::utils::{is_at_end_of_node, is_at_start_of_node};
use crate::{CUT_TAG, Editor, LexicalError, NodeClass, PASTE_TAG, Unregister, UpdateOptions};
use std::rc::Rc;

#[cfg(feature = "extension")]
mod lexical_rich_text_extension;
#[cfg(all(feature = "html", feature = "extension"))]
mod rich_text_import_extension;
mod table_of_contents;

pub use table_of_contents::{TableOfContentsEntry, register_table_of_contents};

#[cfg(feature = "extension")]
pub use lexical_rich_text_extension::{
    RichTextConfigOverrides, RichTextExtensionType, RichTextOutput, rich_text_extension,
};
#[cfg(all(feature = "html", feature = "extension"))]
pub use rich_text_import_extension::{rich_text_import_rules, shadow_root_quote_rule};
#[cfg(feature = "a11y")]
mod heading_announce_extension;
#[cfg(feature = "a11y")]
pub use heading_announce_extension::{
    HeadingAnnounceExtensionConfig, HeadingAnnounceExtensionConfigOverrides,
    HeadingAnnounceExtensionType, HeadingAnnounceOutput, heading_announce_extension,
};

/// The node classes this package registers (`RichTextExtension`'s
/// `nodes`): `HeadingNode` and `QuoteNode`.
pub fn rich_text_nodes() -> Vec<NodeClass> {
    vec![NodeClass::of::<HeadingNode>(), NodeClass::of::<QuoteNode>()]
}

/// `HeadingTagType`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HeadingTagType {
    #[default]
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
}

impl HeadingTagType {
    pub fn as_str(self) -> &'static str {
        match self {
            HeadingTagType::H1 => "h1",
            HeadingTagType::H2 => "h2",
            HeadingTagType::H3 => "h3",
            HeadingTagType::H4 => "h4",
            HeadingTagType::H5 => "h5",
            HeadingTagType::H6 => "h6",
        }
    }

    /// The tag a string names, as `enumValue(['h1', ..., 'h6'])` reads it.
    pub fn parse(tag: &str) -> Option<Self> {
        Some(match tag {
            "h1" => HeadingTagType::H1,
            "h2" => HeadingTagType::H2,
            "h3" => HeadingTagType::H3,
            "h4" => HeadingTagType::H4,
            "h5" => HeadingTagType::H5,
            "h6" => HeadingTagType::H6,
            _ => return None,
        })
    }
}

/// `HeadingNode`: a block with a tag, `h1` to `h6`.
#[derive(Clone, Debug, Default)]
pub struct HeadingNode {
    base: NodeBase,
    element: ElementData,
    tag: HeadingTagType,
}

/// `$createHeadingNode(headingTag = 'h1')`.
pub fn create_heading_node(tag: HeadingTagType) -> NodeRef {
    set_node_key(Box::new(HeadingNode {
        tag,
        ..HeadingNode::default()
    }))
}

/// `$isHeadingNode(node)`.
pub fn is_heading_node(node: &NodeRef) -> bool {
    node.get_latest().is::<HeadingNode>()
}

impl NodeRef {
    /// `HeadingNode.getTag()`; `None` for any other node.
    pub fn get_heading_tag(&self) -> Option<HeadingTagType> {
        self.get_latest()
            .downcast_ref::<HeadingNode>()
            .map(|h| h.tag)
    }

    /// `HeadingNode.setTag(tag)`. Does nothing to any other node.
    pub fn set_heading_tag(&self, tag: HeadingTagType) {
        if is_heading_node(self) {
            with_writable(self.get_key(), |n| {
                n.downcast_mut::<HeadingNode>().expect("a heading").tag = tag;
            });
        }
    }

    /// `QuoteNode.setIsShadowRoot(isShadowRoot)`: sets
    /// [`quote_shadow_root_state`]. Does nothing to any other node.
    pub fn set_quote_is_shadow_root(&self, is_shadow_root: bool) -> Result<(), LexicalError> {
        if is_quote_node(self) {
            set_state(self, &quote_shadow_root_state(), is_shadow_root)?;
        }
        Ok(())
    }
}

/// `json` with `tag` (`withField(enumValue(...))`) after `children`, where
/// the generated `exportHeadingNode` writes it.
fn with_tag(json: Object, tag: Option<HeadingTagType>) -> Object {
    let mut out = Object::new();
    for (key, value) in json.iter() {
        out.insert(key, value.clone());
        if key == "children"
            && let Some(tag) = tag
        {
            out.insert("tag", tag.as_str().into());
        }
    }
    out
}

thread_local! {
    static HEADING_NODE_SCHEMA: NodeSchema = node_schema(vec![(
        "tag",
        with_field(
            enum_value(HEADING_TAGS.map(Value::from).to_vec(), None).expect("the heading tags"),
            |n: &HeadingNode| &n.tag,
            |n: &mut HeadingNode| &mut n.tag,
        )
        .expect("a field"),
    )])
    .expect("HeadingNode's schema");
}

/// The heading tags, `h1` to `h6`.
const HEADING_TAGS: [&str; 6] = ["h1", "h2", "h3", "h4", "h5", "h6"];

/// `headingNodeSchema`: the `tag` field.
pub fn heading_node_schema() -> NodeSchema {
    HEADING_NODE_SCHEMA.with(Clone::clone)
}

impl SchemaValue for HeadingTagType {
    fn to_schema_value(&self) -> Value {
        self.as_str().into()
    }
    fn from_schema_value(value: &Value) -> Option<Self> {
        value.as_str().and_then(HeadingTagType::parse)
    }
}

impl LexicalNode for HeadingNode {
    fn json_schema_chain(&self) -> Vec<NodeSchema> {
        vec![
            heading_node_schema(),
            crate::nodes::element::element_node_schema(),
        ]
    }
    /// The `$config()` of `HeadingNode`: `children: 'inline'`.
    fn static_node_config_chain(&self) -> Vec<crate::structure::NodeConfigLink> {
        crate::structure::inline_holder_chain(self.get_type())
    }

    /// `HeadingNode.updateDOM(prevNode)`: a heading of another level is
    /// made again.
    fn update_dom(&self, prev_node: &dyn LexicalNode) -> bool {
        prev_node
            .downcast_ref::<HeadingNode>()
            .is_some_and(|prev| prev.tag != self.tag)
    }
    fn get_type(&self) -> &'static str {
        "heading"
    }
    /// `HeadingNode.createDOM(config)`: an `<h1>` to `<h6>` with the
    /// theme's `heading[tag]` class names.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        config: &crate::EditorConfig,
        _editor: &Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        let tag = self.tag.as_str();
        let element = crate::lexical_dom::get_document().create_element(tag);
        if let Some(Value::Object(class_names)) = config.theme.get("heading") {
            crate::lexical_dom::add_theme_class_names(&element, class_names, tag);
        }
        Ok(element)
    }
    #[cfg(feature = "html")]
    fn export_dom(
        &self,
        this: &NodeRef,
        editor: &Editor,
    ) -> Result<crate::lexical_dom::DomExportOutput, LexicalError> {
        rich_text_dom::export_block_dom(self, this, editor)
    }
    /// `HeadingNode`'s `importDOM`: `<h1>` to `<h6>`, and a Google Docs
    /// title (a `<span>` of `font-size: 26pt`, and the `<p>` around one).
    #[cfg(feature = "html")]
    fn import_dom(&self) -> Option<crate::lexical_dom::DomConversionMap> {
        Some(rich_text_dom::heading_import_dom())
    }
    /// `HeadingNode.extractWithChild()`: a heading always goes with a
    /// child that is copied.
    fn extract_with_child(
        &self,
        _: &NodeRef,
        _: &NodeRef,
        _: Option<&SelectionRef>,
        _: &str,
    ) -> Result<bool, LexicalError> {
        Ok(true)
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

    /// `exportHeadingNode` and `exportCompactHeadingNode`: an element's
    /// JSON with `tag` after `children` (left out of the compact form when
    /// it's `h1`).
    fn generated_export_json(&self, compact: bool) -> Option<Object> {
        Some(if compact {
            let tag = Some(self.tag).filter(|t| *t != HeadingTagType::H1);
            with_tag(export_compact_element_node(self), tag)
        } else {
            with_tag(export_element_node(self), Some(self.tag))
        })
    }

    /// `updateHeadingNode`.
    fn generated_update_from_json(&mut self, json: &Object) -> bool {
        update_element_node(&mut self.element, json);
        self.tag = json
            .get("tag")
            .and_then(Value::as_str)
            .and_then(HeadingTagType::parse)
            .unwrap_or_default();
        true
    }

    /// `HeadingNode.insertNewAfter(selection, restoreSelection)`: a
    /// paragraph at the heading's end, otherwise the second half of the
    /// heading with its tag, alignment and style. At its start, the heading
    /// itself becomes a paragraph holding the selection.
    fn insert_new_after(
        &self,
        this: &NodeRef,
        selection: &SelectionRef,
        restore_selection: bool,
    ) -> Result<Option<NodeRef>, LexicalError> {
        let anchor = selection.with_range(|r| r.anchor.clone());
        let last_desc = this.get_last_descendant();
        let is_at_end = match &anchor {
            _ if last_desc.is_none() => true,
            Some(anchor) => is_at_end_of_node(anchor, this)?,
            None => false,
        };
        let is_at_start = match &anchor {
            Some(anchor) => is_at_start_of_node(anchor, this)?,
            None => false,
        };
        let new_element = if is_at_end || anchor.is_none() {
            create_paragraph_node()
        } else {
            // The heading is split in two, so the second half keeps the
            // block format and style of the first, like ParagraphNode does.
            let heading = create_heading_node(self.tag);
            heading.set_element_format(self.element.get_format_type());
            heading.set_element_style(self.element.get_style());
            heading
        };
        new_element.set_direction(self.element.get_direction());
        this.insert_after(&new_element, restore_selection)?;
        if is_at_start && !this.is_empty() {
            let paragraph = create_paragraph_node();
            paragraph.select(None, None)?;
            this.replace(&paragraph, true)?;
        }
        Ok(Some(new_element))
    }

    /// `HeadingNode.collapseAtStart()`: an empty heading becomes a
    /// paragraph.
    fn collapse_at_start(&self, this: &NodeRef, _: &SelectionRef) -> Result<bool, LexicalError> {
        if this.is_empty() {
            let paragraph = create_paragraph_node();
            paragraph.append(&this.get_children())?;
            this.replace(&paragraph, false)?;
        }
        Ok(true)
    }
}

/// `QuoteNode`: a block quote. With [`quote_shadow_root_state`] set
/// ([`NodeRef::set_quote_is_shadow_root`]) it is a shadow root holding
/// blocks instead of inline content.
#[derive(Clone, Debug, Default)]
pub struct QuoteNode {
    base: NodeBase,
    element: ElementData,
}

thread_local! {
    static QUOTE_SHADOW_ROOT_STATE: StateConfig<bool> =
        // `booleanValue()`: anything but a boolean is `false`.
        create_state("shadowRoot", |v| v.as_bool().unwrap_or(false));
}

/// `quoteShadowRootState`: whether a quote is a shadow root holding blocks,
/// a flat NodeState of `QuoteNode` (serialized as `shadowRoot` beside its
/// other properties), `false` by default.
pub fn quote_shadow_root_state() -> StateConfig<bool> {
    QUOTE_SHADOW_ROOT_STATE.with(Clone::clone)
}

/// `$createQuoteNode({shadowRoot})`.
pub fn create_quote_node(shadow_root: bool) -> NodeRef {
    let node = set_node_key(Box::new(QuoteNode::default()));
    if shadow_root {
        set_state(&node, &quote_shadow_root_state(), true)
            .expect("a new quote has no other shadowRoot state");
    }
    node
}

/// `$isQuoteNode(node)`.
pub fn is_quote_node(node: &NodeRef) -> bool {
    node.get_latest().is::<QuoteNode>()
}

impl LexicalNode for QuoteNode {
    /// The `$config()` of `QuoteNode`: `children: 'inline'`.
    fn static_node_config_chain(&self) -> Vec<crate::structure::NodeConfigLink> {
        crate::structure::inline_holder_chain(self.get_type())
    }

    fn get_type(&self) -> &'static str {
        "quote"
    }
    /// `QuoteNode.createDOM(config)`: a `<blockquote>` with the theme's
    /// `quote` class names.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        config: &crate::EditorConfig,
        _editor: &Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        let element = crate::lexical_dom::get_document().create_element("blockquote");
        crate::lexical_dom::add_theme_class_names(&element, &config.theme, "quote");
        Ok(element)
    }
    #[cfg(feature = "html")]
    fn export_dom(
        &self,
        this: &NodeRef,
        editor: &Editor,
    ) -> Result<crate::lexical_dom::DomExportOutput, LexicalError> {
        rich_text_dom::export_block_dom(self, this, editor)
    }
    /// `QuoteNode`'s `importDOM`: `<blockquote>`.
    #[cfg(feature = "html")]
    fn import_dom(&self) -> Option<crate::lexical_dom::DomConversionMap> {
        Some(vec![(
            "blockquote".to_owned(),
            crate::lexical_dom::importer(0, rich_text_dom::convert_blockquote_element),
        )])
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

    /// `QuoteNode.isShadowRoot()`: [`quote_shadow_root_state`] of this
    /// version, which is the latest wherever it is asked.
    fn is_shadow_root(&self) -> bool {
        get_state_direct(self, &quote_shadow_root_state())
            .expect("QuoteNode declares its shadowRoot state")
    }

    /// The `stateConfigs` of `QuoteNode.$config()`: the flat
    /// [`quote_shadow_root_state`].
    fn state_configs(&self) -> Vec<RequiredNodeStateConfig> {
        vec![RequiredNodeStateConfig::new(
            &quote_shadow_root_state(),
            true,
        )]
    }

    /// `QuoteNode.canMergeWhenEmpty()`.
    fn can_merge_when_empty(&self) -> bool {
        true
    }

    /// `exportQuoteNode` and `exportCompactQuoteNode` (the NodeState,
    /// `shadowRoot` among it, comes after).
    fn generated_export_json(&self, compact: bool) -> Option<Object> {
        Some(if compact {
            export_compact_element_node(self)
        } else {
            export_element_node(self)
        })
    }

    /// `updateQuoteNode` (the flat `shadowRoot` is applied with the rest of
    /// the NodeState).
    fn generated_update_from_json(&mut self, json: &Object) -> bool {
        update_element_node(&mut self.element, json);
        true
    }

    /// `QuoteNode.insertNewAfter(selection, restoreSelection)`: a paragraph
    /// with the selection's format and style, and this quote's direction,
    /// alignment and style.
    fn insert_new_after(
        &self,
        this: &NodeRef,
        selection: &SelectionRef,
        restore_selection: bool,
    ) -> Result<Option<NodeRef>, LexicalError> {
        let (format, style) = selection
            .with_range(|r| (r.format, r.style.clone()))
            .unwrap_or_default();
        let new_block = create_paragraph_node();
        new_block.set_text_format(format);
        new_block.set_text_style(&style);
        new_block.set_direction(self.element.get_direction());
        new_block.set_element_format(self.element.get_format_type());
        new_block.set_element_style(self.element.get_style());
        this.insert_after(&new_block, restore_selection)?;
        Ok(Some(new_block))
    }

    /// `QuoteNode.collapseAtStart()`: the quote becomes a paragraph holding
    /// its children, or, as a shadow root, is dissolved into its blocks.
    fn collapse_at_start(&self, this: &NodeRef, _: &SelectionRef) -> Result<bool, LexicalError> {
        if self.is_shadow_root() {
            for child in this.get_children() {
                this.insert_before(&child, true)?;
            }
            this.remove(false)?;
            return Ok(true);
        }
        let paragraph = create_paragraph_node();
        for child in this.get_children() {
            paragraph.append(&[child])?;
        }
        this.replace(&paragraph, false)?;
        Ok(true)
    }
}

/// `onPasteForRichText(event, editor)`: in an update tagged `paste`, the
/// clipboard data at the selection.
fn on_paste_for_rich_text(event: &PasteCommandType, editor: &Editor) -> Result<(), LexicalError> {
    event.event().prevent_default();
    let target = editor.clone();
    let clipboard_data = match event {
        PasteCommandType::Clipboard(event) => event.clipboard_data.clone(),
        PasteCommandType::Input(_) | PasteCommandType::Keyboard(_) => None,
    };
    editor.update(
        move || {
            let selection = get_selection();
            if let (Some(clipboard_data), Some(selection)) = (clipboard_data, selection) {
                insert_data_transfer_for_rich_text(&clipboard_data, &selection, &target)?;
            }
            Ok(())
        },
        UpdateOptions {
            // PASTE_TAG gives the paste its own undo entry (#8609).
            tags: vec![PASTE_TAG.to_owned()],
            ..UpdateOptions::default()
        },
    )
}

/// `onCutForRichText(event, editor)`: widens a selection of the whole
/// document to the root's children (so the copy carries the blocks the
/// removal takes, #5835), copies, then removes the selection, all tagged
/// `cut`.
fn on_cut_for_rich_text(
    event: Option<&ClipboardOrKeyboardEvent>,
    editor: &Editor,
) -> Result<(), LexicalError> {
    let cut = || vec![CUT_TAG.to_owned()];
    editor.update(
        || {
            if let Some(selection) = range_selection()
                && !selection.range().is_some_and(|r| r.is_collapsed())
            {
                crate::selection_deletion::internal_expand_selection_to_whole_document(&selection)?;
            }
            Ok(())
        },
        UpdateOptions {
            discrete: true,
            tags: cut(),
            ..UpdateOptions::default()
        },
    )?;
    copy_to_clipboard(
        editor,
        ClipboardOrKeyboardEvent::clipboard_event(event).cloned(),
    )?;
    editor.update(
        || {
            match get_selection() {
                Some(selection) if selection.is_range() => selection.remove_text()?,
                Some(selection) => {
                    for node in selection.get_nodes()? {
                        node.remove(false)?;
                    }
                }
                None => {}
            }
            Ok(())
        },
        UpdateOptions {
            // CUT_TAG gives the cut its own undo entry (#8609).
            tags: cut(),
            ..UpdateOptions::default()
        },
    )
}

/// `DRAG_DROP_PASTE`: files dropped or pasted into the editor
/// (`DRAG_DROP_PASTE_FILE`), unread; a handler reads each with
/// [`DataTransferFile::bytes`], as JS's `mediaFileReader` reads a `File`.
pub const DRAG_DROP_PASTE: LexicalCommand<Vec<DataTransferFile>> =
    LexicalCommand::constant(u64::MAX - 58, "DRAG_DROP_PASTE_FILE");

/// `EscapeFormatTrigger`: the interactions that can escape a format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscapeFormatTrigger {
    Enter,
    Click,
    Arrow,
    Space,
    Tab,
}

/// `TriggerConfig`: the triggers that escape one format, `Some(true)` for
/// each that does (`None` is an absent key). With `only_at_boundary`, the
/// format is escaped only at the start or end of a formatted text node with
/// no sibling in that direction.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TriggerConfig {
    pub enter: Option<bool>,
    pub click: Option<bool>,
    pub arrow: Option<bool>,
    pub space: Option<bool>,
    pub tab: Option<bool>,
    pub only_at_boundary: Option<bool>,
}

impl TriggerConfig {
    /// `config[trigger]`.
    pub fn get(&self, trigger: EscapeFormatTrigger) -> Option<bool> {
        match trigger {
            EscapeFormatTrigger::Enter => self.enter,
            EscapeFormatTrigger::Click => self.click,
            EscapeFormatTrigger::Arrow => self.arrow,
            EscapeFormatTrigger::Space => self.space,
            EscapeFormatTrigger::Tab => self.tab,
        }
    }

    /// `shallowMergeConfig(self, override)`: the keys `override` has
    /// replace this one's.
    pub fn merged(&self, override_: &TriggerConfig) -> TriggerConfig {
        TriggerConfig {
            enter: override_.enter.or(self.enter),
            click: override_.click.or(self.click),
            arrow: override_.arrow.or(self.arrow),
            space: override_.space.or(self.space),
            tab: override_.tab.or(self.tab),
            only_at_boundary: override_.only_at_boundary.or(self.only_at_boundary),
        }
    }
}

/// `EscapeFormatTriggerConfig`: the triggers of each text format, in key
/// order. A format mapped to `None` (JS `null`) has escaping disabled,
/// which matters when overriding the defaults.
pub type EscapeFormatTriggerConfig = Vec<(String, Option<TriggerConfig>)>;

/// `DEFAULT_ESCAPE_FORMAT_TRIGGERS`: `capitalize`, `lowercase` and
/// `uppercase` are escaped on Enter, Space and Tab.
pub fn default_escape_format_triggers() -> EscapeFormatTriggerConfig {
    ["capitalize", "lowercase", "uppercase"]
        .into_iter()
        .map(|format| {
            (
                format.to_owned(),
                Some(TriggerConfig {
                    enter: Some(true),
                    space: Some(true),
                    tab: Some(true),
                    ..TriggerConfig::default()
                }),
            )
        })
        .collect()
}

/// `ShouldHandlePasteAsFiles(files, hasTextContent)`: whether a paste
/// carrying files is handled by dispatching [`DRAG_DROP_PASTE`] with them,
/// rather than as the clipboard's other data. Compared by identity, as JS
/// compares functions.
#[derive(Clone)]
pub struct ShouldHandlePasteAsFiles(pub Rc<ShouldHandlePasteAsFilesFn>);

/// The function of a [`ShouldHandlePasteAsFiles`].
pub type ShouldHandlePasteAsFilesFn = dyn Fn(&[DataTransferFile], bool) -> bool;

impl PartialEq for ShouldHandlePasteAsFiles {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Default for ShouldHandlePasteAsFiles {
    /// [`default_should_handle_paste_as_files`].
    fn default() -> Self {
        ShouldHandlePasteAsFiles(Rc::new(default_should_handle_paste_as_files))
    }
}

/// `defaultShouldHandlePasteAsFiles(files, hasTextContent)`: files are
/// handled only when the clipboard carries no text content.
pub fn default_should_handle_paste_as_files(
    files: &[DataTransferFile],
    has_text_content: bool,
) -> bool {
    !files.is_empty() && !has_text_content
}

/// `RichTextConfig` (`LexicalRichTextExtension.ts`): what
/// [`register_rich_text_with_config`] reads, `RichTextExtension`'s config.
#[derive(Clone, PartialEq)]
pub struct RichTextConfig {
    /// `escapeFormatTriggers`: the formats an interaction escapes,
    /// [`default_escape_format_triggers`] by default.
    pub escape_format_triggers: EscapeFormatTriggerConfig,
    /// `shouldHandlePasteAsFiles`,
    /// [`default_should_handle_paste_as_files`] by default.
    pub should_handle_paste_as_files: ShouldHandlePasteAsFiles,
}

impl Default for RichTextConfig {
    /// `DEFAULT_RICH_TEXT_CONFIG`.
    fn default() -> Self {
        RichTextConfig {
            escape_format_triggers: default_escape_format_triggers(),
            should_handle_paste_as_files: ShouldHandlePasteAsFiles::default(),
        }
    }
}

/// Where `$escapeFormatsForTrigger` looks for a text node boundary.
#[derive(Clone, Copy, PartialEq, Eq)]
enum EscapeDirection {
    Start,
    End,
    Both,
}

/// `$escapeFormatsForTrigger(selection, trigger, direction, config)`:
/// turns off the pending format of each format `config` escapes on
/// `trigger`; one that is `onlyAtBoundary` only at a collapsed caret at the
/// start or end (as `direction` says) of a text node of that format with
/// no sibling there, which also clears the pending style.
fn escape_formats_for_trigger(
    selection: &SelectionRef,
    trigger: EscapeFormatTrigger,
    direction: EscapeDirection,
    config: &EscapeFormatTriggerConfig,
) -> Result<(), LexicalError> {
    let Some(range) = selection.range() else {
        return Ok(());
    };
    let mut is_boundary = false;
    let mut anchor_node = None;
    if range.is_collapsed() && range.anchor.point_type == PointType::Text {
        let node = range.anchor.get_node()?;
        if node.is_text() {
            let offset = range.anchor.offset;
            let at_end =
                offset == node.get_text_content_size() && node.get_next_sibling().is_none();
            let at_start = offset == 0 && node.get_previous_sibling().is_none();
            is_boundary = match direction {
                EscapeDirection::End => at_end,
                EscapeDirection::Start => at_start,
                EscapeDirection::Both => at_end || at_start,
            };
            anchor_node = Some(node);
        }
    }
    let mut did_escape_boundary = false;
    for (format, triggers) in config {
        let Some(triggers) = triggers else {
            continue;
        };
        if triggers.get(trigger) != Some(true) {
            continue;
        }
        if triggers.only_at_boundary == Some(true) {
            if !is_boundary || !anchor_node.as_ref().is_some_and(|n| n.has_format(format)) {
                continue;
            }
            did_escape_boundary = true;
        }
        if selection.has_format(format) {
            selection.toggle_format(format);
        }
    }
    if did_escape_boundary {
        selection.set_style("");
    }
    Ok(())
}

/// `$isSelectionAtEndOfRoot(selection)`.
fn is_selection_at_end_of_root(selection: &RangeSelection) -> bool {
    selection.focus.key == "root" && selection.focus.offset == get_root().get_children_size()
}

/// `$isSelectionAtStartOfRoot(selection)`.
fn is_selection_at_start_of_root(selection: &RangeSelection) -> bool {
    selection.focus.key == "root" && selection.focus.offset == 0
}

/// `$isBlockCursorAtRootEdge(selection, direction)`: a collapsed element
/// point at the start or end of the root beside a child that renders a
/// block cursor, where horizontal arrows have nowhere to go (#7999).
fn is_block_cursor_at_root_edge(selection: &RangeSelection, direction: CaretDirection) -> bool {
    if !selection.is_collapsed() {
        return false;
    }
    let is_next = direction == CaretDirection::Next;
    if !(if is_next {
        is_selection_at_end_of_root(selection)
    } else {
        is_selection_at_start_of_root(selection)
    }) {
        return false;
    }
    let offset = selection.focus.offset;
    let child = if is_next {
        offset
            .checked_sub(1)
            .and_then(|index| get_root().get_child_at_index(index))
    } else {
        get_root().get_child_at_index(offset)
    };
    child.is_some_and(|child| needs_block_cursor_beside(&child))
}

/// `$isSelectionCollapsedAtFrontOfIndentedBlock(selection)`: a caret at
/// the start of an indented block, where Backspace outdents.
fn is_selection_collapsed_at_front_of_indented_block(
    selection: &RangeSelection,
) -> Result<bool, LexicalError> {
    if !selection.is_collapsed() || selection.anchor.offset != 0 {
        return Ok(false);
    }
    let anchor_node = selection.anchor.get_node()?;
    if anchor_node.is_root() {
        return Ok(false);
    }
    let element = get_nearest_block_element_ancestor_or_throw(&anchor_node)?;
    Ok(element.get_indent()? > 0.0
        && (element == anchor_node
            || element.get_first_descendant().as_ref() == Some(&anchor_node)))
}

/// `$tryEnterFromBlockCursor(selection, direction)`: from a block cursor
/// beside a block shadow root, into its content.
fn try_enter_from_block_cursor(
    selection: &RangeSelection,
    direction: CaretDirection,
) -> Result<bool, LexicalError> {
    if !selection.is_collapsed() || selection.anchor.point_type != PointType::Element {
        return Ok(false);
    }
    let caret = caret_from_point(&selection.anchor, direction)?;
    if let Some(child) = caret.get_node_at_caret()
        && is_shadow_root_node(&child)
        && !child.is_inline()
        && let Some(child_caret) = get_child_caret(&child, direction)
    {
        set_selection_from_caret_range(&get_collapsed_caret_range(normalize_caret(&child_caret)))?;
        return Ok(true);
    }
    Ok(false)
}

/// `$tryBlockCursorShadowRootNavigation(selection, direction)`.
fn try_block_cursor_shadow_root_navigation(
    selection: &RangeSelection,
    direction: CaretDirection,
) -> Result<bool, LexicalError> {
    Ok(try_exit_shadow_root_to_block_cursor(selection, direction)?
        || try_enter_from_block_cursor(selection, direction)?)
}

/// `$tryExitShadowRootToBlockCursor(selection, direction)`: from the edge
/// of a shadow root, out to a block cursor beside it (or beside the
/// shadow roots it is nested in) when the sibling there needs one.
fn try_exit_shadow_root_to_block_cursor(
    selection: &RangeSelection,
    direction: CaretDirection,
) -> Result<bool, LexicalError> {
    if !selection.is_collapsed() {
        return Ok(false);
    }
    let focus_caret = caret_from_point(&selection.focus, direction)?;
    // Walk up from focus to find the nearest shadow root ancestor, starting
    // from the focus node itself since it may be a shadow root.
    let Some(shadow_root) = find_matching_parent(focus_caret.origin(), is_shadow_root_node) else {
        return Ok(false);
    };
    // An element point's caret origin is a child of the focus node, which
    // may be (or sit in) a shadow root the selection isn't inside.
    let focus_node = selection.focus.get_node()?;
    if shadow_root != focus_node && !has_ancestor(&focus_node, &shadow_root) {
        return Ok(false);
    }
    // The focus has to be at the edge of the shadow root in this direction.
    let text_range = get_caret_range(focus_caret, get_sibling_caret(&shadow_root, direction))?;
    if text_range.get_text_slices().iter().any(|slice| {
        slice
            .as_ref()
            .is_some_and(|s| s.get_text_content_size() > 0)
    }) {
        return Ok(false);
    }
    // Normalize the anchor in case it started with a ChildCaret.
    let range = get_caret_range(
        text_range.anchor.get_sibling_caret(),
        text_range.focus.clone(),
    )?;
    // From the edge to the shadow root, only parent nodes are expected, as
    // sibling carets.
    let mut prev_origin = range.anchor.origin().clone();
    for caret in range.iter() {
        if !(caret.is_sibling_caret() && Some(caret.origin()) == prev_origin.get_parent().as_ref())
        {
            return Ok(false);
        }
        prev_origin = caret.origin().clone();
    }
    // Walk outward through nested shadow roots until a sibling needs a
    // block cursor beside it.
    let mut prev_shadow = shadow_root.clone();
    for caret in extend_caret_to_range(&get_sibling_caret(&shadow_root, direction))?.iter() {
        // A caret whose origin is the parent of the current level is the
        // "leave" step walking outward; keep going only through shadow
        // roots.
        if Some(caret.origin()) == prev_shadow.get_parent().as_ref() {
            if !is_shadow_root_node(caret.origin()) {
                break;
            }
            prev_shadow = caret.origin().clone();
            continue;
        }
        if needs_block_cursor_beside(caret.origin()) {
            let sr = get_sibling_caret(&prev_shadow, direction);
            set_selection_from_caret_range(&get_caret_range(sr.clone(), sr)?)?;
            return Ok(true);
        }
        break;
    }
    Ok(false)
}

/// `$isSelectableBlockDecorator(node)`.
fn is_selectable_block_decorator(node: &NodeRef) -> bool {
    node.is_decorator() && !node.is_inline() && !node.is_isolated() && node.is_keyboard_selectable()
}

/// `$selectNode(key)`: a node selection of the node.
fn select_node(key: &str) {
    let node_selection = create_node_selection();
    node_selection.add(key);
    set_selection(Some(node_selection));
}

/// `$tryDecoratorLineNavigation(selection, isBackward)`: an up or down
/// arrow that leaves the top-level block beside a selectable block
/// decorator selects the decorator. JS probes `Selection.modify('move',
/// …, 'line')`; here the editor's [`SelectionMovement`](crate::SelectionMovement)
/// says where the line move lands, and without one (as JS without a DOM
/// selection) nothing happens.
fn try_decorator_line_navigation(
    selection: &RangeSelection,
    is_backward: bool,
    editor: &Editor,
) -> Result<bool, LexicalError> {
    if !selection.is_collapsed() {
        return Ok(false);
    }
    let focus = &selection.focus;
    let focus_node = focus.get_node()?;
    let direction = if is_backward {
        CaretDirection::Previous
    } else {
        CaretDirection::Next
    };
    let focus_caret = caret_from_point(focus, direction)?;
    if focus.point_type == PointType::Element
        && focus_node.is_element()
        && is_root_or_shadow_root(&focus_node)
    {
        if let Some(adjacent_child) = focus_caret.get_node_at_caret()
            && is_selectable_block_decorator(&adjacent_child)
        {
            select_node(adjacent_child.get_key());
            return Ok(true);
        }
        return Ok(false);
    }
    let start = if focus_node.is_element() {
        focus_node.clone()
    } else {
        focus_node.get_parent_or_throw()?
    };
    let Some(top_block) = find_matching_parent(&start, |n| {
        n.is_element()
            && !n.is_inline()
            && n.get_parent().is_some_and(|p| is_root_or_shadow_root(&p))
    }) else {
        return Ok(false);
    };
    let Some(adjacent_sibling) = get_sibling_caret(&top_block, direction)
        .get_node_at_caret()
        .filter(is_selectable_block_decorator)
    else {
        return Ok(false);
    };
    // Empty blocks always escape on line-move, so skip the layout probe.
    if top_block.get_text_content_size() == 0 {
        select_node(adjacent_sibling.get_key());
        return Ok(true);
    }
    let Some(movement) = editor
        .selection_movement()
        .filter(|movement| movement.has_dom_selection())
    else {
        return Ok(false);
    };
    let Some(landed) = movement.landed_point(focus, is_backward, SelectionGranularity::Line)?
    else {
        return Ok(false);
    };
    let Some(moved_node) = get_node_by_key(&landed.key) else {
        return Ok(false);
    };
    // No movement means the line move hit the block edge.
    if landed.key == focus.key && landed.offset == focus.offset {
        select_node(adjacent_sibling.get_key());
        return Ok(true);
    }
    if moved_node == top_block || has_ancestor(&moved_node, &top_block) {
        return Ok(false);
    }
    select_node(adjacent_sibling.get_key());
    Ok(true)
}

/// `$exitNodeSelectionToward(node, direction)`: a node selection left by an
/// arrow key becomes a caret beside the node, or at the edge of a block
/// shadow root next to it.
fn exit_node_selection_toward(
    node: &NodeRef,
    direction: CaretDirection,
) -> Result<(), LexicalError> {
    let caret = get_sibling_caret(node, direction);
    if let Some(sibling) = caret.get_adjacent_caret()
        && sibling.origin().is_element()
        && !sibling.origin().is_inline()
        && sibling.origin().get_latest().is_shadow_root()
    {
        set_selection_from_caret_range(&get_collapsed_caret_range(caret))?;
    } else if direction == CaretDirection::Next {
        node.select_next(Some(0), Some(0))?;
    } else {
        node.select_previous(None, None)?;
    }
    Ok(())
}

/// The `'next'` sibling carets of `nodes`, sorted in document order
/// (`.sort($comparePointCaretNext)`).
fn sorted_sibling_carets(nodes: Vec<NodeRef>) -> Result<Vec<PointCaret>, LexicalError> {
    let mut carets: Vec<PointCaret> = nodes
        .iter()
        .map(|node| get_sibling_caret(node, CaretDirection::Next))
        .collect();
    let mut error = None;
    carets.sort_by(|a, b| match compare_point_caret_next(a, b) {
        Ok(ordering) => ordering,
        Err(e) => {
            error.get_or_insert(e);
            std::cmp::Ordering::Equal
        }
    });
    match error {
        Some(error) => Err(error),
        None => Ok(carets),
    }
}

/// `$convertContiguousNodeSelection(selection, direction)`: a node
/// selection of contiguous siblings becomes the range selection that
/// covers them, committed when the update ends (`$flushSyncAfterUpdate`).
fn convert_contiguous_node_selection(
    selection: &SelectionRef,
    direction: CaretDirection,
) -> Result<bool, LexicalError> {
    let carets = sorted_sibling_carets(selection.get_nodes()?)?;
    let (Some(first_caret), Some(last_caret)) = (carets.first(), carets.last()) else {
        return Ok(false);
    };
    for pair in carets.windows(2) {
        if pair[0].get_node_at_caret().as_ref() != Some(pair[1].origin()) {
            return Ok(false);
        }
    }
    set_selection_from_caret_range(&get_caret_range_in_direction(
        &get_caret_range(rewind_sibling_caret(first_caret)?, last_caret.clone())?,
        direction,
    )?)?;
    // The vertical arrow handlers leave the extension to the platform's
    // default action, which reads the committed selection.
    crate::updates::flush_sync_after_update();
    Ok(true)
}

/// `$promoteNodeSelectionToBlockEdge(selection, isBackward, event)`:
/// `MOVE_TO_START` or `MOVE_TO_END` at a node selection collapses it to
/// the start or end of the block around its first or last node (in
/// document order), or of the root.
fn promote_node_selection_to_block_edge(
    selection: &SelectionRef,
    is_backward: bool,
    event: &KeyboardEvent,
) -> Result<bool, LexicalError> {
    event.prevent_default();
    event.stop_propagation();
    let nodes = selection.get_nodes()?;
    if nodes.is_empty() {
        return Ok(true);
    }
    let sorted = sorted_sibling_carets(nodes)?;
    let target = if is_backward {
        sorted.first()
    } else {
        sorted.last()
    }
    .expect("not empty")
    .origin()
    .clone();
    let block = find_matching_parent(&target, |n| {
        n != &target && n.is_element() && !n.is_inline()
    })
    .unwrap_or_else(get_root);
    let offset = if is_backward {
        0
    } else {
        block.get_children_size()
    };
    block.select(Some(offset), Some(offset))?;
    Ok(true)
}

/// The nearest block element at or above `node`.
fn nearest_block(node: &NodeRef) -> Option<NodeRef> {
    find_matching_parent(node, |n| n.is_element() && !n.is_inline())
}

/// The arrow keys' first step: an arrow at a node selection collapses it
/// beside its first node, toward `direction` (or, with Shift, makes a range
/// of contiguous nodes and goes on). Returns whether it handled the key,
/// and the selection to go on with.
fn arrow_node_selection(
    event: &KeyboardEvent,
    direction: impl Fn(&NodeRef) -> CaretDirection,
) -> Result<(bool, Option<SelectionRef>), LexicalError> {
    let mut selection = get_selection();
    if let Some(node_selection) = selection.clone().filter(SelectionRef::is_node) {
        // If selection is on a node, try to move it back to a range.
        let nodes = node_selection.get_nodes()?;
        if let Some(first) = nodes.first() {
            let direction = direction(first);
            if event.shift_key && convert_contiguous_node_selection(&node_selection, direction)? {
                // Fall through.
                selection = get_selection();
            } else {
                event.prevent_default();
                exit_node_selection_toward(first, direction)?;
                return Ok((true, selection));
            }
        }
    }
    Ok((false, selection))
}

/// The current selection when it is a range selection.
fn range_selection() -> Option<SelectionRef> {
    get_selection().filter(SelectionRef::is_range)
}

// The DOM side of `HeadingNode` and `QuoteNode` (the `html` feature).
#[cfg(feature = "html")]
mod rich_text_dom {
    use super::{HeadingTagType, create_heading_node, create_quote_node};
    use crate::dom::DomNode;
    use crate::lexical_dom::{
        DomConversion, DomConversionMap, DomConversionOutput, DomExportOutput, get_document,
        importer, set_direction_from_dom, set_format_from_dom, set_node_indent_from_dom,
    };
    use crate::node::{LexicalNode, NodeRef};
    use crate::{Editor, LexicalError};
    use std::rc::Rc;

    /// `HeadingNode.exportDOM(editor)` and `QuoteNode.exportDOM(editor)`:
    /// `ElementNode`'s, with a `<br>` when empty, the alignment as
    /// `text-align`, and the direction.
    pub(super) fn export_block_dom(
        node: &dyn LexicalNode,
        this: &NodeRef,
        editor: &Editor,
    ) -> Result<DomExportOutput, LexicalError> {
        let output = crate::nodes::element::export_dom(node, this, editor)?;
        if let Some(element) = output.element.as_ref().filter(|e| e.is_element()) {
            if this.is_empty() {
                element.append_child(&get_document().create_element("br"));
            }
            let data = node.element().expect("an element");
            let format_type = data.get_format_type();
            if !format_type.is_empty() {
                element.style().set_property("text-align", format_type, "");
            }
            if let Some(direction) = data.get_direction() {
                element.set_dir(direction.as_str());
            }
        }
        Ok(DomExportOutput {
            element: output.element,
            ..DomExportOutput::default()
        })
    }

    /// `isGoogleDocsTitle(domNode)`.
    fn is_google_docs_title(dom_node: &DomNode) -> bool {
        dom_node.node_name().eq_ignore_ascii_case("span")
            && dom_node.style().get_property_value("font-size") == "26pt"
    }

    pub(super) fn heading_import_dom() -> DomConversionMap {
        let mut map: DomConversionMap = ["h1", "h2", "h3", "h4", "h5", "h6"]
            .into_iter()
            .map(|tag| (tag.to_owned(), importer(0, convert_heading_element)))
            .collect();
        map.push((
            "p".to_owned(),
            Rc::new(|node: &DomNode| {
                let first_child = node.first_child()?;
                is_google_docs_title(&first_child).then(|| DomConversion {
                    conversion: Rc::new(|_| Ok(Some(DomConversionOutput::node(None)))),
                    priority: 3,
                })
            }),
        ));
        map.push((
            "span".to_owned(),
            Rc::new(|node: &DomNode| {
                is_google_docs_title(node).then(|| DomConversion {
                    conversion: Rc::new(|_| {
                        Ok(Some(DomConversionOutput::node(Some(create_heading_node(
                            HeadingTagType::H1,
                        )))))
                    }),
                    priority: 3,
                })
            }),
        ));
        map
    }

    /// `$convertHeadingElement(element)`.
    fn convert_heading_element(
        element: &DomNode,
    ) -> Result<Option<DomConversionOutput>, LexicalError> {
        let node = match HeadingTagType::parse(&element.node_name().to_lowercase()) {
            Some(tag) => {
                let node = create_heading_node(tag);
                set_node_indent_from_dom(element, &node)?;
                set_format_from_dom(&node, element);
                set_direction_from_dom(&node, element);
                Some(node)
            }
            None => None,
        };
        Ok(Some(DomConversionOutput::node(node)))
    }

    /// `$convertBlockquoteElement(element)`.
    pub(super) fn convert_blockquote_element(
        element: &DomNode,
    ) -> Result<Option<DomConversionOutput>, LexicalError> {
        let node = create_quote_node(false);
        set_format_from_dom(&node, element);
        set_node_indent_from_dom(element, &node)?;
        set_direction_from_dom(&node, element);
        Ok(Some(DomConversionOutput::node(Some(node))))
    }
}

/// `KEY_ARROW_LEFT_COMMAND` (`is_left`) and `KEY_ARROW_RIGHT_COMMAND`.
fn horizontal_arrow(
    event: &KeyboardEvent,
    is_left: bool,
    triggers: &EscapeFormatTriggerConfig,
) -> Result<bool, LexicalError> {
    // Left is backward, unless the text is right-to-left.
    let backward_unless_rtl = |node: &NodeRef| {
        if is_parent_rtl(node) == is_left {
            CaretDirection::Next
        } else {
            CaretDirection::Previous
        }
    };
    let (handled, selection) = arrow_node_selection(event, backward_unless_rtl)?;
    if handled {
        return Ok(true);
    }
    let Some(selection) = selection.filter(SelectionRef::is_range) else {
        return Ok(false);
    };
    let range = selection.range().expect("a range selection");
    let direction = backward_unless_rtl(&range.anchor.get_node()?);
    if is_block_cursor_at_root_edge(&range, direction) {
        event.prevent_default();
        return Ok(true);
    }
    if !event.shift_key && try_block_cursor_shadow_root_navigation(&range, direction)? {
        event.prevent_default();
        return Ok(true);
    }
    if !event.shift_key {
        escape_formats_for_trigger(
            &selection,
            EscapeFormatTrigger::Arrow,
            if is_left {
                EscapeDirection::Start
            } else {
                EscapeDirection::End
            },
            triggers,
        )?;
    }
    let range = selection.range().expect("a range selection");
    if should_override_default_character_selection(&range, is_left)? {
        event.prevent_default();
        move_character(&selection, event.shift_key, is_left)?;
        return Ok(true);
    }
    Ok(false)
}

/// `registerRichText(editor)`: registers the rich text command handlers,
/// at `COMMAND_PRIORITY_EDITOR`, with the default [`RichTextConfig`], and
/// returns what unregisters them.
///
/// The key handlers are JS's, less what a platform without `beforeinput`
/// or a DOM has no counterpart for: the `IS_IOS`/`IS_SAFARI` returns that
/// leave Backspace and Enter to `beforeinput`, inline-grid line
/// navigation (CSS grid layout), and the iOS text-file drop. The arrows
/// leave a node selection, step over block cursors at the root's edges and
/// shadow roots, select a block decorator a line move reaches (as the
/// editor's [`SelectionMovement`](crate::SelectionMovement) says), and move
/// across decorators and blocks (`$shouldOverrideDefaultCharacterSelection`,
/// `$moveCharacter`); Backspace outdents an indented block at its start
/// and otherwise dispatches `DELETE_CHARACTER_COMMAND`, as Delete does;
/// Enter escapes formats and dispatches `INSERT_PARAGRAPH_COMMAND`, or
/// `INSERT_LINE_BREAK_COMMAND` with Shift; Escape blurs the editor
/// ([`Editor::blur`]); Space, Tab, a click and the arrows escape the formats
/// `escapeFormatTriggers` names; `MOVE_TO_START` and `MOVE_TO_END` move past
/// an inline decorator at a block's start and collapse a node selection to
/// its block's edge. A click at a node selection clears it, unless it hit
/// (`MouseEvent::target_key`) a selected node or one inside it, where JS
/// asks whether the selected node's DOM contains the event's target.
///
/// `CONTROLLED_TEXT_INSERTION_COMMAND` takes only the string form, and
/// `SELECT_ALL_COMMAND` selects the whole document, or the slot frame the
/// caret is in.
///
/// `COPY_COMMAND`, `CUT_COMMAND` and `PASTE_COMMAND` read the clipboard
/// event's `clipboardData` (none for a keyboard or input event): a copy
/// writes the selection to it (`copyToClipboard`), a cut copies and then
/// removes the selection (`onCutForRichText`), and a paste dispatches
/// [`DRAG_DROP_PASTE`] with its files when `shouldHandlePasteAsFiles` says
/// so, and otherwise inserts its data with `$insertDataTransferForRichText`
/// (`onPasteForRichText`), each in an update of its own with JS's tags. A
/// copy or cut without one writes nothing, where JS goes through the DOM to
/// get an event; a cut still removes the selection. JS's cut removes it
/// after the copy's promise settles, in a later update; here that update is
/// queued with the others. A drop of files dispatches [`DRAG_DROP_PASTE`],
/// a drop of a drag that started in a Lexical editor is inserted at the
/// drop point (`$handleRichTextDrop`), and a drag start writes the
/// selection's clipboard data and the editor's marker to the drag's data.
/// Without a DOM, no event comes from an input inside a decorator
/// (`$isEventFromEmbeddedInput`, `$isTargetWithinDecorator`).
pub fn register_rich_text(editor: &Editor) -> Unregister {
    register_rich_text_with_config(editor, RichTextConfig::default())
}

/// `registerRichText(editor, escapeFormatTriggers,
/// shouldHandlePasteAsFiles)` with the values of `config`: see
/// [`register_rich_text`].
pub fn register_rich_text_with_config(editor: &Editor, config: RichTextConfig) -> Unregister {
    let config = Rc::new(config);
    let triggers = config.clone();
    register_rich_text_with(
        editor,
        Rc::new(move || triggers.escape_format_triggers.clone()),
        Rc::new(move || config.should_handle_paste_as_files.clone()),
    )
}

/// `registerRichText(editor, escapeFormatTriggers,
/// shouldHandlePasteAsFiles)` with the options read (`peek()`) each time a
/// handler needs them, as `RichTextExtension` passes its signals.
pub(crate) fn register_rich_text_with(
    editor: &Editor,
    escape_format_triggers: Rc<dyn Fn() -> EscapeFormatTriggerConfig>,
    should_handle_paste_as_files: Rc<dyn Fn() -> ShouldHandlePasteAsFiles>,
) -> Unregister {
    let triggers = escape_format_triggers;
    let unregisters: Vec<Unregister> = vec![
        {
            let triggers = triggers.clone();
            editor.register_command(
                &CLICK_COMMAND,
                move |event, _| match get_selection() {
                    Some(selection) if selection.is_node() => {
                        // A click on an already-selected node is an
                        // interaction with that node, not a deselect
                        // gesture (meta/lexical#8907).
                        if let Some(target) = event.target_key.as_deref().and_then(get_node_by_key)
                        {
                            for node in selection.get_nodes()? {
                                if node == target || has_ancestor(&target, &node) {
                                    return Ok(false);
                                }
                            }
                        }
                        selection.clear();
                        Ok(true)
                    }
                    Some(selection) => {
                        escape_formats_for_trigger(
                            &selection,
                            EscapeFormatTrigger::Click,
                            EscapeDirection::Both,
                            &triggers(),
                        )?;
                        Ok(false)
                    }
                    None => Ok(false),
                },
                COMMAND_PRIORITY_EDITOR,
            )
        },
        editor.register_command(
            &DELETE_CHARACTER_COMMAND,
            |&is_backward, _| match get_selection() {
                Some(selection) if selection.is_range() => {
                    selection.delete_character(is_backward)?;
                    Ok(true)
                }
                Some(selection) if selection.is_node() => {
                    selection.delete_nodes()?;
                    Ok(true)
                }
                _ => Ok(false),
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &DELETE_WORD_COMMAND,
            |&is_backward, _| {
                let Some(selection) = range_selection() else {
                    return Ok(false);
                };
                selection.delete_word(is_backward)?;
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &DELETE_LINE_COMMAND,
            |&is_backward, _| {
                let Some(selection) = range_selection() else {
                    return Ok(false);
                };
                selection.delete_line(is_backward)?;
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &CONTROLLED_TEXT_INSERTION_COMMAND,
            |text, _| {
                if let Some(selection) = get_selection() {
                    selection.insert_text(text)?;
                }
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &REMOVE_TEXT_COMMAND,
            |_, _| {
                let Some(selection) = range_selection() else {
                    return Ok(false);
                };
                selection.remove_text()?;
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &FORMAT_TEXT_COMMAND,
            |format, _| {
                let Some(selection) = get_selection() else {
                    return Ok(false);
                };
                selection.format_text(format, None)?;
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &SET_TEXT_FORMAT_COMMAND,
            |formats, _| {
                let Some(selection) = get_selection() else {
                    return Ok(false);
                };
                let formats: Vec<(&str, bool)> = formats
                    .iter()
                    .map(|(name, value)| (name.as_str(), *value))
                    .collect();
                selection.set_text_format(&formats)?;
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &FORMAT_ELEMENT_COMMAND,
            |format, _| {
                let Some(selection) = get_selection() else {
                    return Ok(false);
                };
                for node in selection.get_nodes()? {
                    let element = find_matching_parent(&node, |parent| {
                        parent.is_element() && !parent.is_inline()
                    });
                    if let Some(element) = element {
                        element.set_element_format(format);
                    }
                }
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &INSERT_LINE_BREAK_COMMAND,
            |&select_start, _| {
                let Some(selection) = range_selection() else {
                    return Ok(false);
                };
                selection.insert_line_break(select_start)?;
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &INSERT_PARAGRAPH_COMMAND,
            |_, _| {
                let Some(selection) = range_selection() else {
                    return Ok(false);
                };
                selection.insert_paragraph()?;
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        {
            let triggers = triggers.clone();
            editor.register_command(
                &KEY_ENTER_COMMAND,
                move |event, editor| {
                    let mut selection = get_selection();
                    // A block decorator selected alone as a NodeSelection: Enter
                    // inserts a paragraph after it.
                    if let Some(s) = selection.as_ref().filter(|s| s.is_node()) {
                        let nodes = s.get_nodes()?;
                        if let [node] = nodes.as_slice()
                            && node.is_decorator()
                            && !node.is_inline()
                        {
                            selection = Some(node.select_next(None, None)?);
                        }
                    }
                    let Some(selection) = selection.filter(SelectionRef::is_range) else {
                        return Ok(false);
                    };
                    escape_formats_for_trigger(
                        &selection,
                        EscapeFormatTrigger::Enter,
                        EscapeDirection::Both,
                        &triggers(),
                    )?;
                    if let Some(event) = event {
                        // No beforeinput to defer to (JS's iOS and Safari path).
                        event.prevent_default();
                        if event.shift_key {
                            return editor.dispatch_command(&INSERT_LINE_BREAK_COMMAND, false);
                        }
                    }
                    editor.dispatch_command(&INSERT_PARAGRAPH_COMMAND, ())
                },
                COMMAND_PRIORITY_EDITOR,
            )
        },
        editor.register_command(
            &KEY_ESCAPE_COMMAND,
            |_, editor| {
                if range_selection().is_none() {
                    return Ok(false);
                }
                editor.blur();
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &INSERT_TAB_COMMAND,
            |_, _| {
                let tab_node = create_tab_node();
                if let Some(range) = get_selection().and_then(|s| s.range()) {
                    tab_node.set_format(range.format);
                    tab_node.set_style(&range.style);
                }
                insert_nodes(vec![tab_node])?;
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &INDENT_CONTENT_COMMAND,
            |_, _| {
                handle_indent_and_outdent(|block| {
                    let indent = block.get_indent()?;
                    block.set_indent(indent + 1.0)
                })
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &OUTDENT_CONTENT_COMMAND,
            |_, _| {
                handle_indent_and_outdent(|block| {
                    let indent = block.get_indent()?;
                    if indent > 0.0 {
                        block.set_indent((indent - 1.0).max(0.0))?;
                    }
                    Ok(())
                })
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &KEY_ARROW_UP_COMMAND,
            |event, editor| {
                let (handled, selection) =
                    arrow_node_selection(event, |_| CaretDirection::Previous)?;
                if handled {
                    return Ok(true);
                }
                if let Some(selection) = selection.as_ref().and_then(SelectionRef::range) {
                    if is_selection_at_start_of_root(&selection) {
                        event.prevent_default();
                        return Ok(true);
                    }
                    if !event.shift_key
                        && try_block_cursor_shadow_root_navigation(
                            &selection,
                            CaretDirection::Previous,
                        )?
                    {
                        event.prevent_default();
                        return Ok(true);
                    }
                    if !event.shift_key && try_decorator_line_navigation(&selection, true, editor)?
                    {
                        event.prevent_default();
                        return Ok(true);
                    }
                }
                Ok(false)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &KEY_ARROW_DOWN_COMMAND,
            |event, editor| {
                let (handled, selection) = arrow_node_selection(event, |_| CaretDirection::Next)?;
                if handled {
                    return Ok(true);
                }
                if let Some(selection) = selection.as_ref().and_then(SelectionRef::range) {
                    if is_selection_at_end_of_root(&selection) {
                        event.prevent_default();
                        return Ok(true);
                    }
                    if !event.shift_key
                        && try_block_cursor_shadow_root_navigation(
                            &selection,
                            CaretDirection::Next,
                        )?
                    {
                        event.prevent_default();
                        return Ok(true);
                    }
                    if !event.shift_key && try_decorator_line_navigation(&selection, false, editor)?
                    {
                        event.prevent_default();
                        return Ok(true);
                    }
                }
                Ok(false)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        {
            let triggers = triggers.clone();
            editor.register_command(
                &KEY_ARROW_LEFT_COMMAND,
                move |event, _| horizontal_arrow(event, true, &triggers()),
                COMMAND_PRIORITY_EDITOR,
            )
        },
        {
            let triggers = triggers.clone();
            editor.register_command(
                &KEY_ARROW_RIGHT_COMMAND,
                move |event, _| horizontal_arrow(event, false, &triggers()),
                COMMAND_PRIORITY_EDITOR,
            )
        },
        editor.register_command(
            &KEY_BACKSPACE_COMMAND,
            |event, editor| {
                // A platform sends key events to the editor itself, never to
                // a control inside a decorator, so JS's decorator
                // pass-through ($isTargetWithinDecorator) has no
                // counterpart.
                match get_selection() {
                    Some(selection) if selection.is_range() => {
                        let range = selection.range().expect("a range selection");
                        if is_selection_collapsed_at_front_of_indented_block(&range)? {
                            event.prevent_default();
                            return editor.dispatch_command(&OUTDENT_CONTENT_COMMAND, ());
                        }
                    }
                    Some(_) => {}
                    None => return Ok(false),
                }
                event.prevent_default();
                editor.dispatch_command(&DELETE_CHARACTER_COMMAND, true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &KEY_DELETE_COMMAND,
            |event, editor| {
                if get_selection().is_none() {
                    return Ok(false);
                }
                event.prevent_default();
                editor.dispatch_command(&DELETE_CHARACTER_COMMAND, false)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &SELECT_ALL_COMMAND,
            |_, _| {
                // Scoped to the slot frame the caret is in, if any: a
                // whole-document select-all would escape the slot.
                match get_selection().filter(SelectionRef::is_range) {
                    Some(selection)
                        if crate::slot::get_slot_frame(
                            &selection
                                .point(crate::Which::Anchor)
                                .expect("a range")
                                .get_node()?,
                        )
                        .is_some() =>
                    {
                        crate::selection_insert_nodes::select_all_in(&selection)?;
                    }
                    _ => {
                        select_all()?;
                    }
                }
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &DROP_COMMAND,
            |event, editor| {
                let (_, files, _) = event_files(FilesEvent::Drag(event));
                if !files.is_empty() {
                    if let Some(caret) = caret_from_event(event, editor) {
                        let selection =
                            set_selection_from_caret_range(&get_collapsed_caret_range(caret))?;
                        normalize_selection(&selection)?;
                        set_selection(Some(selection));
                    }
                    // The drop point does not always resolve to a caret; the
                    // files are forwarded regardless, or the drop would be
                    // discarded.
                    editor.dispatch_command(&DRAG_DROP_PASTE, files)?;
                    event.prevent_default();
                    return Ok(true);
                }
                handle_rich_text_drop(event, editor)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &DRAGSTART_COMMAND,
            |event, editor| {
                let (is_file_transfer, _, _) = event_files(FilesEvent::Drag(event));
                let selection = range_selection();
                if is_file_transfer && selection.is_none() {
                    return Ok(false);
                }
                if let Some(selection) = selection
                    && !selection.range().is_some_and(|r| r.is_collapsed())
                    && let Some(data_transfer) = &event.data_transfer
                {
                    // Lexical's own serialization lets custom nodes survive a
                    // drop back into a Lexical editor; the marker lets another
                    // editor remove the source range.
                    set_lexical_clipboard_data_transfer(
                        data_transfer,
                        &get_clipboard_data_from_selection(Some(&selection))?,
                    );
                    write_drag_source_to_data_transfer(data_transfer, editor);
                }
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &DRAGOVER_COMMAND,
            |event, editor| {
                let (is_file_transfer, _, _) = event_files(FilesEvent::Drag(event));
                if is_file_transfer && range_selection().is_none() {
                    return Ok(false);
                }
                // Only a decorator, which shows no drop caret, cancels the
                // dragover; text drags keep the platform's drop handling.
                if caret_from_event(event, editor)
                    .is_some_and(|caret| caret.origin().is_decorator())
                {
                    event.prevent_default();
                }
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &COPY_COMMAND,
            |event, editor| {
                copy_to_clipboard(
                    editor,
                    ClipboardOrKeyboardEvent::clipboard_event(event.as_ref()).cloned(),
                )?;
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &CUT_COMMAND,
            |event, editor| {
                on_cut_for_rich_text(event.as_ref(), editor)?;
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &PASTE_COMMAND,
            move |event, editor| {
                let (_, files, has_text_content) = event_files(FilesEvent::Paste(event));
                if (should_handle_paste_as_files().0)(&files, has_text_content)
                    && editor.dispatch_command(&DRAG_DROP_PASTE, files)?
                {
                    return Ok(true);
                }
                if get_selection().is_some() {
                    on_paste_for_rich_text(event, editor)?;
                    return Ok(true);
                }
                Ok(false)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        {
            let triggers = triggers.clone();
            editor.register_command(
                &KEY_SPACE_COMMAND,
                move |_, _| {
                    if let Some(selection) = range_selection() {
                        escape_formats_for_trigger(
                            &selection,
                            EscapeFormatTrigger::Space,
                            EscapeDirection::Both,
                            &triggers(),
                        )?;
                    }
                    Ok(false)
                },
                COMMAND_PRIORITY_EDITOR,
            )
        },
        editor.register_command(
            &KEY_TAB_COMMAND,
            move |_, _| {
                if let Some(selection) = range_selection() {
                    escape_formats_for_trigger(
                        &selection,
                        EscapeFormatTrigger::Tab,
                        EscapeDirection::Both,
                        &triggers(),
                    )?;
                }
                Ok(false)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &MOVE_TO_END,
            |event, _| {
                let selection = get_selection();
                if let Some(selection) = selection.as_ref().filter(|s| s.is_node()) {
                    return promote_node_selection_to_block_edge(selection, false, event);
                }
                let Some(selection) = selection.filter(SelectionRef::is_range) else {
                    return Ok(false);
                };
                let anchor = selection.point(Which::Anchor).expect("a range selection");
                if anchor.point_type != PointType::Element || anchor.offset != 0 {
                    return Ok(false);
                }
                let element = anchor.get_node()?;
                if !element.is_element() {
                    return Ok(false);
                }
                if !element
                    .get_first_child()
                    .is_some_and(|child| child.is_decorator() && child.is_inline())
                {
                    return Ok(false);
                }
                // The platform's caret stops at an inline decorator at
                // element offset 0, so move it here; `selectEnd` handles
                // every last-descendant case.
                let ending = element.select_end()?;
                if event.shift_key {
                    ending.point_set(Which::Anchor, element.get_key(), 0, PointType::Element)?;
                }
                event.prevent_default();
                event.stop_propagation();
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
        editor.register_command(
            &MOVE_TO_START,
            |event, _| {
                let selection = get_selection();
                if let Some(selection) = selection.as_ref().filter(|s| s.is_node()) {
                    return promote_node_selection_to_block_edge(selection, true, event);
                }
                let Some(selection) = selection.filter(SelectionRef::is_range) else {
                    return Ok(false);
                };
                let range = selection.range().expect("a range selection");
                let (anchor, focus) = (&range.anchor, &range.focus);
                let Some(focus_block) = nearest_block(&focus.get_node()?) else {
                    return Ok(false);
                };
                if !focus_block
                    .get_first_child()
                    .is_some_and(|child| child.is_decorator() && child.is_inline())
                {
                    return Ok(false);
                }
                // Cross-block selections fall through to the platform.
                if nearest_block(&anchor.get_node()?).as_ref() != Some(&focus_block) {
                    return Ok(false);
                }
                let block_key = focus_block.get_key();
                if focus.point_type == PointType::Element
                    && focus.key == block_key
                    && focus.offset == 0
                {
                    return Ok(false);
                }
                selection.point_set(Which::Focus, block_key, 0, PointType::Element)?;
                if !event.shift_key {
                    selection.point_set(Which::Anchor, block_key, 0, PointType::Element)?;
                }
                event.prevent_default();
                event.stop_propagation();
                Ok(true)
            },
            COMMAND_PRIORITY_EDITOR,
        ),
    ];
    Box::new(move || {
        for unregister in unregisters {
            unregister();
        }
    })
}
