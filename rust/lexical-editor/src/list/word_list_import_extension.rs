//! `WordListImportExtension.ts`: MS Word list paste support
//! (`@experimental` upstream). Word writes a list as a flat run of
//! `<p class="MsoListParagraph*">` with the marker in a
//! `<span style="mso-list:Ignore">` and the level in the paragraph's
//! `mso-list`; an overlay rebuilds the run as nested lists.

use super::lexical_list_extension::list_extension;
use super::list_item_node::create_list_item_node;
use super::list_node::{ListType, create_list_node};
use crate::dom::DomNode;
use crate::extension_core::{LexicalExtension, config_extension, define_extension};
use crate::html::import::{
    DomImportConfigOverrides, DomImportContext, DomImportRule, DomPreprocessFn, ImportChildrenOpts,
    ImportStateConfig, create_import_state, define_import_rule, define_overlay_rules,
    dom_import_extension, import_overlays, inline_schema, sel,
};
use crate::node::NodeRef;
use crate::{LexicalError, get_style_object_from_css, js_trim};
use std::cell::RefCell;
use std::rc::Rc;

/// `mso-list` is not a property a browser (or jsdom) keeps in `el.style`,
/// and `$inlineStylesFromStyleSheets` rewrites the `style` attribute
/// without it, so the preprocess copies it onto this attribute first.
const MSO_LIST_DATA_ATTR: &str = "data-mso-list";

/// `WORD_LIST_CLASS_RE`: `^MsoListParagraph(CxSp(First|Middle|Last))?$`.
fn is_word_list_class(class_name: &str) -> bool {
    matches!(
        class_name,
        "MsoListParagraph"
            | "MsoListParagraphCxSpFirst"
            | "MsoListParagraphCxSpMiddle"
            | "MsoListParagraphCxSpLast"
    )
}

/// `WORD_NUMBERED_RE.test(marker)`: `^[A-Za-z0-9]+[.)]`.
fn is_numbered_marker(marker: &str) -> bool {
    let rest = marker.trim_start_matches(|c: char| c.is_ascii_alphanumeric());
    rest.len() < marker.len() && (rest.starts_with('.') || rest.starts_with(')'))
}

/// `readWordListLevel(el)`: the number after the first `level` followed by
/// digits in the copied `mso-list` (`l0 level2 lfo1`), or 1.
fn read_word_list_level(el: &DomNode) -> f64 {
    let value = el.get_attribute(MSO_LIST_DATA_ATTR).unwrap_or_default();
    for (i, _) in value.match_indices("level") {
        let rest = &value[i + "level".len()..];
        let digits = &rest[..rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len())];
        if !digits.is_empty() {
            return digits.parse().unwrap_or(1.0);
        }
    }
    1.0
}

/// `findMarkerSpan(el)`: `span[data-mso-list="Ignore"]`.
fn find_marker_span(el: &DomNode) -> Option<DomNode> {
    el.query_selector_all(&format!("span[{MSO_LIST_DATA_ATTR}=\"Ignore\"]"))
        .ok()
        .and_then(|found| found.into_iter().next())
}

/// `readWordMarker(el)`.
fn read_word_marker(el: &DomNode) -> String {
    find_marker_span(el)
        .map(|span| js_trim(&span.text_content()).to_owned())
        .unwrap_or_default()
}

/// `classifyWordListType(marker)`.
fn classify_word_list_type(marker: &str) -> ListType {
    if is_numbered_marker(marker) {
        ListType::Number
    } else {
        ListType::Bullet
    }
}

/// `stripWordMarker(el)`: removes the outermost `<span>` around the marker
/// below the paragraph.
fn strip_word_marker(el: &DomNode) {
    let Some(mut outer) = find_marker_span(el) else {
        return;
    };
    while let Some(parent) = outer.parent_element()
        && parent != *el
        && parent.node_name() == "SPAN"
    {
        outer = parent;
    }
    outer.remove();
}

/// `isWordListParagraph(node)`.
fn is_word_list_paragraph(node: &DomNode) -> bool {
    node.is_element() && is_word_list_class(&node.class_name())
}

/// `WordListItem`.
struct WordListItem {
    el: DomNode,
    level: f64,
    marker: String,
}

/// `$buildWordListTree(ctx, items)`: a sublist goes in an item of its own
/// after the items above it (`$isNestedListNode`'s convention).
fn build_word_list_tree(
    ctx: &DomImportContext,
    items: &[WordListItem],
) -> Result<NodeRef, LexicalError> {
    let root = create_list_node(classify_word_list_type(&items[0].marker), 1.0);
    let mut stack: Vec<(NodeRef, f64)> = vec![(root.clone(), items[0].level)];
    for item in items {
        while stack.len() > 1 && stack.last().expect("a frame").1 > item.level {
            stack.pop();
        }
        if item.level > stack.last().expect("a frame").1 {
            let sub = create_list_node(classify_word_list_type(&item.marker), 1.0);
            let wrapper = create_list_item_node(None);
            wrapper.append(std::slice::from_ref(&sub))?;
            stack.last().expect("a frame").0.append(&[wrapper])?;
            stack.push((sub, item.level));
        }
        strip_word_marker(&item.el);
        let opts = ImportChildrenOpts {
            schema: Some(inline_schema()),
            ..ImportChildrenOpts::default()
        };
        let list_item = create_list_item_node(None);
        list_item.splice(0, 0, &ctx.import_children(&item.el, &opts)?)?;
        stack.last().expect("a frame").0.append(&[list_item])?;
    }
    Ok(root)
}

