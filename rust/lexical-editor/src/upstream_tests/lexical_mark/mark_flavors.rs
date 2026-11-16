//! Ports of `packages/lexical-mark/src/__tests__/unit/MarkFlavors.test.ts`.
//!
//! The tests that read the classes and attributes the renderer writes on a
//! mounted editor's elements, and the type-only tests, are skipped
//! (rust/parity/skips.json).

use crate::clipboard::{
    generate_json_from_selected_nodes, generate_nodes_from_serialized_nodes, insert_generated_nodes,
};
use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, get_extension_dependency_from_editor,
};
use crate::history::{HistoryConfigOverrides, history_extension};
use crate::json::Value;
use crate::mark::{
    MARK_STATE_IDS_ATTRIBUTE, MARK_STATE_KEY, MarkFlavorDefinition, MarkFlavorExtension,
    MarkIDsChange, MarkIDsMode, MarkStateConfigOverrides, MarkStateOutput, add_mark_id,
    add_mark_id_for, create_mark_node, define_mark_extension, get_mark_ids_at_selection,
    get_mark_ids_at_selection_for, get_mark_ids_by_flavor, get_node_mark_ids,
    get_node_mark_ids_for, mark_base_extension, mark_extension, mark_flavor, mark_state_extension,
    migrate_mark_nodes_to_mark_state_for, register_mark_ids_listener, remove_mark_id,
    remove_mark_id_for,
};
use crate::upstream_tests::support::{json_to_equal, panic_message};
use crate::{
    AnyLexicalExtensionArgument, CLEAR_HISTORY_COMMAND, COLLABORATION_TAG,
    CONTROLLED_TEXT_INSERTION_COMMAND, Editor, HISTORIC_TAG, InitialEditorStateType, LexicalError,
    LexicalExtension, NodeRef, PointType, SelectionRef, UNDO_COMMAND, UpdateOptions, Which,
    add_update_tag, config_extension, create_paragraph_node, create_range_selection,
    create_text_node, define_extension, get_root, get_selection, set_selection,
};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

thread_local! {
    static COMMENT_MARKS: MarkFlavorExtension = define_mark_extension(&MarkFlavorDefinition {
        class_name: Some("comment".to_owned()),
        name: "@test/CommentMarks".to_owned(),
        overlap_class_name: Some("comment-overlap".to_owned()),
        state_key: "commentIds".to_owned(),
        ..MarkFlavorDefinition::default()
    })
    .unwrap();
    static SUGGESTION_MARKS: MarkFlavorExtension = define_mark_extension(&MarkFlavorDefinition {
        class_name: Some("suggestion".to_owned()),
        name: "@test/SuggestionMarks".to_owned(),
        state_key: "suggestionIds".to_owned(),
        ..MarkFlavorDefinition::default()
    })
    .unwrap();
}

/// `CommentMarks`.
fn comment_marks() -> MarkFlavorExtension {
    COMMENT_MARKS.with(Rc::clone)
}

/// `SuggestionMarks`.
fn suggestion_marks() -> MarkFlavorExtension {
    SUGGESTION_MARKS.with(Rc::clone)
}

const TEXT: &str = "Ship the migration on Friday";

/// `createEditor(dependencies)`, without a root element.
fn try_create_editor(
    dependencies: Vec<AnyLexicalExtensionArgument>,
) -> Result<LexicalEditorWithDispose, LexicalError> {
    let mut all: Vec<AnyLexicalExtensionArgument> = vec![
        crate::rich_text::rich_text_extension().into(),
        crate::html::import::core_import_extension().into(),
        config_extension(
            &history_extension(),
            [HistoryConfigOverrides {
                delay: Some(0.0),
                ..Default::default()
            }],
        ),
    ];
    all.extend(dependencies);
    let theme = crate::json::parse(r#"{"mark":"mark","markOverlap":"mark-overlap"}"#)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    build_editor_from_extensions([define_extension(LexicalExtension {
        initial_editor_state: Some(InitialEditorStateType::Null),
        dependencies: all,
        theme: Some(theme),
        ..LexicalExtension::<(), (), ()>::new("[root]", ())
    })
    .into()])
}

fn create_editor(dependencies: Vec<AnyLexicalExtensionArgument>) -> LexicalEditorWithDispose {
    try_create_editor(dependencies).unwrap()
}

/// `createEditor()`: with `CommentMarks` and `SuggestionMarks`.
fn create_default_editor() -> LexicalEditorWithDispose {
    create_editor(vec![comment_marks().into(), suggestion_marks().into()])
}

