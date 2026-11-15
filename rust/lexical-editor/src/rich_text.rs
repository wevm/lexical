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

    /// `QuoteNode.setIsShadowRoot(isShadowRoot)`. Does nothing to any other
    /// node.
    pub fn set_quote_is_shadow_root(&self, is_shadow_root: bool) {
        if is_quote_node(self) {
            with_writable(self.get_key(), |n| {
                n.downcast_mut::<QuoteNode>().expect("a quote").shadow_root = is_shadow_root;
            });
        }
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

/// `QuoteNode`: a block quote. With `quoteShadowRootState` set
/// ([`NodeRef::set_quote_is_shadow_root`]) it is a shadow root holding
/// blocks instead of inline content.
///
/// `quoteShadowRootState` is a flat NodeState in JS. NodeState isn't ported
/// (rust/PARITY.md), so the state is a field here, read and written the way
/// a flat state is: `shadowRoot` is applied only when the JSON has it
/// (`booleanValue()`: anything but a boolean is `false`), and written at the
/// end of the JSON, after the generated properties, only when `true`.
#[derive(Clone, Debug, Default)]
pub struct QuoteNode {
    base: NodeBase,
    element: ElementData,
    shadow_root: bool,
}

/// `$createQuoteNode({shadowRoot})`.
pub fn create_quote_node(shadow_root: bool) -> NodeRef {
    set_node_key(Box::new(QuoteNode {
        shadow_root,
        ..QuoteNode::default()
    }))
}

/// `$isQuoteNode(node)`.
pub fn is_quote_node(node: &NodeRef) -> bool {
    node.get_latest().is::<QuoteNode>()
}

impl LexicalNode for QuoteNode {
    fn get_type(&self) -> &'static str {
        "quote"
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

    /// `QuoteNode.isShadowRoot()`.
    fn is_shadow_root(&self) -> bool {
        self.shadow_root
    }

    /// `QuoteNode.canMergeWhenEmpty()`.
    fn can_merge_when_empty(&self) -> bool {
        true
    }

    /// `exportQuoteNode` and `exportCompactQuoteNode`, then the flat state.
    fn export_json(&self, compact: bool) -> Object {
        let mut json = if compact {
            export_compact_element_node(self)
        } else {
            export_element_node(self)
        };
        if self.shadow_root {
            json.insert("shadowRoot", true.into());
        }
        json
    }

    /// The flat state (`$applyFlatStates`), then `updateQuoteNode`.
    fn update_from_json(&mut self, json: &Object) {
        if let Some(raw) = json.get("shadowRoot").filter(|v| !v.is_undefined()) {
            self.shadow_root = matches!(raw, Value::Bool(true));
        }
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
        if self.shadow_root {
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
