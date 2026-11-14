//! `@lexical/html` (the `html` feature): HTML from an editor state
//! ([`generate_html_from_nodes`]) and Lexical nodes from a DOM
//! ([`generate_nodes_from_dom`]), through the nodes' `exportDOM` and
//! `importDOM` and the editor's `html` config. The DOM is
//! [`crate::dom`]; parse HTML into it with [`crate::dom::parse_from_string`].
//!
//! From `packages/lexical-html/src/index.ts`. Not ported: the
//! `DOMImportExtension` and `DOMRenderExtension` pipelines and their rules,
//! the render context, and `$inlineStylesFromStyleSheets`, which needs a CSS
//! selector engine (rust/PARITY.md).

use crate::dom::DomNode;
use crate::lexical_dom::{
    DomChildConversion, DomConversionNode, export_dom, extract_with_child, get_conversion_function,
    get_document, is_block_dom_node, is_inline_dom_node, should_exclude, should_include,
};
use crate::lexical_selection::SelectionRef;
use crate::node::{LexicalNode, NodeRef};
use crate::nodes::artificial::{create_artificial_node, is_artificial_node};
use crate::nodes::line_break::create_line_break_node;
use crate::nodes::paragraph::create_paragraph_node;
use crate::{Editor, LexicalError, get_root};
use std::rc::Rc;

/// `IGNORE_TAGS`.
const IGNORE_TAGS: [&str; 2] = ["STYLE", "SCRIPT"];

/// `$generateNodesFromDOM(editor, dom)`: the Lexical nodes for a document's
/// body (or a node's children), made by the editor's import conversions.
/// Inline content outside a block goes into paragraphs. Call it inside an
/// update.
pub fn generate_nodes_from_dom(
    editor: &Editor,
    dom: &DomNode,
) -> Result<Vec<NodeRef>, LexicalError> {
    let elements = if dom.is_document() {
        dom.body().map(|b| b.child_nodes()).unwrap_or_default()
    } else {
        dom.child_nodes()
    };
    let mut lexical_nodes = Vec::new();
    let mut all_artificial_nodes = Vec::new();
    for element in elements {
        if !IGNORE_TAGS.contains(&element.node_name().as_str()) {
            lexical_nodes.extend(create_nodes_from_dom(
                &element,
                editor,
                &mut all_artificial_nodes,
                false,
                ForChildMap::default(),
                None,
            )?);
        }
    }
    unwrap_artificial_nodes(&all_artificial_nodes)?;
    Ok(lexical_nodes)
}

/// `$generateDOMFromNodes(container, selection, editor)`: appends the DOM
/// of the root's children (or of what `selection` covers) to `container`.
pub fn generate_dom_from_nodes(
    container: &DomNode,
    selection: Option<&SelectionRef>,
    editor: &Editor,
) -> Result<(), LexicalError> {
    for top_level_node in get_root().get_children() {
        append_nodes_to_html(editor, &top_level_node, container, selection)?;
    }
    Ok(())
}

/// `$generateDOMFromRoot(container, root)`: appends the DOM of `root`,
/// itself included, to `container`.
pub fn generate_dom_from_root(
    container: &DomNode,
    root: &NodeRef,
    editor: &Editor,
) -> Result<(), LexicalError> {
    append_nodes_to_html(editor, root, container, None).map(|_| ())
}

/// `$generateHtmlFromNodes(editor, selection)`: the HTML of the editor
/// state (or of what `selection` covers). Call it inside
/// [`Editor::read`] or an update.
pub fn generate_html_from_nodes(
    editor: &Editor,
    selection: Option<&SelectionRef>,
) -> Result<String, LexicalError> {
    let container = get_document().create_element("div");
    generate_dom_from_nodes(&container, selection, editor)?;
    Ok(container.inner_html())
}

/// `$appendNodeToHTML(editor, node, parentElement, selection)`: appends
/// the DOM of one node and its subtree, as the exporter does for each node
/// it walks. Returns whether the node was included.
pub fn append_node_to_html(
    editor: &Editor,
    node: &NodeRef,
    parent_element: &DomNode,
    selection: Option<&SelectionRef>,
) -> Result<bool, LexicalError> {
    append_nodes_to_html(editor, node, parent_element, selection)
}

