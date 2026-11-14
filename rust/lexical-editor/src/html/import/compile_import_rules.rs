//! `import/compileImportRules.ts` and `import/defineOverlayRules.ts`: the
//! dispatch tables a rule list compiles into, and overlays (rule lists
//! compiled once, for one subtree or a whole walk).

use super::define_import_rule::{DomImportFn, DomImportRule};
use super::sel::{Predicate, SelectorKind};
use crate::dom::{DomNode, DomNodeType};
use std::collections::HashMap;
use std::rc::Rc;

/// `CompiledRule`. JS also keeps the rule's name (or a default, such as
/// `p@3`) for the error it logs in development builds; there is no console
/// here.
#[derive(Clone)]
pub(crate) struct CompiledRule {
    pub(crate) predicate: Predicate,
    pub(crate) import: DomImportFn,
}

/// `CompiledDispatch`: the rules in registration order, and the indices to
/// try for each tag (with the wildcard element rules interleaved), for
/// wildcards, text and comments.
pub(crate) struct CompiledDispatch {
    pub(crate) rules: Vec<CompiledRule>,
    by_tag: HashMap<String, Vec<usize>>,
    wildcard_indices: Vec<usize>,
    text_indices: Vec<usize>,
    comment_indices: Vec<usize>,
}

fn merge_sorted_asc(a: &[usize], b: &[usize]) -> Vec<usize> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] <= b[j] {
            out.push(a[i]);
            i += 1;
        } else {
            out.push(b[j]);
            j += 1;
        }
    }
    out.extend_from_slice(&a[i..]);
    out.extend_from_slice(&b[j..]);
    out
}

/// `compileImportRules(rules)`: the dispatch tables. Index 0 is the highest
/// priority.
pub(crate) fn compile_import_rules(rules: &[DomImportRule]) -> CompiledDispatch {
    let mut compiled = Vec::with_capacity(rules.len());
    let mut by_tag: HashMap<String, Vec<usize>> = HashMap::new();
    let mut wildcard_indices = Vec::new();
    let mut text_indices = Vec::new();
    let mut comment_indices = Vec::new();
    for (i, rule) in rules.iter().enumerate() {
        let sel = &rule.match_;
        compiled.push(CompiledRule {
            predicate: sel.predicate(),
            import: rule.import.clone(),
        });
        match sel.kind {
            SelectorKind::Text => text_indices.push(i),
            SelectorKind::Comment => comment_indices.push(i),
            SelectorKind::Element if sel.tags.is_empty() => wildcard_indices.push(i),
            SelectorKind::Element => {
                for tag in &sel.tags {
                    by_tag.entry(tag.clone()).or_default().push(i);
                }
            }
        }
    }
    // Interleave the wildcard element rules into each tag's list in
    // registration order, so one pass over a bucket visits both.
    if !wildcard_indices.is_empty() {
        for list in by_tag.values_mut() {
            *list = merge_sorted_asc(list, &wildcard_indices);
        }
    }
    CompiledDispatch {
        rules: compiled,
        by_tag,
        wildcard_indices,
        text_indices,
        comment_indices,
    }
}

/// `getDispatchIndices(dispatch, node)`: the (interleaved) rule indices
/// for `node`.
pub(crate) fn get_dispatch_indices<'a>(
    dispatch: &'a CompiledDispatch,
    node: &DomNode,
) -> &'a [usize] {
    match node.node_type() {
        DomNodeType::Text => &dispatch.text_indices,
        DomNodeType::Comment => &dispatch.comment_indices,
        DomNodeType::Element => dispatch
            .by_tag
            .get(&node.node_name())
            .unwrap_or(&dispatch.wildcard_indices),
        _ => &[],
    }
}

/// `CompiledOverlayRules`: rules compiled once by [`define_overlay_rules`],
/// for `ImportChildrenOpts::rules` or `ImportOverlays`. Overlays compare by
/// identity.
pub struct CompiledOverlayRules {
    pub(crate) dispatch: Rc<CompiledDispatch>,
    /// The flattened source rules, so an overlay can be inlined into
    /// another list.
    pub(crate) rules: Vec<DomImportRule>,
}

impl PartialEq for CompiledOverlayRules {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

/// `DOMImportRuleEntry`: a rule, or an overlay whose rules are inlined at
/// its position.
#[derive(Clone)]
pub enum DomImportRuleEntry {
    Rule(DomImportRule),
    Overlay(Rc<CompiledOverlayRules>),
}

impl From<DomImportRule> for DomImportRuleEntry {
    fn from(rule: DomImportRule) -> Self {
        Self::Rule(rule)
    }
}

impl From<Rc<CompiledOverlayRules>> for DomImportRuleEntry {
    fn from(overlay: Rc<CompiledOverlayRules>) -> Self {
        Self::Overlay(overlay)
    }
}

/// `flattenRuleEntries(entries)`.
pub(crate) fn flatten_rule_entries(entries: &[DomImportRuleEntry]) -> Vec<DomImportRule> {
    let mut out = Vec::new();
    for entry in entries {
        match entry {
            DomImportRuleEntry::Rule(rule) => out.push(rule.clone()),
            DomImportRuleEntry::Overlay(overlay) => out.extend(overlay.rules.iter().cloned()),
        }
    }
    out
}

/// `defineOverlayRules(entries)`: compiles rules (and other overlays,
/// inlined in place) into an overlay, tried front to back.
pub fn define_overlay_rules(entries: &[DomImportRuleEntry]) -> Rc<CompiledOverlayRules> {
    let rules = flatten_rule_entries(entries);
    Rc::new(CompiledOverlayRules {
        dispatch: Rc::new(compile_import_rules(&rules)),
        rules,
    })
}
