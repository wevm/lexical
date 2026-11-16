//! Ports of
//! `packages/lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts`.

#![cfg(feature = "html")]

use crate::clipboard::{
    ClipboardImportConfigOverrides, ImportMimeTypeConfig, ImportMimeTypeFunction,
    ImportMimeTypePriority, clipboard_import_extension, insert_data_transfer_for_rich_text,
    insert_generated_nodes,
};
use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::html::import::{
    GenerateNodesFromDomOptions, ImportSourceKind, context_value, core_import_extension,
    generate_nodes_from_dom_via_extension, import_source,
};
use crate::upstream_tests::support::update;
use crate::{
    AnyLexicalExtensionArgument, DataTransfer, Editor, InitialEditorStateType, LexicalExtension,
    PointType, SelectionRef, Which, config_extension, create_paragraph_node,
    create_range_selection, create_text_node, define_extension, get_root, get_selection,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// `$initialEditorState`: an empty paragraph, selected.
fn initial_editor_state() -> InitialEditorStateType {
    InitialEditorStateType::Update(Rc::new(|_| {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.select(None, None)?;
        Ok(())
    }))
}

fn build(
    initial_editor_state: InitialEditorStateType,
    dependencies: Vec<AnyLexicalExtensionArgument>,
) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(initial_editor_state),
        dependencies,
        ..LexicalExtension::<(), (), ()>::new("host", ())
    })
    .into()])
    .unwrap()
}

fn configured(
    import_mime_type: ImportMimeTypeConfig,
    priority: Option<ImportMimeTypePriority>,
) -> AnyLexicalExtensionArgument {
    config_extension(
        &clipboard_import_extension(),
        [ClipboardImportConfigOverrides {
            import_mime_type: Some(import_mime_type),
            priority,
        }],
    )
}

fn data_transfer_with(mime_type: &str, data: &str) -> DataTransfer {
    let dt = DataTransfer::new();
    dt.set_data(mime_type, data);
    dt
}

/// `$insertDataTransferForRichText(dt, $getSelection(), editor)` in a
/// discrete update.
fn paste(editor: &Editor, dt: DataTransfer) {
    let ed = editor.clone();
    update(editor, move || {
        let selection = get_selection()
            .filter(SelectionRef::is_range)
            .expect("expected RangeSelection");
        insert_data_transfer_for_rich_text(&dt, &selection, &ed)
    });
}

/// `$pasteHtml(editor, html)`.
fn paste_html(editor: &Editor, html: &str) {
    paste(editor, data_transfer_with("text/html", html));
}

/// The last child of the root, which must be a paragraph, and its text.
fn last_paragraph_text(editor: &Editor) -> String {
    editor
        .read(|| {
            let last_child = get_root().get_last_child().unwrap();
            assert_eq!(last_child.get_type(), "paragraph", "expected paragraph");
            last_child.get_text_content()
        })
        .unwrap()
}

/// A paragraph of `text`, inserted at `selection` (`$insertGeneratedNodes`).
fn insert_paragraph_of(text: &str, selection: &SelectionRef) -> Result<(), crate::LexicalError> {
    let p = create_paragraph_node();
    p.append(&[create_text_node(text)])?;
    let editor = crate::updates::get_active_editor().expect("an active editor");
    insert_generated_nodes(&editor, vec![p], selection)
}

