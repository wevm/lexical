//! `CodeNode.ts`: `CodeNode`, `$createCodeNode`, `$isCodeNode`,
//! `DEFAULT_CODE_LANGUAGE`, `getDefaultCodeLanguage` and
//! `$exitCodeNodeOnEnter`.

use super::code_highlight_node::{create_code_highlight_node, is_code_highlight_node};
use super::flat_structure_utils::get_first_code_node_of_line;
use super::generated_json::{export_code_node, export_compact_code_node, update_code_node};
use crate::LexicalError;
use crate::json::Object;
use crate::lexical_selection::{PointType, SelectionRef, Which};
use crate::lexical_utils::{set_node_key, with_writable};
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::nodes::element::ElementData;
use crate::nodes::line_break::{LineBreakNode, create_line_break_node};
use crate::nodes::paragraph::create_paragraph_node;
use crate::nodes::tab::{TabNode, create_tab_node};

/// `DEFAULT_CODE_LANGUAGE`.
pub const DEFAULT_CODE_LANGUAGE: &str = "javascript";

/// `getDefaultCodeLanguage()`.
pub fn get_default_code_language() -> &'static str {
    DEFAULT_CODE_LANGUAGE
}

/// `CodeNode`: a code block, holding [`CodeHighlightNode`]s, tabs and line
/// breaks.
///
/// [`CodeHighlightNode`]: super::CodeHighlightNode
#[derive(Clone, Debug, Default)]
pub struct CodeNode {
    base: NodeBase,
    pub(super) element: ElementData,
    /// `__language`: `None` where JS's is `undefined`. JS's constructor and
    /// setter turn every falsy language into `undefined`, so a `null` never
    /// stays.
    pub(super) language: Option<String>,
    /// `__theme`, as `language`.
    pub(super) theme: Option<String>,
    /// `__isSyntaxHighlightSupported`: set by a highlighter, not serialized,
    /// and carried across a clone (`afterCloneFrom`).
    pub(super) is_syntax_highlight_supported: bool,
}

impl CodeNode {
    /// `__language`.
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    /// `__theme`.
    pub fn theme(&self) -> Option<&str> {
        self.theme.as_deref()
    }

    /// `__isSyntaxHighlightSupported`.
    pub fn is_syntax_highlight_supported(&self) -> bool {
        self.is_syntax_highlight_supported
    }
}

/// `language || undefined`.
fn truthy(value: Option<&str>) -> Option<String> {
    value.filter(|v| !v.is_empty()).map(str::to_owned)
}

/// `$createCodeNode(language, theme)`: `None` and `""` are both no
/// language (no theme).
pub fn create_code_node(language: Option<&str>, theme: Option<&str>) -> NodeRef {
    let node = set_node_key(Box::new(CodeNode::default()));
    node.set_language(language);
    node.set_theme(theme);
    node
}

/// `$isCodeNode(node)`.
pub fn is_code_node(node: &NodeRef) -> bool {
    node.get_latest().is::<CodeNode>()
}

fn with_code<R>(node: &NodeRef, f: impl FnOnce(&CodeNode) -> R) -> Option<R> {
    node.get_latest().downcast_ref::<CodeNode>().map(f)
}

fn set_code(node: &NodeRef, f: impl FnOnce(&mut CodeNode)) {
    if is_code_node(node) {
        with_writable(node.get_key(), |n| {
            f(n.downcast_mut::<CodeNode>().expect("a code node"))
        });
    }
}

impl NodeRef {
    /// `CodeNode.getLanguage()`; `None` when it is `undefined`, or for any
    /// other node.
    pub fn get_language(&self) -> Option<String> {
        with_code(self, |c| c.language.clone()).flatten()
    }

    /// `CodeNode.setLanguage(language)`: an empty one is `undefined`. Does
    /// nothing to any other node.
    pub fn set_language(&self, language: Option<&str>) {
        set_code(self, |c| c.language = truthy(language));
    }

    /// `CodeNode.getTheme()`; `None` when it is `undefined`, or for any
    /// other node.
    pub fn get_theme(&self) -> Option<String> {
        with_code(self, |c| c.theme.clone()).flatten()
    }

    /// `CodeNode.setTheme(theme)`: an empty one is `undefined`. Does nothing
    /// to any other node.
    pub fn set_theme(&self, theme: Option<&str>) {
        set_code(self, |c| c.theme = truthy(theme));
    }