/// `$appendTerminatingLineBreak(element, lastIncludedChild)`: a block
/// whose last child is a line break gets the managed `<br>` the reconciler
/// adds, since a browser doesn't render a block's last `<br>`.
fn append_terminating_line_break(element: &DomNode, last_included_child: Option<&NodeRef>) {
    let is_line_break = last_included_child.is_some_and(|n| n.get_type() == "linebreak");
    if is_line_break
        && element.is_element()
        && is_block_dom_node(element)
        && element.last_child().is_some_and(|c| c.node_name() == "BR")
    {
        let br = get_document().create_element("br");
        br.set_attribute("data-lexical-managed-linebreak", "true");
        element.append_child(&br);
    }
}

/// `$sliceSelectedTextNodeContent(selection, textNode, 'clone')`
/// (`@lexical/selection`): the text node's value, cut to the part the
/// selection covers when an end of the selection is in it.
fn slice_selected_text_node_content(
    selection: &SelectionRef,
    text_node: &NodeRef,
) -> Result<Rc<dyn LexicalNode>, LexicalError> {
    let latest = text_node.get_latest();
    let Some(range) = selection.range() else {
        return Ok(latest);
    };
    let key = text_node.get_key();
    let (anchor, focus) = (&range.anchor, &range.focus);
    if (anchor.key != key && focus.key != key)
        || !text_node.is_selected(Some(selection))?
        || text_node.is_token()
        || text_node.is_segmented()
    {
        return Ok(latest);
    }
    let (start, end) = if selection.is_backward()? {
        (focus, anchor)
    } else {
        (anchor, focus)
    };
    let text = &latest.text().expect("a text node").text;
    let (start_offset, end_offset) = if start.point_type == crate::PointType::Element
        || end.point_type == crate::PointType::Element
    {
        let slices = crate::caret_range_from_selection(&range)?.get_text_slices();
        match crate::get_text_point_caret_slice_for_node(&slices, text_node) {
            Some(slice) => {
                let (s, e) = slice.get_slice_indices();
                (s, Some(e))
            }
            None => (
                if start.key == key { start.offset } else { 0 },
                (end.key == key).then_some(end.offset),
            ),
        }
    } else {
        (
            if start.key == key { start.offset } else { 0 },
            (end.key == key).then_some(end.offset),
        )
    };
    // JS's slice clamps its ends and takes an empty slice when they cross.
    let len = crate::utf16::len(text);
    let end_offset = end_offset.unwrap_or(len).min(len);
    let start_offset = start_offset.min(end_offset);
    let sliced = crate::utf16::slice(text, start_offset, end_offset);
    if sliced == *text {
        return Ok(latest);
    }
    let mut clone = latest.clone_node();
    clone.text_mut().expect("a text node").text = sliced;
    Ok(Rc::from(clone))
}

