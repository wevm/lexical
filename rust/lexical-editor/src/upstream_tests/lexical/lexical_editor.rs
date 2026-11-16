//! Ports of `packages/lexical/src/__tests__/unit/LexicalEditor.test.tsx`.
//!
//! Upstream's `init()` mounts its editor (`createTestEditor`) on a
//! `contenteditable` div in a React root; here that is
//! [`initialize_unit_test`]'s editor, on a root element with no view, so it
//! reconciles and reports mutations as the mounted one does. Where upstream
//! asserts the rendered HTML, these assert the model it renders
//! ([`shape`]). JS's `onError` has no counterpart: an update's error is
//! what `update` returns. `await Promise.resolve()`, the microtask that
//! commits a non-discrete update in JS, is [`flush`].

use crate::constants::{IS_BOLD, IS_ITALIC, IS_UNDERLINE};
use crate::editor::{
    COMMAND_PRIORITY_BEFORE_CRITICAL, COMMAND_PRIORITY_BEFORE_EDITOR, COMMAND_PRIORITY_BEFORE_HIGH,
    COMMAND_PRIORITY_BEFORE_LOW, COMMAND_PRIORITY_BEFORE_NORMAL, COMMAND_PRIORITY_CRITICAL,
    COMMAND_PRIORITY_EDITOR, COMMAND_PRIORITY_HIGH, COMMAND_PRIORITY_LOW, COMMAND_PRIORITY_NORMAL,
    WarningHandler,
};
use crate::merge_register::merge_register;
use crate::nodes::element::Direction;
use crate::nodes::element::ElementData;
use crate::nodes::text::{TextData, TextNode};
use crate::upstream_tests::support::{
    create_test_editor, create_test_element_node, json_to_equal, set_test_root_element,
};
use crate::upstream_tests::support::{
    initialize_unit_test, panic_message, shape, update, update_with,
};
use crate::warnings::with_warning_handler;
use crate::{
    CreateEditorArgs, Editor, EditorState, LexicalError, LexicalNode, LexicalNodeReplacement,
    MutationListenerOptions, MutationListenerPayload, NodeBase, NodeClass, NodeMutation,
    NodeMutations, NodeRef, UpdateListenerPayload, UpdateOptions, create_command,
    create_paragraph_node, create_range_selection, create_text_node, get_editor, get_node_by_key,
    get_root, set_composition_key, set_selection,
};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// `init(onError, nodes, onWarn)`.
fn init() -> Editor {
    initialize_unit_test(CreateEditorArgs::default())
}

/// `init(errorListener, undefined, onWarn)`.
fn init_with_on_warn(on_warn: Option<WarningHandler>) -> Editor {
    initialize_unit_test(CreateEditorArgs {
        on_warn,
        ..CreateEditorArgs::default()
    })
}

/// `await Promise.resolve()`: the microtask that commits the pending
/// update.
fn flush(editor: &Editor) -> Result<(), LexicalError> {
    crate::updates::commit_pending_updates(editor)
}

/// `await Promise.resolve().then()`, or any wait that runs every
/// microtask: commits until nothing is pending.
fn drain(editor: &Editor) -> Result<(), LexicalError> {
    for _ in 0..10 {
        if editor.pending().is_none() {
            break;
        }
        flush(editor)?;
    }
    Ok(())
}

/// `editor.update(fn)`, not discrete.
fn update_async(
    editor: &Editor,
    f: impl FnOnce() -> Result<(), LexicalError> + 'static,
) -> Result<(), LexicalError> {
    editor.update(f, UpdateOptions::default())
}

/// `editor.update(fn, {onUpdate})`, not discrete.
fn update_on(
    editor: &Editor,
    f: impl FnOnce() -> Result<(), LexicalError> + 'static,
    on_update: impl Fn() + 'static,
) -> Result<(), LexicalError> {
    editor.update(
        f,
        UpdateOptions {
            on_update: Some(Rc::new(on_update)),
            ..UpdateOptions::default()
        },
    )
}

/// `editor.update(fn, {discrete: true})`, returning its result.
fn update_discrete(
    editor: &Editor,
    f: impl FnOnce() -> Result<(), LexicalError> + 'static,
) -> Result<(), LexicalError> {
    editor.update(
        f,
        UpdateOptions {
            discrete: true,
            ..UpdateOptions::default()
        },
    )
}

/// A log of strings, shared with listeners.
#[derive(Clone, Default)]
struct Log(Rc<RefCell<Vec<String>>>);

impl Log {
    fn push(&self, s: impl Into<String>) {
        self.0.borrow_mut().push(s.into());
    }
    fn take(&self) -> Vec<String> {
        self.0.take()
    }
    fn get(&self) -> Vec<String> {
        self.0.borrow().clone()
    }
    fn len(&self) -> usize {
        self.0.borrow().len()
    }
}

/// A counter standing in for `vi.fn()`.
#[derive(Clone, Default)]
struct Calls(Rc<Cell<usize>>);

impl Calls {
    fn record(&self) {
        self.0.set(self.0.get() + 1);
    }
    fn count(&self) -> usize {
        self.0.get()
    }
}

fn paragraph_with(text: &str) -> Result<NodeRef, LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node(text)])?;
    Ok(paragraph)
}

fn token(text: &str) -> NodeRef {
    let node = create_text_node(text);
    node.set_mode("token").unwrap();
    node
}

fn unmergeable(text: &str) -> NodeRef {
    let node = create_text_node(text);
    node.toggle_unmergeable();
    node
}

fn text_keys(editor: &Editor) -> Vec<String> {
    editor
        .read(|| {
            get_root()
                .get_all_text_nodes()
                .iter()
                .map(|n| n.get_key().to_owned())
                .collect()
        })
        .unwrap()
}

fn text_contents(editor: &Editor) -> Vec<String> {
    editor
        .read(|| {
            get_root()
                .get_all_text_nodes()
                .iter()
                .map(NodeRef::get_text_content)
                .collect()
        })
        .unwrap()
}

fn root_text(editor: &Editor) -> String {
    editor.read(|| get_root().get_text_content()).unwrap()
}