    /// `CodeNode.getIsSyntaxHighlightSupported()`; false for any other node.
    pub fn get_is_syntax_highlight_supported(&self) -> bool {
        with_code(self, |c| c.is_syntax_highlight_supported).unwrap_or(false)
    }

    /// `CodeNode.setIsSyntaxHighlightSupported(isSupported)`. Does nothing
    /// to any other node.
    pub fn set_is_syntax_highlight_supported(&self, is_supported: bool) {
        set_code(self, |c| c.is_syntax_highlight_supported = is_supported);
    }
}

/// `CodeNode.insertNewAfter(selection, restoreSelection)`: a line break,
/// followed by the leading tabs and spaces of the line the selection starts
/// on, so the new line keeps its indentation. JS first exits the block
/// (`$exitCodeNodeOnEnter`) when the editor has no `CodeExtension`, which
/// handles that on `KEY_ENTER_COMMAND` instead; this port has no extension
/// API, so it always does, as a JS editor made with `createEditor` does.
fn insert_new_after(selection: &SelectionRef) -> Result<Option<NodeRef>, LexicalError> {
    // `noExtensionDeprecation()` warns once; there is no console here.
    if let Some(el) = exit_code_node_on_enter(selection)? {
        return Ok(Some(el));
    }
    // If the selection is within the codeblock, find all leading tabs and
    // spaces of the current line. Create a new line that has all those
    // tabs and spaces, such that leading indentation is preserved.
    let anchor = |s: &SelectionRef| s.point(Which::Anchor).expect("a range selection");
    let focus = selection.point(Which::Focus).expect("a range selection");
    let first_point = if anchor(selection).is_before(&focus)? {
        anchor(selection)
    } else {
        focus
    };
    let first_selection_node = first_point.get_node()?;
    if first_selection_node.is_text() {
        let mut node = Some(get_first_code_node_of_line(&first_selection_node));
        let mut insert_nodes = Vec::new();
        while let Some(n) = node {
            if n.get_latest().is::<TabNode>() {
                insert_nodes.push(create_tab_node());
                node = n.get_next_sibling();
            } else if is_code_highlight_node(&n) {
                let text = n.get_text_content();
                let text_size = n.get_text_content_size();
                // A space is one UTF-16 unit, so counting characters counts
                // units.
                let spaces = text.chars().take_while(|&c| c == ' ').count();
                if spaces != 0 {
                    insert_nodes.push(create_code_highlight_node(&" ".repeat(spaces), None));
                }
                if spaces != text_size {
                    break;
                }
                node = n.get_next_sibling();
            } else {
                break;
            }
        }
        let anchor_offset = anchor(selection).offset;
        // `splitText(...)[0]` of an empty node is `undefined`, which JS
        // then reads a property of.
        let Some(split) = first_selection_node
            .split_text(&[anchor_offset as i64])?
            .into_iter()
            .next()
        else {
            return Err(LexicalError::new(
                "Cannot read properties of undefined (reading 'getIndexWithinParent')",
            ));
        };
        // JS reads `anchor.offset` again here, after the split moved it.
        let x = if anchor(selection).offset == 0 { 0 } else { 1 };
        let index = split
            .get_index_within_parent()
            .expect("a split text node has a parent")
            + x;
        let code_node = first_selection_node.get_parent_or_throw()?;
        let mut nodes_to_insert = vec![create_line_break_node()];
        nodes_to_insert.extend(insert_nodes.iter().cloned());
        code_node.splice(index, 0, &nodes_to_insert)?;
        if let Some(last) = insert_nodes.last() {
            last.select(None, None)?;
        } else if anchor(selection).offset == 0 {
            split.select_previous(None, None)?;
        } else {
            split
                .get_next_sibling()
                .expect("the line break inserted after the split")
                .select_next(Some(0), Some(0))?;
        }
    }
    if is_code_node(&first_selection_node) {
        let offset = anchor(selection).offset;
        first_selection_node.splice(offset, 0, &[create_line_break_node()])?;
        first_selection_node.select(Some(offset + 1), Some(offset + 1))?;
    }
    Ok(None)
}

