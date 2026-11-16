//! `import/schemas.ts`: the child schemas a traversal applies to what it
//! produced, with `ChildSchema` from `import/types.ts`.

use super::core_import_rules::is_alignment_value;
use crate::LexicalError;
use crate::dom::DomNode;
use crate::node::NodeRef;
use crate::nodes::paragraph::create_paragraph_node;
use std::rc::Rc;

/// Whether a child is accepted, given the parent node (none at the top).
pub type AcceptsFn = Rc<dyn Fn(&NodeRef, Option<&NodeRef>) -> bool>;

/// Packages a maximal run of rejected siblings; `[]` falls back to
/// `on_reject`.
pub type PackageRunFn = Rc<
    dyn Fn(Vec<NodeRef>, Option<&NodeRef>, Option<&DomNode>) -> Result<Vec<NodeRef>, LexicalError>,
>;

/// A last pass over the assembled children.
pub type FinalizeFn =
    Rc<dyn Fn(Vec<NodeRef>, Option<&NodeRef>) -> Result<Vec<NodeRef>, LexicalError>>;

/// `ChildSchema['onReject']`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OnReject {
    /// Let the run through unchanged.
    Hoist,
    /// Discard the run (the default).
    #[default]
    Drop,
}

/// `ChildSchema`: which children a parent accepts and what becomes of the
/// rest.
#[derive(Clone)]
pub struct ChildSchema {
    pub name: Option<String>,
    /// `$accepts`.
    pub accepts: AcceptsFn,
    /// `$packageRun`.
    pub package_run: Option<PackageRunFn>,
    pub on_reject: OnReject,
    /// `$finalize`.
    pub finalize: Option<FinalizeFn>,
}

/// `$isBlockLevel(node)`: a block element or a block decorator.
pub fn is_block_level(node: &NodeRef) -> bool {
    (node.is_element() && !node.is_inline()) || (node.is_decorator() && !node.is_inline())
}

/// `$distributeInlineWrapper(children, $makeWrapper)`: wraps each run of
/// inline children in a fresh wrapper, and recurses into block children so
/// they stay at the top level.
pub fn distribute_inline_wrapper(
    children: &[NodeRef],
    make_wrapper: &dyn Fn() -> NodeRef,
) -> Result<Vec<NodeRef>, LexicalError> {
    let mut out = Vec::new();
    let mut inline_run: Vec<NodeRef> = Vec::new();
    let flush = |out: &mut Vec<NodeRef>, run: &mut Vec<NodeRef>| -> Result<(), LexicalError> {
        if run.is_empty() {
            return Ok(());
        }
        let wrapper = make_wrapper();
        wrapper.splice(0, 0, &std::mem::take(run))?;
        out.push(wrapper);
        Ok(())
    };
    for child in children {
        if is_block_level(child) {
            flush(&mut out, &mut inline_run)?;
            if child.is_element() {
                let wrapped = distribute_inline_wrapper(&child.get_children(), make_wrapper)?;
                child.splice(0, child.get_children_size(), &wrapped)?;
            }
            out.push(child.clone());
        } else {
            inline_run.push(child.clone());
        }
    }
    flush(&mut out, &mut inline_run)?;
    Ok(out)
}

/// `$applySchema(schema, children, parent, domParent)`.
pub(crate) fn apply_schema(
    schema: &ChildSchema,
    children: Vec<NodeRef>,
    parent: Option<&NodeRef>,
    dom_parent: Option<&DomNode>,
) -> Result<Vec<NodeRef>, LexicalError> {
    let mut out = Vec::new();
    let mut run: Option<Vec<NodeRef>> = None;
    let flush =
        |out: &mut Vec<NodeRef>, run: &mut Option<Vec<NodeRef>>| -> Result<(), LexicalError> {
            let Some(rejected) = run.take() else {
                return Ok(());
            };
            if let Some(package_run) = &schema.package_run {
                let packaged = package_run(rejected.clone(), parent, dom_parent)?;
                if !packaged.is_empty() {
                    out.extend(packaged);
                    return Ok(());
                }
            }
            if schema.on_reject == OnReject::Hoist {
                out.extend(rejected);
            }
            Ok(())
        };
    for child in children {
        if (schema.accepts)(&child, parent) {
            flush(&mut out, &mut run)?;
            out.push(child);
        } else {
            run.get_or_insert_with(Vec::new).push(child);
        }
    }
    flush(&mut out, &mut run)?;
    match &schema.finalize {
        Some(finalize) => finalize(out, parent),
        None => Ok(out),
    }
}

/// `$propagateTextAlignToBlockChildren(children, domParent)`: gives the
/// block children without a format the `text-align` of `domParent`.
pub fn propagate_text_align_to_block_children(
    children: Vec<NodeRef>,
    dom_parent: Option<&DomNode>,
) -> Vec<NodeRef> {
    let Some(dom_parent) = dom_parent.filter(|d| d.is_element()) else {
        return children;
    };
    let text_align = dom_parent.style().get_property_value("text-align");
    if !is_alignment_value(&text_align) {
        return children;
    }
    for child in &children {
        if child.is_element()
            && !child.is_inline()
            && child.element_data().get_format_type().is_empty()
        {
            child.set_element_format(&text_align);
        }
    }
    children
}

/// `$paragraphPackageRun(run, parent, domParent)`: the run in a paragraph
/// with the `text-align` of `domParent`; a lone line break becomes an empty
/// paragraph.
fn paragraph_package_run(
    run: Vec<NodeRef>,
    _parent: Option<&NodeRef>,
    dom_parent: Option<&DomNode>,
) -> Result<Vec<NodeRef>, LexicalError> {
    let run = if run.len() == 1 && run[0].get_type() == "linebreak" {
        Vec::new()
    } else {
        run
    };
    let paragraph = create_paragraph_node();
    if let Some(dom_parent) = dom_parent.filter(|d| d.is_element()) {
        let text_align = dom_parent.style().get_property_value("text-align");
        if is_alignment_value(&text_align) {
            paragraph.set_element_format(&text_align);
        }
    }
    paragraph.splice(0, 0, &run)?;
    Ok(vec![paragraph])
}

fn block_level_schema(name: &str, package_run: Option<PackageRunFn>) -> ChildSchema {
    ChildSchema {
        name: Some(name.to_owned()),
        accepts: Rc::new(|child: &NodeRef, _: Option<&NodeRef>| is_block_level(child)),
        package_run,
        on_reject: OnReject::Drop,
        finalize: None,
    }
}

/// `BlockSchema`: blocks, with each inline run wrapped in a paragraph.
pub fn block_schema() -> ChildSchema {
    block_level_schema("BlockSchema", Some(Rc::new(paragraph_package_run)))
}

/// `InlineSchema`: inline children; block runs are dropped.
pub fn inline_schema() -> ChildSchema {
    ChildSchema {
        name: Some("InlineSchema".to_owned()),
        accepts: Rc::new(|child: &NodeRef, _: Option<&NodeRef>| !is_block_level(child)),
        package_run: None,
        on_reject: OnReject::Drop,
        finalize: None,
    }
}

/// `NestedBlockSchema`: blocks, with inline runs passed through.
pub fn nested_block_schema() -> ChildSchema {
    block_level_schema(
        "NestedBlockSchema",
        Some(Rc::new(
            |run: Vec<NodeRef>, _: Option<&NodeRef>, _: Option<&DomNode>| Ok(run),
        )),
    )
}

/// `RootSchema`: the top level of an import, as `BlockSchema`.
pub fn root_schema() -> ChildSchema {
    block_level_schema("RootSchema", Some(Rc::new(paragraph_package_run)))
}