/// `$appendNodesToHTML(editor, currentNode, parentElementAppend,
/// selection, domConfig)`: appends the node's DOM, with its children's,
/// to `parent`, or only its children's when the node isn't included.
/// Returns whether it was included.
fn append_nodes_to_html(
    editor: &Editor,
    current_node: &NodeRef,
    parent: &DomNode,
    selection: Option<&SelectionRef>,
) -> Result<bool, LexicalError> {
    let mut include = should_include(current_node, selection)?;
    let exclude = should_exclude(current_node);
    let target = match selection {
        Some(selection) if current_node.is_text() => {
            slice_selected_text_node_content(selection, current_node)?
        }
        _ => current_node.get_latest(),
    };
    let export_props = export_dom(&*target, current_node, editor)?;
    let Some(element) = export_props.element else {
        return Ok(false);
    };
    let fragment = get_document().create_document_fragment();
    let children = match export_props.get_child_nodes {
        Some(get_child_nodes) => get_child_nodes(),
        None if current_node.is_element() => current_node.get_children(),
        None => Vec::new(),
    };
    // A whole element host in a node selection exports all of its subtree.
    let child_selection =
        if include && selection.is_some_and(SelectionRef::is_node) && current_node.is_element() {
            None
        } else {
            selection
        };
    let mut last_included_child = None;
    for child_node in &children {
        let include_child = append_nodes_to_html(editor, child_node, &fragment, child_selection)?;
        if include_child {
            last_included_child = Some(child_node.clone());
        }
        if !include
            && include_child
            && extract_with_child(current_node, child_node, selection, "html")?
        {
            include = true;
        }
    }
    if include && !exclude {
        if element.is_element() || element.is_document_fragment() {
            match export_props.append {
                Some(append) => append(&fragment)?,
                None => {
                    element.append_child(&fragment);
                }
            }
            append_terminating_line_break(&element, last_included_child.as_ref());
        }
        if element.is_document_fragment() {
            if let Some(after) = export_props.after
                && let Some(new_element) = after(&element)?
            {
                element.replace_children(&[new_element]);
            }
            parent.append_child(&element);
        } else {
            parent.append_child(&element);
            if let Some(after) = export_props.after
                && let Some(new_element) = after(&element)?
            {
                element.replace_with(&[new_element]);
            }
        }
    } else {
        parent.append_child(&fragment);
    }
    Ok(include)
}

/// The `forChild` conversions in effect, by the DOM node name that set
/// them, in the order first set (a JS `Map`).
#[derive(Clone, Default)]
struct ForChildMap(Vec<(String, DomChildConversion)>);

impl ForChildMap {
    fn set(&mut self, key: String, conversion: DomChildConversion) {
        match self.0.iter_mut().find(|(k, _)| *k == key) {
            Some(entry) => entry.1 = conversion,
            None => self.0.push((key, conversion)),
        }
    }
}

/// `$isBlockElementNode(node)`: an element that isn't inline.
fn is_block_element_node(node: &NodeRef) -> bool {
    node.is_element() && !node.is_inline()
}

/// `$createNodesFromDOM(node, editor, allArtificialNodes,
/// hasBlockAncestorLexicalNode, forChildMap, parentLexicalNode)`.
fn create_nodes_from_dom(
    node: &DomNode,
    editor: &Editor,
    all_artificial_nodes: &mut Vec<NodeRef>,
    has_block_ancestor_lexical_node: bool,
    mut for_child_map: ForChildMap,
    parent_lexical_node: Option<&NodeRef>,
) -> Result<Vec<NodeRef>, LexicalError> {
    let mut lexical_nodes = Vec::new();
    let node_name = node.node_name();
    if IGNORE_TAGS.contains(&node_name.as_str()) {
        return Ok(lexical_nodes);
    }
    let mut current_lexical_node: Option<NodeRef> = None;
    let transform_output = match get_conversion_function(node, editor) {
        Some(conversion) => conversion(node)?,
        None => None,
    };
    let mut post_transform = None;
    if let Some(output) = transform_output {
        post_transform = output.after;
        let transform_nodes = output.node;
        current_lexical_node = match &transform_nodes {
            DomConversionNode::None => None,
            DomConversionNode::One(n) => Some(n.clone()),
            DomConversionNode::Many(nodes) => nodes.last().cloned(),
        };
        if current_lexical_node.is_some() {
            for (_, for_child_function) in &for_child_map.0 {
                let Some(current) = current_lexical_node.take() else {
                    break;
                };
                current_lexical_node = for_child_function(&current, parent_lexical_node)?;
                if current_lexical_node.is_none() {
                    break;
                }
            }
            if let Some(current) = &current_lexical_node {
                match transform_nodes {
                    DomConversionNode::Many(nodes) => lexical_nodes.extend(nodes),
                    _ => lexical_nodes.push(current.clone()),
                }
            }
        }
        if let Some(for_child) = output.for_child {
            for_child_map.set(node_name.clone(), for_child);
        }
    }

    let has_block_ancestor_lexical_node_for_children = match &current_lexical_node {
        Some(current) if crate::nodes::element::is_root_or_shadow_root(&*current.get_latest()) => {
            false
        }
        Some(current) => is_block_element_node(current) || has_block_ancestor_lexical_node,
        None => has_block_ancestor_lexical_node,
    };

    let mut child_lexical_nodes = Vec::new();
    for child in node.child_nodes() {
        child_lexical_nodes.extend(create_nodes_from_dom(
            &child,
            editor,
            all_artificial_nodes,
            has_block_ancestor_lexical_node_for_children,
            for_child_map.clone(),
            current_lexical_node.as_ref(),
        )?);
    }

    if let Some(post_transform) = post_transform {
        child_lexical_nodes = post_transform(child_lexical_nodes)?;
    }

    if is_block_dom_node(node) {
        child_lexical_nodes = if !has_block_ancestor_lexical_node_for_children {
            wrap_continuous_inlines(node, child_lexical_nodes, &mut create_paragraph_node)?
        } else {
            wrap_continuous_inlines(node, child_lexical_nodes, &mut || {
                let artificial_node = create_artificial_node();
                all_artificial_nodes.push(artificial_node.clone());
                artificial_node
            })?
        };
    }

    match current_lexical_node {
        None => {
            if !child_lexical_nodes.is_empty() {
                // A DOM node without a conversion hoists its children.
                lexical_nodes.extend(child_lexical_nodes);
            } else if is_block_dom_node(node) && is_dom_node_between_two_inline_nodes(node) {
                // An empty block between inline nodes is a line break.
                lexical_nodes.push(create_line_break_node());
            }
        }
        Some(current) => {
            if current.is_element() {
                current.append(&child_lexical_nodes)?;
            }
        }
    }
    Ok(lexical_nodes)
}

