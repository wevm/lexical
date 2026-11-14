//! `import/coreImportRules.ts`: the `DOMImportExtension` rules for
//! paragraphs, text, line breaks, the inline format tags, `<hr>` and
//! transparent blocks (`@experimental` upstream).

use super::define_import_rule::{DomImportRule, define_import_rule};
use super::import_context::{
    WhitespaceImportConfig, import_text_format, import_text_style, import_whitespace_config,
};
use super::run_import::ImportChildrenOpts;
use super::schemas::{block_schema, propagate_text_align_to_block_children};
use super::sel;
use crate::constants::{
    IS_BOLD, IS_CAPITALIZE, IS_CODE, IS_HIGHLIGHT, IS_ITALIC, IS_LOWERCASE, IS_STRIKETHROUGH,
    IS_SUBSCRIPT, IS_SUPERSCRIPT, IS_UNDERLINE, IS_UPPERCASE,
};
use crate::dom::DomNode;
use crate::extension::create_horizontal_rule_node;
use crate::html::internal::context_record::context_value;
use crate::lexical_dom::{
    is_block_dom_node, is_last_child_in_block_node, is_only_child_in_block_node,
    set_direction_from_dom, set_format_from_dom, set_node_indent_from_dom,
};
use crate::node::NodeRef;
use crate::nodes::line_break::create_line_break_node;
use crate::nodes::paragraph::create_paragraph_node;
use crate::nodes::text::create_text_node;
use crate::selection_insert_nodes::generate_nodes_from_raw_text;
use crate::set_dom_style::StyleObject;

/// `ALIGNMENT_VALUES`.
const ALIGNMENT_VALUES: [&str; 6] = ["center", "end", "justify", "left", "right", "start"];

/// `isAlignmentValue(value)`: a non-empty `ElementFormatType`.
pub fn is_alignment_value(value: &str) -> bool {
    ALIGNMENT_VALUES.contains(&value)
}

/// `FormatOverride`: bits to set and to clear.
#[derive(Default)]
struct FormatOverride {
    set: i32,
    clear: i32,
}

/// `FormatStyle`: the inline style properties that affect text format
/// (empty for unset).
#[derive(Clone, Default)]
struct FormatStyle {
    font_weight: String,
    font_style: String,
    text_decoration: String,
    vertical_align: String,
    text_transform: String,
}

/// `TAG_DEFAULT_STYLE`: the style each inline format tag implies.
fn tag_default_style(node_name: &str) -> Option<FormatStyle> {
    let mut style = FormatStyle::default();
    match node_name {
        "B" | "STRONG" => style.font_weight = "bold".to_owned(),
        "EM" | "I" => style.font_style = "italic".to_owned(),
        "S" => style.text_decoration = "line-through".to_owned(),
        "SUB" => style.vertical_align = "sub".to_owned(),
        "SUP" => style.vertical_align = "super".to_owned(),
        "U" => style.text_decoration = "underline".to_owned(),
        _ => return None,
    }
    Some(style)
}

/// `TAG_ONLY_SET`: tags whose format has no CSS analog.
fn tag_only_set(node_name: &str) -> i32 {
    match node_name {
        "CODE" => IS_CODE,
        "MARK" => IS_HIGHLIGHT,
        _ => 0,
    }
}

fn read_element_format_style(el: &DomNode) -> FormatStyle {
    let style = el.style();
    FormatStyle {
        font_style: style.get_property_value("font-style"),
        font_weight: style.get_property_value("font-weight"),
        text_decoration: style.get_property_value("text-decoration"),
        text_transform: style.get_property_value("text-transform"),
        vertical_align: style.get_property_value("vertical-align"),
    }
}

fn merge_styles(defaults: FormatStyle, over: FormatStyle) -> FormatStyle {
    let pick = |a: String, b: String| if a.is_empty() { b } else { a };
    FormatStyle {
        font_style: pick(over.font_style, defaults.font_style),
        font_weight: pick(over.font_weight, defaults.font_weight),
        text_decoration: pick(over.text_decoration, defaults.text_decoration),
        text_transform: pick(over.text_transform, defaults.text_transform),
        vertical_align: pick(over.vertical_align, defaults.vertical_align),
    }
}