// upstream: lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > default importer handles a basic <p> paste (no extension configured)
#[test]
fn default_importer_handles_a_basic_p_paste() {
    let editor = build(initial_editor_state(), vec![]);
    paste_html(&editor, "<p>hello</p>");
    assert_eq!(last_paragraph_text(&editor), "hello");
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > a registered text/html handler runs before the default and can stop the chain
#[test]
fn a_registered_text_html_handler_runs_before_the_default_and_can_stop_the_chain() {
    let called = Rc::new(Cell::new(0));
    let count = called.clone();
    let handler: ImportMimeTypeFunction = Rc::new(move |_html, selection, _next, _dt| {
        count.set(count.get() + 1);
        insert_paragraph_of("[custom]", selection)?;
        Ok(true)
    });
    let editor = build(
        initial_editor_state(),
        vec![configured(
            vec![("text/html".to_owned(), vec![handler])],
            None,
        )],
    );
    paste_html(&editor, "<p>ignored</p>");
    assert_eq!(last_paragraph_text(&editor), "[custom]");
    assert_eq!(called.get(), 1);
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > handler can call next() to defer to the default
#[test]
fn handler_can_call_next_to_defer_to_the_default() {
    let deferred = Rc::new(Cell::new(false));
    let flag = deferred.clone();
    let handler: ImportMimeTypeFunction = Rc::new(move |_html, _selection, next, _dt| {
        flag.set(true);
        next(None, None)
    });
    let editor = build(
        initial_editor_state(),
        vec![configured(
            vec![("text/html".to_owned(), vec![handler])],
            None,
        )],
    );
    paste_html(&editor, "<p>hello</p>");
    assert_eq!(last_paragraph_text(&editor), "hello");
    assert!(deferred.get());
    editor.dispose();
}

/// One case of upstream's `test.each`: `forwards $name through the
/// remaining handlers via next()`.
fn forwards_through_the_remaining_handlers(
    replacement_data: &'static str,
    replace_selection: bool,
) {
    let dt = Rc::new(data_transfer_with("text/plain", "original"));
    type Seen = (String, SelectionRef, *const DataTransfer, String);
    let seen: Rc<RefCell<Vec<Seen>>> = Rc::default();
    let replacement_selection: Rc<RefCell<Option<SelectionRef>>> = Rc::default();
    let record = |seen: &Rc<RefCell<Vec<Seen>>>| {
        let seen = seen.clone();
        move |data: &str, selection: &SelectionRef, dt: &DataTransfer| {
            seen.borrow_mut().push((
                data.to_owned(),
                selection.clone(),
                std::ptr::from_ref(dt),
                dt.get_data("text/plain"),
            ));
        }
    };
    let (first, second) = (record(&seen), record(&seen));
    let replacement = replacement_selection.clone();
    let handlers: Vec<ImportMimeTypeFunction> = vec![
        Rc::new(move |data, selection, next, dt| {
            first(data, selection, dt);
            next(Some(data), Some(selection))
        }),
        Rc::new(move |data, selection, next, dt| {
            second(data, selection, dt);
            next(None, None)
        }),
        Rc::new(move |_data, _selection, next, _dt| {
            if replace_selection {
                next(Some(replacement_data), replacement.borrow().as_ref())
            } else {
                next(Some(replacement_data), None)
            }
        }),
    ];
    let editor = build(
        InitialEditorStateType::Update(Rc::new(|_| {
            let first = create_text_node("first");
            let p1 = create_paragraph_node();
            p1.append(std::slice::from_ref(&first))?;
            let p2 = create_paragraph_node();
            p2.append(&[create_text_node("second")])?;
            get_root().append(&[p1, p2])?;
            first.select(None, None)?;
            Ok(())
        })),
        vec![configured(vec![("text/plain".to_owned(), handlers)], None)],
    );
    let ed = (*editor).clone();
    let (dt_in, seen_in, replacement) = (dt.clone(), seen.clone(), replacement_selection.clone());
    update(&editor, move || {
        let original_selection = get_selection()
            .filter(SelectionRef::is_range)
            .expect("a RangeSelection");
        let second = get_root().get_last_child_or_throw()?;
        let selection = create_range_selection();
        selection.point_set(Which::Anchor, second.get_key(), 0, PointType::Element)?;
        selection.point_set(Which::Focus, second.get_key(), 1, PointType::Element)?;
        *replacement.borrow_mut() = Some(selection.clone());

        insert_data_transfer_for_rich_text(&dt_in, &original_selection, &ed)?;

        let seen = seen_in.borrow();
        assert_eq!(seen.len(), 2);
        for (data, forwarded_selection, data_transfer, plain) in seen.iter() {
            assert_eq!(data, replacement_data);
            let expected = if replace_selection {
                &selection
            } else {
                &original_selection
            };
            assert!(forwarded_selection.ptr_eq(expected));
            assert!(std::ptr::eq(*data_transfer, Rc::as_ptr(&dt_in)));
            assert_eq!(plain, "original");
        }
        Ok(())
    });
    let texts = editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .map(|n| n.get_text_content())
                .collect::<Vec<_>>()
        })
        .unwrap();
    let expected = if replace_selection {
        vec!["first".to_owned(), replacement_data.to_owned()]
    } else {
        vec![format!("first{replacement_data}"), "second".to_owned()]
    };
    assert_eq!(texts, expected);
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > forwards replacement data through the remaining handlers via next()
#[test]
fn forwards_replacement_data_through_the_remaining_handlers() {
    forwards_through_the_remaining_handlers("replacement", false);
}

// upstream: lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > forwards empty replacement data through the remaining handlers via next()
#[test]
fn forwards_empty_replacement_data_through_the_remaining_handlers() {
    forwards_through_the_remaining_handlers("", false);
}

// upstream: lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > forwards replacement selection through the remaining handlers via next()
#[test]
fn forwards_replacement_selection_through_the_remaining_handlers() {
    forwards_through_the_remaining_handlers("original", true);
}

// upstream: lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > forwards both replacements through the remaining handlers via next()
#[test]
fn forwards_both_replacements_through_the_remaining_handlers() {
    forwards_through_the_remaining_handlers("replacement", true);
}

// upstream: lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > app-defined MIME type is reached when added to both stack and priority
#[test]
fn app_defined_mime_type_is_reached_when_added_to_both_stack_and_priority() {
    let saw = Rc::new(RefCell::new(String::new()));
    let out = saw.clone();
    let handler: ImportMimeTypeFunction = Rc::new(move |data, selection, _next, _dt| {
        *out.borrow_mut() = data.to_owned();
        insert_paragraph_of(&format!("[{data}]"), selection)?;
        Ok(true)
    });
    let editor = build(
        initial_editor_state(),
        vec![configured(
            vec![("application/vnd.myapp+json".to_owned(), vec![handler])],
            // Between lexical-editor (0) and text/html (10).
            Some(vec![("application/vnd.myapp+json".to_owned(), 5.0)]),
        )],
    );
    let dt = data_transfer_with("text/html", "<p>html-fallback</p>");
    dt.set_data("application/vnd.myapp+json", r#"{"a":1}"#);
    paste(&editor, dt);
    assert_eq!(*saw.borrow(), r#"{"a":1}"#);
    assert_eq!(last_paragraph_text(&editor), r#"[{"a":1}]"#);
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > priority weights compose without coordination between extensions
#[test]
fn priority_weights_compose_without_coordination_between_extensions() {
    let my_app_calls = Rc::new(Cell::new(0));
    let html_calls = Rc::new(Cell::new(0));
    let (my_app, html) = (my_app_calls.clone(), html_calls.clone());
    let editor = build(
        initial_editor_state(),
        vec![configured(
            vec![
                (
                    "application/vnd.myapp+json".to_owned(),
                    vec![Rc::new(move |_, _, _, _| {
                        my_app.set(my_app.get() + 1);
                        Ok(true)
                    })],
                ),
                (
                    "text/html".to_owned(),
                    vec![Rc::new(move |_, _, next, _| {
                        html.set(html.get() + 1);
                        next(None, None)
                    })],
                ),
            ],
            // myapp gets weight 1 → runs ahead of html (default weight 10).
            Some(vec![("application/vnd.myapp+json".to_owned(), 1.0)]),
        )],
    );
    let dt = data_transfer_with("text/html", "<p>x</p>");
    dt.set_data("application/vnd.myapp+json", "{}");
    paste(&editor, dt);
    assert_eq!(my_app_calls.get(), 1);
    assert_eq!(html_calls.get(), 0);
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::ClipboardImportExtension > text/html can be routed through DOMImportExtension
#[test]
fn text_html_can_be_routed_through_dom_import_extension() {
    let handler: ImportMimeTypeFunction = Rc::new(|html, selection, _next, _dt| {
        let dom = crate::dom::parse_from_string(html);
        let nodes = generate_nodes_from_dom_via_extension(
            &dom,
            Some(&GenerateNodesFromDomOptions {
                context: Some(vec![context_value(
                    &import_source(),
                    ImportSourceKind::Paste,
                )]),
                ..GenerateNodesFromDomOptions::default()
            }),
        )?;
        let editor = crate::updates::get_active_editor().expect("an active editor");
        insert_generated_nodes(&editor, nodes, selection)?;
        Ok(true)
    });
    let editor = build(
        initial_editor_state(),
        vec![
            core_import_extension().into(),
            configured(vec![("text/html".to_owned(), vec![handler])], None),
        ],
    );
    paste_html(&editor, "<p>via <strong>new</strong> pipeline</p>");
    assert_eq!(last_paragraph_text(&editor), "via new pipeline");
    editor.dispose();
}

/// `makeEditor()`: two paragraphs, the editor's selection at the end of
/// the second.
fn make_editor() -> LexicalEditorWithDispose {
    build(
        InitialEditorStateType::Update(Rc::new(|_| {
            let second = create_text_node("second");
            let p1 = create_paragraph_node();
            p1.append(&[create_text_node("first")])?;
            let p2 = create_paragraph_node();
            p2.append(std::slice::from_ref(&second))?;
            let root = get_root();
            root.clear()?;
            root.append(&[p1, p2])?;
            second.select(None, None)?;
            Ok(())
        })),
        vec![],
    )
}

/// `$insertOverFirstParagraph(editor, dataTransfer)`.
fn insert_over_first_paragraph(editor: &Editor, dt: DataTransfer) {
    let ed = editor.clone();
    update(editor, move || {
        let first_paragraph = get_root().get_first_child_or_throw()?;
        let selection = create_range_selection();
        selection.point_set(
            Which::Anchor,
            first_paragraph.get_key(),
            0,
            PointType::Element,
        )?;
        selection.point_set(
            Which::Focus,
            first_paragraph.get_key(),
            1,
            PointType::Element,
        )?;
        insert_data_transfer_for_rich_text(&dt, &selection, &ed)
    });
}

/// `paragraphTexts(editor)`.
fn paragraph_texts(editor: &Editor) -> Vec<String> {
    editor
        .read(|| {
            get_root()
                .get_children()
                .iter()
                .map(|n| n.get_text_content())
                .collect()
        })
        .unwrap()
}

// upstream: lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::$insertDataTransferForRichText selection argument (#6278) > text/plain is inserted at the supplied selection
#[test]
fn text_plain_is_inserted_at_the_supplied_selection() {
    let editor = make_editor();
    insert_over_first_paragraph(&editor, data_transfer_with("text/plain", "replacement"));
    assert_eq!(paragraph_texts(&editor), ["replacement", "second"]);
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::$insertDataTransferForRichText selection argument (#6278) > multi-line text/plain is inserted at the supplied selection
#[test]
fn multi_line_text_plain_is_inserted_at_the_supplied_selection() {
    let editor = make_editor();
    insert_over_first_paragraph(&editor, data_transfer_with("text/plain", "one\ntwo"));
    assert_eq!(paragraph_texts(&editor), ["one", "two", "second"]);
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::$insertDataTransferForRichText selection argument (#6278) > text/uri-list is inserted at the supplied selection
#[test]
fn text_uri_list_is_inserted_at_the_supplied_selection() {
    let editor = make_editor();
    insert_over_first_paragraph(
        &editor,
        data_transfer_with("text/uri-list", "https://lexical.dev"),
    );
    assert_eq!(paragraph_texts(&editor), ["https://lexical.dev", "second"]);
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::$insertDataTransferForRichText selection argument (#6278) > text/html is inserted at the supplied selection
#[test]
fn text_html_is_inserted_at_the_supplied_selection() {
    let editor = make_editor();
    insert_over_first_paragraph(
        &editor,
        data_transfer_with("text/html", "<p>replacement</p>"),
    );
    assert_eq!(paragraph_texts(&editor), ["replacement", "second"]);
    editor.dispose();
}

// upstream: lexical-clipboard/src/__tests__/unit/ClipboardImportExtension.test.ts::$insertDataTransferForRichText selection argument (#6278) > text/plain still lands at the caret on the ordinary paste path
#[test]
fn text_plain_still_lands_at_the_caret_on_the_ordinary_paste_path() {
    let editor = make_editor();
    paste(&editor, data_transfer_with("text/plain", "!"));
    assert_eq!(paragraph_texts(&editor), ["first", "second!"]);
    editor.dispose();
}
