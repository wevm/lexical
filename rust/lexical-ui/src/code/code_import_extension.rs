//! `CodeImportExtension.ts`: the `DOMImportExtension` rules for code
//! blocks (`@experimental` upstream), with the GitHub raw file view
//! detectors and the VS Code paste overlay, which `CodeExtension`
//! contributes.

use super::code_node::create_code_node;
use super::flat_structure_utils::plainify_code_content;
use crate::LexicalError;
use crate::dom::DomNode;
use crate::html::import::{
    CompiledOverlayRules, DomImportRule, DomPreprocessFn, ImportChildrenOpts, define_import_rule,
    define_overlay_rules, import_overlays, is_block_level, sel,
};
use crate::node::NodeRef;
use std::rc::Rc;

const LANGUAGE_DATA_ATTRIBUTE: &str = "data-language";
const THEME_DATA_ATTRIBUTE: &str = "data-theme";

/// `$normalizeCodeChildren(children)`: the children's plain text, with a
/// line break on each side of a block, as code highlight nodes.
fn normalize_code_children(children: &[NodeRef]) -> Vec<NodeRef> {
    let mut text = String::new();
    let mut previous_was_block = false;
    for (i, child) in children.iter().enumerate() {
        let is_block = is_block_level(child);
        if i > 0 && (is_block || previous_was_block) {
            text.push('\n');
        }
        text.push_str(&child.get_text_content());
        previous_was_block = is_block;
    }
    plainify_code_content(&text)
}

/// A code block of `language` and `theme` with `children`.
fn code_block(
    language: Option<String>,
    theme: Option<String>,
    children: &[NodeRef],
) -> Result<Vec<NodeRef>, LexicalError> {
    let node = create_code_node(language.as_deref(), theme.as_deref());
    node.splice(0, 0, children)?;
    Ok(vec![node])
}

/// `isMonospaceElement(el)`: a `font-family` that mentions `monospace`
/// (case-sensitively, as `String#match` with a string does).
fn is_monospace_element(el: &DomNode) -> bool {
    el.style()
        .get_property_value("font-family")
        .contains("monospace")
}

/// `isMonospaceDescendant(node)`.
fn is_monospace_descendant(node: &DomNode) -> bool {
    let mut parent = node.parent_element();
    while let Some(p) = parent {
        if is_monospace_element(&p) {
            return true;
        }
        parent = p.parent_element();
    }
    false
}

/// `GitHubCodeTableOverlayRules`: inside a GitHub code table every `<tr>`
/// and `<td>` gives way to its children.
fn github_code_table_overlay_rules() -> Rc<CompiledOverlayRules> {
    define_overlay_rules(&[define_import_rule(
        Some("@lexical/code/github-code-table/unwrap"),
        sel::tag(&["tr", "td"]),
        |ctx, el, _| ctx.import_children(el, &ImportChildrenOpts::default()),
    )
    .into()])
}

/// `PreRule`.
fn pre_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/code/pre"),
        sel::tag(&["pre"]),
        |ctx, el, _| {
            let children = ctx.import_children(el, &ImportChildrenOpts::default())?;
            code_block(
                el.get_attribute(LANGUAGE_DATA_ATTRIBUTE),
                el.get_attribute(THEME_DATA_ATTRIBUTE),
                &normalize_code_children(&children),
            )
        },
    )
}

/// `MultilineCodeRule`: a `<code>` with a newline or a `<br>` is a code
/// block; a single line defers to the inline format rule.
fn multiline_code_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/code/code-multiline"),
        sel::tag(&["code"]),
        |ctx, el, next| {
            let is_multi_line = el.text_content().contains('\n')
                || el
                    .query_selector_all("br")
                    .is_ok_and(|found| !found.is_empty());
            if !is_multi_line {
                return next();
            }
            let children = ctx.import_children(el, &ImportChildrenOpts::default())?;
            code_block(
                el.get_attribute(LANGUAGE_DATA_ATTRIBUTE),
                el.get_attribute(THEME_DATA_ATTRIBUTE),
                &normalize_code_children(&children),
            )
        },
    )
}

/// `isMonospacePreElement(el)`: a `font-family` with `monospace` (in any
/// case) and a `white-space` of `pre*`, the shape of each line VS Code
/// copies.
fn is_monospace_pre_element(el: &DomNode) -> bool {
    if !el.is_element() {
        return false;
    }
    let style = el.style();
    style
        .get_property_value("font-family")
        .to_lowercase()
        .contains("monospace")
        && style.get_property_value("white-space").starts_with("pre")
}

/// `splitMonospaceWrapperLines(el)`: a wrapper's lines (a `<div>` child's
/// text, an empty line for a `<br>`, and runs of other children joined),
/// or `None` when it has no block child.
fn split_monospace_wrapper_lines(el: &DomNode) -> Option<Vec<String>> {
    let mut has_block_child = false;
    let mut lines = Vec::new();
    let mut acc: Option<String> = None;
    for child in el.child_nodes() {
        if child.is_element() {
            match child.tag_name().as_str() {
                "DIV" => {
                    lines.extend(acc.take());
                    lines.push(child.text_content());
                    has_block_child = true;
                }
                "BR" => {
                    lines.extend(acc.take());
                    lines.push(String::new());
                    has_block_child = true;
                }
                _ => acc.get_or_insert_default().push_str(&child.text_content()),
            }
        } else if child.is_text() {
            let t = child.text_content();
            if !t.is_empty() {
                acc.get_or_insert_default().push_str(&t);
            }
        }
    }
    lines.extend(acc);
    has_block_child.then_some(lines)
}