fn update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    editor
        .update(
            f,
            UpdateOptions {
                discrete: true,
                ..UpdateOptions::default()
            },
        )
        .unwrap();
}

/// `$init()`.
fn init() -> Result<(), LexicalError> {
    let root = get_root();
    root.clear()?;
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node(TEXT)])?;
    root.append(&[paragraph])
}

/// `$select(start, end)`: a selection of [start, end) of the paragraph's
/// text, made the editor's selection inside an update.
fn select(start: usize, end: usize) -> SelectionRef {
    let texts = get_root().get_all_text_nodes();
    let point = |offset: usize, prefer_next: bool| {
        let mut acc = 0;
        for (i, text) in texts.iter().enumerate() {
            let size = text.get_text_content_size();
            if offset < acc + size
                || (offset == acc + size && (!prefer_next || i == texts.len() - 1))
            {
                return (text.clone(), offset - acc);
            }
            acc += size;
        }
        panic!("offset {offset} out of range");
    };
    let (a, ao) = point(start, true);
    let (b, bo) = point(end, false);
    let selection = create_range_selection();
    selection
        .point_set(Which::Anchor, a.get_key(), ao, PointType::Text)
        .unwrap();
    selection
        .point_set(Which::Focus, b.get_key(), bo, PointType::Text)
        .unwrap();
    if !crate::updates::is_currently_read_only_mode() {
        set_selection(Some(selection.clone()));
    }
    selection
}