/// `FORMAT_BIT_STYLE_PROPS`: the properties `ImportTextFormat` owns.
const FORMAT_BIT_STYLE_PROPS: [&str; 5] = [
    "font-weight",
    "font-style",
    "text-decoration",
    "text-transform",
    "vertical-align",
];

fn style_format_override(style: &FormatStyle) -> FormatOverride {
    let mut o = FormatOverride::default();
    match style.font_weight.as_str() {
        "700" | "bold" => o.set |= IS_BOLD,
        "normal" | "400" => o.clear |= IS_BOLD,
        _ => {}
    }
    match style.font_style.as_str() {
        "italic" => o.set |= IS_ITALIC,
        "normal" => o.clear |= IS_ITALIC,
        _ => {}
    }
    if !style.text_decoration.is_empty() {
        let parts: Vec<&str> = style.text_decoration.split(' ').collect();
        if parts.contains(&"underline") {
            o.set |= IS_UNDERLINE;
        }
        if parts.contains(&"line-through") {
            o.set |= IS_STRIKETHROUGH;
        }
        if parts.contains(&"none") {
            o.clear |= IS_UNDERLINE | IS_STRIKETHROUGH;
        }
    }
    // TextNode.exportDOM writes one of these for the capitalization
    // formats, which are mutually exclusive (#8915).
    match style.text_transform.as_str() {
        "lowercase" => {
            o.set |= IS_LOWERCASE;
            o.clear |= IS_UPPERCASE | IS_CAPITALIZE;
        }
        "uppercase" => {
            o.set |= IS_UPPERCASE;
            o.clear |= IS_LOWERCASE | IS_CAPITALIZE;
        }
        "capitalize" => {
            o.set |= IS_CAPITALIZE;
            o.clear |= IS_LOWERCASE | IS_UPPERCASE;
        }
        "none" => o.clear |= IS_LOWERCASE | IS_UPPERCASE | IS_CAPITALIZE,
        _ => {}
    }
    match style.vertical_align.as_str() {
        "sub" => {
            o.set |= IS_SUBSCRIPT;
            o.clear |= IS_SUPERSCRIPT;
        }
        "super" => {
            o.set |= IS_SUPERSCRIPT;
            o.clear |= IS_SUBSCRIPT;
        }
        "baseline" => o.clear |= IS_SUBSCRIPT | IS_SUPERSCRIPT,
        _ => {}
    }
    o
}

fn apply_format_override(format: i32, ov: &FormatOverride) -> i32 {
    (format & !ov.clear) | ov.set
}

/// `InlineFormatRule`: the inline format tags and `<span>`. The tag's
/// implied style, merged with the element's inline style, sets and clears
/// `ImportTextFormat` bits for the children.
fn inline_format_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/html/inline-format"),
        sel::tag(&[
            "b", "strong", "em", "i", "code", "mark", "s", "sub", "sup", "u", "span",
        ]),
        |ctx, el, _| {
            let format_cfg = import_text_format();
            let inherited = ctx.get(&format_cfg);
            let name = el.node_name();
            let el_style = read_element_format_style(el);
            let effective = match tag_default_style(&name) {
                Some(defaults) => merge_styles(defaults, el_style),
                None => el_style,
            };
            let merged = apply_format_override(inherited, &style_format_override(&effective))
                | tag_only_set(&name);
            if merged == inherited {
                return ctx.import_children(el, &ImportChildrenOpts::default());
            }
            ctx.import_children(
                el,
                &ImportChildrenOpts {
                    context: Some(vec![context_value(&format_cfg, merged)]),
                    ..ImportChildrenOpts::default()
                },
            )
        },
    )
}

fn is_inside_preserve_whitespace(node: &DomNode, ws: &WhitespaceImportConfig) -> bool {
    let mut current = node.parent_node();
    while let Some(c) = current {
        if (ws.preserves_whitespace)(&c) {
            return true;
        }
        current = c.parent_node();
    }
    false
}

