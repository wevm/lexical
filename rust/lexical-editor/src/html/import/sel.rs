//! `import/sel.ts`: the selectors import rules match DOM nodes with
//! (`@experimental` upstream), built with [`any`], [`tag`], [`text`],
//! [`comment`] and [`css`] (JS's `sel` namespace) and refined with
//! [`CompiledSelector`]'s builder methods.
//!
//! JS's `attr` and `styleAny` take a `RegExp`. The default build has no
//! regular expressions, so a caller passes a [`RegexMatcher`]: a function
//! that returns the match, as `String.prototype.match` does.

use crate::dom::DomNode;
use std::collections::HashMap;
use std::rc::Rc;

/// `RegExpMatchArray`: what a [`RegexMatcher`] found: `groups[0]` is the
/// whole match and each further index a group (`None` when it didn't
/// participate).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RegExpMatch {
    pub groups: Vec<Option<String>>,
    /// `match.groups`.
    pub named_groups: HashMap<String, String>,
    /// `match.index`, in UTF-16 code units.
    pub index: usize,
}

/// `value.match(re)`, for a `RegExp` given to `attr` or `styleAny`.
pub type RegexMatcher = Rc<dyn Fn(&str) -> Option<RegExpMatch>>;

/// The captures of a rule's selector (`ctx.captures`), by name.
pub type ImportCaptures = HashMap<String, RegExpMatch>;

/// `Predicate`: whether a node matches, recording captures.
pub type Predicate = Rc<dyn Fn(&DomNode, &mut ImportCaptures) -> bool>;

/// `SelectorKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectorKind {
    Element,
    Text,
    Comment,
}

/// `CompiledSelector` (with `ElementSelectorBuilder`'s methods): the
/// `match` of a rule.
#[derive(Clone)]
pub struct CompiledSelector {
    pub(crate) kind: SelectorKind,
    /// Upper-case tag names the selector is restricted to, in insertion
    /// order; empty for a wildcard element selector and for text and
    /// comment selectors.
    pub(crate) tags: Vec<String>,
    predicates: Vec<Predicate>,
}

/// What `attr(name, value)` asks of the attribute.
#[derive(Clone)]
pub enum AttrMatch {
    /// `true`: present.
    Present,
    /// A string: equal to it.
    Equals(String),
    /// A `RegExp`: matching it, recorded as `ctx.captures[capture]` when
    /// `capture` names one.
    Matches(RegexMatcher, Option<String>),
}

/// What `styleAny(prop, value)` asks of the inline style property.
#[derive(Clone)]
pub enum StyleMatch {
    Equals(String),
    Matches(RegexMatcher, Option<String>),
}

impl CompiledSelector {
    /// The selector's composed predicate.
    pub(crate) fn predicate(&self) -> Predicate {
        match self.kind {
            SelectorKind::Text => Rc::new(|node: &DomNode, _: &mut ImportCaptures| node.is_text()),
            SelectorKind::Comment => Rc::new(|node: &DomNode, _: &mut ImportCaptures| {
                node.node_type() == crate::dom::DomNodeType::Comment
            }),
            SelectorKind::Element => combine_predicates(&self.predicates),
        }
    }

    fn refine(&self, additional: Predicate) -> Self {
        let mut predicates = self.predicates.clone();
        predicates.push(additional);
        build_selector(self.tags.clone(), predicates)
    }

    /// `classAll(...classes)`: every listed class is on the element.
    pub fn class_all(&self, classes: &[&str]) -> Self {
        self.refine(build_class_all_predicate(classes))
    }

    /// `classAny(...classes)`: one of the listed classes is on the element.
    pub fn class_any(&self, classes: &[&str]) -> Self {
        self.refine(build_class_any_predicate(classes))
    }

    /// `attr(name, value, options)`.
    pub fn attr(&self, name: &str, value: AttrMatch) -> Self {
        self.refine(build_attr_predicate(name, value))
    }

    /// `styleAny(prop, value, options)`: the element's inline style
    /// property equals or matches `value`.
    pub fn style_any(&self, prop: &str, value: StyleMatch) -> Self {
        let prop = prop.to_owned();
        self.refine(match value {
            StyleMatch::Equals(value) => Rc::new(move |node: &DomNode, _: &mut ImportCaptures| {
                node.is_element() && node.style().get_property_value(&prop) == value
            }),
            StyleMatch::Matches(re, capture) => {
                Rc::new(move |node: &DomNode, captures: &mut ImportCaptures| {
                    if !node.is_element() {
                        return false;
                    }
                    let v = node.style().get_property_value(&prop);
                    if v.is_empty() {
                        return false;
                    }
                    let Some(m) = re(&v) else {
                        return false;
                    };
                    if let Some(capture) = &capture {
                        captures.insert(capture.clone(), m);
                    }
                    true
                })
            }
        })
    }
}

