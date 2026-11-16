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
    export(case, Vec::new())
}

/// `html/export-mark`: as `html/export`, with `MarkNode` and `LinkNode`.
pub fn check_export_mark(case: &Value, _file: &Value) -> Result<Outcome, String> {
    export(
        case,
        [lexical::mark::mark_nodes(), lexical::link::link_nodes()].concat(),
    )
}

fn export(case: &Value, nodes: Vec<lexical::NodeClass>) -> Result<Outcome, String> {
    let theme = match case.get("theme") {
        Some(Value::Object(theme)) => theme.clone(),
        _ => lexical::json::Object::new(),
    };
    let editor = create_editor(CreateEditorArgs {
        theme,
        nodes,
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
pub(super) fn node_to_json(node: &NodeRef) -> Value {
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
/// built from `CoreImportExtension` and `HorizontalRuleExtension`, makes the
/// nodes it made in jsdom (with the document's `<style>` sheets inlined).
pub fn check_import_extension(case: &Value, _file: &Value) -> Result<Outcome, String> {
    use lexical::html::import::{core_import_extension, generate_nodes_from_dom_via_extension};
    use lexical::{LexicalExtension, define_extension};
    let host = define_extension(LexicalExtension::<(), (), ()> {
        dependencies: vec![
            core_import_extension().into(),
            lexical::extension::horizontal_rule_extension().into(),
        ],
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
/// `WordListImportExtension` and `HorizontalRuleExtension` (and, for
/// `shadowRootQuote`, `ShadowRootQuoteRule`; for `nodeStructure`,
/// `NodeStructureExtension`).
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
        lexical::extension::horizontal_rule_extension().into(),
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
    if case.get("nodeStructure").and_then(Value::as_bool) == Some(true) {
        dependencies.push(lexical::extension::node_structure_extension().into());
    }
    let host = define_extension(LexicalExtension::<(), (), ()> {
        dependencies,
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

/// The nodes of `property/documents` (`DOCUMENT_NODES`).
fn document_nodes() -> Vec<lexical::NodeClass> {
    [
        lexical::rich_text::rich_text_nodes(),
        lexical::list::list_nodes(),
        lexical::link::link_nodes(),
        lexical::code::code_nodes(),
        lexical::overflow::overflow_nodes(),
        lexical::hashtag::hashtag_nodes(),
    ]
    .concat()
}

/// `clipboardEditor(scenario)`, with the handle that disposes an editor
/// built from extensions.
fn clipboard_editor(
    scenario: &str,
) -> Result<
    (
        lexical::Editor,
        Option<lexical::extension::LexicalEditorWithDispose>,
    ),
    String,
> {
    use lexical::clipboard::{
        ClipboardImportConfigOverrides, ExportMimeTypeFunction, GetClipboardDataConfigOverrides,
        ImportMimeTypeFunction, clipboard_dom_import_extension, clipboard_import_extension,
        get_clipboard_data_extension,
    };
    use lexical::{LexicalExtension, config_extension, define_extension};
    if scenario == "headless" {
        let editor = create_editor(CreateEditorArgs {
            nodes: document_nodes(),
            namespace: "conformance".to_owned(),
            ..Default::default()
        });
        return Ok((editor, None));
    }
    let dependencies = match scenario {
        // `CUSTOM_EXPORT`.
        "custom-export" => {
            let omit: ExportMimeTypeFunction = Rc::new(|_, _| Ok(None));
            let node_count: ExportMimeTypeFunction = Rc::new(|selection, _| {
                selection
                    .map(|s| s.get_nodes().map(|nodes| nodes.len().to_string()))
                    .transpose()
            });
            let wrap: ExportMimeTypeFunction = Rc::new(|_, next| {
                Ok(next()?.map(|html| {
                    if html.is_empty() {
                        html
                    } else {
                        format!("<!--wrapped-->{html}")
                    }
                }))
            });
            vec![config_extension(
                &get_clipboard_data_extension(),
                [GetClipboardDataConfigOverrides {
                    export_mime_type: Some(vec![
                        ("application/x-lexical-editor".to_owned(), vec![omit]),
                        ("application/x-node-count".to_owned(), vec![node_count]),
                        ("text/html".to_owned(), vec![wrap]),
                    ]),
                }],
            )]
        }
        "dom-import" => vec![
            lexical::rich_text::rich_text_extension().into(),
            lexical::list::check_list_extension().into(),
            lexical::link::link_extension().into(),
            lexical::code::code_extension().into(),
            clipboard_dom_import_extension().into(),
        ],
        "next-data" => {
            let first: ImportMimeTypeFunction =
                Rc::new(|html, _, next, _| next(Some(&format!("<p><b>first</b></p>{html}")), None));
            vec![config_extension(
                &clipboard_import_extension(),
                [ClipboardImportConfigOverrides {
                    import_mime_type: Some(vec![("text/html".to_owned(), vec![first])]),
                    ..Default::default()
                }],
            )]
        }
        "priority" => vec![config_extension(
            &clipboard_import_extension(),
            [ClipboardImportConfigOverrides {
                priority: Some(vec![("text/plain".to_owned(), 5.0)]),
                ..Default::default()
            }],
        )],
        other => return Err(format!("unknown scenario {other}")),
    };
    let host = define_extension(LexicalExtension::<(), (), ()> {
        dependencies,
        nodes: document_nodes(),
        ..LexicalExtension::new("[conformance]", ())
    });
    let built = lexical::extension::build_editor_from_extensions([host.into()])
        .map_err(|e| e.to_string())?;
    Ok(((*built).clone(), Some(built)))
}

/// `$nodeAtPath(path)`.
fn node_at_path(path: &Value) -> Result<NodeRef, lexical::LexicalError> {
    let mut node: NodeRef = lexical::get_root();
    for i in path.as_array().into_iter().flatten() {
        let i = i.as_f64().unwrap_or(-1.0) as usize;
        node = node
            .get_child_at_index(i)
            .ok_or_else(|| lexical::LexicalError::new(format!("no node at {path}")))?;
    }
    Ok(node)
}

/// `$path(node)`.
fn path_of(node: &NodeRef) -> Value {
    let mut path = Vec::new();
    let mut current = node.clone();
    while let Some(parent) = current.get_parent() {
        path.insert(
            0,
            Value::from(current.get_index_within_parent().unwrap_or(0) as f64),
        );
        current = parent;
    }
    Value::Array(path)
}

/// `$selectPaths([anchor, focus])`.
fn select_paths(selection: &Value) -> Result<(), lexical::LexicalError> {
    use lexical::{PointType, Which, create_range_selection, set_selection};
    let range = create_range_selection();
    let points = selection.as_array().cloned().unwrap_or_default();
    for (which, point) in [Which::Anchor, Which::Focus].into_iter().zip(points) {
        let parts = point.as_array().cloned().unwrap_or_default();
        let node = node_at_path(&parts[0])?;
        let offset = parts[1].as_f64().unwrap_or(0.0) as usize;
        let point_type = match parts[2].as_str() {
            Some("text") => PointType::Text,
            _ => PointType::Element,
        };
        range.point_set(which, node.get_key(), offset, point_type)?;
    }
    set_selection(Some(range));
    Ok(())
}

/// `$selectionPaths()`.
fn selection_paths() -> Value {
    use lexical::Which;
    let Some(selection) = lexical::get_selection().filter(|s| s.is_range()) else {
        return Value::Null;
    };
    let point = |which| {
        let p = selection.point(which).expect("a range selection");
        let node = lexical::get_node_by_key(p.key.as_str()).expect("the point's node");
        Value::Array(vec![
            path_of(&node),
            (p.offset as f64).into(),
            p.point_type.as_str().into(),
        ])
    };
    Value::Array(vec![point(Which::Anchor), point(Which::Focus)])
}

/// `html/clipboard`: at the case's selection, `$getClipboardDataFromSelection`
/// gives JS's entries (`copy`), or `$insertDataTransferForRichText` of its
/// data leaves JS's editor state and selection (`paste`), in the case's
/// scenario's editor (`clipboardEditor`).
pub fn check_clipboard(case: &Value, _file: &Value) -> Result<Outcome, String> {
    use lexical::DataTransfer;
    use lexical::clipboard::{
        get_clipboard_data_from_selection, insert_data_transfer_for_rich_text,
    };
    let (editor, built) = clipboard_editor(case.str("scenario")?)?;
    let state = editor
        .parse_editor_state(case.str("editorState")?)
        .map_err(|e| format!("editorState: {e}"))?;
    editor
        .set_editor_state(&state, None)
        .map_err(|e| e.to_string())?;
    let selection = case.get("selection").cloned().unwrap_or(Value::Null);
    let options = UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    };
    let result = match case.get("data") {
        None => {
            let out = Rc::new(RefCell::new(Value::Null));
            let write = out.clone();
            editor
                .update(
                    move || {
                        select_paths(&selection)?;
                        let data =
                            get_clipboard_data_from_selection(lexical::get_selection().as_ref())?;
                        *write.borrow_mut() = Value::Array(
                            data.into_iter()
                                .map(|(k, v)| Value::Array(vec![k.into(), v.into()]))
                                .collect(),
                        );
                        Ok(())
                    },
                    options,
                )
                .map_err(|e| e.to_string())
                .and_then(|()| {
                    same(
                        "clipboard",
                        &out.borrow().to_string(),
                        &case
                            .get("clipboard")
                            .cloned()
                            .unwrap_or(Value::Null)
                            .to_string(),
                    )
                })
        }
        Some(data) => {
            let data_transfer = DataTransfer::new();
            for entry in data.as_array().into_iter().flatten() {
                if let Some([k, v]) = entry.as_array().map(Vec::as_slice) {
                    data_transfer.set_data(k.as_str().unwrap_or(""), v.as_str().unwrap_or(""));
                }
            }
            let paste_editor = editor.clone();
            editor
                .update(
                    move || {
                        select_paths(&selection)?;
                        let selection = lexical::get_selection()
                            .ok_or_else(|| lexical::LexicalError::new("no selection"))?;
                        insert_data_transfer_for_rich_text(
                            &data_transfer,
                            &selection,
                            &paste_editor,
                        )
                    },
                    options,
                )
                .map_err(|e| e.to_string())
                .and_then(|()| {
                    same(
                        "state",
                        editor
                            .get_editor_state()
                            .to_json(false)
                            .to_string()
                            .as_str(),
                        case.str("state")?,
                    )?;
                    let after = editor.read(selection_paths).map_err(|e| e.to_string())?;
                    same(
                        "after",
                        &after.to_string(),
                        &case
                            .get("after")
                            .cloned()
                            .unwrap_or(Value::Null)
                            .to_string(),
                    )
                })
        }
    };
    if let Some(built) = built {
        built.dispose();
    }
    result?;
    Ok(Outcome::Pass)
}

/// `blockIdEditor(scenario)`: the package extensions, `BlockIdExtension`
/// with generated ids id1, id2... and `BlockIdHTMLExtension` with the
/// scenario's config.
fn block_id_editor(scenario: &str) -> Result<lexical::extension::LexicalEditorWithDispose, String> {
    use lexical::extension::{BlockIdConfigOverrides, GenerateId, block_id_extension};
    use lexical::html::{BlockIdHtmlConfigOverrides, block_id_html_extension};
    use lexical::{LexicalExtension, config_extension, define_extension};
    let html = match scenario {
        "default" => BlockIdHtmlConfigOverrides::default(),
        "export-off" => BlockIdHtmlConfigOverrides {
            export: Some(false),
            ..Default::default()
        },
        "handles" => BlockIdHtmlConfigOverrides {
            handles: Some(true),
            ..Default::default()
        },
        "import-off" => BlockIdHtmlConfigOverrides {
            import: Some(false),
            ..Default::default()
        },
        other => return Err(format!("unknown scenario {other}")),
    };
    let n = std::cell::Cell::new(0);
    let generate_id = GenerateId(Rc::new(move || {
        n.set(n.get() + 1);
        format!("id{}", n.get())
    }));
    let host = define_extension(LexicalExtension::<(), (), ()> {
        dependencies: vec![
            lexical::rich_text::rich_text_extension().into(),
            lexical::list::check_list_extension().into(),
            lexical::link::link_extension().into(),
            lexical::code::code_extension().into(),
            config_extension(
                &block_id_extension(),
                [BlockIdConfigOverrides {
                    generate_id: Some(generate_id),
                    ..Default::default()
                }],
            ),
            config_extension(&block_id_html_extension(), [html]),
        ],
        nodes: document_nodes(),
        ..LexicalExtension::new("[conformance]", ())
    });
    lexical::extension::build_editor_from_extensions([host.into()]).map_err(|e| e.to_string())
}

/// `html/block-id`: `BlockIdHTMLExtension`'s export of a document (its
/// positional ids, or handles) and the editor state after DOMImportExtension
/// imports HTML carrying ids into the root.
pub fn check_block_id(case: &Value, _file: &Value) -> Result<Outcome, String> {
    use lexical::html::import::generate_nodes_from_dom_via_extension;
    let built = block_id_editor(case.str("scenario")?)?;
    let editor = (*built).clone();
    let result = (|| -> Result<(), String> {
        let discrete = UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        };
        if let Some(json) = case.get("editorState").and_then(Value::as_str) {
            let state = editor
                .parse_editor_state(json)
                .map_err(|e| format!("editorState: {e}"))?;
            editor
                .set_editor_state(&state, None)
                .map_err(|e| e.to_string())?;
        } else {
            // Edited first, so that the import is not a load.
            let merge = UpdateOptions {
                tags: vec![lexical::HISTORY_MERGE_TAG.to_owned()],
                ..discrete.clone()
            };
            editor
                .update(
                    || lexical::get_root().append(&[lexical::create_paragraph_node()]),
                    merge.clone(),
                )
                .and_then(|()| editor.update(|| lexical::get_root().clear(), merge))
                .map_err(|e| e.to_string())?;
        }
        let Some(import_html) = case.get("importHtml").and_then(Value::as_str) else {
            let html = editor
                .read(|| generate_html_from_nodes(&editor, None))
                .and_then(|html| html)
                .map_err(|e| e.to_string())?;
            return same("html", html.as_str(), case.str("html")?);
        };
        let dom = parse_from_string(import_html);
        editor
            .update(
                move || {
                    let nodes = generate_nodes_from_dom_via_extension(&dom, None)?
                        .into_iter()
                        .map(|node| {
                            if node.is_inline() {
                                let paragraph = lexical::create_paragraph_node();
                                paragraph.append(&[node])?;
                                Ok(paragraph)
                            } else {
                                Ok(node)
                            }
                        })
                        .collect::<Result<Vec<_>, lexical::LexicalError>>()?;
                    lexical::get_root().append(&nodes)
                },
                discrete,
            )
            .map_err(|e| e.to_string())?;
        same(
            "state",
            editor
                .get_editor_state()
                .to_json(false)
                .to_string()
                .as_str(),
            case.str("state")?,
        )
    })();
    built.dispose();
    result?;
    Ok(Outcome::Pass)
}

/// The editor of an `html/mark-state` case: `RichTextExtension`,
/// `LinkExtension` and the scenario's mark flavors (`MARK_STATE_SCENARIOS`).
fn mark_state_editor(
    scenario: &str,
) -> Result<lexical::extension::LexicalEditorWithDispose, String> {
    use lexical::mark::{MarkStateConfigOverrides, mark_state_extension};
    use lexical::{
        AnyLexicalExtensionArgument, LexicalExtension, config_extension, define_extension,
    };
    let export = || MarkStateConfigOverrides {
        export_ids_to_html: Some(true),
        ..Default::default()
    };
    let flavor = |key: &str| {
        crate::scripts::mark_flavor(key).ok_or_else(|| format!("unknown mark flavor {key}"))
    };
    let flavors: Vec<AnyLexicalExtensionArgument> = match scenario {
        "default" => vec![mark_state_extension().into()],
        "export" => vec![config_extension(&mark_state_extension(), [export()])],
        "flavors" => vec![
            config_extension(&mark_state_extension(), [export()]),
            config_extension(&flavor("CommentMarks")?, [export()]),
            flavor("SuggestionMarks")?.into(),
        ],
        other => return Err(format!("unknown scenario {other}")),
    };
    let host = define_extension(LexicalExtension::<(), (), ()> {
        dependencies: [
            vec![
                lexical::rich_text::rich_text_extension().into(),
                lexical::link::link_extension().into(),
            ],
            flavors,
        ]
        .concat(),
        ..LexicalExtension::new("[conformance]", ())
    });
    lexical::extension::build_editor_from_extensions([host.into()]).map_err(|e| e.to_string())
}

/// `html/mark-state`: the HTML export of a document's mark ids
/// (`exportIDsToHTML`), and the nodes the ids attribute's import rule makes.
pub fn check_mark_state(case: &Value, _file: &Value) -> Result<Outcome, String> {
    use lexical::html::import::generate_nodes_from_dom_via_extension;
    let built = mark_state_editor(case.str("scenario")?)?;
    let editor = (*built).clone();
    let result = (|| -> Result<(), String> {
        let Some(import_html) = case.get("importHtml").and_then(Value::as_str) else {
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
            return same("html", html.as_str(), case.str("html")?);
        };
        let dom = parse_from_string(import_html);
        let nodes = Rc::new(RefCell::new(String::new()));
        let out = nodes.clone();
        editor
            .update(
                move || {
                    let nodes = generate_nodes_from_dom_via_extension(&dom, None)?;
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
        let nodes = nodes.borrow().clone();
        same("nodes", nodes.as_str(), case.str("nodes")?)
    })();
    built.dispose();
    result?;
    Ok(Outcome::Pass)
}
