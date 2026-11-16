use crate::case::{Case, Outcome, same};
use lexical::dom::{DomNode, is_legacy_entity_name, named_entity, parse_from_string};
use lexical::html::{generate_html_from_nodes, generate_nodes_from_dom};
use lexical::json::Value;
use lexical::{CreateEditorArgs, NodeRef, UpdateOptions, create_editor};
use std::cell::RefCell;
use std::rc::Rc;

/// `html/parse`: `DOMParser` builds the same document as jsdom's, compared
/// by the `innerHTML` of its head and body.
pub fn check_parse(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let document = parse_from_string(case.str("html")?);
    let inner = |e: Option<DomNode>| e.map(|e| e.inner_html());
    same(
        "head",
        inner(document.head()).as_deref(),
        Some(case.str("head")?),
    )?;
    same(
        "body",
        inner(document.body()).as_deref(),
        Some(case.str("body")?),
    )?;
    Ok(Outcome::Pass)
}

/// `html/entities`: each named character reference decodes to the text
/// jsdom gives, with and (for a legacy name) without its semicolon, and the
/// generated table (`rust/scripts/generate-dom-tables.mjs`) agrees.
pub fn check_entity(case: &Value, file: &Value) -> Result<Outcome, String> {
    let name = case.str("name")?;
    let value = case.str("value")?;
    let legacy = case.field("legacy")?.as_bool().ok_or("legacy")?;
    same("table value", named_entity(name), Some(value))?;
    same("table legacy", is_legacy_entity_name(name), legacy)?;
    let count = file.arr("cases")?.len();
    let table = lexical::dom::named_entity_count();
    same("names in the table", table, count)?;
    // Without its semicolon, a name that isn't legacy may still start with
    // one that is (`&centerdot` is `&cent` + "erdot").
    let (html, expected) = if legacy {
        (
            format!("<p>&{name};|&{name}</p>"),
            format!("{value}|{value}"),
        )
    } else {
        (format!("<p>&{name};</p>"), value.to_owned())
    };
    let body = parse_from_string(&html).body().ok_or("no body")?;
    same("textContent", body.text_content(), expected)?;
    Ok(Outcome::Pass)
}

/// `html/export`: `$generateHtmlFromNodes` writes the HTML jsdom gave for
/// the document, with the case's theme.
pub fn check_export(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let theme = match case.get("theme") {
        Some(Value::Object(theme)) => theme.clone(),
        _ => lexical::json::Object::new(),
    };
    let editor = create_editor(CreateEditorArgs {
        theme,
        ..Default::default()
    });
    let state = editor
        .parse_editor_state(case.str("editorState")?)
        .map_err(|e| format!("editorState: {e}"))?;
    editor
        .set_editor_state(&state, None)
        .map_err(|e| e.to_string())?;
    let html = editor
        .read(|| generate_html_from_nodes(&editor, None))
        .and_then(|html| html)
        .map_err(|e| e.to_string())?;
    same("html", html.as_str(), case.str("html")?)?;
    Ok(Outcome::Pass)
}

/// A node and its subtree as JSON, as `exportJSON` writes each node
/// (`$nodeToJSON`).
fn node_to_json(node: &NodeRef) -> Value {
    let mut json = node.export_json(false);
    if node.is_element() {
        json.insert(
            "children",
            Value::Array(node.get_children().iter().map(node_to_json).collect()),
        );
    }
    Value::Object(json)
}

/// `html/import`: `$generateNodesFromDOM` makes the nodes jsdom's import
/// made of the parsed HTML.
pub fn check_import(case: &Value, _file: &Value) -> Result<Outcome, String> {
    import(case, create_editor(CreateEditorArgs::default()))
}

/// `html/import-nodes`: as `html/import`, with the node packages'
/// importers (the nodes of `property/documents` and `HorizontalRuleNode`).
pub fn check_import_nodes(case: &Value, _file: &Value) -> Result<Outcome, String> {
    let editor = create_editor(CreateEditorArgs {
        nodes: [
            lexical::rich_text::rich_text_nodes(),
            lexical::list::list_nodes(),
            lexical::link::link_nodes(),
            lexical::code::code_nodes(),
            lexical::overflow::overflow_nodes(),
            lexical::hashtag::hashtag_nodes(),
            lexical::extension::extension_nodes(),
        ]
        .concat(),
        ..Default::default()
    });
    import(case, editor)
}