/// `looksLikeVscodePaste(root)`: a monospace pre wrapper with a block
/// child (Chrome), or two monospace pre siblings (Safari).
fn looks_like_vscode_paste(root: &DomNode) -> bool {
    for child in root.children() {
        if is_monospace_pre_element(&child) {
            if split_monospace_wrapper_lines(&child).is_some() {
                return true;
            }
            if child
                .next_element_sibling()
                .is_some_and(|next| is_monospace_pre_element(&next))
            {
                return true;
            }
            continue;
        }
        if looks_like_vscode_paste(&child) {
            return true;
        }
    }
    false
}

/// `VscodeWrapperRule`: Chrome's one wrapper around per-line `<div>`s and
/// `<br>`s, as one code block.
fn vscode_wrapper_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/code/vscode-wrapper"),
        sel::tag(&["div"]),
        |_, el, next| {
            if !is_monospace_pre_element(el) || is_monospace_descendant(el) {
                return next();
            }
            match split_monospace_wrapper_lines(el) {
                Some(lines) if !lines.is_empty() => {
                    code_block(None, None, &plainify_code_content(&lines.join("\n")))
                }
                _ => next(),
            }
        },
    )
}

/// `VscodeLineRunRule`: Safari's run of monospace pre `<div>` and `<br>`
/// siblings, as one code block made by the run's first element.
fn vscode_line_run_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/code/vscode-line-run"),
        sel::tag(&["div", "br"]),
        |_, el, next| {
            if !is_monospace_pre_element(el) || is_monospace_descendant(el) {
                return next();
            }
            if el
                .previous_element_sibling()
                .is_some_and(|prev| is_monospace_pre_element(&prev))
            {
                // An earlier sibling's run took this one in.
                return Ok(Vec::new());
            }
            let mut lines = Vec::new();
            let mut cur = Some(el.clone());
            while let Some(node) = cur.filter(is_monospace_pre_element) {
                lines.push(if node.tag_name() == "BR" {
                    String::new()
                } else {
                    node.text_content()
                });
                cur = node.next_element_sibling();
            }
            if lines.len() < 2 {
                return next();
            }
            code_block(None, None, &plainify_code_content(&lines.join("\n")))
        },
    )
}

/// `$installVscodeCodePasteOverlay` (`@experimental`): installs the VS
/// Code overlay (its wrapper and line run rules) when the DOM looks like a
/// VS Code code block paste.
pub fn install_vscode_code_paste_overlay() -> DomPreprocessFn {
    let overlay =
        define_overlay_rules(&[vscode_wrapper_rule().into(), vscode_line_run_rule().into()]);
    Rc::new(move |dom, ctx, next| {
        let root = if dom.is_document() {
            dom.body()
        } else {
            Some(dom.clone())
        };
        if root.is_some_and(|root| looks_like_vscode_paste(&root)) {
            let overlay = overlay.clone();
            ctx.session.update(&import_overlays(), move |mut prev| {
                prev.push(overlay);
                prev
            });
        }
        next()
    })
}

/// `DivRule`: a monospace `<div>` (Google Docs' code block) is a code
/// block.
fn div_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/code/div"),
        sel::tag(&["div"]),
        |ctx, el, next| {
            if !is_monospace_element(el) {
                return next();
            }
            let children = ctx.import_children(el, &ImportChildrenOpts::default())?;
            code_block(None, None, &normalize_code_children(&children))
        },
    )
}

/// `GitHubCodeTableRule`: GitHub's raw file view table is a code block,
/// its rows and cells unwrapped by an overlay.
fn github_code_table_rule() -> DomImportRule {
    let overlay = github_code_table_overlay_rules();
    define_import_rule(
        Some("@lexical/code/github-code-table"),
        sel::tag(&["table"]).class_all(&["js-file-line-container"]),
        move |ctx, el, _| {
            let opts = ImportChildrenOpts {
                rules: Some(overlay.clone()),
                ..ImportChildrenOpts::default()
            };
            let children = ctx.import_children(el, &opts)?;
            code_block(None, None, &normalize_code_children(&children))
        },
    )
}

/// `GitHubCodeCellByClassRule`: a stray GitHub code line cell gives way to
/// its children.
fn github_code_cell_by_class_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/code/github-code-cell-by-class"),
        sel::tag(&["td"]).class_all(&["js-file-line"]),
        |ctx, el, _| ctx.import_children(el, &ImportChildrenOpts::default()),
    )
}

/// `CodeImportRules` (`@experimental`): the import rules for `CodeNode`,
/// the GitHub detectors first. `CodeExtension` contributes them, with
/// `CoreImportExtension` and [`install_vscode_code_paste_overlay`].
pub fn code_import_rules() -> Vec<DomImportRule> {
    vec![
        github_code_table_rule(),
        github_code_cell_by_class_rule(),
        multiline_code_rule(),
        pre_rule(),
        div_rule(),
    ]
}