/// The paragraphs an earlier sibling's run took in, shared by the
/// session (JS's `WeakSet`), compared by identity.
#[derive(Clone, Default)]
struct Consumed(Rc<RefCell<Vec<DomNode>>>);

impl PartialEq for Consumed {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

thread_local! {
    /// `WordListConsumed`: `null` until a session's first Word list
    /// paragraph, which installs its own set.
    static WORD_LIST_CONSUMED: ImportStateConfig<Option<Consumed>> =
        create_import_state("@lexical/list/word-consumed-list-items", || None);
}

/// `WordListParagraphRule`: the first paragraph of a run makes the run's
/// list; the others make nothing.
fn word_list_paragraph_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/list/word-list-paragraph"),
        sel::tag(&["p"]).class_any(&[
            "MsoListParagraph",
            "MsoListParagraphCxSpFirst",
            "MsoListParagraphCxSpMiddle",
            "MsoListParagraphCxSpLast",
        ]),
        |ctx, el, _| {
            let cfg = WORD_LIST_CONSUMED.with(Clone::clone);
            let consumed = match ctx.session.get(&cfg) {
                Some(consumed) => consumed,
                None => {
                    let consumed = Consumed::default();
                    ctx.session.set(&cfg, Some(consumed.clone()));
                    consumed
                }
            };
            if consumed.0.borrow().contains(el) {
                return Ok(Vec::new());
            }
            let mut items = Vec::new();
            let mut cur = Some(el.clone());
            while let Some(node) = cur.filter(is_word_list_paragraph) {
                consumed.0.borrow_mut().push(node.clone());
                items.push(WordListItem {
                    level: read_word_list_level(&node),
                    marker: read_word_marker(&node),
                    el: node.clone(),
                });
                // MsoListParagraph (no CxSp suffix) is a run of one.
                if node.class_list().contains("MsoListParagraphCxSpLast")
                    || node.class_name() == "MsoListParagraph"
                {
                    break;
                }
                cur = node.next_element_sibling();
            }
            Ok(vec![build_word_list_tree(ctx, &items)?])
        },
    )
}

/// `WordOfficeParagraphRule`: Office's `<o:p>` paragraph end makes nothing.
fn word_office_paragraph_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/list/word-o-p"),
        sel::tag(&["o:p"]),
        |_, _, _| Ok(Vec::new()),
    )
}

/// `$installWordListPasteOverlay`: for a document Word generated, copies
/// each `mso-list` onto [`MSO_LIST_DATA_ATTR`] and installs the overlay.
fn install_word_list_paste_overlay() -> DomPreprocessFn {
    let overlay = define_overlay_rules(&[
        word_office_paragraph_rule().into(),
        word_list_paragraph_rule().into(),
    ]);
    Rc::new(move |dom, ctx, next| {
        let meta = dom
            .query_selector_all("meta[name=\"Generator\"]")
            .ok()
            .and_then(|found| found.into_iter().next());
        let is_word = meta.is_some_and(|meta| {
            meta.get_attribute("content")
                .unwrap_or_default()
                .to_lowercase()
                .contains("microsoft word")
        });
        if is_word {
            for el in dom
                .query_selector_all("[style*=\"mso-list\"]")
                .unwrap_or_default()
            {
                let style =
                    get_style_object_from_css(&el.get_attribute("style").unwrap_or_default());
                if let Some(mso_list) = style.get("mso-list").filter(|v| !v.is_empty()) {
                    el.set_attribute(MSO_LIST_DATA_ATTR, mso_list);
                }
            }
            let overlay = overlay.clone();
            ctx.session.update(&import_overlays(), move |mut prev| {
                prev.push(overlay);
                prev
            });
        }
        next()
    })
}

fn make() -> Rc<LexicalExtension> {
    define_extension(LexicalExtension {
        dependencies: vec![
            // The overlay makes ListNodes, so the nodes must be registered.
            list_extension().into(),
            config_extension(
                &dom_import_extension(),
                [DomImportConfigOverrides {
                    preprocess: Some(vec![install_word_list_paste_overlay()]),
                    ..DomImportConfigOverrides::default()
                }],
            ),
        ],
        ..LexicalExtension::new("@lexical/list/WordListImport", ())
    })
}

thread_local! {
    static WORD_LIST_IMPORT_EXTENSION: Rc<LexicalExtension> = make();
}

/// `WordListImportExtension` (`@experimental`): MS Word list paste support.
/// `ListExtension` doesn't depend on it, so an editor opts in by depending
/// on it.
pub fn word_list_import_extension() -> Rc<LexicalExtension> {
    WORD_LIST_IMPORT_EXTENSION.with(Rc::clone)
}