fn first_descendant_format(editor: &Editor) -> i32 {
    editor
        .read(|| {
            get_root()
                .get_first_descendant()
                .unwrap()
                .text_data()
                .get_format() as i32
        })
        .unwrap()
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > registerNodeTransform > Calls the RootNode transform last on every update
#[test]
fn calls_the_root_node_transform_last_on_every_update() {
    let editor = init();
    let events = Log::default();
    for node_type in ["root", "paragraph", "text"] {
        let events = events.clone();
        let _ = editor.register_node_transform(node_type, move |node| {
            events.push(format!("transform {} {}", node.get_type(), node.get_key()));
            Ok(())
        });
    }
    let _ = editor.register_node_transform("paragraph", |node| {
        if let Some(last) = node.get_last_child()
            && last.is_text()
            && last.get_text_content() == "[third]"
        {
            node.append(&[token("fourth")])?;
        }
        Ok(())
    });
    // Clear any transforms that occurred with the initial state on register.
    flush(&editor).unwrap();
    events.take();
    let paragraph_key = update_with(&editor, || {
        let paragraph = create_paragraph_node();
        paragraph.append(&[token("first"), token("second")])?;
        let root = get_root();
        root.clear()?;
        root.append(std::slice::from_ref(&paragraph))?;
        Ok(paragraph.get_key().to_owned())
    });
    let keys = text_keys(&editor);
    assert_eq!(
        events.take(),
        [
            format!("transform text {}", keys[0]),
            format!("transform text {}", keys[1]),
            format!("transform paragraph {paragraph_key}"),
            "transform root root".to_owned(),
        ]
    );
    // Add a transform that mutates the text.
    let _ = editor.register_node_transform("text", |node| {
        let text = node.get_text_content();
        if !text.starts_with('[') {
            node.set_text_content(&format!("[{text}]"));
        }
        Ok(())
    });
    flush(&editor).unwrap();
    let keys = text_keys(&editor);
    assert_eq!(
        events.take(),
        [
            // Leaf transforms run once with mutations,
            format!("transform text {}", keys[0]),
            format!("transform text {}", keys[1]),
            // and again with none.
            format!("transform text {}", keys[0]),
            format!("transform text {}", keys[1]),
            // The paragraph is not intentionally dirty.
            "transform root root".to_owned(),
        ]
    );
    assert_eq!(text_contents(&editor), ["[first]", "[second]"]);
    let paragraph = paragraph_key.clone();
    update_async(&editor, move || {
        for node in get_root().get_all_text_nodes() {
            let text = node.get_text_content();
            node.set_text_content(&format!(":{}:", &text[1..text.len() - 1]));
        }
        get_node_by_key(&paragraph)
            .unwrap()
            .append(&[token("third")])
    })
    .unwrap();
    flush(&editor).unwrap();
    let keys = text_keys(&editor);
    let text = |i: usize| format!("transform text {}", keys[i]);
    assert_eq!(
        events.take(),
        [
            text(0),
            text(1),
            text(2),
            text(0),
            text(1),
            text(2),
            format!("transform paragraph {paragraph_key}"),
            "transform root root".to_owned(),
            text(3),
            text(2),
            text(3),
            format!("transform paragraph {paragraph_key}"),
            "transform root root".to_owned(),
        ]
    );
    assert_eq!(
        text_contents(&editor),
        ["[:first:]", "[:second:]", "[third]", "[fourth]"]
    );
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > read() > Can read the editor state
#[test]
fn can_read_the_editor_state() {
    // The DOM half (`$getNearestNodeFromDOMNode` on the rendered elements)
    // has no counterpart without a DOM.
    let editor = init();
    assert_eq!(root_text(&editor), "");
    assert!(editor.read(get_editor).unwrap() == editor);
    let on_update = Calls::default();
    let calls = on_update.clone();
    update_on(
        &editor,
        || get_root().append(&[paragraph_with("This works!")?]),
        move || calls.record(),
    )
    .unwrap();
    assert_eq!(on_update.count(), 0);
    // This read flushes the pending update.
    assert_eq!(root_text(&editor), "This works!");
    assert_eq!(on_update.count(), 1);
    // No unexpected reconciliation.
    flush(&editor).unwrap();
    assert_eq!(on_update.count(), 1);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > read() > runs transforms the editor state
#[test]
fn read_runs_transforms_the_editor_state() {
    let editor = init();
    assert_eq!(root_text(&editor), "");
    assert!(editor.read(get_editor).unwrap() == editor);
    let _ = editor.register_node_transform("text", |node| {
        if node.get_text_content() == "This works!" {
            node.replace(&create_text_node("Transforms work!"), false)?;
        }
        Ok(())
    });
    let on_update = Calls::default();
    let calls = on_update.clone();
    update_on(
        &editor,
        || get_root().append(&[paragraph_with("This works!")?]),
        move || calls.record(),
    )
    .unwrap();
    assert_eq!(on_update.count(), 0);
    assert_eq!(root_text(&editor), "Transforms work!");
    assert_eq!(on_update.count(), 1);
    flush(&editor).unwrap();
    assert_eq!(on_update.count(), 1);
    assert_eq!(root_text(&editor), "Transforms work!");
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > read() > can be nested in an update or read
#[test]
fn read_can_be_nested_in_an_update_or_read() {
    let editor = init();
    let inner = editor.clone();
    update_async(&editor, move || {
        let root = get_root();
        root.append(&[paragraph_with("This works!")?])?;
        inner
            .read(|| assert_eq!(get_root().get_text_content(), "This works!"))
            .unwrap();
        let nested = inner.clone();
        inner
            .read(|| {
                nested
                    .update(
                        || {
                            assert_eq!(get_root().get_text_content(), "This works!");
                            Ok(())
                        },
                        UpdateOptions::default(),
                    )
                    .unwrap();
            })
            .unwrap();
        // Updating after a nested read fails: the update has been
        // committed.
        assert!(crate::upstream_tests::support::panics(|| {
            root.append(&[paragraph_with("update-read-update").unwrap()])
        }));
        Ok(())
    })
    .unwrap();
    let inner = editor.clone();
    editor
        .read(|| {
            inner
                .read(|| assert_eq!(get_root().get_text_content(), "This works!"))
                .unwrap();
        })
        .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Should handle nested updates in the correct sequence
#[test]
fn should_handle_nested_updates_in_the_correct_sequence() {
    let editor = init();
    let on_update = Calls::default();
    let calls = on_update.clone();
    let _ = editor.register_update_listener(move |_| calls.record());
    update_async(&editor, || {
        get_root().append(&[paragraph_with("This works!")?])
    })
    .unwrap();
    let log = Log::default();
    let (l, e) = (log.clone(), editor.clone());
    let l2 = log.clone();
    update_on(
        &editor,
        move || {
            l.push("A1");
            get_root().mark_dirty();
            let (l, e2) = (l.clone(), e.clone());
            let l3 = l.clone();
            e.update(
                move || {
                    l.push("B1");
                    let (l, l4) = (l.clone(), l.clone());
                    e2.update(
                        move || {
                            l.push("C1");
                            Ok(())
                        },
                        UpdateOptions {
                            on_update: Some(Rc::new(move || l4.push("F1"))),
                            ..UpdateOptions::default()
                        },
                    )
                },
                UpdateOptions {
                    on_update: Some(Rc::new(move || l3.push("E1"))),
                    ..UpdateOptions::default()
                },
            )
        },
        move || l2.push("D1"),
    )
    .unwrap();
    drain(&editor).unwrap();
    assert_eq!(on_update.count(), 1);
    assert_eq!(log.take(), ["A1", "B1", "C1", "D1", "E1", "F1"]);

    let (l, e) = (log.clone(), editor.clone());
    let l2 = log.clone();
    update_on(
        &editor,
        move || {
            l2.push("A2");
            get_root().mark_dirty();
            Ok(())
        },
        move || {
            l.push("B2");
            let (l3, l4) = (l.clone(), l.clone());
            e.update(
                move || {
                    set_composition_key(Some("root"));
                    l3.push("D2");
                    Ok(())
                },
                UpdateOptions {
                    on_update: Some(Rc::new(move || l4.push("F2"))),
                    ..UpdateOptions::default()
                },
            )
            .unwrap();
            l.push("C2");
            let (l5, l6) = (l.clone(), l.clone());
            e.update(
                move || {
                    l5.push("E2");
                    Ok(())
                },
                UpdateOptions {
                    on_update: Some(Rc::new(move || l6.push("G2"))),
                    ..UpdateOptions::default()
                },
            )
            .unwrap();
        },
    )
    .unwrap();
    drain(&editor).unwrap();
    assert_eq!(log.take(), ["A2", "B2", "C2", "D2", "E2", "F2", "G2"]);

    let (l, e) = (log.clone(), editor.clone());
    let _ = editor.register_node_transform("text", move |_| {
        l.push("TextTransform A3");
        let (l2, l3) = (l.clone(), l.clone());
        e.update(
            move || {
                l2.push("TextTransform B3");
                Ok(())
            },
            UpdateOptions {
                on_update: Some(Rc::new(move || l3.push("TextTransform C3"))),
                ..UpdateOptions::default()
            },
        )
    });
    drain(&editor).unwrap();
    assert_eq!(
        log.take(),
        ["TextTransform A3", "TextTransform B3", "TextTransform C3"]
    );
    let (l, l2) = (log.clone(), log.clone());
    update_on(
        &editor,
        move || {
            l.push("A3");
            get_root().get_last_descendant().unwrap().mark_dirty();
            Ok(())
        },
        move || l2.push("B3"),
    )
    .unwrap();
    drain(&editor).unwrap();
    assert_eq!(
        log.take(),
        [
            "A3",
            "TextTransform A3",
            "TextTransform B3",
            "B3",
            "TextTransform C3"
        ]
    );
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > nested update after selection update triggers exactly 1 update
#[test]
fn nested_update_after_selection_update_triggers_exactly_1_update() {
    let editor = init();
    let payloads = Rc::new(RefCell::new(Vec::new()));
    let heard = payloads.clone();
    let _ =
        editor.register_update_listener(move |payload| heard.borrow_mut().push(payload.clone()));
    let prev_editor_state = editor.get_editor_state();
    let e = editor.clone();
    update_async(&editor, move || {
        set_selection(Some(create_range_selection()));
        e.update(
            || get_root().append(&[paragraph_with("Sync update")?]),
            UpdateOptions::default(),
        )
    })
    .unwrap();
    flush(&editor).unwrap();
    assert_eq!(
        editor
            .read_with_mode(crate::EditorReadMode::Latest, || get_root()
                .get_text_content())
            .unwrap(),
        "Sync update"
    );
    let payloads = payloads.borrow();
    assert_eq!(payloads.len(), 1);
    // `computeUpdateListenerPayload(editor, prevEditorState, true)`: every
    // node is dirty and created, nothing was normalized, there are no tags.
    let payload = &payloads[0];
    assert!(payload.prev_editor_state.ptr_eq(&prev_editor_state));
    assert!(payload.editor_state.ptr_eq(&editor.get_editor_state()));
    let mut dirty_elements: Vec<_> = payload
        .dirty_elements
        .iter()
        .map(|(k, intentional)| (k.as_str(), *intentional))
        .collect();
    dirty_elements.sort();
    assert_eq!(dirty_elements, [("1", true), ("root", true)]);
    assert_eq!(payload.dirty_leaves, ["2"]);
    assert!(payload.normalized_nodes.is_empty());
    assert!(payload.tags.is_empty());
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > update does not call onUpdate callback when no dirty nodes
#[test]
fn update_does_not_call_on_update_callback_when_no_dirty_nodes() {
    let editor = init();
    let on_update = Calls::default();
    let calls = on_update.clone();
    update_on(&editor, || Ok(()), move || calls.record()).unwrap();
    assert_eq!(on_update.count(), 0);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Synchronously runs three transforms, two of them depend on the other
#[test]
fn synchronously_runs_three_transforms_two_of_them_depend_on_the_other() {
    let editor = init();
    let italics = editor.register_node_transform("text", |node| {
        if node.get_text_content() == "foo" && node.has_format("bold") && !node.has_format("italic")
        {
            node.toggle_format("italic");
        }
        Ok(())
    });
    let bold = editor.register_node_transform("text", |node| {
        if node.get_text_content() == "foo" && !node.has_format("bold") {
            node.toggle_format("bold");
        }
        Ok(())
    });
    let underline = editor.register_node_transform("text", |node| {
        if node.get_text_content() == "foo"
            && node.has_format("bold")
            && !node.has_format("underline")
        {
            node.toggle_format("underline");
        }
        Ok(())
    });
    update_async(&editor, || get_root().append(&[paragraph_with("foo")?])).unwrap();
    flush(&editor).unwrap();
    italics();
    bold();
    underline();
    assert_eq!(shape(&editor), "<paragraph>[foo]</paragraph>");
    assert_eq!(
        first_descendant_format(&editor),
        IS_BOLD | IS_ITALIC | IS_UNDERLINE
    );
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Synchronously runs three transforms, two of them depend on the other (2)
#[test]
fn synchronously_runs_three_transforms_two_of_them_depend_on_the_other_2() {
    let editor = init();
    let skip_first = Rc::new(Cell::new(true));
    let paragraph_transform = editor.register_node_transform("paragraph", move |paragraph| {
        if skip_first.replace(false) {
            return Ok(());
        }
        if paragraph.is_empty() {
            paragraph.append(&[create_text_node("foo")])?;
        }
        Ok(())
    });
    let bold = editor.register_node_transform("text", |node| {
        if node.get_text_content() == "foo" && !node.has_format("bold") {
            node.toggle_format("bold");
        }
        Ok(())
    });
    let italics = editor.register_node_transform("paragraph", |paragraph| {
        if let Some(child) = paragraph.get_last_descendant()
            && child.is_text()
            && child.has_format("bold")
            && !child.has_format("italic")
        {
            child.toggle_format("italic");
        }
        Ok(())
    });
    update_async(&editor, || get_root().append(&[create_paragraph_node()])).unwrap();
    flush(&editor).unwrap();
    update_async(&editor, || {
        get_root().get_first_child().unwrap().mark_dirty();
        Ok(())
    })
    .unwrap();
    flush(&editor).unwrap();
    paragraph_transform();
    bold();
    italics();
    assert_eq!(shape(&editor), "<paragraph>[foo]</paragraph>");
    assert_eq!(first_descendant_format(&editor), IS_BOLD | IS_ITALIC);
}

fn split_and_pad(node: &NodeRef) -> Result<(), LexicalError> {
    let parts = node.split_text(&[2])?;
    parts[0].insert_before(&create_text_node(""), true)?;
    parts[1].insert_after(&create_text_node(""), true)?;
    parts[1].insert_after(&create_text_node("!"), true)?;
    parts[1].insert_after(&create_text_node(""), true)?;
    Ok(())
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Synchronously runs three transforms, two of them depend on previously merged text content
#[test]
fn synchronously_runs_three_transforms_on_previously_merged_text_content() {
    let editor = init();
    let has_run = Rc::new([Cell::new(false), Cell::new(false), Cell::new(false)]);
    let h = has_run.clone();
    let foo = editor.register_node_transform("text", move |node| {
        if node.get_text_content() == "Foo" && !h[0].get() {
            split_and_pad(node)?;
            h[0].set(true);
        }
        Ok(())
    });
    let h = has_run.clone();
    let mega_foo = editor.register_node_transform("paragraph", move |paragraph| {
        if let Some(child) = paragraph.get_first_child()
            && child.is_text()
            && child.get_text_content() == "Foo!"
            && !h[1].get()
        {
            split_and_pad(&child)?;
            h[1].set(true);
        }
        Ok(())
    });
    let h = has_run.clone();
    let bold_foo = editor.register_node_transform("text", move |node| {
        if node.get_text_content() == "Foo!!" && !h[2].get() {
            node.toggle_format("bold");
            let parts = node.split_text(&[2])?;
            parts[0].insert_before(&create_text_node(""), true)?;
            parts[1].insert_after(&create_text_node(""), true)?;
            h[2].set(true);
        }
        Ok(())
    });
    update_async(&editor, || get_root().append(&[paragraph_with("Foo")?])).unwrap();
    flush(&editor).unwrap();
    foo();
    mega_foo();
    bold_foo();
    assert_eq!(shape(&editor), "<paragraph>[Foo!!]</paragraph>");
    assert_eq!(first_descendant_format(&editor), IS_BOLD);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > text transform runs when node is removed
#[test]
fn text_transform_runs_when_node_is_removed() {
    let editor = init();
    let execute_transform = Calls::default();
    let has_been_removed = Rc::new(Cell::new(false));
    let (calls, removed) = (execute_transform.clone(), has_been_removed.clone());
    let remove = editor.register_node_transform("text", move |_| {
        if removed.get() {
            calls.record();
        }
        Ok(())
    });
    update_async(&editor, || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.append(&[unmergeable("Foo"), unmergeable("Bar")])
    })
    .unwrap();
    flush(&editor).unwrap();
    let removed = has_been_removed.clone();
    update_async(&editor, move || {
        get_root().get_last_descendant().unwrap().remove(false)?;
        removed.set(true);
        Ok(())
    })
    .unwrap();
    flush(&editor).unwrap();
    assert_eq!(execute_transform.count(), 1);
    remove();
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > transforms only run on nodes that were explicitly marked as dirty
#[test]
fn transforms_only_run_on_nodes_that_were_explicitly_marked_as_dirty() {
    let editor = init();
    let counting = Rc::new(Cell::new(false));
    let (paragraph_calls, text_calls) = (Calls::default(), Calls::default());
    let (c, calls) = (counting.clone(), paragraph_calls.clone());
    let remove_paragraph = editor.register_node_transform("paragraph", move |_| {
        if c.get() {
            calls.record();
        }
        Ok(())
    });
    let (c, calls) = (counting.clone(), text_calls.clone());
    let remove_text = editor.register_node_transform("text", move |_| {
        if c.get() {
            calls.record();
        }
        Ok(())
    });
    update_async(&editor, || get_root().append(&[paragraph_with("Foo")?])).unwrap();
    flush(&editor).unwrap();
    let c = counting.clone();
    update_async(&editor, move || {
        let paragraph = get_root().get_first_child().unwrap();
        let text = paragraph.get_first_child().unwrap();
        crate::lexical_utils::get_writable(text.get_key());
        c.set(true);
        Ok(())
    })
    .unwrap();
    flush(&editor).unwrap();
    assert_eq!(paragraph_calls.count(), 0);
    assert_eq!(text_calls.count(), 1);
    remove_paragraph();
    remove_text();
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > transforms do not discard unintentional dirtyElements
#[cfg(feature = "list")]
#[test]
fn transforms_do_not_discard_unintentional_dirty_elements() {
    use crate::list::{ListType, create_list_item_node, create_list_node};
    // ListNode registers a transform that merges adjacent lists (#7333).
    let editor = init();
    fn nested_list(text: &str) -> Result<NodeRef, LexicalError> {
        let inner_item = create_list_item_node(None);
        inner_item.append(&[create_text_node(text)])?;
        let inner = create_list_node(ListType::Bullet, 1.0);
        inner.append(&[inner_item])?;
        let item = create_list_item_node(None);
        item.append(&[inner])?;
        let list = create_list_node(ListType::Bullet, 1.0);
        list.append(&[item])?;
        Ok(list)
    }
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[
            nested_list("1")?,
            create_paragraph_node(),
            nested_list("2")?,
        ])
    });
    assert_eq!(
        shape(&editor),
        "<list><listitem><list><listitem>[1]</listitem></list></listitem></list>\
         <paragraph></paragraph>\
         <list><listitem><list><listitem>[2]</listitem></list></listitem></list>"
    );
    update(&editor, || {
        for node in get_root().get_children() {
            if node.get_type() == "paragraph" {
                node.remove(false)?;
            }
        }
        Ok(())
    });
    assert_eq!(
        shape(&editor),
        "<list><listitem><list><listitem>[1]</listitem><listitem>[2]</listitem></list>\
         </listitem></list>"
    );
}

/// `describe('transforms on siblings')`: six unmergeable text nodes, three
/// in each of two paragraphs, then a text transform counting the runs on
/// each, and `operation` in an update.
fn transforms_on_siblings(operation: &'static str, expected: &[usize]) {
    let editor = init();
    let keys = update_with(&editor, || {
        let root = get_root();
        let paragraph0 = create_paragraph_node();
        let paragraph1 = create_paragraph_node();
        let text_nodes: Vec<NodeRef> = (0..6).map(|i| unmergeable(&i.to_string())).collect();
        root.append(&[paragraph0.clone(), paragraph1.clone()])?;
        paragraph0.append(&text_nodes[..3])?;
        paragraph1.append(&text_nodes[3..])?;
        Ok(text_nodes
            .iter()
            .map(|n| n.get_key().to_owned())
            .collect::<Vec<_>>())
    });
    let counts = Rc::new(RefCell::new(vec![0usize; 6]));
    let c = counts.clone();
    let remove = editor.register_node_transform("text", move |node| {
        let i: usize = node.get_text_content().parse().unwrap();
        let mut counts = c.borrow_mut();
        if counts.len() <= i {
            counts.resize(i + 1, 0);
        }
        counts[i] += 1;
        Ok(())
    });
    // Upstream's async `beforeEach` lets the registration's update commit.
    flush(&editor).unwrap();
    update_async(&editor, move || {
        let text_node1 = get_node_by_key(&keys[1]).unwrap();
        let text_node4 = get_node_by_key(&keys[4]).unwrap();
        match operation {
            "remove" => text_node1.remove(false),
            "replace" => text_node4.replace(&text_node1, false).map(drop),
            "insertBefore" => text_node4.insert_before(&text_node1, true).map(drop),
            "insertAfter" => text_node4.insert_after(&text_node1, true).map(drop),
            "splitText" => {
                text_node1.set_text_content("67");
                text_node1.split_text(&[1]).map(drop)
            }
            _ => get_root()
                .get_first_child()
                .unwrap()
                .append(&[unmergeable("6")]),
        }
    })
    .unwrap();
    flush(&editor).unwrap();
    remove();
    assert_eq!(*counts.borrow(), expected);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > transforms on siblings > on remove
#[test]
fn transforms_on_siblings_on_remove() {
    transforms_on_siblings("remove", &[2, 1, 2, 1, 1, 1]);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > transforms on siblings > on replace
#[test]
fn transforms_on_siblings_on_replace() {
    transforms_on_siblings("replace", &[2, 2, 2, 2, 1, 2]);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > transforms on siblings > on insertBefore
#[test]
fn transforms_on_siblings_on_insert_before() {
    transforms_on_siblings("insertBefore", &[2, 2, 2, 2, 2, 1]);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > transforms on siblings > on insertAfter
#[test]
fn transforms_on_siblings_on_insert_after() {
    transforms_on_siblings("insertAfter", &[2, 2, 2, 1, 2, 2]);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > transforms on siblings > on splitText
#[test]
fn transforms_on_siblings_on_split_text() {
    transforms_on_siblings("splitText", &[2, 1, 2, 1, 1, 1, 1, 1]);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > transforms on siblings > on append
#[test]
fn transforms_on_siblings_on_append() {
    transforms_on_siblings("append", &[1, 1, 2, 1, 1, 1, 1]);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Detects infinite recursivity on transforms
#[test]
fn detects_infinite_recursivity_on_transforms() {
    let editor = init();
    let bold = editor.register_node_transform("text", |node| {
        node.toggle_format("bold");
        Ok(())
    });
    flush(&editor).unwrap();
    // JS's invariant is an error `onError` hears once; Rust's is a panic.
    let error = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        update_async(&editor, || get_root().append(&[paragraph_with("foo")?])).unwrap();
        flush(&editor)
    }))
    .expect_err("the transforms recurse endlessly");
    assert!(
        panic_message(&*error)
            .starts_with("One or more transforms are endlessly triggering additional transforms")
    );
    bold();
}

/// `runCascade(onWarn)`: an update listener that always enqueues another
/// update.
fn run_cascade(on_warn: Option<WarningHandler>) -> (Editor, crate::Unregister) {
    let editor = init_with_on_warn(on_warn);
    let e = editor.clone();
    let unregister = editor.register_update_listener(move |_| {
        let _ = e.update(
            || {
                get_root().mark_dirty();
                Ok(())
            },
            UpdateOptions::default(),
        );
    });
    (editor, unregister)
}

fn mark_root_dirty() -> Result<(), LexicalError> {
    get_root().mark_dirty();
    Ok(())
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Detects infinite recursivity on update listeners (dev: default onWarn throws)
#[test]
fn detects_infinite_recursivity_on_update_listeners() {
    // The default `onWarn` throws; the error surfaces from the commit that
    // trips the guard (JS's scheduled microtask), not from `update`.
    let (editor, unregister) = run_cascade(None);
    update_async(&editor, mark_root_dirty).unwrap();
    let mut caught = Vec::new();
    for _ in 0..200 {
        if let Err(error) = flush(&editor) {
            caught.push(error);
            break;
        }
    }
    assert_eq!(caught.len(), 1);
    assert!(caught[0].message().contains("endlessly enqueueing"));
    assert!(
        caught[0]
            .message()
            .contains(&format!("Editor namespace: {}", editor.get_namespace()))
    );
    unregister();
    // The editor is usable again once the cascade is cut.
    update_discrete(&editor, mark_root_dirty).unwrap();
    for _ in 0..10 {
        flush(&editor).unwrap();
    }
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > routes the recursion guard through a custom onWarn for embedder telemetry
#[test]
fn routes_the_recursion_guard_through_a_custom_on_warn() {
    let warnings = Log::default();
    let heard = warnings.clone();
    let (editor, unregister) = run_cascade(Some(Rc::new(move |warning: LexicalError| {
        heard.push(warning.message());
        Ok(())
    })));
    update_async(&editor, mark_root_dirty).unwrap();
    for _ in 0..200 {
        // Nothing surfaces as an error.
        flush(&editor).unwrap();
    }
    let warnings = warnings.get();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("endlessly enqueueing"));
    assert!(warnings[0].contains(&format!("Editor namespace: {}", editor.get_namespace())));
    unregister();
    update_discrete(&editor, mark_root_dirty).unwrap();
    for _ in 0..10 {
        flush(&editor).unwrap();
    }
}

fn counting_on_warn() -> (Calls, Option<WarningHandler>) {
    let warnings = Calls::default();
    let heard = warnings.clone();
    (
        warnings,
        Some(Rc::new(move |_| {
            heard.record();
            Ok(())
        })),
    )
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > does not trip the recursion guard on bounded re-enqueueing across separate actions
#[test]
fn does_not_trip_the_recursion_guard_on_bounded_re_enqueueing() {
    let (warnings, on_warn) = counting_on_warn();
    let editor = init_with_on_warn(on_warn);
    let e = editor.clone();
    let unregister = editor.register_update_listener(move |_| {
        // Bounded: a single no-op update per commit.
        e.update(|| Ok(()), UpdateOptions::default()).unwrap();
    });
    for _ in 0..150 {
        update_async(&editor, mark_root_dirty).unwrap();
        // Each action yields to the event loop.
        for _ in 0..4 {
            flush(&editor).unwrap();
        }
    }
    unregister();
    assert_eq!(warnings.count(), 0);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > does not trip the recursion guard on a fast input burst driven by commands
#[test]
fn does_not_trip_the_recursion_guard_on_a_fast_input_burst_driven_by_commands() {
    let (warnings, on_warn) = counting_on_warn();
    let editor = init_with_on_warn(on_warn);
    let burst = create_command::<()>("BURST_COMMAND");
    let unregister_command = editor.register_command(
        &burst,
        |_, editor| {
            editor.update(mark_root_dirty, UpdateOptions::default())?;
            Ok(true)
        },
        COMMAND_PRIORITY_EDITOR,
    );
    let e = editor.clone();
    let unregister_update = editor.register_update_listener(move |_| {
        e.update(|| Ok(()), UpdateOptions::default()).unwrap();
    });
    for _ in 0..150 {
        editor.dispatch_command(&burst, ()).unwrap();
        for _ in 0..4 {
            flush(&editor).unwrap();
        }
    }
    unregister_update();
    unregister_command();
    assert_eq!(warnings.count(), 0);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > applies (and warns in DEV) when a command dispatched from a read-only context mutates the editor
#[test]
fn applies_and_warns_when_a_command_dispatched_from_a_read_only_context_mutates() {
    let editor = init();
    let command = create_command::<()>("READONLY_MUTATE_COMMAND");
    let unregister = editor.register_command(
        &command,
        |_, _| {
            let root = get_root();
            root.clear()?;
            root.append(&[paragraph_with("mutated")?])?;
            Ok(true)
        },
        COMMAND_PRIORITY_EDITOR,
    );
    let warnings = Log::default();
    let heard = warnings.clone();
    with_warning_handler(
        move |message| heard.push(message),
        || {
            let e = editor.clone();
            let c = command.clone();
            editor
                .read(move || e.dispatch_command(&c, ()).unwrap())
                .unwrap();
            flush(&editor).unwrap();
            let heard = warnings.take();
            assert_eq!(heard.len(), 1);
            assert!(heard[0].contains("read-only context"));
            assert_eq!(root_text(&editor), "mutated");
            // A top-level (writable) dispatch doesn't warn.
            editor.dispatch_command(&command, ()).unwrap();
            assert_eq!(warnings.len(), 0);
        },
    );
    unregister();
}

// The mutation listener tests.

/// The calls a mutation listener received: the mutations by key, and the
/// update tags.
#[derive(Clone, Default)]
struct MutationCalls(Rc<RefCell<Vec<MutationCall>>>);

/// One call of a mutation listener: its mutations and update tags.
type MutationCall = (NodeMutations, Vec<String>);

impl MutationCalls {
    fn listener(&self) -> impl Fn(&NodeMutations, &MutationListenerPayload) + 'static {
        let calls = self.0.clone();
        move |mutations, payload| {
            calls
                .borrow_mut()
                .push((mutations.clone(), payload.update_tags.clone()));
        }
    }
    fn count(&self) -> usize {
        self.0.borrow().len()
    }
    /// The mutations of call `i`, by key.
    fn get(&self, i: usize) -> BTreeMap<String, NodeMutation> {
        self.0.borrow()[i].0.iter().cloned().collect()
    }
    fn tags(&self, i: usize) -> Vec<String> {
        self.0.borrow()[i].1.clone()
    }
    fn last_tags(&self) -> Vec<String> {
        self.0.borrow().last().unwrap().1.clone()
    }
}

fn mutations<const N: usize>(
    entries: [(&String, NodeMutation); N],
) -> BTreeMap<String, NodeMutation> {
    entries.into_iter().map(|(k, m)| (k.clone(), m)).collect()
}

/// `editor.registerMutationListener(klass, listener)`.
fn listen(editor: &Editor, node_type: &str, calls: &MutationCalls) {
    let _ = editor
        .register_mutation_listener(
            node_type,
            calls.listener(),
            MutationListenerOptions::default(),
        )
        .unwrap();
}

/// An update listener payload in comparable form.
#[derive(Debug, PartialEq)]
struct Payload {
    dirty_elements: BTreeMap<String, bool>,
    dirty_leaves: BTreeSet<String>,
    mutated_nodes: BTreeMap<String, BTreeMap<String, NodeMutation>>,
    normalized_nodes: BTreeSet<String>,
    tags: BTreeSet<String>,
    editor_state: EditorStateId,
    prev_editor_state: EditorStateId,
}

impl PartialEq for EditorStateId {
    fn eq(&self, other: &Self) -> bool {
        self.0.ptr_eq(&other.0)
    }
}

/// An editor state compared by identity, as `toBe`.
#[derive(Clone)]
struct EditorStateId(EditorState);

impl std::fmt::Debug for EditorStateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EditorState@{:p}", Rc::as_ptr(&self.0.0))
    }
}

impl Payload {
    fn of(payload: &UpdateListenerPayload) -> Self {
        Payload {
            dirty_elements: payload.dirty_elements.iter().cloned().collect(),
            dirty_leaves: payload.dirty_leaves.iter().cloned().collect(),
            mutated_nodes: payload
                .mutated_nodes
                .iter()
                .flatten()
                .map(|(t, m)| ((*t).to_owned(), m.iter().cloned().collect()))
                .collect(),
            normalized_nodes: payload.normalized_nodes.iter().cloned().collect(),
            tags: payload.tags.iter().cloned().collect(),
            editor_state: EditorStateId(payload.editor_state.clone()),
            prev_editor_state: EditorStateId(payload.prev_editor_state.clone()),
        }
    }
}

/// `computeUpdateListenerPayload(editor, prevEditorState, true)`: every
/// node of the new state dirty, the nodes of the previous state destroyed
/// and those of the new one created or updated, by node type.
fn compute_update_listener_payload(editor: &Editor, prev_editor_state: &EditorState) -> Payload {
    let mut dirty_elements = BTreeMap::new();
    let mut dirty_leaves = BTreeSet::new();
    let mut mutated_nodes: BTreeMap<String, BTreeMap<String, NodeMutation>> = BTreeMap::new();
    let prev_map = prev_editor_state.0.node_map.borrow();
    let mut element_types = BTreeSet::new();
    for (key, node) in prev_map.iter() {
        if node.element().is_some() {
            element_types.insert(node.get_type().to_owned());
        }
        mutated_nodes
            .entry(node.get_type().to_owned())
            .or_default()
            .insert(key.clone(), NodeMutation::Destroyed);
    }
    let editor_state = editor.get_editor_state();
    for (key, node) in editor_state.0.node_map.borrow().iter() {
        if node.element().is_some() {
            element_types.insert(node.get_type().to_owned());
            dirty_elements.insert(key.clone(), true);
        } else {
            dirty_leaves.insert(key.clone());
        }
        let mutation = if prev_map.contains_key(key) {
            NodeMutation::Updated
        } else {
            NodeMutation::Created
        };
        mutated_nodes
            .entry(node.get_type().to_owned())
            .or_default()
            .insert(key.clone(), mutation);
    }
    // A corner case of element tracking: dirtyElements has the keys of
    // destroyed elements.
    for (node_type, m) in &mutated_nodes {
        if element_types.contains(node_type) {
            for (key, mutation) in m {
                if *mutation == NodeMutation::Destroyed {
                    dirty_elements.insert(key.clone(), true);
                }
            }
        }
    }
    Payload {
        dirty_elements,
        dirty_leaves,
        mutated_nodes,
        normalized_nodes: BTreeSet::new(),
        tags: BTreeSet::new(),
        editor_state: EditorStateId(editor_state.clone()),
        prev_editor_state: EditorStateId(prev_editor_state.clone()),
    }
}

/// `await editor.update(fn)`.
fn await_update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    update_async(editor, f).unwrap();
    flush(editor).unwrap();
}

/// Keys recorded in updates.
#[derive(Clone, Default)]
struct Keys(Rc<RefCell<Vec<String>>>);

impl Keys {
    fn push(&self, node: &NodeRef) {
        self.0.borrow_mut().push(node.get_key().to_owned());
    }
    fn get(&self, i: usize) -> String {
        self.0.borrow()[i].clone()
    }
}

fn formatted(text: &str, format: &str) -> NodeRef {
    let node = create_text_node(text);
    node.toggle_format(format);
    node
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation listener
#[test]
fn mutation_listener() {
    let editor = init();
    let paragraph_node_mutations = MutationCalls::default();
    let text_node_mutations = MutationCalls::default();
    let on_update = Rc::new(RefCell::new(Vec::new()));
    let heard = on_update.clone();
    let _ = editor.register_update_listener(move |p| heard.borrow_mut().push(Payload::of(p)));
    listen(&editor, "paragraph", &paragraph_node_mutations);
    listen(&editor, "text", &text_node_mutations);
    let (paragraph_keys, text_node_keys) = (Keys::default(), Keys::default());
    let prev_editor_state = editor.get_editor_state();

    // No await (batched with the next).
    let (p, t) = (paragraph_keys.clone(), text_node_keys.clone());
    update_async(&editor, move || {
        let paragraph = create_paragraph_node();
        let text_node = create_text_node("foo");
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.append(std::slice::from_ref(&text_node))?;
        p.push(&paragraph);
        t.push(&text_node);
        Ok(())
    })
    .unwrap();
    let t = text_node_keys.clone();
    await_update(&editor, move || {
        let text_node = get_node_by_key(&t.get(0)).unwrap();
        let text_node2 = formatted("bar", "bold");
        let text_node3 = formatted("xyz", "italic");
        text_node.insert_after(&text_node2, true)?;
        text_node2.insert_after(&text_node3, true)?;
        t.push(&text_node2);
        t.push(&text_node3);
        Ok(())
    });
    assert_eq!(on_update.borrow().len(), 1);
    assert_eq!(
        on_update.borrow().last().unwrap(),
        &compute_update_listener_payload(&editor, &prev_editor_state)
    );

    let prev_editor_state = editor.get_editor_state();
    await_update(&editor, || get_root().clear());
    assert_eq!(on_update.borrow().len(), 2);
    assert_eq!(
        on_update.borrow().last().unwrap(),
        &compute_update_listener_payload(&editor, &prev_editor_state)
    );

    let prev_editor_state = editor.get_editor_state();
    let (p, t) = (paragraph_keys.clone(), text_node_keys.clone());
    await_update(&editor, move || {
        let paragraph = create_paragraph_node();
        p.push(&paragraph);
        // Created and deleted in the same update (not attached).
        t.push(&create_text_node("zzz"));
        get_root().append(&[paragraph])
    });
    assert_eq!(on_update.borrow().len(), 3);
    assert_eq!(
        on_update.borrow().last().unwrap(),
        &compute_update_listener_payload(&editor, &prev_editor_state)
    );

    let (p, t) = (|i| paragraph_keys.get(i), |i| text_node_keys.get(i));
    assert_eq!(paragraph_node_mutations.count(), 3);
    assert_eq!(text_node_mutations.count(), 2);
    assert_eq!(
        paragraph_node_mutations.get(0),
        mutations([(&p(0), NodeMutation::Created)])
    );
    assert_eq!(
        paragraph_node_mutations.get(1).get(&p(0)),
        Some(&NodeMutation::Destroyed)
    );
    assert_eq!(
        paragraph_node_mutations.get(2),
        mutations([(&p(1), NodeMutation::Created)])
    );
    assert_eq!(
        text_node_mutations.get(0),
        mutations([
            (&t(0), NodeMutation::Created),
            (&t(1), NodeMutation::Created),
            (&t(2), NodeMutation::Created),
        ])
    );
    assert_eq!(
        text_node_mutations.get(1),
        mutations([
            (&t(0), NodeMutation::Destroyed),
            (&t(1), NodeMutation::Destroyed),
            (&t(2), NodeMutation::Destroyed),
        ])
    );
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation listener on newly initialized editor
#[test]
fn mutation_listener_on_newly_initialized_editor() {
    let editor = crate::create_editor(CreateEditorArgs::default());
    let text_node_mutations = MutationCalls::default();
    listen(&editor, "text", &text_node_mutations);
    assert_eq!(text_node_mutations.count(), 0);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation listener with setEditorState
#[test]
fn mutation_listener_with_set_editor_state() {
    let editor = init();
    await_update(&editor, || get_root().append(&[create_paragraph_node()]));
    let initial_editor_state = editor.get_editor_state();
    let text_node_mutations = MutationCalls::default();
    listen(&editor, "text", &text_node_mutations);
    let text_node_keys = Keys::default();
    let t = text_node_keys.clone();
    await_update(&editor, move || {
        let text_node1 = create_text_node("foo");
        get_root()
            .get_first_child()
            .unwrap()
            .append(std::slice::from_ref(&text_node1))?;
        t.push(&text_node1);
        Ok(())
    });
    let foo_editor_state = editor.get_editor_state();
    editor
        .set_editor_state(&initial_editor_state, None)
        .unwrap();
    // No effect on the mutation listeners.
    let parsed_foo_editor_state = editor
        .parse_editor_state(&crate::json::stringify(&foo_editor_state.to_json(false)).unwrap())
        .unwrap();
    let t = text_node_keys.clone();
    await_update(&editor, move || {
        let text_node2 = formatted("bar", "bold");
        let text_node3 = formatted("xyz", "italic");
        get_root()
            .get_first_child()
            .unwrap()
            .append(&[text_node2.clone(), text_node3.clone()])?;
        t.push(&text_node2);
        t.push(&text_node3);
        Ok(())
    });
    editor
        .set_editor_state(&parsed_foo_editor_state, None)
        .unwrap();
    let t = |i| text_node_keys.get(i);
    assert_eq!(text_node_mutations.count(), 4);
    assert_eq!(
        text_node_mutations.get(0),
        mutations([(&t(0), NodeMutation::Created)])
    );
    assert_eq!(
        text_node_mutations.get(1),
        mutations([(&t(0), NodeMutation::Destroyed)])
    );
    assert_eq!(
        text_node_mutations.get(2),
        mutations([
            (&t(1), NodeMutation::Created),
            (&t(2), NodeMutation::Created)
        ])
    );
    // +1 key newly generated by parseEditorState.
    let last = text_node_mutations.get(3);
    assert_eq!(last.len(), 3);
    assert_eq!(last.get(&t(1)), Some(&NodeMutation::Destroyed));
    assert_eq!(last.get(&t(2)), Some(&NodeMutation::Destroyed));
}

/// `TestTextNode` (`test_text`): a `TextNode` subclass.
#[derive(Clone, Debug, Default)]
struct TestTextNode {
    base: NodeBase,
    text: TextData,
}

impl LexicalNode for TestTextNode {
    fn get_type(&self) -> &'static str {
        "test_text"
    }
    fn super_types(&self) -> Vec<&'static str> {
        vec!["text"]
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn text(&self) -> Option<&TextData> {
        Some(&self.text)
    }
    fn text_mut(&mut self) -> Option<&mut TextData> {
        Some(&mut self.text)
    }
}

/// `TestTextNode`, and `{replace: TextNode, with: node => new
/// TestTextNode(node.getTextContent()), withKlass: TestTextNode}`.
fn text_replacement() -> Vec<NodeClass> {
    text_replacement_with_klass(true)
}

/// [`text_replacement`], with or without its `withKlass`.
fn text_replacement_with_klass(with_klass: bool) -> Vec<NodeClass> {
    vec![
        NodeClass::of::<TestTextNode>(),
        LexicalNodeReplacement {
            replace: NodeClass::of::<TextNode>(),
            with: Rc::new(|node: &NodeRef| {
                Ok(crate::lexical_utils::set_node_key(Box::new(TestTextNode {
                    text: TextData {
                        text: node.get_text_content(),
                        ..TextData::default()
                    },
                    ..TestTextNode::default()
                })))
            }),
            with_klass: with_klass.then(NodeClass::of::<TestTextNode>),
        }
        .into(),
    ]
}

fn init_with_nodes(nodes: Vec<NodeClass>) -> Editor {
    initialize_unit_test(CreateEditorArgs {
        nodes,
        ..CreateEditorArgs::default()
    })
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation listener set for original node should work with the replaced node
#[test]
fn mutation_listener_set_for_original_node_works_with_the_replaced_node() {
    let editor = init_with_nodes(text_replacement());
    let text_node_mutations = MutationCalls::default();
    let text_node_mutations_b = MutationCalls::default();
    listen(&editor, "text", &text_node_mutations);
    let text_node_keys = Keys::default();
    // No await (batched with the next).
    let t = text_node_keys.clone();
    update_async(&editor, move || {
        let paragraph = create_paragraph_node();
        let text_node = create_text_node("foo");
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.append(std::slice::from_ref(&text_node))?;
        t.push(&text_node);
        Ok(())
    })
    .unwrap();
    let t = text_node_keys.clone();
    await_update(&editor, move || {
        let text_node = get_node_by_key(&t.get(0)).unwrap();
        let text_node2 = formatted("bar", "bold");
        let text_node3 = formatted("xyz", "italic");
        text_node.insert_after(&text_node2, true)?;
        text_node2.insert_after(&text_node3, true)?;
        t.push(&text_node2);
        t.push(&text_node3);
        Ok(())
    });
    listen(&editor, "text", &text_node_mutations_b);
    await_update(&editor, || get_root().clear());
    let t = text_node_keys.clone();
    await_update(&editor, move || {
        let paragraph = create_paragraph_node();
        // Created and deleted in the same update (not attached).
        t.push(&create_text_node("zzz"));
        get_root().append(&[paragraph])
    });
    let t = |i| text_node_keys.get(i);
    let created = mutations([
        (&t(0), NodeMutation::Created),
        (&t(1), NodeMutation::Created),
        (&t(2), NodeMutation::Created),
    ]);
    let destroyed = mutations([
        (&t(0), NodeMutation::Destroyed),
        (&t(1), NodeMutation::Destroyed),
        (&t(2), NodeMutation::Destroyed),
    ]);
    assert_eq!(text_node_mutations.count(), 2);
    assert_eq!(text_node_mutations_b.count(), 2);
    assert_eq!(text_node_mutations.get(0), created);
    assert!(text_node_mutations.tags(0).is_empty());
    assert_eq!(text_node_mutations.get(1), destroyed);
    assert!(text_node_mutations.tags(1).is_empty());
    assert_eq!(text_node_mutations_b.get(0), created);
    assert_eq!(text_node_mutations_b.tags(0), ["registerMutationListener"]);
    assert_eq!(text_node_mutations_b.get(1), destroyed);
    assert!(text_node_mutations_b.tags(1).is_empty());
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation listener should work with the replaced node
#[test]
fn mutation_listener_should_work_with_the_replaced_node() {
    let editor = init_with_nodes(text_replacement());
    let text_node_mutations = MutationCalls::default();
    let text_node_mutations_b = MutationCalls::default();
    listen(&editor, "test_text", &text_node_mutations);
    let text_node_keys = Keys::default();
    let t = text_node_keys.clone();
    await_update(&editor, move || {
        let paragraph = create_paragraph_node();
        let text_node = create_text_node("foo");
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.append(std::slice::from_ref(&text_node))?;
        t.push(&text_node);
        Ok(())
    });
    listen(&editor, "test_text", &text_node_mutations_b);
    let key = text_node_keys.get(0);
    assert_eq!(text_node_mutations.count(), 1);
    assert_eq!(
        text_node_mutations.get(0),
        mutations([(&key, NodeMutation::Created)])
    );
    assert!(text_node_mutations.tags(0).is_empty());
    assert_eq!(
        text_node_mutations_b.get(0),
        mutations([(&key, NodeMutation::Created)])
    );
    assert_eq!(text_node_mutations_b.tags(0), ["registerMutationListener"]);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation listeners does not trigger when other node types are mutated
#[test]
fn mutation_listeners_do_not_trigger_when_other_node_types_are_mutated() {
    let editor = init();
    let paragraph_node_mutations = MutationCalls::default();
    let text_node_mutations = MutationCalls::default();
    listen(&editor, "paragraph", &paragraph_node_mutations);
    listen(&editor, "text", &text_node_mutations);
    await_update(&editor, || get_root().append(&[create_paragraph_node()]));
    assert_eq!(paragraph_node_mutations.count(), 1);
    assert_eq!(text_node_mutations.count(), 0);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation listeners with normalization
#[test]
fn mutation_listeners_with_normalization() {
    let editor = init();
    let text_node_mutations = MutationCalls::default();
    listen(&editor, "text", &text_node_mutations);
    let text_node_keys = Keys::default();
    let t = text_node_keys.clone();
    await_update(&editor, move || {
        let paragraph = create_paragraph_node();
        let text_node1 = create_text_node("foo");
        let text_node2 = create_text_node("bar");
        t.push(&text_node1);
        t.push(&text_node2);
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.append(&[text_node1, text_node2])
    });
    let t = text_node_keys.clone();
    await_update(&editor, move || {
        let text_node3 = formatted("xyz", "bold");
        get_root()
            .get_first_child()
            .unwrap()
            .append(std::slice::from_ref(&text_node3))?;
        t.push(&text_node3);
        Ok(())
    });
    let t = text_node_keys.clone();
    await_update(&editor, move || {
        // Normalizes with foobar.
        get_node_by_key(&t.get(2)).unwrap().toggle_format("bold");
        Ok(())
    });
    let t = |i| text_node_keys.get(i);
    assert_eq!(text_node_mutations.count(), 3);
    assert_eq!(
        text_node_mutations.get(0),
        mutations([(&t(0), NodeMutation::Created)])
    );
    assert_eq!(text_node_mutations.get(1).len(), 2);
    assert_eq!(
        text_node_mutations.get(1).get(&t(2)),
        Some(&NodeMutation::Created)
    );
    assert_eq!(text_node_mutations.get(2).len(), 2);
    assert_eq!(
        text_node_mutations.get(2).get(&t(0)),
        Some(&NodeMutation::Updated)
    );
    assert_eq!(
        text_node_mutations.get(2).get(&t(2)),
        Some(&NodeMutation::Destroyed)
    );
}

/// A block element of its own type, standing in for `TableRowNode` and
/// `TableCellNode`, which aren't ported.
#[derive(Clone, Debug, Default)]
struct StandInElementNode<const CELL: bool> {
    base: NodeBase,
    element: ElementData,
}

impl<const CELL: bool> LexicalNode for StandInElementNode<CELL> {
    fn get_type(&self) -> &'static str {
        if CELL { "test_cell" } else { "test_row" }
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn element(&self) -> Option<&ElementData> {
        Some(&self.element)
    }
    fn element_mut(&mut self) -> Option<&mut ElementData> {
        Some(&mut self.element)
    }
}

type TestRowNode = StandInElementNode<false>;
type TestCellNode = StandInElementNode<true>;

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > mutation "update" listener
#[test]
fn mutation_update_listener() {
    // The table half uses the table nodes for structure only: a
    // TestElementNode stands in for the table, TestRowNode and TestCellNode
    // for its row and cells, and a format change for `toggleHeaderStyle`.
    let editor = init_with_nodes(vec![
        NodeClass::of::<TestRowNode>(),
        NodeClass::of::<TestCellNode>(),
    ]);
    let paragraph_node_mutations = MutationCalls::default();
    let text_node_mutations = MutationCalls::default();
    listen(&editor, "paragraph", &paragraph_node_mutations);
    listen(&editor, "text", &text_node_mutations);
    let (paragraph_node_keys, text_node_keys) = (Keys::default(), Keys::default());
    let (p, t) = (paragraph_node_keys.clone(), text_node_keys.clone());
    await_update(&editor, move || {
        let paragraph = create_paragraph_node();
        let text_node1 = create_text_node("foo");
        t.push(&text_node1);
        p.push(&paragraph);
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.append(&[text_node1])
    });
    assert_eq!(paragraph_node_mutations.count(), 1);
    assert_eq!(text_node_mutations.count(), 1);
    assert_eq!(text_node_mutations.get(0).len(), 1);
    assert_eq!(paragraph_node_mutations.get(0).len(), 1);
    // Change the first text node's content.
    let t = text_node_keys.clone();
    await_update(&editor, move || {
        get_node_by_key(&t.get(0)).unwrap().set_text_content("Test");
        Ok(())
    });
    // Append a text node to the paragraph.
    let p = paragraph_node_keys.clone();
    await_update(&editor, move || {
        get_node_by_key(&p.get(0))
            .unwrap()
            .append(&[create_text_node("foo")])
    });
    assert_eq!(text_node_mutations.count(), 3);
    // The text node was updated when its text content changed.
    assert_eq!(
        text_node_mutations.get(1).get(&text_node_keys.get(0)),
        Some(&NodeMutation::Updated)
    );
    assert_eq!(paragraph_node_mutations.count(), 2);
    // The paragraph was updated when a text node was appended.
    assert_eq!(
        paragraph_node_mutations
            .get(1)
            .get(&paragraph_node_keys.get(0)),
        Some(&NodeMutation::Updated)
    );

    let table_cell_mutations = MutationCalls::default();
    let table_row_mutations = MutationCalls::default();
    listen(&editor, "test_cell", &table_cell_mutations);
    listen(&editor, "test_row", &table_row_mutations);
    let (row_key, cell_key) = (Keys::default(), Keys::default());
    let (r, c) = (row_key.clone(), cell_key.clone());
    // Create the table.
    await_update(&editor, move || {
        let table_cell = crate::create::<TestCellNode>();
        let table_row = crate::create::<TestRowNode>();
        let table = create_test_element_node();
        table_row.append(std::slice::from_ref(&table_cell))?;
        table.append(std::slice::from_ref(&table_row))?;
        get_root().append(&[table])?;
        r.push(&table_row);
        c.push(&table_cell);
        Ok(())
    });
    // Add a cell to the row.
    let r = row_key.clone();
    await_update(&editor, move || {
        get_node_by_key(&r.get(0))
            .unwrap()
            .append(&[crate::create::<TestCellNode>()])
    });
    // Update the cell.
    let c = cell_key.clone();
    await_update(&editor, move || {
        get_node_by_key(&c.get(0))
            .unwrap()
            .set_element_format("center");
        Ok(())
    });
    assert_eq!(table_cell_mutations.count(), 3);
    // The cell was updated when its format changed.
    assert_eq!(
        table_cell_mutations.get(2).get(&cell_key.get(0)),
        Some(&NodeMutation::Updated)
    );
    assert_eq!(table_row_mutations.count(), 2);
    // The row was updated when a child was added.
    assert_eq!(
        table_row_mutations.get(1).get(&row_key.get(0)),
        Some(&NodeMutation::Updated)
    );
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > allows using the same listener for multiple node types
#[test]
fn allows_using_the_same_listener_for_multiple_node_types() {
    let editor = init();
    let listener = MutationCalls::default();
    listen(&editor, "text", &listener);
    listen(&editor, "paragraph", &listener);
    let (paragraph_key, text_node_key) = (Keys::default(), Keys::default());
    let (p, t) = (paragraph_key.clone(), text_node_key.clone());
    await_update(&editor, move || {
        let paragraph = create_paragraph_node();
        let text_node = create_text_node("Hello world");
        p.push(&paragraph);
        t.push(&text_node);
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.append(&[text_node])
    });
    assert_eq!(listener.count(), 2);
    assert_eq!(
        listener.get(0),
        mutations([(&text_node_key.get(0), NodeMutation::Created)])
    );
    assert_eq!(
        listener.get(1),
        mutations([(&paragraph_key.get(0), NodeMutation::Created)])
    );
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > calls mutation listener with initial state
#[test]
fn calls_mutation_listener_with_initial_state() {
    let (a, b, c) = (
        MutationCalls::default(),
        MutationCalls::default(),
        MutationCalls::default(),
    );
    let editor = init();
    listen(&editor, "text", &a);
    assert_eq!(a.count(), 0);
    await_update(&editor, || {
        get_root().append(&[paragraph_with("Hello world")?])
    });
    assert_eq!(a.count(), 1);
    assert!(
        !a.last_tags()
            .iter()
            .any(|t| t == "registerMutationListener")
    );
    listen(&editor, "text", &b);
    let _ = editor
        .register_mutation_listener(
            "text",
            c.listener(),
            MutationListenerOptions {
                skip_initialization: true,
            },
        )
        .unwrap();
    assert_eq!(a.count(), 1);
    assert_eq!(b.count(), 1);
    assert!(
        b.last_tags()
            .iter()
            .any(|t| t == "registerMutationListener")
    );
    assert_eq!(c.count(), 0);
    await_update(&editor, || {
        get_root().append(&[paragraph_with("Another update!")?])
    });
    assert_eq!(a.count(), 2);
    assert_eq!(b.count(), 2);
    assert_eq!(c.count(), 1);
    for listener in [&a, &b, &c] {
        assert!(
            !listener
                .last_tags()
                .iter()
                .any(|t| t == "registerMutationListener")
        );
    }
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > still detects a runaway cascade that dispatches commands from a mutation listener
#[test]
fn still_detects_a_runaway_cascade_that_dispatches_commands_from_a_mutation_listener() {
    let (warnings, on_warn) = counting_on_warn();
    let editor = init_with_on_warn(on_warn);
    let noop_command = create_command::<()>("NOOP_COMMAND");
    let text_key = update_with(&editor, || {
        let text = create_text_node("a");
        let root = get_root();
        root.clear()?;
        let paragraph = create_paragraph_node();
        paragraph.append(std::slice::from_ref(&text))?;
        root.append(&[paragraph])?;
        Ok(text.get_key().to_owned())
    });
    drain(&editor).unwrap();
    let e = editor.clone();
    let unregister_mutation = editor
        .register_mutation_listener(
            "text",
            move |_, _| {
                e.dispatch_command(&noop_command, ()).unwrap();
            },
            MutationListenerOptions::default(),
        )
        .unwrap();
    // Unbounded: flips the text content again on every commit, forever.
    let (e, key) = (editor.clone(), text_key.clone());
    let unregister_update = editor.register_update_listener(move |_| {
        let key = key.clone();
        e.update(
            move || {
                if let Some(text) = get_node_by_key(&key).filter(NodeRef::is_text) {
                    let next = if text.get_text_content() == "a" {
                        "b"
                    } else {
                        "a"
                    };
                    text.set_text_content(next);
                }
                Ok(())
            },
            UpdateOptions::default(),
        )
        .unwrap();
    });
    update_async(&editor, move || {
        if let Some(text) = get_node_by_key(&text_key).filter(NodeRef::is_text) {
            text.set_text_content("b");
        }
        Ok(())
    })
    .unwrap();
    // Only commits: the runaway never yields to the host.
    for _ in 0..800 {
        if warnings.count() > 0 {
            break;
        }
        flush(&editor).unwrap();
    }
    unregister_update();
    unregister_mutation();
    drain(&editor).unwrap();
    assert_eq!(warnings.count(), 1);
    // The editor is still usable once the cascade is cut.
    update_discrete(&editor, mark_root_dirty).unwrap();
    for _ in 0..10 {
        flush(&editor).unwrap();
    }
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > read() > does not flush an update started by a commit listener until every listener of that commit has run (#7709)
#[test]
fn read_does_not_flush_an_update_started_by_a_commit_listener_until_every_listener_has_run() {
    let editor = init();
    await_update(&editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph_with("first")?])
    });
    let updates = Rc::new(RefCell::new(Vec::<(EditorState, EditorState)>::new()));
    let heard = updates.clone();
    let _ = editor.register_update_listener(move |p| {
        heard
            .borrow_mut()
            .push((p.prev_editor_state.clone(), p.editor_state.clone()));
    });
    let appended = Rc::new(Cell::new(false));
    let e = editor.clone();
    let skip = MutationListenerOptions {
        skip_initialization: true,
    };
    let _ = editor
        .register_mutation_listener(
            "paragraph",
            move |_, _| {
                if !appended.replace(true) {
                    e.update(
                        || get_root().append(&[paragraph_with("third")?]),
                        UpdateOptions::default(),
                    )
                    .unwrap();
                }
            },
            skip,
        )
        .unwrap();
    let reads = Log::default();
    let (e, r) = (editor.clone(), reads.clone());
    let _ = editor
        .register_mutation_listener(
            "paragraph",
            move |_, _| r.push(e.read(|| get_root().get_text_content()).unwrap()),
            skip,
        )
        .unwrap();
    let warnings = Log::default();
    let heard = warnings.clone();
    with_warning_handler(
        move |message| heard.push(message),
        || {
            update(&editor, || get_root().append(&[paragraph_with("second")?]));
            // The read must not commit the update the first mutation listener
            // started.
            assert_eq!(updates.borrow().len(), 1);
            assert!(updates.borrow()[0].1.ptr_eq(&editor.get_editor_state()));
            assert_eq!(reads.get(), ["first\n\nsecond"]);
            assert_eq!(warnings.len(), 1);
            assert!(warnings.get()[0].contains("editor.read()"));
            // The pending update commits afterwards, from that state.
            flush(&editor).unwrap();
            let updates = updates.borrow();
            assert_eq!(updates.len(), 2);
            assert!(updates[1].0.ptr_eq(&updates[0].1));
            assert!(updates[1].1.ptr_eq(&editor.get_editor_state()));
            assert_eq!(reads.get(), ["first\n\nsecond", "first\n\nsecond\n\nthird"]);
            assert_eq!(warnings.len(), 1);
        },
    );
}

// Initial state, recovery, root element changes and parsing.

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Should create an editor with an initial editor state
#[test]
fn should_create_an_editor_with_an_initial_editor_state() {
    crate::reset_random_key();
    let initial_editor = create_test_editor(CreateEditorArgs::default());
    update_async(&initial_editor, || {
        get_root().append(&[paragraph_with("This works!")?])
    })
    .unwrap();
    set_test_root_element(&initial_editor);
    drain(&initial_editor).unwrap();
    assert_eq!(
        shape(&initial_editor),
        "<paragraph>[This works!]</paragraph>"
    );
    let initial_editor_state = initial_editor.get_editor_state();
    initial_editor.set_root_element(None).unwrap();
    let editor = create_test_editor(CreateEditorArgs {
        editor_state: Some(initial_editor_state.clone()),
        ..CreateEditorArgs::default()
    });
    set_test_root_element(&editor);
    assert_eq!(
        editor.get_editor_state().to_json(false),
        initial_editor_state.to_json(false)
    );
    assert_eq!(shape(&editor), "<paragraph>[This works!]</paragraph>");
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Should be able to recover from an update error
#[test]
fn should_be_able_to_recover_from_an_update_error() {
    let editor = init();
    update_async(&editor, || {
        let root = get_root();
        if root.get_first_child().is_none() {
            root.append(&[paragraph_with("This works!")?])?;
        }
        Ok(())
    })
    .unwrap();
    drain(&editor).unwrap();
    assert_eq!(shape(&editor), "<paragraph>[This works!]</paragraph>");
    let error = update_async(&editor, || {
        // `$assertNodeType(..., $isElementNode)` on the paragraph's text.
        let paragraph = get_root().get_first_child().filter(NodeRef::is_element);
        let element = paragraph
            .and_then(|p| p.get_first_child())
            .filter(NodeRef::is_element)
            .ok_or_else(|| LexicalError::new("Expected node to be an ElementNode"))?;
        element.get_first_child().unwrap().set_text_content("Foo");
        Ok(())
    });
    // `onError` hears of it once.
    assert!(error.is_err());
    drain(&editor).unwrap();
    assert_eq!(shape(&editor), "<paragraph>[This works!]</paragraph>");
}

/// `Retains pendingEditor while rootNode is not set (${editable})`.
fn retains_pending_editor_while_root_node_is_not_set(editable: bool) {
    const JSON_EDITOR_STATE: &str = r#"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"123","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#;
    let editor = init();
    let content_editable = editor.get_root_element();
    editor.set_editable(editable);
    // Dirty, so the `_readOnly` invariant below is tested.
    update_async(&editor, mark_root_dirty).unwrap();
    editor.set_root_element(None).unwrap();
    assert!(editor.get_editor_state().0.read_only.get());
    let editor_state = editor.parse_editor_state(JSON_EDITOR_STATE).unwrap();
    editor.set_editor_state(&editor_state, None).unwrap();
    update_async(&editor, || Ok(())).unwrap();
    editor.set_root_element(content_editable).unwrap();
    // As a structure: key order isn't part of the format.
    assert!(json_to_equal(
        &editor.get_editor_state().to_json(false),
        &crate::json::parse(JSON_EDITOR_STATE).unwrap()
    ));
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Retains pendingEditor while rootNode is not set (editable)
#[test]
fn retains_pending_editor_while_root_node_is_not_set_editable() {
    retains_pending_editor_while_root_node_is_not_set(true);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Retains pendingEditor while rootNode is not set (non-editable)
#[test]
fn retains_pending_editor_while_root_node_is_not_set_non_editable() {
    retains_pending_editor_while_root_node_is_not_set(false);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > setRootElement preserves queued updates and tags across reset (#7360)
#[test]
fn set_root_element_preserves_queued_updates_and_tags_across_reset() {
    let new_editor = create_test_editor(CreateEditorArgs::default());
    let queued_ran = Rc::new(Cell::new(false));
    let ran = queued_ran.clone();
    new_editor
        .0
        .updates
        .borrow_mut()
        .push_back(crate::editor::QueuedUpdate {
            id: 0,
            update_fn: Box::new(move || {
                ran.set(true);
                Ok(())
            }),
            options: UpdateOptions::default(),
        });
    new_editor.0.tags.borrow_mut().add("custom_tag");
    let listener_tags = Rc::new(RefCell::new(None::<Vec<String>>));
    let heard = listener_tags.clone();
    let _ = new_editor.register_update_listener(move |payload| {
        if payload.tags.iter().any(|t| t == "custom_tag") {
            *heard.borrow_mut() = Some(payload.tags.clone());
        }
    });
    // `setRootElement` commits, which drains the queue.
    set_test_root_element(&new_editor);
    assert!(queued_ran.get());
    let tags = listener_tags.borrow();
    assert!(tags.as_ref().unwrap().iter().any(|t| t == "custom_tag"));
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > parseEditorState() > exportJSON API - parses parsed JSON
#[test]
fn parse_editor_state_parses_parsed_json() {
    let editor = init();
    await_update(&editor, || {
        let paragraph = create_paragraph_node();
        paragraph.set_direction(Some(Direction::Rtl));
        let original_text = create_text_node("Hello world");
        original_text.select(Some(6), Some(11))?;
        paragraph.append(&[original_text])?;
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph])
    });
    let stringified = crate::json::stringify(&editor.get_editor_state().to_json(false)).unwrap();
    let parsed = editor
        .parse_editor_state_json(&crate::json::parse(&stringified).unwrap())
        .unwrap();
    parsed.read(|| {
        let root = get_root();
        let paragraph = root.get_first_child().unwrap();
        assert_eq!(paragraph.get_type(), "paragraph");
        assert_eq!(
            paragraph.element_data().get_direction(),
            Some(Direction::Rtl)
        );
        assert!(root.get_text_content().contains("Hello world"));
    });
}

/// The `parseEditorState()` suites' `beforeEach`: a paragraph with `Hello
/// world` and `selection` made in the update, round-tripped through JSON.
fn parsed_round_trip(selection: fn(&NodeRef) -> Result<(), LexicalError>) -> (Editor, EditorState) {
    let editor = init();
    await_update(&editor, move || {
        let paragraph = create_paragraph_node();
        let original_text = create_text_node("Hello world");
        selection(&original_text)?;
        paragraph.append(&[original_text])?;
        get_root().append(&[paragraph])
    });
    let stringified = crate::json::stringify(&editor.get_editor_state().to_json(false)).unwrap();
    let parsed = editor.parse_editor_state(&stringified).unwrap();
    (editor, parsed)
}

fn range_selection_state() -> (Editor, EditorState) {
    parsed_round_trip(|text| text.select(Some(6), Some(11)).map(drop))
}

fn node_selection_state() -> (Editor, EditorState) {
    parsed_round_trip(|text| {
        let selection = crate::create_node_selection();
        selection.add(text.get_key());
        set_selection(Some(selection));
        Ok(())
    })
}

fn key_field(key: &Option<String>) -> String {
    key.clone().unwrap_or_else(|| "null".to_owned())
}

/// The fields of an element, as upstream's `toEqual` lists them.
fn element_fields(node: &NodeRef) -> BTreeMap<&'static str, String> {
    let latest = node.get_latest();
    let base = latest.base();
    let element = latest.element().unwrap();
    BTreeMap::from([
        (
            "__dir",
            element
                .dir
                .map_or("null".to_owned(), |d| d.as_str().to_owned()),
        ),
        ("__first", key_field(&element.first)),
        ("__format", element.format.to_string()),
        ("__indent", element.indent.to_string()),
        ("__key", base.key.clone()),
        ("__last", key_field(&element.last)),
        ("__next", key_field(&base.next)),
        ("__parent", key_field(&base.parent)),
        ("__prev", key_field(&base.prev)),
        ("__size", element.size.to_string()),
        ("__slotHost", key_field(&base.slot_host)),
        (
            "__slots",
            if base.slots.is_some() {
                "{...}"
            } else {
                "null"
            }
            .to_owned(),
        ),
        ("__style", element.style.clone()),
        ("__textFormat", element.text_format.to_string()),
        ("__textStyle", element.text_style.clone()),
        ("__type", latest.get_type().to_owned()),
    ])
}

fn expected_element(
    key: &str,
    child: &str,
    parent: &str,
    node_type: &str,
) -> BTreeMap<&'static str, String> {
    BTreeMap::from([
        ("__dir", "null".to_owned()),
        ("__first", child.to_owned()),
        ("__format", "0".to_owned()),
        ("__indent", "0".to_owned()),
        ("__key", key.to_owned()),
        ("__last", child.to_owned()),
        ("__next", "null".to_owned()),
        ("__parent", parent.to_owned()),
        ("__prev", "null".to_owned()),
        ("__size", "1".to_owned()),
        ("__slotHost", "null".to_owned()),
        ("__slots", "null".to_owned()),
        ("__style", String::new()),
        ("__textFormat", "0".to_owned()),
        ("__textStyle", String::new()),
        ("__type", node_type.to_owned()),
    ])
}

/// `Parses the nodes of a stringified editor state`, for either suite.
/// The root's `__cachedText` is the editor's in Rust (`root_text_cache`):
/// null is its not caching the parsed root.
fn expect_parsed_nodes(editor: &Editor, parsed_editor_state: &EditorState) {
    parsed_editor_state.read(|| {
        let parsed_root = get_root();
        let parsed_paragraph = parsed_root.get_first_child().unwrap();
        assert_eq!(parsed_paragraph.get_type(), "paragraph");
        let parsed_text = parsed_paragraph.get_first_child().unwrap();
        assert!(parsed_text.is_text());
        let paragraph_key = parsed_paragraph.get_key().to_owned();
        let text_key = parsed_text.get_key().to_owned();
        assert!(
            !editor
                .0
                .root_text_cache
                .borrow()
                .as_ref()
                .is_some_and(|(cached, _)| Rc::ptr_eq(cached, &parsed_root.get_latest()))
        );
        assert_eq!(
            element_fields(&parsed_root),
            expected_element("root", &paragraph_key, "null", "root")
        );
        assert_eq!(
            element_fields(&parsed_paragraph),
            expected_element(&paragraph_key, &text_key, "root", "paragraph")
        );
        let latest = parsed_text.get_latest();
        let (base, text) = (latest.base(), latest.text().unwrap());
        assert_eq!(
            (
                text.detail,
                text.format,
                base.key.as_str(),
                text.mode,
                &base.next,
                base.parent.as_deref(),
                &base.prev,
                text.style.as_str(),
                text.text.as_str(),
                latest.get_type(),
            ),
            (
                0.0,
                0.0,
                text_key.as_str(),
                0,
                &None,
                Some(paragraph_key.as_str()),
                &None,
                "",
                "Hello world",
                "text",
            )
        );
    });
}

fn expect_parsed_text_content(editor: &Editor, parsed_editor_state: &EditorState) {
    parsed_editor_state.read(|| {
        let root = get_root().get_latest();
        assert!(
            !editor
                .0
                .root_text_cache
                .borrow()
                .as_ref()
                .is_some_and(|(cached, _)| Rc::ptr_eq(cached, &root))
        );
    });
    assert_eq!(
        parsed_editor_state.read(|| get_root().get_text_content()),
        "Hello world"
    );
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > parseEditorState() > range selection > Parses the nodes of a stringified editor state
#[test]
fn parse_editor_state_range_selection_parses_the_nodes() {
    let (editor, parsed) = range_selection_state();
    expect_parsed_nodes(&editor, &parsed);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > parseEditorState() > range selection > Parses the text content of the editor state
#[test]
fn parse_editor_state_range_selection_parses_the_text_content() {
    let (editor, parsed) = range_selection_state();
    expect_parsed_text_content(&editor, &parsed);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > parseEditorState() > node selection > Parses the nodes of a stringified editor state
#[test]
fn parse_editor_state_node_selection_parses_the_nodes() {
    let (editor, parsed) = node_selection_state();
    expect_parsed_nodes(&editor, &parsed);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > parseEditorState() > node selection > Parses the text content of the editor state
#[test]
fn parse_editor_state_node_selection_parses_the_text_content() {
    let (editor, parsed) = node_selection_state();
    expect_parsed_text_content(&editor, &parsed);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > $parseSerializedNode() > parses serialized nodes
#[test]
fn parse_serialized_node_parses_serialized_nodes() {
    let editor = init();
    await_update(&editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph_with("Hello world")?])
    });
    let json = editor.get_editor_state().to_json(false);
    let children = json
        .get("root")
        .and_then(|root| root.get("children"))
        .unwrap()
        .clone();
    let text = update_with(&editor, move || {
        let crate::json::Value::Array(children) = children else {
            panic!("children")
        };
        let nodes = children
            .iter()
            .map(crate::parse_serialized_node)
            .collect::<Result<Vec<_>, _>>()?;
        let root = get_root();
        root.append(&nodes)?;
        Ok(root.get_text_content())
    });
    assert_eq!(text, "Hello world\n\nHello world");
}

/// The `Node children` suite's `reset()`: an editor with one paragraph.
fn node_children_editor() -> Editor {
    let editor = init();
    await_update(&editor, || get_root().append(&[create_paragraph_node()]));
    editor
}

fn element_with_text(text: &str) -> Result<NodeRef, LexicalError> {
    let element = create_test_element_node();
    element.append(&[create_text_node(text)])?;
    Ok(element)
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Node children > moves node to different tree branches
#[test]
fn node_children_moves_node_to_different_tree_branches() {
    let editor = node_children_editor();
    let keys = update_with(&editor, || {
        let paragraph = get_root().get_first_child().unwrap();
        let element1 = element_with_text("A")?;
        let element2 = element_with_text("B")?;
        let keys = [
            paragraph.get_key().to_owned(),
            element1.get_key().to_owned(),
            element1.get_first_child().unwrap().get_key().to_owned(),
            element2.get_key().to_owned(),
            element2.get_first_child().unwrap().get_key().to_owned(),
        ];
        paragraph.append(&[element1, element2])?;
        Ok(keys)
    });
    let (k1, k2) = (keys[1].clone(), keys[3].clone());
    await_update(&editor, move || {
        get_node_by_key(&k1)
            .unwrap()
            .append(&[get_node_by_key(&k2).unwrap()])
    });
    {
        let state = editor.get_editor_state();
        let node_map = state.0.node_map.borrow();
        for key in &keys {
            assert!(node_map.contains_key(key));
        }
        // And the root.
        assert_eq!(node_map.len(), keys.len() + 1);
    }
    assert_eq!(
        shape(&editor),
        "<paragraph><test_block>[A]<test_block>[B]</test_block></test_block></paragraph>"
    );
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Node children > moves node to different tree branches (inverse)
#[test]
fn node_children_moves_node_to_different_tree_branches_inverse() {
    let editor = node_children_editor();
    let (k1, k2) = update_with(&editor, || {
        let paragraph = get_root().get_first_child().unwrap();
        let element1 = element_with_text("A")?;
        let element2 = element_with_text("B")?;
        let keys = (element1.get_key().to_owned(), element2.get_key().to_owned());
        paragraph.append(&[element1, element2])?;
        Ok(keys)
    });
    await_update(&editor, move || {
        get_node_by_key(&k2)
            .unwrap()
            .append(&[get_node_by_key(&k1).unwrap()])
    });
    assert_eq!(
        shape(&editor),
        "<paragraph><test_block>[B]<test_block>[A]</test_block></test_block></paragraph>"
    );
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > Node children > moves node to different tree branches (node appended twice in two different branches)
#[test]
fn node_children_moves_node_appended_twice_in_two_different_branches() {
    let editor = node_children_editor();
    let keys = update_with(&editor, || {
        let paragraph = get_root().get_first_child().unwrap();
        let elements = ["A", "B", "C"]
            .into_iter()
            .map(element_with_text)
            .collect::<Result<Vec<_>, _>>()?;
        let keys: Vec<String> = elements.iter().map(|e| e.get_key().to_owned()).collect();
        paragraph.append(&elements)?;
        Ok(keys)
    });
    await_update(&editor, move || {
        let element3 = get_node_by_key(&keys[2]).unwrap();
        get_node_by_key(&keys[1])
            .unwrap()
            .append(std::slice::from_ref(&element3))?;
        get_node_by_key(&keys[0]).unwrap().append(&[element3])
    });
    assert_eq!(
        shape(&editor),
        "<paragraph><test_block>[A]<test_block>[C]</test_block></test_block><test_block>[B]</test_block></paragraph>"
    );
}

// Commands, listeners and tags.

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > can subscribe and unsubscribe from commands and the callback is fired
#[test]
fn can_subscribe_and_unsubscribe_from_commands() {
    let editor = init();
    let calls = Rc::new(RefCell::new(Vec::<(String, Editor)>::new()));
    let command = create_command::<String>("TEST_COMMAND");
    let heard = calls.clone();
    let remove_command_listener = editor.register_command(
        &command,
        move |payload, editor| {
            heard.borrow_mut().push((payload.clone(), editor.clone()));
            Ok(false)
        },
        COMMAND_PRIORITY_EDITOR,
    );
    for _ in 0..3 {
        editor
            .dispatch_command(&command, "testPayload".to_owned())
            .unwrap();
    }
    assert_eq!(calls.borrow().len(), 3);
    assert!(
        calls
            .borrow()
            .iter()
            .all(|(p, e)| p == "testPayload" && *e == editor)
    );
    remove_command_listener();
    for _ in 0..3 {
        editor
            .dispatch_command(&command, "testPayload".to_owned())
            .unwrap();
    }
    assert_eq!(calls.borrow().len(), 3);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > removes the command from the command map when no listener are attached
#[test]
fn removes_the_command_from_the_command_map_when_no_listener_are_attached() {
    let editor = init();
    let calls = Log::default();
    let command = create_command::<()>("TEST_COMMAND");
    let heard = calls.clone();
    let remove_command_listener = editor.register_command(
        &command,
        move |_, _| {
            heard.push("one");
            Ok(false)
        },
        COMMAND_PRIORITY_EDITOR,
    );
    let heard = calls.clone();
    let remove_command_listener_two = editor.register_command(
        &command,
        move |_, _| {
            heard.push("two");
            Ok(false)
        },
        COMMAND_PRIORITY_EDITOR,
    );
    let sizes = |editor: &Editor| -> Vec<usize> {
        (0..5)
            .map(|p| editor.command_listeners(&command, p).len())
            .collect()
    };
    assert!(editor.has_command(&command));
    assert_eq!(sizes(&editor), [2, 0, 0, 0, 0]);
    // The listeners in the first list are the two, in order.
    editor.dispatch_command(&command, ()).unwrap();
    assert_eq!(calls.take(), ["one", "two"]);
    remove_command_listener();
    assert_eq!(sizes(&editor), [1, 0, 0, 0, 0]);
    editor.dispatch_command(&command, ()).unwrap();
    assert_eq!(calls.take(), ["two"]);
    remove_command_listener_two();
    assert!(!editor.has_command(&command));
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > can register transforms before updates
#[test]
fn can_register_transforms_before_updates() {
    let editor = init();
    let remove_text_transform = editor.register_node_transform("text", |_| Ok(()));
    let remove_paragraph_transform = editor.register_node_transform("paragraph", |_| Ok(()));
    await_update(&editor, || get_root().append(&[create_paragraph_node()]));
    remove_text_transform();
    remove_paragraph_transform();
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > textcontent listener
#[test]
fn textcontent_listener() {
    let editor = init();
    let fn_calls = Log::default();
    update_async(&editor, || get_root().append(&[paragraph_with("foo")?])).unwrap();
    let heard = fn_calls.clone();
    let _ = editor.register_text_content_listener(move |text| heard.push(text));
    await_update(&editor, || {
        get_root()
            .get_last_descendant()
            .unwrap()
            .insert_after(&create_text_node("bar"), true)
            .map(drop)
    });
    assert_eq!(fn_calls.get(), ["foobar"]);
    await_update(&editor, || {
        get_root()
            .get_last_descendant()
            .unwrap()
            .insert_after(&crate::nodes::line_break::create_line_break_node(), true)
            .map(drop)
    });
    assert_eq!(fn_calls.get(), ["foobar", "foobar\n"]);
    await_update(&editor, || {
        let root = get_root();
        root.clear()?;
        let paragraph = paragraph_with("bar")?;
        let paragraph2 = paragraph_with("yar")?;
        root.append(std::slice::from_ref(&paragraph))?;
        paragraph.insert_after(&paragraph2, true).map(drop)
    });
    assert_eq!(fn_calls.len(), 3);
    assert_eq!(fn_calls.get()[2], "bar\n\nyar");
    await_update(&editor, || {
        let paragraph = paragraph_with("bar2")?;
        let paragraph2 = paragraph_with("yar2")?;
        get_root()
            .get_last_child()
            .unwrap()
            .insert_after(&paragraph, true)?;
        paragraph.insert_after(&paragraph2, true).map(drop)
    });
    assert_eq!(fn_calls.len(), 4);
    assert_eq!(fn_calls.get()[3], "bar\n\nyar\n\nbar2\n\nyar2");
}

fn update_tags(editor: &Editor) -> BTreeSet<String> {
    editor.0.tags.borrow().iter().map(str::to_owned).collect()
}

fn tagged(tags: &[&str]) -> UpdateOptions {
    UpdateOptions {
        tags: tags.iter().map(|&t| t.to_owned()).collect(),
        ..UpdateOptions::default()
    }
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > multiple update tags
#[test]
fn multiple_update_tags() {
    let editor = init();
    editor
        .update(
            || {
                create_text_node("");
                Ok(())
            },
            tagged(&["a", "b"]),
        )
        .unwrap();
    assert_eq!(
        update_tags(&editor),
        BTreeSet::from(["a", "b"].map(String::from))
    );
    let e = editor.clone();
    editor
        .update(
            move || e.update(|| Ok(()), tagged(&["e", "f"])),
            tagged(&["c", "d"]),
        )
        .unwrap();
    assert_eq!(
        update_tags(&editor),
        BTreeSet::from(["a", "b", "c", "d", "e", "f"].map(String::from))
    );
    flush(&editor).unwrap();
    assert!(update_tags(&editor).is_empty());
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > does not leak a no-op update tag into the next update
#[test]
fn does_not_leak_a_no_op_update_tag_into_the_next_update() {
    let editor = init();
    let observed_tags = Rc::new(RefCell::new(Vec::<Vec<String>>::new()));
    let heard = observed_tags.clone();
    let unregister =
        editor.register_update_listener(move |p| heard.borrow_mut().push(p.tags.clone()));
    editor
        .update(
            || Ok(()),
            UpdateOptions {
                on_update: Some(Rc::new(|| {})),
                discrete: true,
                ..tagged(&["collaboration"])
            },
        )
        .unwrap();
    assert!(update_tags(&editor).is_empty());
    update(&editor, || get_root().append(&[create_paragraph_node()]));
    unregister();
    let observed = observed_tags.borrow();
    assert!(
        !observed
            .last()
            .unwrap()
            .iter()
            .any(|t| t == "collaboration")
    );
}

/// The two committed states the `setEditorState` tag tests start from:
/// returns the `historic` one, leaving `current` committed.
fn historic_and_current(editor: &Editor) -> EditorState {
    update(editor, || get_root().append(&[paragraph_with("historic")?]));
    let historic_state = editor.get_editor_state();
    update(editor, || {
        let root = get_root();
        root.clear()?;
        root.append(&[paragraph_with("current")?])
    });
    historic_state
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > setEditorState keeps its tag when committed from inside an update
#[test]
fn set_editor_state_keeps_its_tag_when_committed_from_inside_an_update() {
    let editor = init();
    let historic_state = historic_and_current(&editor);
    let observed = Rc::new(RefCell::new(Vec::<(String, Vec<String>)>::new()));
    let heard = observed.clone();
    let unregister = editor.register_update_listener(move |p| {
        heard.borrow_mut().push((
            p.editor_state.read(|| get_root().get_text_content()),
            p.tags.clone(),
        ));
    });
    let e = editor.clone();
    update(&editor, move || {
        e.set_editor_state(&historic_state, Some("historic-tag"))
    });
    unregister();
    let observed = observed.borrow();
    let historic_commit = observed
        .iter()
        .find(|(content, _)| content == "historic")
        .unwrap();
    assert!(historic_commit.1.iter().any(|t| t == "historic-tag"));
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > defers onUpdate callbacks when setEditorState commits inside an update
#[test]
fn defers_on_update_callbacks_when_set_editor_state_commits_inside_an_update() {
    let editor = init();
    let historic_state = historic_and_current(&editor);
    let events = Log::default();
    let (e, ev, ev2) = (editor.clone(), events.clone(), events.clone());
    update_on(
        &editor,
        move || {
            ev.push("update start");
            e.set_editor_state(&historic_state, None)?;
            ev.push("update end");
            Ok(())
        },
        move || ev2.push("onUpdate"),
    )
    .unwrap();
    events.push("after update");
    assert_eq!(events.get(), ["update start", "update end", "after update"]);
    drain(&editor).unwrap();
    assert_eq!(
        events.get(),
        ["update start", "update end", "after update", "onUpdate"]
    );
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > setEditorState reports an empty editor state outside production
#[test]
fn set_editor_state_reports_an_empty_editor_state_outside_production() {
    let editor = init();
    update(&editor, || get_root().append(&[paragraph_with("before")?]));
    let empty = editor
        .parse_editor_state(
            r#"{"root":{"children":[],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#,
        )
        .unwrap();
    // `onError` hears of it.
    let error = editor.set_editor_state(&empty, None).unwrap_err();
    assert!(error.message().contains("the editor state is empty"));
    assert_eq!(root_text(&editor), "before");
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > editable listener
#[test]
fn editable_listener() {
    let editor = init();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let heard = calls.clone();
    let _ = editor.register_editable_listener(move |editable| heard.borrow_mut().push(editable));
    assert!(editor.is_editable());
    editor.set_editable(false);
    assert!(!editor.is_editable());
    editor.set_editable(true);
    assert_eq!(*calls.borrow(), [false, true]);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > does not add new listeners while triggering existing
#[test]
fn does_not_add_new_listeners_while_triggering_existing() {
    let update_listener = Calls::default();
    let mutation_listener = Calls::default();
    let node_transform_listener = Calls::default();
    let text_content_listener = Calls::default();
    let editable_listener = Calls::default();
    let command_listener = Calls::default();
    let test_command = create_command::<bool>("TEST_COMMAND");
    let editor = init();

    let (e, c) = (editor.clone(), update_listener.clone());
    let _ = editor.register_update_listener(move |_| {
        c.record();
        let c = c.clone();
        let _ = e.register_update_listener(move |_| c.record());
    });
    let (e, c) = (editor.clone(), mutation_listener.clone());
    let _ = editor
        .register_mutation_listener(
            "text",
            move |_, _| {
                c.record();
                let c = c.clone();
                let _ = e
                    .register_mutation_listener(
                        "text",
                        move |_, _| c.record(),
                        MutationListenerOptions {
                            skip_initialization: true,
                        },
                    )
                    .unwrap();
            },
            MutationListenerOptions::default(),
        )
        .unwrap();
    let (e, c) = (editor.clone(), node_transform_listener.clone());
    let _ = editor.register_node_transform("paragraph", move |_| {
        c.record();
        let c = c.clone();
        let _ = e.register_node_transform("paragraph", move |_| {
            c.record();
            Ok(())
        });
        Ok(())
    });
    let (e, c) = (editor.clone(), editable_listener.clone());
    let _ = editor.register_editable_listener(move |_| {
        c.record();
        let c = c.clone();
        let _ = e.register_editable_listener(move |_| c.record());
    });
    let (e, c) = (editor.clone(), text_content_listener.clone());
    let _ = editor.register_text_content_listener(move |_| {
        c.record();
        let c = c.clone();
        let _ = e.register_text_content_listener(move |_| c.record());
    });
    let (e, c, command) = (
        editor.clone(),
        command_listener.clone(),
        test_command.clone(),
    );
    let _ = editor.register_command(
        &test_command,
        move |_, _| {
            c.record();
            let c = c.clone();
            let _ = e.register_command(
                &command,
                move |_, _| {
                    c.record();
                    Ok(false)
                },
                COMMAND_PRIORITY_LOW,
            );
            Ok(false)
        },
        COMMAND_PRIORITY_LOW,
    );
    await_update(&editor, || {
        get_root().append(&[paragraph_with("Hello world")?])
    });
    editor.dispatch_command(&test_command, false).unwrap();
    editor.set_editable(false);
    assert_eq!(update_listener.count(), 1);
    assert_eq!(editable_listener.count(), 1);
    assert_eq!(command_listener.count(), 1);
    assert_eq!(text_content_listener.count(), 1);
    assert_eq!(node_transform_listener.count(), 1);
    assert_eq!(mutation_listener.count(), 1);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > calls command listeners in deque order
#[test]
fn calls_command_listeners_in_deque_order() {
    let calls = Log::default();
    let reg_order = Cell::new(0);
    let name = |priority: i8| match priority {
        COMMAND_PRIORITY_BEFORE_EDITOR => "before:editor",
        COMMAND_PRIORITY_BEFORE_LOW => "before:low",
        COMMAND_PRIORITY_EDITOR => "after:editor",
        _ => "after:low",
    };
    let test_command = create_command::<()>("TEST_COMMAND");
    let editor = init();
    let listener = |priority: i8| {
        let idx = reg_order.replace(reg_order.get() + 1);
        let calls = calls.clone();
        let label = format!("{}:{idx}", name(priority));
        editor.register_command(
            &test_command,
            move |_, _| {
                calls.push(label.clone());
                Ok(false)
            },
            priority,
        )
    };
    let unregister = merge_register(vec![
        listener(COMMAND_PRIORITY_EDITOR),
        listener(COMMAND_PRIORITY_LOW),
        listener(COMMAND_PRIORITY_EDITOR),
        listener(COMMAND_PRIORITY_LOW),
        listener(COMMAND_PRIORITY_BEFORE_EDITOR),
        listener(COMMAND_PRIORITY_BEFORE_LOW),
        listener(COMMAND_PRIORITY_BEFORE_EDITOR),
        listener(COMMAND_PRIORITY_BEFORE_LOW),
    ]);
    assert_eq!(calls.len(), 0);
    editor.dispatch_command(&test_command, ()).unwrap();
    assert_eq!(
        calls.take(),
        [
            "before:low:7",
            "before:low:5",
            "after:low:1",
            "after:low:3",
            "before:editor:6",
            "before:editor:4",
            "after:editor:0",
            "after:editor:2",
        ]
    );
    unregister();
    editor.dispatch_command(&test_command, ()).unwrap();
    assert_eq!(calls.len(), 0);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > maps priorities correctly
#[test]
fn maps_priorities_correctly() {
    // Upstream finds the constants by reflecting over the module's exports.
    let priorities = [
        ("COMMAND_PRIORITY_EDITOR", COMMAND_PRIORITY_EDITOR),
        ("COMMAND_PRIORITY_LOW", COMMAND_PRIORITY_LOW),
        ("COMMAND_PRIORITY_NORMAL", COMMAND_PRIORITY_NORMAL),
        ("COMMAND_PRIORITY_HIGH", COMMAND_PRIORITY_HIGH),
        ("COMMAND_PRIORITY_CRITICAL", COMMAND_PRIORITY_CRITICAL),
        (
            "COMMAND_PRIORITY_BEFORE_EDITOR",
            COMMAND_PRIORITY_BEFORE_EDITOR,
        ),
        ("COMMAND_PRIORITY_BEFORE_LOW", COMMAND_PRIORITY_BEFORE_LOW),
        (
            "COMMAND_PRIORITY_BEFORE_NORMAL",
            COMMAND_PRIORITY_BEFORE_NORMAL,
        ),
        ("COMMAND_PRIORITY_BEFORE_HIGH", COMMAND_PRIORITY_BEFORE_HIGH),
        (
            "COMMAND_PRIORITY_BEFORE_CRITICAL",
            COMMAND_PRIORITY_BEFORE_CRITICAL,
        ),
    ];
    let (mut before, mut after): (Vec<_>, Vec<_>) = priorities
        .into_iter()
        .partition(|(k, _)| k.starts_with("COMMAND_PRIORITY_BEFORE"));
    assert!(before.iter().all(|(_, v)| *v < 0));
    assert!(after.iter().all(|(_, v)| *v >= 0));
    before.sort_by_key(|(_, v)| *v);
    after.sort_by_key(|(_, v)| *v);
    assert_eq!(before.len(), 5);
    assert_eq!(after.len(), 5);
    assert_eq!(
        before
            .iter()
            .map(|(k, _)| k.replace("COMMAND_PRIORITY_BEFORE_", ""))
            .collect::<Vec<_>>(),
        after
            .iter()
            .map(|(k, _)| k.replace("COMMAND_PRIORITY_", ""))
            .collect::<Vec<_>>()
    );
    let editor = init();
    let command = create_command::<()>("TEST_COMMAND");
    let mut listeners = Vec::new();
    let calls = Log::default();
    for count in [0, 1] {
        for arr in [&after, &before] {
            for &(k, v) in arr.iter() {
                let calls = calls.clone();
                listeners.push(editor.register_command(
                    &command,
                    move |_, _| {
                        calls.push(format!("{k} {count}"));
                        Ok(false)
                    },
                    v,
                ));
            }
        }
    }
    editor.dispatch_command(&command, ()).unwrap();
    assert_eq!(
        calls.take(),
        [
            "COMMAND_PRIORITY_BEFORE_CRITICAL 1",
            "COMMAND_PRIORITY_BEFORE_CRITICAL 0",
            "COMMAND_PRIORITY_CRITICAL 0",
            "COMMAND_PRIORITY_CRITICAL 1",
            "COMMAND_PRIORITY_BEFORE_HIGH 1",
            "COMMAND_PRIORITY_BEFORE_HIGH 0",
            "COMMAND_PRIORITY_HIGH 0",
            "COMMAND_PRIORITY_HIGH 1",
            "COMMAND_PRIORITY_BEFORE_NORMAL 1",
            "COMMAND_PRIORITY_BEFORE_NORMAL 0",
            "COMMAND_PRIORITY_NORMAL 0",
            "COMMAND_PRIORITY_NORMAL 1",
            "COMMAND_PRIORITY_BEFORE_LOW 1",
            "COMMAND_PRIORITY_BEFORE_LOW 0",
            "COMMAND_PRIORITY_LOW 0",
            "COMMAND_PRIORITY_LOW 1",
            "COMMAND_PRIORITY_BEFORE_EDITOR 1",
            "COMMAND_PRIORITY_BEFORE_EDITOR 0",
            "COMMAND_PRIORITY_EDITOR 0",
            "COMMAND_PRIORITY_EDITOR 1",
        ]
    );
    merge_register(listeners)();
    editor.dispatch_command(&command, ()).unwrap();
    assert_eq!(calls.len(), 0);
}

// Discrete and nested updates.

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > can use discrete for synchronous updates
#[test]
fn can_use_discrete_for_synchronous_updates() {
    let editor = init();
    let payloads = Rc::new(RefCell::new(Vec::new()));
    let heard = payloads.clone();
    let _ = editor.register_update_listener(move |p| heard.borrow_mut().push(Payload::of(p)));
    let prev_editor_state = editor.get_editor_state();
    update(&editor, || {
        get_root().append(&[paragraph_with("Sync update")?])
    });
    assert_eq!(
        editor
            .read_with_mode(crate::EditorReadMode::Latest, || get_root()
                .get_text_content())
            .unwrap(),
        "Sync update"
    );
    let payloads = payloads.borrow();
    assert_eq!(payloads.len(), 1);
    // `computeUpdateListenerPayload(editor, prevEditorState, true)`, the
    // mutated nodes aside: no mutation listener asks for them.
    let mut expected = compute_update_listener_payload(&editor, &prev_editor_state);
    expected.mutated_nodes = BTreeMap::new();
    assert_eq!(payloads[0], expected);
}

/// `createTestHeadlessEditor()`.
fn create_test_headless_editor(editor_state: Option<EditorState>) -> Editor {
    crate::headless::create_headless_editor(CreateEditorArgs {
        nodes: create_test_editor(CreateEditorArgs::default())
            .registered_nodes()
            .values()
            .cloned()
            .collect(),
        editor_state,
        ..CreateEditorArgs::default()
    })
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > can use discrete after a non-discrete update to flush the entire queue
#[test]
fn can_use_discrete_after_a_non_discrete_update_to_flush_the_entire_queue() {
    let headless = create_test_headless_editor(None);
    let on_update = Calls::default();
    let calls = on_update.clone();
    let _ = headless.register_update_listener(move |_| calls.record());
    update_async(&headless, || {
        get_root().append(&[paragraph_with("Async update")?])
    })
    .unwrap();
    update(&headless, || {
        get_root().append(&[paragraph_with("Sync update")?])
    });
    assert_eq!(
        headless
            .read_with_mode(crate::EditorReadMode::Latest, || get_root()
                .get_text_content())
            .unwrap(),
        "Async update\n\nSync update"
    );
    assert_eq!(on_update.count(), 1);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > can use discrete after a non-discrete setEditorState to flush the entire queue
#[test]
fn can_use_discrete_after_a_non_discrete_set_editor_state_to_flush_the_entire_queue() {
    let editor = init();
    update(&editor, || {
        get_root().append(&[paragraph_with("Async update")?])
    });
    let headless = create_test_headless_editor(Some(editor.get_editor_state()));
    update(&headless, || {
        get_root().append(&[paragraph_with("Sync update")?])
    });
    assert_eq!(
        headless
            .read_with_mode(crate::EditorReadMode::Latest, || get_root()
                .get_text_content())
            .unwrap(),
        "Async update\n\nSync update"
    );
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > can use discrete in a nested update to flush the entire queue
#[test]
fn can_use_discrete_in_a_nested_update_to_flush_the_entire_queue() {
    let editor = init();
    let on_update = Calls::default();
    let calls = on_update.clone();
    let _ = editor.register_update_listener(move |_| calls.record());
    let e = editor.clone();
    update_async(&editor, move || {
        get_root().append(&[paragraph_with("Async update")?])?;
        update_discrete(&e, || get_root().append(&[paragraph_with("Sync update")?]))
    })
    .unwrap();
    assert_eq!(
        editor
            .read_with_mode(crate::EditorReadMode::Latest, || get_root()
                .get_text_content())
            .unwrap(),
        "Async update\n\nSync update"
    );
    assert_eq!(on_update.count(), 1);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > can read in a nested update
#[test]
fn can_read_in_a_nested_update() {
    let editor = init();
    let e = editor.clone();
    let shape_after_read = update_with(&editor, move || {
        get_root().append(&[paragraph_with("foo")?])?;
        e.read(|| {})?;
        let shape_after_read = shape(&e);
        e.update(
            || get_root().append(&[paragraph_with("bar")?]),
            UpdateOptions::default(),
        )?;
        Ok(shape_after_read)
    });
    drain(&editor).unwrap();
    assert_eq!(shape_after_read, "<paragraph>[foo]</paragraph>");
    assert_eq!(
        shape(&editor),
        "<paragraph>[foo]</paragraph><paragraph>[bar]</paragraph>"
    );
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > reconciles state without root element
#[test]
fn reconciles_state_without_root_element() {
    let editor = create_test_editor(CreateEditorArgs::default());
    let state = editor
        .parse_editor_state(r#"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Hello world","type":"text","version":1}],"direction":"ltr","format":"","indent":0,"type":"paragraph","version":1}],"direction":"ltr","format":"","indent":0,"type":"root","version":1}}"#)
        .unwrap();
    editor.set_editor_state(&state, None).unwrap();
    assert_eq!(
        editor.get_editor_state().to_json(false),
        state.to_json(false)
    );
    assert!(editor.pending().is_none());
}

// Node replacement.

/// `createTestEditor({nodes})`, collecting the warnings creating it gives.
fn create_replacement_editor(nodes: Vec<NodeClass>) -> (Editor, Vec<String>) {
    let warnings = Log::default();
    let heard = warnings.clone();
    let editor = with_warning_handler(
        move |message| heard.push(message),
        || {
            create_test_editor(CreateEditorArgs {
                nodes,
                ..CreateEditorArgs::default()
            })
        },
    );
    (editor, warnings.get())
}

fn replace_without_with_klass(name: &str) -> String {
    format!(
        "Override for {name} specifies 'replace' without 'withKlass'. 'withKlass' will be required in a future version."
    )
}

/// The node type of `$createTextNode('123')`, appended in a paragraph, in an
/// update of the editor, on a root element.
fn create_text_123(editor: &Editor) -> (&'static str, String) {
    set_test_root_element(editor);
    let created = update_with(editor, || {
        let paragraph = create_paragraph_node();
        let text = create_text_node("123");
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.append(std::slice::from_ref(&text))?;
        Ok((text.get_type(), text.get_text_content()))
    });
    drain(editor).unwrap();
    created
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > node replacement > should work correctly
#[test]
fn node_replacement_should_work_correctly() {
    let (editor, _) = create_replacement_editor(text_replacement());
    assert_eq!(create_text_123(&editor), ("test_text", "123".to_owned()));
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > node replacement > node transform to the nodes specified by "replace" should not be applied to the nodes specified by "with" when "withKlass" is not specified
#[test]
fn node_replacement_transform_not_applied_to_with_without_with_klass() {
    let (editor, warnings) = create_replacement_editor(text_replacement_with_klass(false));
    assert_eq!(warnings, [replace_without_with_klass("TextNode")]);
    let transforms = Calls::default();
    let calls = transforms.clone();
    let remove_transform = editor.register_node_transform("text", move |_| {
        calls.record();
        Ok(())
    });
    assert_eq!(create_text_123(&editor), ("test_text", "123".to_owned()));
    assert_eq!(transforms.count(), 0);
    remove_transform();
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > node replacement > node transform to the nodes specified by "replace" should be applied also to the nodes specified by "with" when "withKlass" is specified
#[test]
fn node_replacement_transform_applied_to_with_with_with_klass() {
    let (editor, _) = create_replacement_editor(text_replacement());
    let transforms = Calls::default();
    let calls = transforms.clone();
    let remove_transform = editor.register_node_transform("text", move |_| {
        calls.record();
        Ok(())
    });
    assert_eq!(create_text_123(&editor), ("test_text", "123".to_owned()));
    assert_eq!(transforms.count(), 1);
    remove_transform();
}

/// `ParagraphNode` subclasses, by their types.
macro_rules! sub_paragraph {
    ($name:ident, $type:literal) => {
        #[doc = concat!("A `ParagraphNode` subclass, `", $type, "`.")]
        #[derive(Clone, Debug, Default)]
        struct $name {
            base: NodeBase,
            element: ElementData,
        }

        impl LexicalNode for $name {
            fn get_type(&self) -> &'static str {
                $type
            }
            fn super_types(&self) -> Vec<&'static str> {
                vec!["paragraph"]
            }
            fn base(&self) -> &NodeBase {
                &self.base
            }
            fn base_mut(&mut self) -> &mut NodeBase {
                &mut self.base
            }
            fn element(&self) -> Option<&ElementData> {
                Some(&self.element)
            }
            fn element_mut(&mut self) -> Option<&mut ElementData> {
                Some(&mut self.element)
            }
        }
    };
}

sub_paragraph!(CustomParagraphNode, "custom-paragraph");
sub_paragraph!(ResolvedParagraph, "resolved-paragraph");
sub_paragraph!(LegacyParagraph, "legacy-paragraph");

/// `[Sub, {replace: ParagraphNode, with: () => new Sub(), withKlass}]`.
fn paragraph_replacement<T: LexicalNode + Default>(with_klass: bool) -> Vec<NodeClass> {
    vec![
        NodeClass::of::<T>(),
        LexicalNodeReplacement {
            replace: NodeClass::of::<crate::nodes::paragraph::ParagraphNode>(),
            with: Rc::new(|_: &NodeRef| Ok(crate::create::<T>())),
            with_klass: with_klass.then(NodeClass::of::<T>),
        }
        .into(),
    ]
}

/// A serialized state whose paragraph has the default type, as an older
/// persisted document would.
const PARAGRAPH_JSON: &str =
    r#"{"root":{"children":[{"children":[],"type":"paragraph"}],"type":"root"}}"#;

fn expect_custom_paragraph_after_parse(editor: &Editor) {
    let parsed = editor.parse_editor_state(PARAGRAPH_JSON).unwrap();
    editor.set_editor_state(&parsed, None).unwrap();
    let (is_custom, node_type) = editor
        .read(|| {
            let paragraph = get_root().get_first_child().unwrap();
            (
                paragraph.get_latest().is::<CustomParagraphNode>(),
                paragraph.get_type(),
            )
        })
        .unwrap();
    assert!(is_custom);
    assert_eq!(node_type, "custom-paragraph");
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > node replacement > applies the replacement when deserializing a base node type via parseEditorState (#8640 regression)
#[test]
fn node_replacement_applies_when_deserializing_a_base_node_type() {
    let (editor, _) = create_replacement_editor(paragraph_replacement::<CustomParagraphNode>(true));
    expect_custom_paragraph_after_parse(&editor);
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > node replacement > $create resolves withKlass directly, and still honors a `with` without one
#[test]
fn node_replacement_create_resolves_with_klass_and_honors_a_with_without_one() {
    use crate::nodes::paragraph::ParagraphNode;
    let (resolved, _) = create_replacement_editor(paragraph_replacement::<ResolvedParagraph>(true));
    assert!(update_with(&resolved, || Ok(
        crate::create::<ParagraphNode>()
            .get_latest()
            .is::<ResolvedParagraph>()
    )));
    let (legacy, warnings) =
        create_replacement_editor(paragraph_replacement::<LegacyParagraph>(false));
    assert_eq!(warnings, [replace_without_with_klass("ParagraphNode")]);
    assert!(update_with(&legacy, || Ok(crate::create::<ParagraphNode>(
    )
    .get_latest()
    .is::<LegacyParagraph>())));
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > node replacement > applies the replacement on import for `with` without `withKlass`
#[test]
fn node_replacement_applies_on_import_for_with_without_with_klass() {
    let (editor, warnings) =
        create_replacement_editor(paragraph_replacement::<CustomParagraphNode>(false));
    assert_eq!(warnings, [replace_without_with_klass("ParagraphNode")]);
    expect_custom_paragraph_after_parse(&editor);
}

// HTML config.

/// `createTestEditor({html})` on a root element.
#[cfg(feature = "html")]
fn html_config_editor(html: crate::lexical_dom::HtmlConfig) -> Editor {
    let editor = create_test_editor(CreateEditorArgs {
        html,
        ..CreateEditorArgs::default()
    });
    set_test_root_element(&editor);
    editor
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > html config > should override export output function
#[cfg(feature = "html")]
#[test]
fn html_config_should_override_export_output_function() {
    use crate::lexical_dom::{DomExportOutput, HtmlConfig};
    let editor = html_config_editor(HtmlConfig {
        export: vec![(
            "text",
            Rc::new(|_, _, target: &NodeRef| {
                assert!(target.is_text());
                let tag = if target.has_format("bold") {
                    "bor"
                } else {
                    "foo"
                };
                Ok(DomExportOutput::element(
                    crate::dom::get_document().create_element(tag),
                ))
            }),
        )],
        ..HtmlConfig::default()
    });
    let e = editor.clone();
    update_async(&editor, move || {
        let paragraph = create_paragraph_node();
        let text = create_text_node("");
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.append(std::slice::from_ref(&text))?;
        let selection = crate::create_node_selection();
        selection.add(text.get_key());
        assert_eq!(
            crate::html::generate_html_from_nodes(&e, Some(&selection))?,
            "<foo></foo>"
        );
        text.toggle_format("bold");
        assert_eq!(
            crate::html::generate_html_from_nodes(&e, Some(&selection))?,
            "<bor></bor>"
        );
        Ok(())
    })
    .unwrap();
}

// upstream: lexical/src/__tests__/unit/LexicalEditor.test.tsx::LexicalEditor tests > html config > should override import conversion function
#[cfg(feature = "html")]
#[test]
fn html_config_should_override_import_conversion_function() {
    use crate::lexical_dom::{DomConversionOutput, HtmlConfig, importer};
    let editor = html_config_editor(HtmlConfig {
        import: vec![(
            "figure".to_owned(),
            importer(4, |_| {
                Ok(Some(DomConversionOutput::node(Some(create_text_node(
                    "yolo",
                )))))
            }),
        )],
        ..HtmlConfig::default()
    });
    let e = editor.clone();
    update_async(&editor, move || {
        let dom = crate::dom::parse_from_string("<figure></figure>");
        let node = crate::html::generate_nodes_from_dom(&e, &dom)?[0].clone();
        let latest = node.get_latest();
        let (base, text) = (latest.base(), latest.text().unwrap());
        assert_eq!(
            (
                text.detail,
                text.format,
                base.key.as_str(),
                text.mode,
                &base.next,
                &base.parent,
                &base.prev,
                text.style.as_str(),
                text.text.as_str(),
                latest.get_type(),
            ),
            (
                0.0,
                0.0,
                node.get_key(),
                0,
                &None,
                &None,
                &None,
                "",
                "yolo",
                "text",
            )
        );
        Ok(())
    })
    .unwrap();
}
