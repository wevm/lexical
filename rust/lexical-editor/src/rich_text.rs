//! `@lexical/rich-text` (the `rich-text` feature): `HeadingNode` and
//! `QuoteNode`, from `packages/lexical-rich-text/src/index.ts`, with their
//! generated JSON (`LexicalRichTextGeneratedJSON.ts`).
//!
//! `registerRichText`'s command handlers ([`register_rich_text`]) that act
//! on the model. The handlers of DOM events (click, arrow keys, Backspace,
//! Delete, Enter, Escape, Space, Tab, copy, cut, paste and drag and drop)
//! wait for an input layer, and the DOM parts (`createDOM`, `exportDOM`,
//! `importDOM`) come with `html`.

use crate::commands::{
    CONTROLLED_TEXT_INSERTION_COMMAND, DELETE_CHARACTER_COMMAND, DELETE_LINE_COMMAND,
    DELETE_WORD_COMMAND, FORMAT_ELEMENT_COMMAND, FORMAT_TEXT_COMMAND, INDENT_CONTENT_COMMAND,
    INSERT_LINE_BREAK_COMMAND, INSERT_PARAGRAPH_COMMAND, INSERT_TAB_COMMAND,
    OUTDENT_CONTENT_COMMAND, REMOVE_TEXT_COMMAND, SELECT_ALL_COMMAND, SET_TEXT_FORMAT_COMMAND,
};
use crate::editor::COMMAND_PRIORITY_EDITOR;
use crate::generated_json::{
    export_compact_element_node, export_element_node, update_element_node,
};
use crate::json::{Object, Value};
use crate::lexical_selection::SelectionRef;
use crate::lexical_selection::get_selection;
use crate::lexical_utils::{set_node_key, with_writable};
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::node_state::{
    RequiredNodeStateConfig, StateConfig, create_state, get_state_direct, set_state,
};
use crate::nodes::element::ElementData;
use crate::nodes::paragraph::create_paragraph_node;
use crate::nodes::tab::create_tab_node;
use crate::selection_insert_nodes::{insert_nodes, select_all};
use crate::utils::{find_matching_parent, handle_indent_and_outdent};
use crate::utils::{is_at_end_of_node, is_at_start_of_node};
use crate::{Editor, LexicalError, NodeClass, Unregister};

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

impl LexicalNode for HeadingNode {
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
    fn export_json(&self, compact: bool) -> Object {
        if compact {
            let tag = Some(self.tag).filter(|t| *t != HeadingTagType::H1);
            with_tag(export_compact_element_node(self), tag)
        } else {
            with_tag(export_element_node(self), Some(self.tag))
        }
    }

    /// `updateHeadingNode`.
    fn update_from_json(&mut self, json: &Object) {
        update_element_node(&mut self.element, json);
        self.tag = json
            .get("tag")
            .and_then(Value::as_str)
            .and_then(HeadingTagType::parse)
            .unwrap_or_default();
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
    fn export_json(&self, compact: bool) -> Object {
        if compact {
            export_compact_element_node(self)
        } else {
            export_element_node(self)
        }
    }

    /// `updateQuoteNode` (the flat `shadowRoot` is applied with the rest of
    /// the NodeState).
    fn update_from_json(&mut self, json: &Object) {
        update_element_node(&mut self.element, json);
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

/// `registerRichText(editor)`: registers the rich text command handlers
/// that act on the model, at `COMMAND_PRIORITY_EDITOR`, and returns what
/// unregisters them.
///
/// JS's `escapeFormatTriggers` (for click and arrow keys) and
/// `shouldHandlePasteAsFiles` (for paste) go with those events' handlers,
/// which aren't ported. `CONTROLLED_TEXT_INSERTION_COMMAND` takes only the
/// string form, and `SELECT_ALL_COMMAND` always selects the whole document,
/// as JS does outside a slot (slots aren't ported).
pub fn register_rich_text(editor: &Editor) -> Unregister {
    let unregisters: Vec<Unregister> = vec![
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
            &SELECT_ALL_COMMAND,
            |_, _| {
                select_all()?;
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