impl LexicalNode for CodeNode {
    fn get_type(&self) -> &'static str {
        "code"
    }
    /// `CodeNode.createDOM(config)`: a `<code>`.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        Ok(code_dom::code_element(self, "code", &config.theme))
    }
    /// `CodeNode.exportDOM(editor)`: a `<pre>`, with what `createDOM`
    /// writes and without the indent and direction.
    #[cfg(feature = "html")]
    fn export_dom(
        &self,
        _this: &NodeRef,
        editor: &crate::Editor,
    ) -> Result<crate::lexical_dom::DomExportOutput, LexicalError> {
        Ok(crate::lexical_dom::DomExportOutput::element(
            code_dom::code_element(self, "pre", &editor.config().theme),
        ))
    }
    #[cfg(feature = "html")]
    fn import_dom(&self) -> Option<crate::lexical_dom::DomConversionMap> {
        Some(code_dom::import_dom())
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

    /// `CodeNode.canIndent()`.
    fn can_indent(&self) -> bool {
        false
    }

    /// `CodeNode.insertNewAfter(selection, restoreSelection)`.
    fn insert_new_after(
        &self,
        _: &NodeRef,
        selection: &SelectionRef,
        _: bool,
    ) -> Result<Option<NodeRef>, LexicalError> {
        insert_new_after(selection)
    }

    /// `CodeNode.collapseAtStart()`: the block becomes a paragraph with its
    /// children.
    fn collapse_at_start(&self, this: &NodeRef, _: &SelectionRef) -> Result<bool, LexicalError> {
        let paragraph = create_paragraph_node();
        for child in this.get_children() {
            paragraph.append(std::slice::from_ref(&child))?;
        }
        this.replace(&paragraph, false)?;
        Ok(true)
    }

    /// `exportCodeNode` and `exportCompactCodeNode`.
    fn export_json(&self, compact: bool) -> Object {
        if compact {
            export_compact_code_node(self)
        } else {
            export_code_node(self)
        }
    }

    /// `updateCodeNode`.
    fn update_from_json(&mut self, json: &Object) {
        update_code_node(self, json);
    }
}

/// `$exitCodeNodeOnEnter(selection)`: at the end of a code block that ends
/// with two line breaks, a collapsed selection removes them and moves to a
/// new paragraph after the block, which it returns.
pub fn exit_code_node_on_enter(selection: &SelectionRef) -> Result<Option<NodeRef>, LexicalError> {
    let Some(range) = selection.range() else {
        return Ok(None);
    };
    let anchor = &range.anchor;
    if range.is_collapsed() && anchor.point_type == PointType::Element {
        let code_node = anchor.get_node()?;
        if is_code_node(&code_node) {
            let children_size = code_node.get_children_size();
            if children_size >= 2 && anchor.offset == children_size {
                let last_child = code_node.get_last_child();
                let is_line_break = |n: &Option<NodeRef>| {
                    n.as_ref()
                        .is_some_and(|n| n.get_latest().is::<LineBreakNode>())
                };
                if is_line_break(&last_child)
                    && is_line_break(&last_child.as_ref().and_then(NodeRef::get_previous_sibling))
                {
                    let new_element = create_paragraph_node();
                    code_node.splice(children_size - 2, 2, &[])?;
                    code_node.insert_after(&new_element, false)?;
                    new_element.select(None, None)?;
                    return Ok(Some(new_element));
                }
            }
        }
    }
    Ok(None)
}

// The DOM side of `CodeNode` (the `html` feature).

#[cfg(feature = "html")]
pub(super) mod code_dom {
    use super::{CodeNode, create_code_node};
    use crate::LexicalError;
    use crate::dom::DomNode;
    use crate::json::Object;
    use crate::lexical_dom::{
        DomConversion, DomConversionMap, DomConversionOutput, add_theme_class_names, closest,
        get_document, importer,
    };
    use crate::set_dom_style::set_dom_style_from_css;
    use std::rc::Rc;

    /// `LANGUAGE_DATA_ATTRIBUTE`.
    const LANGUAGE_DATA_ATTRIBUTE: &str = "data-language";
    /// `HIGHLIGHT_LANGUAGE_DATA_ATTRIBUTE`.
    const HIGHLIGHT_LANGUAGE_DATA_ATTRIBUTE: &str = "data-highlight-language";
    /// `THEME_DATA_ATTRIBUTE`.
    const THEME_DATA_ATTRIBUTE: &str = "data-theme";