/// `findAdjacentTextOnLine(text, forward, wsConfig)`.
fn find_adjacent_text_on_line(
    text: &DomNode,
    forward: bool,
    ws: &WhitespaceImportConfig,
) -> Option<DomNode> {
    let mut node = text.clone();
    loop {
        let sibling = loop {
            let sibling = if forward {
                node.next_sibling()
            } else {
                node.previous_sibling()
            };
            match sibling {
                Some(s) => break s,
                None => node = node.parent_node()?,
            }
        };
        node = sibling;
        if !(ws.is_inline)(&node) {
            return None;
        }
        while let Some(descendant) = if forward {
            node.first_child()
        } else {
            node.last_child()
        } {
            node = descendant;
        }
        if node.is_text() {
            return Some(node);
        }
        if node.node_name() == "BR" {
            return None;
        }
    }
}

/// `/[ \t\n]/` on one UTF-16 unit.
fn is_collapsible_space(u: u16) -> bool {
    u == 0x20 || u == 0x09 || u == 0x0A
}

/// `collapseWhitespace(textNode, wsConfig)`: CRs dropped and runs of spaces,
/// tabs and newlines made one space, which a line's start or end, or a
/// space ending the text before, drops.
fn collapse_whitespace(text_node: &DomNode, ws: &WhitespaceImportConfig) -> String {
    let mut units: Vec<u16> = Vec::new();
    let mut in_space = false;
    for u in text_node
        .text_content()
        .encode_utf16()
        .filter(|&u| u != 0x0D)
    {
        if is_collapsible_space(u) {
            if !in_space {
                units.push(0x20);
            }
            in_space = true;
        } else {
            units.push(u);
            in_space = false;
        }
    }
    if units.is_empty() {
        return String::new();
    }
    if units[0] == 0x20 {
        let mut neighbor = Some(text_node.clone());
        let mut is_start_of_line = true;
        while let Some(found) = neighbor
            .as_ref()
            .and_then(|n| find_adjacent_text_on_line(n, false, ws))
        {
            let content: Vec<u16> = found.text_content().encode_utf16().collect();
            neighbor = Some(found);
            if let Some(&last) = content.last() {
                if is_collapsible_space(last) {
                    units.remove(0);
                }
                is_start_of_line = false;
                break;
            }
        }
        if is_start_of_line {
            units.remove(0);
        }
    }
    if units.last() == Some(&0x20) {
        let mut neighbor = Some(text_node.clone());
        let mut is_end_of_line = true;
        while let Some(found) = neighbor
            .as_ref()
            .and_then(|n| find_adjacent_text_on_line(n, true, ws))
        {
            let content = found.text_content();
            neighbor = Some(found);
            if !strip_leading_whitespace_run(&content).is_empty() {
                is_end_of_line = false;
                break;
            }
        }
        if is_end_of_line {
            units.pop();
        }
    }
    String::from_utf16_lossy(&units)
}

/// `text.replace(/^( |\t|\r?\n)+/, '')`.
fn strip_leading_whitespace_run(text: &str) -> &str {
    let bytes = text.as_bytes();
    let mut i = 0;
    loop {
        match bytes.get(i) {
            Some(b' ' | b'\t' | b'\n') => i += 1,
            Some(b'\r') if bytes.get(i + 1) == Some(&b'\n') => i += 2,
            _ => return &text[i..],
        }
    }
}

/// `$applyFormat(node, format)`.
fn apply_format(node: &NodeRef, format: i32) {
    if format != 0 && node.is_text() {
        node.set_format(f64::from(format));
    }
}

/// `styleObjectToCSS(style)`: the inverse of `getStyleObjectFromCSS`,
/// without the properties the format bits own.
fn style_object_to_css(style: &StyleObject) -> String {
    let mut css = String::new();
    for (prop, value) in style.iter() {
        if FORMAT_BIT_STYLE_PROPS.contains(&prop) {
            continue;
        }
        css.push_str(&format!("{prop}: {value}; "));
    }
    css.trim_end().to_owned()
}

/// `$applyTextStyle(node, style)`.
fn apply_text_style(node: &NodeRef, style: &StyleObject) {
    if node.is_text() {
        let css = style_object_to_css(style);
        if !css.is_empty() {
            node.set_style(&css);
        }
    }
}