/// `runs(editor, flavors)`: the text runs with their ids per flavor:
/// `"Friday"[c:c1 s:s7]`.
fn runs_of(editor: &Editor, flavors: &[MarkFlavorExtension]) -> String {
    let flavors = flavors.to_vec();
    editor
        .read(move || {
            get_root()
                .get_all_text_nodes()
                .iter()
                .map(|node| {
                    let ids: Vec<String> = flavors
                        .iter()
                        .filter_map(|flavor| {
                            let ids = get_node_mark_ids_for(flavor, node).unwrap();
                            (!ids.is_empty()).then(|| {
                                format!("{}:{}", mark_flavor(flavor).state_key(), ids.join(","))
                            })
                        })
                        .collect();
                    let text = Value::from(node.get_text_content().as_str()).to_string();
                    if ids.is_empty() {
                        text
                    } else {
                        format!("{text}[{}]", ids.join(" "))
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap()
}

fn runs(editor: &Editor) -> String {
    runs_of(editor, &[comment_marks(), suggestion_marks()])
}

/// `$textNode(text)`.
fn text_node(text: &str) -> NodeRef {
    get_root()
        .get_all_text_nodes()
        .into_iter()
        .find(|n| n.get_text_content() == text)
        .unwrap_or_else(|| panic!("no TextNode {text:?}"))
}

fn output(editor: &Editor, flavor: &MarkFlavorExtension) -> Rc<MarkStateOutput> {
    get_extension_dependency_from_editor(editor, flavor)
        .unwrap()
        .output
        .unwrap()
}

/// `nodeKeys(editor, flavor)`: each id with the sorted texts of its leaves.
fn node_keys(editor: &Editor, flavor: &MarkFlavorExtension) -> BTreeMap<String, Vec<String>> {
    let map = output(editor, flavor).mark_id_node_keys.value();
    editor
        .read(move || {
            map.iter()
                .map(|(id, keys)| {
                    let texts = get_root().get_all_text_nodes();
                    let mut found: Vec<String> = keys
                        .iter()
                        .map(|key| {
                            texts
                                .iter()
                                .find(|n| n.get_key() == key)
                                .map_or_else(|| "?".to_owned(), NodeRef::get_text_content)
                        })
                        .collect();
                    found.sort();
                    (id.to_owned(), found)
                })
                .collect()
        })
        .unwrap()
}

fn keys_of(entries: &[(&str, &[&str])]) -> BTreeMap<String, Vec<String>> {
    entries
        .iter()
        .map(|(id, texts)| {
            (
                (*id).to_owned(),
                texts.iter().map(|t| (*t).to_owned()).collect(),
            )
        })
        .collect()
}

/// `type(editor, text)`, inside an update.
fn type_text(editor: &Editor, text: &str) -> Result<(), LexicalError> {
    editor.dispatch_command(&CONTROLLED_TEXT_INSERTION_COMMAND, text.to_owned())?;
    Ok(())
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > each flavor sees only its own ids
#[test]
fn each_flavor_sees_only_its_own_ids() {
    let editor = create_default_editor();
    let (comments, suggestions) = (comment_marks(), suggestion_marks());
    update(&editor, init);
    // "migration on Friday" commented, "Friday" with a suggestion
    let c = comments.clone();
    update(&editor, move || {
        add_mark_id_for(&c, "c1", &select(9, 28))?;
        Ok(())
    });
    let s = suggestions.clone();
    update(&editor, move || {
        add_mark_id_for(&s, "s7", &select(22, 28))?;
        Ok(())
    });
    assert_eq!(
        runs(&editor),
        r#""Ship the " "migration on "[commentIds:c1] "Friday"[commentIds:c1 suggestionIds:s7]"#
    );
    let (c, s) = (comments.clone(), suggestions.clone());
    editor
        .read(move || {
            let friday = text_node("Friday");
            assert_eq!(get_node_mark_ids_for(&c, &friday).unwrap(), ["c1"]);
            assert_eq!(get_node_mark_ids_for(&s, &friday).unwrap(), ["s7"]);
            assert_eq!(
                get_mark_ids_at_selection_for(&c, Some(&select(10, 24)), MarkIDsMode::Any).unwrap(),
                ["c1"]
            );
            assert_eq!(
                get_mark_ids_at_selection_for(&s, Some(&select(10, 24)), MarkIDsMode::Any).unwrap(),
                ["s7"]
            );
            assert!(
                get_mark_ids_at_selection_for(&s, Some(&select(10, 24)), MarkIDsMode::All)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                get_mark_ids_by_flavor(&friday).unwrap(),
                [
                    ("commentIds".to_owned(), vec!["c1".to_owned()]),
                    ("suggestionIds".to_owned(), vec!["s7".to_owned()]),
                ]
            );
        })
        .unwrap();
    assert_eq!(
        node_keys(&editor, &comments),
        keys_of(&[("c1", &["Friday", "migration on "])])
    );
    assert_eq!(
        node_keys(&editor, &suggestions),
        keys_of(&[("s7", &["Friday"])])
    );

    // Removing everywhere touches only its own flavor; equal neighbors merge
    let s = suggestions.clone();
    update(&editor, move || remove_mark_id_for(&s, "s7", None));
    assert_eq!(
        runs(&editor),
        r#""Ship the " "migration on Friday"[commentIds:c1]"#
    );
    assert!(node_keys(&editor, &suggestions).is_empty());
    // The same id in two flavors is two different marks
    let s = suggestions.clone();
    update(&editor, move || {
        add_mark_id_for(&s, "c1", &select(0, 4))?;
        Ok(())
    });
    let c = comments.clone();
    update(&editor, move || remove_mark_id_for(&c, "c1", None));
    assert_eq!(
        runs(&editor),
        r#""Ship"[suggestionIds:c1] " the migration on Friday""#
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > typing at the end of both marks gives the text the ids of both flavors by the same rule
// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > typing inside both marks gives the text the ids of both flavors by the same rule
// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > typing at the start of both marks gives the text the ids of both flavors by the same rule
#[test]
fn typing_gives_the_text_the_ids_of_both_flavors_by_the_same_rule() {
    for (_, at, expected) in [
        (
            "at the end of both marks",
            28,
            r#""Ship the migration on " "Friday"[commentIds:c1 suggestionIds:s7] "!""#,
        ),
        (
            "inside both marks",
            25,
            r#""Ship the migration on " "Fri!day"[commentIds:c1 suggestionIds:s7]"#,
        ),
        (
            "at the start of both marks",
            22,
            r#""Ship the migration on !" "Friday"[commentIds:c1 suggestionIds:s7]"#,
        ),
    ] {
        let editor = create_default_editor();
        update(&editor, init);
        update(&editor, || {
            add_mark_id_for(&comment_marks(), "c1", &select(22, 28))?;
            add_mark_id_for(&suggestion_marks(), "s7", &select(22, 28))?;
            Ok(())
        });
        let e = (*editor).clone();
        update(&editor, move || {
            select(at, at);
            type_text(&e, "!")
        });
        assert_eq!(runs(&editor), expected);
    }
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > text typed at the edge of one flavor joins the other where it is inside
#[test]
fn text_typed_at_the_edge_of_one_flavor_joins_the_other_where_it_is_inside() {
    let editor = create_default_editor();
    update(&editor, init);
    update(&editor, || {
        add_mark_id_for(&comment_marks(), "c1", &select(9, 28))?;
        add_mark_id_for(&suggestion_marks(), "s7", &select(22, 28))?;
        Ok(())
    });
    let e = (*editor).clone();
    update(&editor, move || {
        select(22, 22);
        type_text(&e, "next ")
    });
    assert_eq!(
        runs(&editor),
        r#""Ship the " "migration on next "[commentIds:c1] "Friday"[commentIds:c1 suggestionIds:s7]"#
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > text replacing a selection gets the ids all of it had, per flavor
#[test]
fn text_replacing_a_selection_gets_the_ids_all_of_it_had_per_flavor() {
    let editor = create_default_editor();
    update(&editor, init);
    update(&editor, || {
        add_mark_id_for(&comment_marks(), "c1", &select(22, 28))?;
        add_mark_id_for(&suggestion_marks(), "s7", &select(9, 28))?;
        Ok(())
    });
    let e = (*editor).clone();
    update(&editor, move || {
        select(22, 28);
        type_text(&e, "Monday")
    });
    assert_eq!(
        runs(&editor),
        r#""Ship the " "migration on "[suggestionIds:s7] "Monday"[commentIds:c1 suggestionIds:s7]"#
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > two flavors on the same text > pasted text keeps the ids of every flavor
#[test]
fn pasted_text_keeps_the_ids_of_every_flavor() {
    let editor = create_default_editor();
    update(&editor, init);
    update(&editor, || {
        add_mark_id_for(&comment_marks(), "c1", &select(22, 28))?;
        add_mark_id_for(&suggestion_marks(), "s7", &select(25, 28))?;
        Ok(())
    });
    let e = (*editor).clone();
    update(&editor, move || {
        let json = generate_json_from_selected_nodes(&e, Some(&select(22, 28)))?;
        select(0, 0);
        let selection = get_selection()
            .filter(SelectionRef::is_range)
            .expect("no selection");
        let nodes: Vec<Value> = json.nodes.into_iter().map(Value::Object).collect();
        insert_generated_nodes(
            &e,
            generate_nodes_from_serialized_nodes(&nodes)?,
            &selection,
        )
    });
    assert_eq!(
        runs(&editor),
        r#""Fri"[commentIds:c1] "day"[commentIds:c1 suggestionIds:s7] "Ship the migration on " "Fri"[commentIds:c1] "day"[commentIds:c1 suggestionIds:s7]"#
    );
}

type Calls = Rc<RefCell<Vec<String>>>;

/// `record(editor, flavor)`: the listener's calls, as
/// `+[added] -[removed] =[ids]` and the historic and collaboration tags.
fn record(editor: &Editor, flavor: &MarkFlavorExtension) -> (Calls, crate::Unregister) {
    let calls: Calls = Rc::default();
    let heard = calls.clone();
    let unregister = register_mark_ids_listener(editor, flavor, move |change: &MarkIDsChange| {
        let list = |set: &[String]| {
            let mut set = set.to_vec();
            set.sort();
            set.join(",")
        };
        let tags: Vec<&str> = [HISTORIC_TAG, COLLABORATION_TAG]
            .into_iter()
            .filter(|tag| change.tags.iter().any(|t| t == tag))
            .collect();
        let mut call = format!(
            "+[{}] -[{}] =[{}]",
            list(&change.added),
            list(&change.removed),
            list(&change.ids)
        );
        if !tags.is_empty() {
            call.push(' ');
            call.push_str(&tags.join(","));
        }
        heard.borrow_mut().push(call);
    })
    .unwrap();
    (calls, unregister)
}

fn last(calls: &Calls) -> String {
    calls.borrow().last().cloned().unwrap_or_default()
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > reports added, removed (orphaned) and restored ids across undo and paste
#[test]
fn reports_added_removed_and_restored_ids_across_undo_and_paste() {
    let editor = create_default_editor();
    update(&editor, init);
    editor.dispatch_command(&CLEAR_HISTORY_COMMAND, ()).unwrap();
    let (comments, _u1) = record(&editor, &comment_marks());
    let (suggestions, _u2) = record(&editor, &suggestion_marks());
    assert!(comments.borrow().is_empty());

    update(&editor, || {
        add_mark_id_for(&comment_marks(), "c1", &select(22, 28))?;
        Ok(())
    });
    update(&editor, || {
        add_mark_id_for(&comment_marks(), "c2", &select(0, 4))?;
        Ok(())
    });
    assert_eq!(
        *comments.borrow(),
        ["+[c1] -[] =[c1]", "+[c2] -[] =[c1,c2]"]
    );

    // Typing inside a mark changes no ids: nothing to report
    let e = (*editor).clone();
    update(&editor, move || {
        select(25, 25);
        type_text(&e, "x")
    });
    assert_eq!(comments.borrow().len(), 2);

    // Copy the commented text, then delete all of it: c1 is orphaned
    let copied: Rc<RefCell<Vec<Value>>> = Rc::default();
    let (c, e) = (copied.clone(), (*editor).clone());
    update(&editor, move || {
        *c.borrow_mut() = generate_json_from_selected_nodes(&e, Some(&select(22, 29)))?
            .nodes
            .into_iter()
            .map(Value::Object)
            .collect();
        Ok(())
    });
    update(&editor, || select(21, 29).remove_text());
    assert_eq!(last(&comments), "+[] -[c1] =[c2]");

    // Undo brings it back
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    // The undo commits at the next microtask; a read commits it now
    editor.read(|| {}).unwrap();
    assert_eq!(
        last(&comments),
        format!("+[c1] -[] =[c1,c2] {HISTORIC_TAG}")
    );
    assert!(runs(&editor).contains(r#""Frixday"[commentIds:c1]"#));

    // Deleted again, then pasted back
    update(&editor, || select(21, 29).remove_text());
    assert_eq!(last(&comments), "+[] -[c1] =[c2]");
    let e = (*editor).clone();
    update(&editor, move || {
        let selection = select(21, 21);
        insert_generated_nodes(
            &e,
            generate_nodes_from_serialized_nodes(&copied.borrow())?,
            &selection,
        )
    });
    assert_eq!(last(&comments), "+[c1] -[] =[c1,c2]");

    // The other flavor's listener saw nothing
    assert!(suggestions.borrow().is_empty());
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > a collaborator update is tagged
#[test]
fn a_collaborator_update_is_tagged() {
    let editor = create_default_editor();
    update(&editor, init);
    let (calls, _unregister) = record(&editor, &comment_marks());
    update(&editor, || {
        add_update_tag(COLLABORATION_TAG);
        add_mark_id_for(&comment_marks(), "c1", &select(0, 4))?;
        Ok(())
    });
    assert_eq!(
        *calls.borrow(),
        [format!("+[c1] -[] =[c1] {COLLABORATION_TAG}")]
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > the first call lists the ids already in the document, and unregistering stops it
#[test]
fn the_first_call_lists_the_ids_already_in_the_document_and_unregistering_stops_it() {
    let editor = create_default_editor();
    update(&editor, init);
    update(&editor, || {
        add_mark_id_for(&comment_marks(), "c1", &select(0, 4))?;
        add_mark_id_for(&comment_marks(), "c2", &select(5, 8))?;
        Ok(())
    });
    let (calls, unregister) = record(&editor, &comment_marks());
    assert_eq!(*calls.borrow(), ["+[c1,c2] -[] =[c1,c2]"]);
    unregister();
    update(&editor, || remove_mark_id_for(&comment_marks(), "c1", None));
    assert_eq!(calls.borrow().len(), 1);
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > disabling reports nothing, enabling reports the net change
#[test]
fn disabling_reports_nothing_enabling_reports_the_net_change() {
    let editor = create_default_editor();
    update(&editor, init);
    update(&editor, || {
        add_mark_id_for(&comment_marks(), "c1", &select(0, 4))?;
        Ok(())
    });
    let (calls, _unregister) = record(&editor, &comment_marks());
    let disabled = output(&editor, &comment_marks()).disabled.clone();
    disabled.set_value(true);
    update(&editor, || {
        remove_mark_id_for(&comment_marks(), "c1", None)?;
        add_mark_id_for(&comment_marks(), "c3", &select(5, 8))?;
        Ok(())
    });
    assert_eq!(*calls.borrow(), ["+[c1] -[] =[c1]"]);
    disabled.set_value(false);
    assert_eq!(*calls.borrow(), ["+[c1] -[] =[c1]", "+[c3] -[c1] =[c3]"]);
    // Disabled, and back with no net change: nothing
    disabled.set_value(true);
    disabled.set_value(false);
    assert_eq!(calls.borrow().len(), 2);
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > a listener registered while disabled gets every id when enabled
#[test]
fn a_listener_registered_while_disabled_gets_every_id_when_enabled() {
    let editor = create_default_editor();
    update(&editor, init);
    update(&editor, || {
        add_mark_id_for(&comment_marks(), "c1", &select(0, 4))?;
        Ok(())
    });
    let (early, _u1) = record(&editor, &comment_marks());
    let disabled = output(&editor, &comment_marks()).disabled.clone();
    disabled.set_value(true);
    let (late, _u2) = record(&editor, &comment_marks());
    assert!(late.borrow().is_empty());
    disabled.set_value(false);
    assert_eq!(*late.borrow(), ["+[c1] -[] =[c1]"]);
    // The listener registered before has nothing new to hear
    assert_eq!(*early.borrow(), ["+[c1] -[] =[c1]"]);
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > a listener that throws on its first call is not registered
#[test]
fn a_listener_that_throws_on_its_first_call_is_not_registered() {
    let editor = create_default_editor();
    update(&editor, init);
    update(&editor, || {
        add_mark_id_for(&comment_marks(), "c1", &select(0, 4))?;
        Ok(())
    });
    // A Rust listener can't return an error; JS's throw is a panic.
    let calls = Rc::new(Cell::new(0));
    let heard = calls.clone();
    let thrown = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = register_mark_ids_listener(&editor, &comment_marks(), move |_| {
            heard.set(heard.get() + 1);
            panic!("listener failed");
        });
    }))
    .unwrap_err();
    assert!(panic_message(&*thrown).contains("listener failed"));
    update(&editor, || remove_mark_id_for(&comment_marks(), "c1", None));
    assert_eq!(calls.get(), 1);
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > a flavor disabled by another flavor listener in the same commit reports nothing
#[test]
fn a_flavor_disabled_by_another_flavor_listener_in_the_same_commit_reports_nothing() {
    let editor = create_default_editor();
    update(&editor, init);
    let suggestions_output = output(&editor, &suggestion_marks());
    let disabled = suggestions_output.disabled.clone();
    let d = disabled.clone();
    let _u1 = register_mark_ids_listener(&editor, &comment_marks(), move |_| {
        d.set_value(true);
    })
    .unwrap();
    let (suggestions, _u2) = record(&editor, &suggestion_marks());
    update(&editor, || {
        add_mark_id_for(&comment_marks(), "c1", &select(0, 4))?;
        add_mark_id_for(&suggestion_marks(), "s1", &select(0, 4))?;
        Ok(())
    });
    assert!(disabled.value());
    assert!(suggestions.borrow().is_empty());
    assert_eq!(suggestions_output.mark_id_node_keys.value().len(), 0);
    disabled.set_value(false);
    assert_eq!(*suggestions.borrow(), ["+[s1] -[] =[s1]"]);
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > registerMarkIDsListener > an editor without the flavor throws, naming the fix
#[test]
fn an_editor_without_the_flavor_throws_naming_the_fix() {
    let editor = create_editor(vec![comment_marks().into()]);
    let error = register_mark_ids_listener(&editor, &suggestion_marks(), |_| {}).err();
    assert_eq!(
        error.unwrap().message(),
        "registerMarkIDsListener: the editor was not built with @test/SuggestionMarks. Add it to the dependencies of the extension the editor is built from."
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > the default flavor > MarkStateExtension is the flavor on markIds
#[test]
fn mark_state_extension_is_the_flavor_on_mark_ids() {
    let default = mark_state_extension();
    assert_eq!(mark_flavor(&default).state_key(), MARK_STATE_KEY);
    assert_eq!(mark_flavor(&default).attribute(), MARK_STATE_IDS_ATTRIBUTE);
    assert_eq!(default.name, "@lexical/mark/MarkState");
    assert_eq!(
        mark_flavor(&comment_marks()).attribute(),
        "data-lexical-comment-ids"
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > the default flavor > the functions without a flavor act on MarkStateExtension, beside other flavors
#[test]
fn the_functions_without_a_flavor_act_on_mark_state_extension_beside_other_flavors() {
    let editor = create_editor(vec![mark_state_extension().into(), comment_marks().into()]);
    update(&editor, init);
    update(&editor, || {
        add_mark_id("m1", &select(0, 4))?;
        add_mark_id_for(&mark_state_extension(), "m2", &select(0, 4))?;
        add_mark_id_for(&comment_marks(), "c1", &select(0, 4))?;
        Ok(())
    });
    editor
        .read(|| {
            let ship = text_node("Ship");
            assert_eq!(get_node_mark_ids(&ship).unwrap(), ["m1", "m2"]);
            assert_eq!(
                get_node_mark_ids_for(&mark_state_extension(), &ship).unwrap(),
                ["m1", "m2"]
            );
            assert_eq!(
                get_mark_ids_at_selection(Some(&select(1, 1)), MarkIDsMode::Any).unwrap(),
                ["m1", "m2"]
            );
            assert_eq!(
                get_mark_ids_at_selection_for(
                    &comment_marks(),
                    Some(&select(1, 1)),
                    MarkIDsMode::Any
                )
                .unwrap(),
                ["c1"]
            );
            let json = crate::node_state::export_json_with_state(&*ship.get_latest(), false);
            assert!(json_to_equal(
                json.get("$").unwrap(),
                &crate::json::parse(r#"{"markIds":["m1","m2"],"commentIds":["c1"]}"#).unwrap()
            ));
        })
        .unwrap();
    update(&editor, || remove_mark_id("m1", None));
    assert_eq!(
        runs_of(&editor, &[mark_state_extension(), comment_marks()]),
        r#""Ship"[markIds:m2 commentIds:c1] " the migration on Friday""#
    );
    let flavors = get_extension_dependency_from_editor(&editor, &mark_base_extension())
        .unwrap()
        .output
        .unwrap()
        .flavors();
    assert_eq!(
        flavors.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
        ["markIds", "commentIds"]
    );
    assert!(Rc::ptr_eq(&flavors[1].1, &comment_marks()));
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > the default flavor > $migrateMarkNodesToMarkState can target a flavor
#[test]
fn migrate_mark_nodes_to_mark_state_can_target_a_flavor() {
    let editor = create_editor(vec![mark_extension().into(), comment_marks().into()]);
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        let mark = create_mark_node(&["t1".to_owned()]);
        mark.append(&[create_text_node("migration")])?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_text_node("Ship the "), mark])?;
        root.append(&[paragraph])
    });
    update(&editor, || {
        assert_eq!(
            migrate_mark_nodes_to_mark_state_for(&comment_marks(), None)?,
            1
        );
        Ok(())
    });
    assert_eq!(
        runs_of(&editor, &[mark_state_extension(), comment_marks()]),
        r#""Ship the " "migration"[commentIds:t1]"#
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > serialization > JSON round trips the ids of every flavor
#[test]
fn json_round_trips_the_ids_of_every_flavor() {
    let editor = create_default_editor();
    update(&editor, init);
    update(&editor, || {
        add_mark_id_for(&comment_marks(), "c1", &select(9, 28))?;
        add_mark_id_for(&suggestion_marks(), "s7", &select(22, 28))?;
        Ok(())
    });
    let json = editor.get_editor_state().to_json(false);
    let friday = json
        .get("root")
        .and_then(|r| r.get("children"))
        .and_then(|c| c.as_array())
        .and_then(|c| c[0].get("children"))
        .and_then(|c| c.as_array())
        .map(|c| c[2].clone())
        .unwrap();
    assert!(json_to_equal(
        friday.get("$").unwrap(),
        &crate::json::parse(r#"{"commentIds":["c1"],"suggestionIds":["s7"]}"#).unwrap()
    ));

    let target = create_default_editor();
    target
        .set_editor_state(&target.parse_editor_state(&json.to_string()).unwrap(), None)
        .unwrap();
    assert_eq!(runs(&target), runs(&editor));
    assert_eq!(
        node_keys(&target, &comment_marks()),
        node_keys(&editor, &comment_marks())
    );
    assert!(json_to_equal(
        &target.get_editor_state().to_json(false),
        &json
    ));
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > serialization > ids of a flavor the editor lacks are kept as unknown state
#[test]
fn ids_of_a_flavor_the_editor_lacks_are_kept_as_unknown_state() {
    let editor = create_default_editor();
    update(&editor, init);
    update(&editor, || {
        add_mark_id_for(&suggestion_marks(), "s7", &select(22, 28))?;
        Ok(())
    });
    let json = editor.get_editor_state().to_json(false).to_string();
    let target = create_editor(vec![comment_marks().into()]);
    target
        .set_editor_state(&target.parse_editor_state(&json).unwrap(), None)
        .unwrap();
    assert_eq!(target.get_editor_state().to_json(false).to_string(), json);
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > serialization > HTML round trips each flavor through its own attribute
#[test]
fn html_round_trips_each_flavor_through_its_own_attribute() {
    let editor = create_editor(vec![
        config_extension(
            &comment_marks(),
            [MarkStateConfigOverrides {
                export_ids_to_html: Some(true),
                ..MarkStateConfigOverrides::default()
            }],
        ),
        suggestion_marks().into(),
    ]);
    update(&editor, init);
    update(&editor, || {
        add_mark_id_for(&comment_marks(), "c1", &select(22, 28))?;
        add_mark_id_for(&suggestion_marks(), "s7", &select(22, 28))?;
        Ok(())
    });
    let e = (*editor).clone();
    let html = editor
        .read(move || crate::html::generate_html_from_nodes(&e, None))
        .unwrap()
        .unwrap();
    assert!(html.contains(r#"data-lexical-comment-ids="c1""#), "{html}");
    // SuggestionMarks does not export its ids
    assert!(!html.contains("s7"), "{html}");

    let target = create_default_editor();
    update(&target, move || {
        let dom = crate::dom::parse_from_string(&html.replace(
            r#"data-lexical-comment-ids="c1""#,
            r#"data-lexical-comment-ids="c1" data-lexical-suggestion-ids="s9""#,
        ));
        let root = get_root();
        root.clear()?;
        root.append(&crate::html::import::generate_nodes_from_dom_via_extension(
            &dom, None,
        )?)
    });
    assert_eq!(
        runs(&target),
        r#""Ship the migration on " "Friday"[commentIds:c1 suggestionIds:s9]"#
    );
}

fn build_error(dependencies: Vec<AnyLexicalExtensionArgument>) -> String {
    try_create_editor(dependencies)
        .err()
        .expect("the build fails")
        .message()
        .to_owned()
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > errors > two flavors with the same stateKey fail when the editor is built
#[test]
fn two_flavors_with_the_same_state_key_fail_when_the_editor_is_built() {
    let notes = define_mark_extension(&MarkFlavorDefinition {
        name: "@test/Notes".to_owned(),
        state_key: "commentIds".to_owned(),
        ..MarkFlavorDefinition::default()
    })
    .unwrap();
    assert!(
        build_error(vec![comment_marks().into(), notes.into()]).contains(
            "MarkBaseExtension: @test/CommentMarks and @test/Notes both use stateKey \"commentIds\". Give each flavor its own stateKey."
        )
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > errors > two flavors with the same attribute fail when the editor is built
#[test]
fn two_flavors_with_the_same_attribute_fail_when_the_editor_is_built() {
    let notes = define_mark_extension(&MarkFlavorDefinition {
        attribute: Some("data-lexical-comment-ids".to_owned()),
        name: "@test/Notes".to_owned(),
        state_key: "noteIds".to_owned(),
        ..MarkFlavorDefinition::default()
    })
    .unwrap();
    assert!(
        build_error(vec![comment_marks().into(), notes.into()]).contains(
            "MarkBaseExtension: @test/CommentMarks and @test/Notes both render the attribute data-lexical-comment-ids. Give each flavor its own attribute."
        )
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > errors > attributes that differ only in case clash
#[test]
fn attributes_that_differ_only_in_case_clash() {
    let notes = define_mark_extension(&MarkFlavorDefinition {
        attribute: Some("DATA-LEXICAL-COMMENT-IDS".to_owned()),
        name: "@test/Notes".to_owned(),
        state_key: "noteIds".to_owned(),
        ..MarkFlavorDefinition::default()
    })
    .unwrap();
    assert_eq!(mark_flavor(&notes).attribute(), "data-lexical-comment-ids");
    assert!(
        build_error(vec![comment_marks().into(), notes.into()])
            .contains("both render the attribute data-lexical-comment-ids")
    );
}

// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > errors > stateKey "" is rejected with the fix in the message
// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > errors > stateKey "a b" is rejected with the fix in the message
// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > errors > stateKey "markIds" is rejected with the fix in the message
// upstream: lexical-mark/src/__tests__/unit/MarkFlavors.test.ts::mark flavors > errors > stateKey "__proto__" is rejected with the fix in the message
#[test]
fn state_key_is_rejected_with_the_fix_in_the_message() {
    for state_key in ["", "a b", MARK_STATE_KEY, "__proto__"] {
        let error = define_mark_extension(&MarkFlavorDefinition {
            name: "@test/Bad".to_owned(),
            state_key: state_key.to_owned(),
            ..MarkFlavorDefinition::default()
        })
        .err()
        .expect("rejected");
        assert!(
            error.message().contains(&format!(
                "defineMarkExtension: invalid stateKey {} for @test/Bad. Use a non-empty name without whitespace, other than \"markIds\" (the key of MarkStateExtension) and \"__proto__\", such as \"commentIds\".",
                Value::from(state_key)
            )),
            "{error}"
        );
    }
}