    /// What `createDOM` (a `<code>`) and `exportDOM` (a `<pre>`) both
    /// write: the theme's `code` class names, `spellcheck`, the language
    /// and theme attributes, and the style.
    pub(crate) fn code_element(code: &CodeNode, tag: &str, theme: &Object) -> DomNode {
        let element = get_document().create_element(tag);
        add_theme_class_names(&element, theme, "code");
        element.set_attribute("spellcheck", "false");
        if let Some(language) = code.language() {
            element.set_attribute(LANGUAGE_DATA_ATTRIBUTE, language);
            if code.is_syntax_highlight_supported() {
                element.set_attribute(HIGHLIGHT_LANGUAGE_DATA_ATTRIBUTE, language);
            }
        }
        if let Some(theme) = code.theme() {
            element.set_attribute(THEME_DATA_ATTRIBUTE, theme);
        }
        let style = code.element.get_style();
        if !style.is_empty() {
            set_dom_style_from_css(&element.style(), style, "");
        }
        element
    }

    /// `hasChildDOMNodeTag(node, tagName)`.
    fn has_child_dom_node_tag(node: &DomNode, tag_name: &str) -> bool {
        node.child_nodes().iter().any(|child| {
            (child.is_element() && child.tag_name() == tag_name)
                || has_child_dom_node_tag(child, tag_name)
        })
    }

    /// `CodeNode`'s `importDOM`: `<pre>`, a multi-line `<code>`, a `<div>`
    /// in a monospace font, and GitHub's code tables.
    pub(crate) fn import_dom() -> DomConversionMap {
        let noop = || -> crate::lexical_dom::DomConversionFn {
            Rc::new(|_| Ok(Some(DomConversionOutput::node(None))))
        };
        vec![
            (
                "code".to_owned(),
                Rc::new(|node: &DomNode| {
                    let is_multi_line =
                        node.text_content().contains('\n') || has_child_dom_node_tag(node, "BR");
                    is_multi_line.then(|| DomConversion {
                        conversion: Rc::new(convert_pre_element),
                        priority: 1,
                    })
                }),
            ),
            ("div".to_owned(), importer(1, convert_div_element)),
            ("pre".to_owned(), importer(0, convert_pre_element)),
            (
                "table".to_owned(),
                Rc::new(|node: &DomNode| {
                    is_git_hub_code_table(node).then(|| DomConversion {
                        conversion: Rc::new(|_| {
                            Ok(Some(DomConversionOutput::node(Some(create_code_node(
                                None, None,
                            )))))
                        }),
                        priority: 3,
                    })
                }),
            ),
            (
                "td".to_owned(),
                Rc::new(move |node: &DomNode| {
                    let table = closest(node, "table");
                    (is_git_hub_code_cell(node)
                        || table.as_ref().is_some_and(is_git_hub_code_table))
                    .then(|| DomConversion {
                        conversion: noop(),
                        priority: 3,
                    })
                }),
            ),
            (
                "tr".to_owned(),
                Rc::new(move |node: &DomNode| {
                    closest(node, "table")
                        .as_ref()
                        .is_some_and(is_git_hub_code_table)
                        .then(|| DomConversion {
                            conversion: noop(),
                            priority: 3,
                        })
                }),
            ),
        ]
    }

    /// `$convertPreElement(domNode)`: a code block with the element's
    /// language and theme.
    fn convert_pre_element(
        dom_node: &DomNode,
    ) -> Result<Option<DomConversionOutput>, LexicalError> {
        let language = dom_node.get_attribute(LANGUAGE_DATA_ATTRIBUTE);
        let theme = dom_node.get_attribute(THEME_DATA_ATTRIBUTE);
        Ok(Some(DomConversionOutput::node(Some(create_code_node(
            language.as_deref(),
            theme.as_deref(),
        )))))
    }

    /// `$convertDivElement(domNode)`: a code block for a `<div>` in a
    /// monospace font.
    fn convert_div_element(
        dom_node: &DomNode,
    ) -> Result<Option<DomConversionOutput>, LexicalError> {
        let is_code = is_code_element(dom_node);
        Ok(Some(DomConversionOutput::node(
            is_code.then(|| create_code_node(None, None)),
        )))
    }

    /// `isCodeElement(div)`: `font-family` names `monospace`.
    fn is_code_element(div: &DomNode) -> bool {
        div.style()
            .get_property_value("font-family")
            .contains("monospace")
    }

    /// `isGitHubCodeCell(cell)`.
    fn is_git_hub_code_cell(cell: &DomNode) -> bool {
        cell.class_list().contains("js-file-line")
    }

    /// `isGitHubCodeTable(table)`.
    fn is_git_hub_code_table(table: &DomNode) -> bool {
        table.class_list().contains("js-file-line-container")
    }
}
