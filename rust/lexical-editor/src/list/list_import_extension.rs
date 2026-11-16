//! `ListImportExtension.ts`: the `DOMImportExtension` rules for lists
//! (`@experimental` upstream), which `ListExtension` contributes.

use super::list_item_node::{create_list_item_node, is_list_item_node};
use super::list_node::list_dom::ol_start;
use super::list_node::{ListType, create_list_node, is_list_node};
use crate::LexicalError;
use crate::dom::DomNode;
use crate::html::import::sel::is_element_of_tag;
use crate::html::import::{
    ChildSchema, DomImportContext, DomImportRule, ImportChildrenOpts, OnReject, define_import_rule,
    is_block_level, propagate_text_align_to_block_children, sel,
};
use crate::lexical_dom::{set_direction_from_dom, set_format_from_dom};
use crate::node::NodeRef;
use crate::nodes::line_break::create_line_break_node;
use crate::nodes::paragraph::ParagraphNode;
use std::rc::Rc;

/// `isDomChecklist(domNode)`: `matches('[__lexicallisttype="check"],
/// .contains-task-list, [data-is-checklist="1"]')`, or a child with
/// `aria-checked` (`:scope > [aria-checked]`).
fn is_dom_checklist(dom_node: &DomNode) -> bool {
    dom_node.get_attribute("__lexicallisttype").as_deref() == Some("check")
        || dom_node.class_list().contains("contains-task-list")
        || dom_node.get_attribute("data-is-checklist").as_deref() == Some("1")
        || dom_node
            .children()
            .iter()
            .any(|child| child.has_attribute("aria-checked"))
}

/// `$normalizeListChildren(children, listNode)`: nested lists lifted out of
/// their items into sibling items (the legacy `$normalizeChildren`
/// shape), and any child that isn't an item wrapped in one. The wrappers
/// are `listNode.createListItemNode()`'s, a plain `ListItemNode`.
fn normalize_list_children(children: Vec<NodeRef>) -> Result<Vec<NodeRef>, LexicalError> {
    let mut out = Vec::new();
    for child in children {
        if is_list_item_node(&child) {
            let inner_children = child.get_children();
            out.push(child);
            if inner_children.len() > 1 {
                for inner in inner_children.into_iter().filter(is_list_node) {
                    let item = create_list_item_node(None);
                    item.append(&[inner])?;
                    out.push(item);
                }
            }
        } else {
            let item = create_list_item_node(None);
            item.append(&[child])?;
            out.push(item);
        }
    }
    Ok(out)
}

/// `ListRule`.
fn list_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/list/list"),
        sel::tag(&["ol", "ul"]),
        |ctx, el, _| {
            let node = if is_element_of_tag(el, "ol") {
                create_list_node(ListType::Number, ol_start(el))
            } else if is_dom_checklist(el) {
                create_list_node(ListType::Check, 1.0)
            } else {
                create_list_node(ListType::Bullet, 1.0)
            };
            set_direction_from_dom(&node, el);
            // The list's `text-align` goes onto each item, where the
            // reconciler renders it (legacy `wrapContinuousInlines` did the
            // same).
            let children =
                normalize_list_children(ctx.import_children(el, &ImportChildrenOpts::default())?)?;
            node.splice(
                0,
                0,
                &propagate_text_align_to_block_children(children, Some(el)),
            )?;
            Ok(vec![node])
        },
    )
}

/// `$liftFormatFromSingleParagraph(listItemNode, children)`: an item whose
/// one child is an aligned paragraph (as Google Docs writes) takes its
/// alignment and its children (the legacy `setFormatFromChildren`).
fn lift_format_from_single_paragraph(
    list_item_node: &NodeRef,
    children: Vec<NodeRef>,
) -> Vec<NodeRef> {
    if let [first_child] = children.as_slice()
        && first_child.get_latest().is::<ParagraphNode>()
        && list_item_node.element_data().get_format_type().is_empty()
    {
        let format_type = first_child.element_data().get_format_type();
        if !format_type.is_empty() {
            list_item_node.set_element_format(format_type);
            return first_child.get_children();
        }
    }
    children
}