fn import(case: &Value, editor: lexical::Editor) -> Result<Outcome, String> {
    let dom = parse_from_string(case.str("html")?);
    let nodes = Rc::new(RefCell::new(String::new()));
    let (out, update_editor) = (nodes.clone(), editor.clone());
    editor
        .update(
            move || {
                let nodes = generate_nodes_from_dom(&update_editor, &dom)?;
                *out.borrow_mut() =
                    Value::Array(nodes.iter().map(node_to_json).collect()).to_string();
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .map_err(|e| e.to_string())?;
    same("nodes", nodes.borrow().as_str(), case.str("nodes")?)?;
    Ok(Outcome::Pass)
}

/// `html/import-extension`: `DOMImportExtension`'s import, in an editor
/// built from `CoreImportExtension` and `HorizontalRuleNode`, makes the
/// nodes it made in jsdom (with the document's `<style>` sheets inlined).
pub fn check_import_extension(case: &Value, _file: &Value) -> Result<Outcome, String> {
    use lexical::html::import::{core_import_extension, generate_nodes_from_dom_via_extension};
    use lexical::{LexicalExtension, define_extension};
    let host = define_extension(LexicalExtension::<(), (), ()> {
        dependencies: vec![core_import_extension().into()],
        nodes: lexical::extension::extension_nodes()
            .into_iter()
            .filter(|class| class.node_type() == "horizontalrule")
            .collect(),
        ..LexicalExtension::new("[conformance]", ())
    });
    let editor = lexical::extension::build_editor_from_extensions([host.into()])
        .map_err(|e| e.to_string())?;
    let dom = parse_from_string(case.str("html")?);
    let nodes = Rc::new(RefCell::new(String::new()));
    let out = nodes.clone();
    let result = editor.update(
        move || {
            let nodes = generate_nodes_from_dom_via_extension(&dom, None)?;
            *out.borrow_mut() = Value::Array(nodes.iter().map(node_to_json).collect()).to_string();
            Ok(())
        },
        UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        },
    );
    editor.dispose();
    result.map_err(|e| e.to_string())?;
    same("nodes", nodes.borrow().as_str(), case.str("nodes")?)?;
    Ok(Outcome::Pass)
}

/// Adds `value` to the element's `attr`, space-separated (`$tagExport`).
fn tag_export(
    output: lexical::lexical_dom::DomExportOutput,
    attr: &str,
    value: &str,
) -> lexical::lexical_dom::DomExportOutput {
    if let Some(element) = output.element.as_ref().filter(|e| e.is_element()) {
        let tagged = match element.get_attribute(attr) {
            Some(prev) if !prev.is_empty() => format!("{prev} {value}"),
            _ => value.to_owned(),
        };
        element.set_attribute(attr, &tagged);
    }
    output
}

/// The render states of `html/export-render`'s scenarios, made once
/// (`RENDER_FLAG` and `RENDER_LABEL`).
struct RenderStates {
    flag: lexical::html::RenderStateConfig<bool>,
    label: lexical::html::RenderStateConfig<String>,
}

thread_local! {
    static RENDER_STATES: RenderStates = RenderStates {
        flag: lexical::html::create_render_state("flag", || false),
        label: lexical::html::create_render_state("label", || "none".to_owned()),
    };
}

/// `RENDER_SCENARIOS[name]` of html.test.ts.
fn render_scenario(name: &str) -> Result<lexical::html::DomRenderConfigOverrides, String> {
    use lexical::html::import::context_value;
    use lexical::html::{
        DomOverrideOptions, DomRenderConfigOverrides, DomRenderMatchConfig, NodeMatch,
        dom_override, get_render_context_value, render_context_export, render_context_root,
    };
    use lexical::nodes::paragraph::ParagraphNode;
    use lexical::nodes::tab::TabNode;
    use lexical::nodes::text::TextNode;
    let (flag, label) = RENDER_STATES.with(|s| (s.flag.clone(), s.label.clone()));
    let paragraph = || Some(vec![NodeMatch::class::<ParagraphNode>()]);
    let is_paragraph = |node: &NodeRef| node.get_type() == "paragraph";
    let tag = |attr: &'static str, value: &'static str| DomRenderMatchConfig {
        export_dom: Some(Rc::new(move |_, _, next, _| {
            Ok(tag_export(next()?, attr, value))
        })),
        ..Default::default()
    };
    let (overrides, context_defaults) = match name {
        "context" => (
            vec![dom_override(
                paragraph(),
                DomRenderMatchConfig {
                    export_dom: Some(Rc::new(move |_, _, next, editor| {
                        let value = [
                            get_render_context_value(&label, editor),
                            get_render_context_value(&render_context_export(), editor).to_string(),
                            get_render_context_value(&render_context_root(), editor).to_string(),
                        ]
                        .join(" ");
                        Ok(tag_export(next()?, "data-label", &value))
                    })),
                    ..Default::default()
                },
                DomOverrideOptions::default(),
            )],
            vec![context_value(
                &RENDER_STATES.with(|s| s.label.clone()),
                "default".to_owned(),
            )],
        ),
        "create-dom" => (
            vec![dom_override(
                paragraph(),
                DomRenderMatchConfig {
                    create_dom: Some(Rc::new(|_, this, next, _| {
                        let element = next()?;
                        element.set_attribute("data-size", &this.get_children_size().to_string());
                        Ok(element)
                    })),
                    ..Default::default()
                },
                DomOverrideOptions::default(),
            )],
            Vec::new(),
        ),
        "editor-gate" => (
            vec![dom_override(
                None,
                tag("data-gate", "on"),
                DomOverrideOptions {
                    disabled_for_editor: Some(Rc::new(move |reader| reader.get(&flag))),
                    ..Default::default()
                },
            )],
            vec![context_value(&RENDER_STATES.with(|s| s.flag.clone()), true)],
        ),
        "exclude" => (
            vec![dom_override(
                paragraph(),
                DomRenderMatchConfig {
                    should_exclude: Some(Rc::new(|node, _, next, _| {
                        Ok(node.get_children_size() > 2 || next()?)
                    })),
                    ..Default::default()
                },
                DomOverrideOptions::default(),
            )],
            Vec::new(),
        ),
        "include" => (
            vec![dom_override(
                None,
                DomRenderMatchConfig {
                    extract_with_child: Some(Rc::new(move |node, child, _, _, next, _| {
                        Ok(
                            (is_paragraph(node) && child.is_text() && child.has_format("bold"))
                                || next()?,
                        )
                    })),
                    should_include: Some(Rc::new(move |node, _, next, _| {
                        Ok(!is_paragraph(node) && next()?)
                    })),
                    ..Default::default()
                },
                DomOverrideOptions::default(),
            )],
            Vec::new(),
        ),
        "none" => (Vec::new(), Vec::new()),
        "order" => (
            vec![
                dom_override(None, tag("data-order", "wildcard"), Default::default()),
                dom_override(
                    Some(vec![NodeMatch::predicate(|_, node| {
                        lexical::utf16::len(&node.get_text_content()) > 2
                    })]),
                    tag("data-order", "predicate"),
                    Default::default(),
                ),
                dom_override(
                    Some(vec![NodeMatch::class::<TabNode>()]),
                    tag("data-order", "tab"),
                    Default::default(),
                ),
                dom_override(
                    Some(vec![NodeMatch::class::<TextNode>()]),
                    tag("data-order", "text"),
                    Default::default(),
                ),
            ],
            Vec::new(),
        ),
        "session-gate" => (
            vec![dom_override(
                None,
                tag("data-session", "on"),
                DomOverrideOptions {
                    disabled_for_session: Some(Rc::new(|reader| {
                        reader.get(&render_context_root())
                    })),
                    ..Default::default()
                },
            )],
            Vec::new(),
        ),
        other => return Err(format!("unknown scenario {other}")),
    };
    Ok(DomRenderConfigOverrides {
        overrides: Some(overrides),
        context_defaults: Some(context_defaults),
    })
}

/// `html/export-render`: an editor built with `DOMRenderExtension` and the
/// case's scenario exports the HTML jsdom gave (`$renderExport`).
pub fn check_export_render(case: &Value, _file: &Value) -> Result<Outcome, String> {
    use lexical::html::import::context_updater;
    use lexical::html::{
        dom_render_extension, generate_dom_from_root, set_render_context_value, with_render_context,
    };
    use lexical::{LexicalExtension, config_extension, define_extension, get_root};
    let host = define_extension(LexicalExtension::<(), (), ()> {
        dependencies: vec![config_extension(
            &dom_render_extension(),
            [render_scenario(case.str("scenario")?)?],
        )],
        ..LexicalExtension::new("[conformance]", ())
    });
    let editor = lexical::extension::build_editor_from_extensions([host.into()])
        .map_err(|e| e.to_string())?;
    let state = editor
        .parse_editor_state(case.str("editorState")?)
        .map_err(|e| format!("editorState: {e}"))?;
    editor
        .set_editor_state(&state, None)
        .map_err(|e| e.to_string())?;
    if case.get("flagOff").and_then(Value::as_bool) == Some(true) {
        let flag = RENDER_STATES.with(|s| s.flag.clone());
        set_render_context_value(&flag, false, &editor).map_err(|e| e.to_string())?;
    }
    let op = case.str("op")?;
    let html = editor
        .read(|| match op {
            "root" => {
                let container = lexical::dom::get_document().create_element("div");
                generate_dom_from_root(&container, &get_root(), &editor)?;
                Ok(container.inner_html())
            }
            "scoped" => {
                let label = RENDER_STATES.with(|s| s.label.clone());
                let cfg = [context_updater(&label, |l| format!("{l}+scoped"))];
                with_render_context(&cfg, &editor, || generate_html_from_nodes(&editor, None))
            }
            _ => generate_html_from_nodes(&editor, None),
        })
        .and_then(|html| html);
    editor.dispose();
    same(
        "html",
        html.map_err(|e| e.to_string())?.as_str(),
        case.str("html")?,
    )?;
    Ok(Outcome::Pass)
}

/// `html/import-packages`: `DOMImportExtension`'s import with the
/// packages' rules, in an editor built from `RichTextExtension`,
/// `CheckListExtension`, `LinkExtension`, `CodeExtension`,
/// `WordListImportExtension` and `HorizontalRuleNode` (and, for
/// `shadowRootQuote`, `ShadowRootQuoteRule`).
pub fn check_import_packages(case: &Value, _file: &Value) -> Result<Outcome, String> {
    use lexical::html::import::{
        DomImportConfigOverrides, dom_import_extension, generate_nodes_from_dom_via_extension,
    };
    use lexical::{LexicalExtension, config_extension, define_extension};
    let mut dependencies = vec![
        lexical::rich_text::rich_text_extension().into(),
        lexical::list::check_list_extension().into(),
        lexical::link::link_extension().into(),
        lexical::code::code_extension().into(),
        lexical::list::word_list_import_extension().into(),
    ];
    if case.get("shadowRootQuote").and_then(Value::as_bool) == Some(true) {
        dependencies.push(config_extension(
            &dom_import_extension(),
            [DomImportConfigOverrides {
                rules: Some(vec![lexical::rich_text::shadow_root_quote_rule().into()]),
                ..Default::default()
            }],
        ));
    }
    // `HorizontalRuleExtension` isn't ported; its node is.
    let host = define_extension(LexicalExtension::<(), (), ()> {
        dependencies,
        nodes: lexical::extension::extension_nodes()
            .into_iter()
            .filter(|class| class.node_type() == "horizontalrule")
            .collect(),
        ..LexicalExtension::new("[conformance]", ())
    });
    let editor = lexical::extension::build_editor_from_extensions([host.into()])
        .map_err(|e| e.to_string())?;
    let dom = parse_from_string(case.str("html")?);
    let nodes = Rc::new(RefCell::new(String::new()));
    let out = nodes.clone();
    let result = editor.update(
        move || {
            let nodes = generate_nodes_from_dom_via_extension(&dom, None)?;
            *out.borrow_mut() = Value::Array(nodes.iter().map(node_to_json).collect()).to_string();
            Ok(())
        },
        UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        },
    );
    editor.dispose();
    result.map_err(|e| e.to_string())?;
    same("nodes", nodes.borrow().as_str(), case.str("nodes")?)?;
    Ok(Outcome::Pass)
}