/// `wrapContinuousInlines(domNode, nodes, createWrapperFn)`: each run of
/// inline nodes in a wrapper, and the block's `text-align` on the blocks
/// without an alignment.
fn wrap_continuous_inlines(
    dom_node: &DomNode,
    nodes: Vec<NodeRef>,
    create_wrapper_fn: &mut dyn FnMut() -> NodeRef,
) -> Result<Vec<NodeRef>, LexicalError> {
    let text_align = dom_node.style().get_property_value("text-align");
    let mut out = Vec::new();
    let mut continuous_inlines = Vec::new();
    let len = nodes.len();
    for (i, node) in nodes.iter().enumerate() {
        if is_block_element_node(node) {
            if !text_align.is_empty() && node.element_data().get_format() == 0 {
                node.set_element_format(&text_align);
            }
            out.push(node.clone());
        } else {
            continuous_inlines.push(node.clone());
            if i == len - 1 || is_block_element_node(&nodes[i + 1]) {
                let wrapper = create_wrapper_fn();
                // `setFormat` clears the alignment for a `text-align` that names none.
                wrapper.set_element_format(&text_align);
                wrapper.append(&continuous_inlines)?;
                out.push(wrapper);
                continuous_inlines.clear();
            }
        }
    }
    Ok(out)
}

/// `$unwrapArtificialNodes(allArtificialNodes)`: replaces each artificial
/// node with its children, with a line break between adjacent ones.
fn unwrap_artificial_nodes(all_artificial_nodes: &[NodeRef]) -> Result<(), LexicalError> {
    for node in all_artificial_nodes {
        if node.get_parent().is_some()
            && node
                .get_next_sibling()
                .is_some_and(|n| is_artificial_node(&n))
        {
            node.insert_after(&create_line_break_node(), true)?;
        }
    }
    for node in all_artificial_nodes {
        if let Some(parent) = node.get_parent() {
            let index = node
                .get_index_within_parent()
                .expect("a child has an index");
            parent.splice(index, 1, &node.get_children())?;
        }
    }
    Ok(())
}

/// `isDomNodeBetweenTwoInlineNodes(node)`.
fn is_dom_node_between_two_inline_nodes(node: &DomNode) -> bool {
    match (node.next_sibling(), node.previous_sibling()) {
        (Some(next), Some(previous)) => is_inline_dom_node(&next) && is_inline_dom_node(&previous),
        _ => false,
    }
}