fn combine_predicates(preds: &[Predicate]) -> Predicate {
    match preds {
        [] => Rc::new(|node: &DomNode, _: &mut ImportCaptures| node.is_element()),
        [one] => one.clone(),
        _ => {
            let preds = preds.to_vec();
            Rc::new(move |node: &DomNode, captures: &mut ImportCaptures| {
                preds.iter().all(|p| p(node, captures))
            })
        }
    }
}

/// `buildSelector(tags, predicates)`.
pub(crate) fn build_selector(tags: Vec<String>, predicates: Vec<Predicate>) -> CompiledSelector {
    CompiledSelector {
        kind: SelectorKind::Element,
        tags,
        predicates,
    }
}

fn normalize_class_list(classes: &[&str]) -> Vec<String> {
    classes
        .iter()
        .filter(|c| !c.is_empty())
        .map(|c| (*c).to_owned())
        .collect()
}

/// `buildClassAllPredicate(classes)`.
pub(crate) fn build_class_all_predicate(classes: &[&str]) -> Predicate {
    let ns = normalize_class_list(classes);
    if ns.is_empty() {
        return Rc::new(|_: &DomNode, _: &mut ImportCaptures| true);
    }
    Rc::new(move |node: &DomNode, _: &mut ImportCaptures| {
        if !node.is_element() {
            return false;
        }
        let cl = node.class_list();
        ns.iter().all(|c| cl.contains(c))
    })
}

/// `buildClassAnyPredicate(classes)`.
pub(crate) fn build_class_any_predicate(classes: &[&str]) -> Predicate {
    let ns = normalize_class_list(classes);
    if ns.is_empty() {
        return Rc::new(|_: &DomNode, _: &mut ImportCaptures| false);
    }
    Rc::new(move |node: &DomNode, _: &mut ImportCaptures| {
        if !node.is_element() {
            return false;
        }
        let cl = node.class_list();
        ns.iter().any(|c| cl.contains(c))
    })
}

/// `buildAttrPredicate(name, value, options)`.
pub(crate) fn build_attr_predicate(name: &str, value: AttrMatch) -> Predicate {
    let name = name.to_owned();
    match value {
        AttrMatch::Present => Rc::new(move |node: &DomNode, _: &mut ImportCaptures| {
            node.is_element() && node.has_attribute(&name)
        }),
        AttrMatch::Equals(value) => Rc::new(move |node: &DomNode, _: &mut ImportCaptures| {
            node.is_element() && node.get_attribute(&name).as_deref() == Some(value.as_str())
        }),
        AttrMatch::Matches(re, capture) => {
            Rc::new(move |node: &DomNode, captures: &mut ImportCaptures| {
                if !node.is_element() {
                    return false;
                }
                let Some(v) = node.get_attribute(&name) else {
                    return false;
                };
                let Some(m) = re(&v) else {
                    return false;
                };
                if let Some(capture) = &capture {
                    captures.insert(capture.clone(), m);
                }
                true
            })
        }
    }
}

/// `sel.any()`: any element.
pub fn any() -> CompiledSelector {
    build_selector(Vec::new(), Vec::new())
}

/// `sel.comment()`: comment nodes.
pub fn comment() -> CompiledSelector {
    CompiledSelector {
        kind: SelectorKind::Comment,
        tags: Vec::new(),
        predicates: Vec::new(),
    }
}

/// `sel.text()`: text nodes.
pub fn text() -> CompiledSelector {
    CompiledSelector {
        kind: SelectorKind::Text,
        tags: Vec::new(),
        predicates: Vec::new(),
    }
}

/// `sel.tag(...tags)`: elements by tag name, in any case. JS requires at
/// least one with an invariant; so does this, with a panic.
pub fn tag(tags: &[&str]) -> CompiledSelector {
    assert!(!tags.is_empty(), "sel.tag() requires at least one tag name");
    let mut upper: Vec<String> = Vec::new();
    for t in tags {
        let t = t.to_uppercase();
        if !upper.contains(&t) {
            upper.push(t);
        }
    }
    build_selector(upper, Vec::new())
}

/// `sel.css(source)`: [`parse_selector`](super::parse_css::parse_selector).
pub fn css(source: &str) -> Result<CompiledSelector, crate::LexicalError> {
    super::parse_css::parse_selector(source)
}

/// `isElementOfTag(node, tag)`: an element named `tag`, in any case.
pub fn is_element_of_tag(node: &DomNode, tag: &str) -> bool {
    node.is_element() && node.node_name() == tag.to_uppercase()
}