/// `TextRule`: `#text`, its whitespace kept (as line breaks and tabs)
/// inside an ancestor that preserves it, else collapsed against its
/// neighbors.
fn text_rule() -> DomImportRule {
    define_import_rule(Some("@lexical/html/#text"), sel::text(), |ctx, el, _| {
        let format = ctx.get(&import_text_format());
        let style = ctx.get(&import_text_style());
        let ws = ctx.get(&import_whitespace_config());
        if is_inside_preserve_whitespace(el, &ws) {
            let out = generate_nodes_from_raw_text(&el.text_content());
            for node in &out {
                apply_format(node, format);
                apply_text_style(node, &style);
            }
            return Ok(out);
        }
        let collapsed = collapse_whitespace(el, &ws);
        if collapsed.is_empty() {
            return Ok(Vec::new());
        }
        let text = create_text_node(&collapsed);
        apply_format(&text, format);
        apply_text_style(&text, &style);
        Ok(vec![text])
    })
}

/// `IgnoreScriptStyleRule`: drops `<script>` and `<style>` and their
/// contents.
fn ignore_script_style_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/html/script-style-ignore"),
        sel::tag(&["script", "style"]),
        |_, _, _| Ok(Vec::new()),
    )
}

/// `LineBreakRule`: `<br>`, but for one that is the only or last child of
/// a block.
fn line_break_rule() -> DomImportRule {
    define_import_rule(Some("@lexical/html/br"), sel::tag(&["br"]), |_, el, _| {
        Ok(
            if is_only_child_in_block_node(el) || is_last_child_in_block_node(el) {
                Vec::new()
            } else {
                vec![create_line_break_node()]
            },
        )
    })
}

/// `ParagraphRule`: `<p>`, with its format, indent, direction and `align`.
fn paragraph_rule() -> DomImportRule {
    define_import_rule(Some("@lexical/html/p"), sel::tag(&["p"]), |ctx, el, _| {
        let p = create_paragraph_node();
        set_format_from_dom(&p, el);
        set_node_indent_from_dom(el, &p)?;
        if p.element_data().get_format_type().is_empty()
            && let Some(align) = el.get_attribute("align")
            && is_alignment_value(&align)
        {
            p.set_element_format(&align);
        }
        set_direction_from_dom(&p, el);
        // No schema: a paragraph takes any inline run as it is.
        let children = ctx.import_children(el, &ImportChildrenOpts::default())?;
        p.splice(0, 0, &children)?;
        Ok(vec![p])
    })
}

/// `HorizontalRuleRule`: `<hr>`, when `HorizontalRuleNode` is registered;
/// otherwise the next rule.
pub(crate) fn horizontal_rule_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/html/hr"),
        sel::tag(&["hr"]),
        |ctx, _, next| {
            if ctx
                .editor()
                .registered_nodes()
                .contains_key("horizontalrule")
            {
                Ok(vec![create_horizontal_rule_node()])
            } else {
                next()
            }
        },
    )
}

/// `TransparentBlockRule`: any other block element, its children through
/// `BlockSchema`, so each inline run becomes a paragraph with the element's
/// `text-align`.
fn transparent_block_rule() -> DomImportRule {
    define_import_rule(
        Some("@lexical/html/transparent-block"),
        sel::any(),
        |ctx, el, next| {
            if !is_block_dom_node(el) {
                return next();
            }
            let children = ctx.import_children(
                el,
                &ImportChildrenOpts {
                    schema: Some(block_schema()),
                    ..ImportChildrenOpts::default()
                },
            )?;
            Ok(propagate_text_align_to_block_children(children, Some(el)))
        },
    )
}

/// `CoreImportRules`: the rules for the core nodes and `<hr>`.
pub fn core_import_rules() -> Vec<DomImportRule> {
    vec![
        ignore_script_style_rule(),
        paragraph_rule(),
        horizontal_rule_rule(),
        transparent_block_rule(),
        text_rule(),
        line_break_rule(),
        inline_format_rule(),
    ]
}
