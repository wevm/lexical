//! `internal/compileDOMRenderConfigOverrides.ts`: an editor's render config
//! with `DOMRenderExtension`'s overrides layered over it.
//!
//! JS dispatches the class overrides of consecutive steps by node type,
//! through the registered subtypes of each class (`getRegisteredSubtypeMap`),
//! and the others through their guards. Every step wraps the hook so far,
//! so dispatching each class override through an `instanceof` guard
//! instead composes the same: only the time to find the override differs.

use super::types::{DomRenderMatch, NodeMatch, NodePredicate, is_instance};
use crate::LexicalError;
use crate::extension::get_known_types_and_nodes;
use crate::extension_core::InitialEditorConfig;
use crate::lexical_dom::EditorDomRenderConfig;
use crate::node::{LexicalNode, NodeRef};
use std::rc::Rc;

/// An override with one of its matches: the node guard and the override.
struct Step {
    matches: NodePredicate,
    node_match: Rc<DomRenderMatch>,
}

fn always(_: &dyn LexicalNode, _: &NodeRef) -> bool {
    true
}

/// `getPredicate` for one match: an `instanceof` guard for a class, which
/// must be registered, or the guard itself.
fn get_predicate(
    known_types: &[&'static str],
    m: &NodeMatch,
) -> Result<NodePredicate, LexicalError> {
    match m {
        NodeMatch::Class { node_type, .. } => {
            if !known_types.contains(node_type) {
                return Err(LexicalError::new(format!(
                    "Node class with type {node_type} not registered in editor"
                )));
            }
            let node_type: &'static str = node_type;
            Ok(Rc::new(move |node, _| is_instance(node, node_type)))
        }
        NodeMatch::Predicate(predicate) => Ok(predicate.clone()),
    }
}

/// `sortedOverrides(overrides)`: the class overrides, one per class with
/// a class before the classes that extend it (a stable sort by depth), then
/// the guards, then the wildcards.
fn sorted_steps(
    known_types: &[&'static str],
    overrides: &[Rc<DomRenderMatch>],
) -> Result<Vec<Step>, LexicalError> {
    let mut by_wildcard = Vec::new();
    let mut by_predicate = Vec::new();
    let mut by_node: Vec<(usize, Step)> = Vec::new();
    for o in overrides {
        let Some(nodes) = &o.nodes else {
            by_wildcard.push(Step {
                matches: Rc::new(always),
                node_match: o.clone(),
            });
            continue;
        };
        for m in nodes {
            let step = Step {
                matches: get_predicate(known_types, m)?,
                node_match: o.clone(),
            };
            match m {
                NodeMatch::Class { super_types, .. } => by_node.push((super_types.len(), step)),
                NodeMatch::Predicate(_) => by_predicate.push(step),
            }
        }
    }
    by_node.sort_by_key(|(depth, _)| *depth);
    Ok(by_node
        .into_iter()
        .map(|(_, step)| step)
        .chain(by_predicate)
        .chain(by_wildcard)
        .collect())
}

/// `compileDOMRenderConfigOverrides(editorConfig, {overrides})`: the
/// editor's render config (its `dom`, or the default) with each override
/// wrapping the hooks so far, for the nodes it matches.
pub(crate) fn compile_dom_render_config_overrides(
    editor_config: &InitialEditorConfig,
    overrides: &[Rc<DomRenderMatch>],
) -> Result<EditorDomRenderConfig, LexicalError> {
    let known_types = get_known_types_and_nodes(editor_config).types;
    let mut dom = editor_config.dom.clone().unwrap_or_default();
    for step in sorted_steps(&known_types, overrides)? {
        let Step {
            matches,
            node_match,
        } = step;
        let c = &node_match.config;
        if let Some(o) = c.create_dom.clone() {
            let (prev, matches) = (dom.create_dom.clone(), matches.clone());
            dom.create_dom = Rc::new(move |node, this, editor| {
                if matches(node, this) {
                    o(node, this, &|| prev(node, this, editor), editor)
                } else {
                    prev(node, this, editor)
                }
            });
        }
        if let Some(o) = c.export_dom.clone() {
            let (prev, matches) = (dom.export_dom.clone(), matches.clone());
            dom.export_dom = Rc::new(move |node, this, editor| {
                if matches(node, this) {
                    o(node, this, &|| prev(node, this, editor), editor)
                } else {
                    prev(node, this, editor)
                }
            });
        }
        if let Some(o) = c.extract_with_child.clone() {
            let (prev, matches) = (dom.extract_with_child.clone(), matches.clone());
            dom.extract_with_child = Rc::new(move |node, child, selection, destination, editor| {
                let next = || prev(node, child, selection, destination, editor);
                if matches(&*node.get_latest(), node) {
                    o(node, child, selection, destination, &next, editor)
                } else {
                    next()
                }
            });
        }
        if let Some(o) = c.should_exclude.clone() {
            let (prev, matches) = (dom.should_exclude.clone(), matches.clone());
            dom.should_exclude = Rc::new(move |node, selection, editor| {
                let next = || prev(node, selection, editor);
                if matches(&*node.get_latest(), node) {
                    o(node, selection, &next, editor)
                } else {
                    next()
                }
            });
        }
        if let Some(o) = c.should_include.clone() {
            let (prev, matches) = (dom.should_include.clone(), matches.clone());
            dom.should_include = Rc::new(move |node, selection, editor| {
                let next = || prev(node, selection, editor);
                if matches(&*node.get_latest(), node) {
                    o(node, selection, &next, editor)
                } else {
                    next()
                }
            });
        }
    }
    Ok(dom)
}