/// `$flattenListItemBlocks(children)`: an item holds inline content, so
/// each block child (other than a nested list, which
/// [`normalize_list_children`] lifts) becomes its inline content, with a
/// line break between it and the runs around it (`<li>1<div>2</div>3</li>`
/// gives `1<br>2<br>3`). A childless block decorator stands on its own
/// line, but where `NodeStructureExtension` enforces node structure the
/// normalization splits the list around it, so no line break goes next to
/// it.
fn flatten_list_item_blocks(children: Vec<NodeRef>) -> Result<Vec<NodeRef>, LexicalError> {
    let is_boundary = |node: &NodeRef| is_block_level(node) && !is_list_node(node);
    if !children.iter().any(is_boundary) {
        return Ok(children);
    }
    // `INTERNAL_isNodeStructureEnforced($getEditor())`.
    let lifts = crate::structure::is_structure_enforced_on_active_editor();
    let mut segments: Vec<(Vec<NodeRef>, bool)> = Vec::new();
    let mut inline_run: Vec<NodeRef> = Vec::new();
    for child in children {
        if is_boundary(&child) {
            if !inline_run.is_empty() {
                segments.push((std::mem::take(&mut inline_run), false));
            }
            segments.push(if child.is_element() {
                (child.get_children(), false)
            } else {
                (vec![child], lifts)
            });
        } else {
            inline_run.push(child);
        }
    }
    if !inline_run.is_empty() {
        segments.push((inline_run, false));
    }
    let mut out = Vec::new();
    for i in 0..segments.len() {
        if !out.is_empty() && !segments[i - 1].1 && !segments[i].1 {
            out.push(create_line_break_node());
        }
        out.extend(segments[i].0.iter().cloned());
    }
    Ok(out)
}

/// An item made from `<li>` (`ListItemRule`'s body and
/// `$buildChecklistItem`'s).
fn import_list_item(
    ctx: &DomImportContext,
    el: &DomNode,
    checked: Option<bool>,
) -> Result<Vec<NodeRef>, LexicalError> {
    let node = create_list_item_node(checked);
    set_format_from_dom(&node, el);
    set_direction_from_dom(&node, el);
    // The sole paragraph's format is lifted before flattening, which would
    // unwrap the paragraph and lose it.
    let children = lift_format_from_single_paragraph(
        &node,
        ctx.import_children(el, &ImportChildrenOpts::default())?,
    );
    node.splice(0, 0, &flatten_list_item_blocks(children)?)?;
    Ok(vec![node])
}

/// `ListItemRule`.
fn list_item_rule() -> DomImportRule {
    define_import_rule(Some("@lexical/list/li"), sel::tag(&["li"]), |ctx, el, _| {
        let checked = match el.get_attribute("aria-checked").as_deref() {
            Some("true") => Some(true),
            Some("false") => Some(false),
            _ => None,
        };
        import_list_item(ctx, el, checked)
    })
}

/// `:scope > input[type="checkbox"]`.
fn checkbox_child(el: &DomNode) -> Option<DomNode> {
    el.children().into_iter().find(|child| {
        is_element_of_tag(child, "input")
            && child.get_attribute("type").as_deref() == Some("checkbox")
    })
}

/// `TaskListItemRule`: a GitHub task list item, checked by its checkbox.
fn task_list_item_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/list/li-task-list-item"),
        sel::tag(&["li"]).class_all(&["task-list-item"]),
        |ctx, el, next| match checkbox_child(el) {
            Some(input) => import_list_item(ctx, el, Some(input.has_attribute("checked"))),
            None => next(),
        },
    )
}

/// `JoplinChecklistItemRule`: a Joplin checklist item, checked by the
/// checkbox in its `.checkbox-wrapper`.
fn joplin_checklist_item_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/list/li-joplin-checkbox"),
        sel::tag(&["li"]).class_all(&["joplin-checkbox"]),
        |ctx, el, next| {
            let input = el
                .children()
                .into_iter()
                .find(|child| child.class_list().contains("checkbox-wrapper"))
                .and_then(|wrapper| checkbox_child(&wrapper));
            match input {
                Some(input) => import_list_item(ctx, el, Some(input.has_attribute("checked"))),
                None => next(),
            }
        },
    )
}

/// `ListSchema` (`@experimental`): a `ChildSchema` that accepts only
/// `ListItemNode` and (immediately nested) `ListNode` children; runs of
/// other children are wrapped in a new `ListItemNode`.
pub fn list_schema() -> ChildSchema {
    ChildSchema {
        name: Some("ListSchema".to_owned()),
        accepts: Rc::new(|child, _| is_list_item_node(child) || is_list_node(child)),
        package_run: Some(Rc::new(|run, _, _| {
            let item = create_list_item_node(None);
            item.splice(0, 0, &run)?;
            Ok(vec![item])
        })),
        on_reject: OnReject::default(),
        finalize: None,
    }
}

/// `ListImportRules` (`@experimental`): the import rules for `ListNode`
/// and `ListItemNode`, with the GitHub task list and Joplin checkbox
/// heuristics, which come first so they win over the generic `li` rule.
/// `ListExtension` contributes them, with `CoreImportExtension`.
pub fn list_import_rules() -> Vec<DomImportRule> {
    vec![
        task_list_item_rule(),
        joplin_checklist_item_rule(),
        list_rule(),
        list_item_rule(),
    ]
}
