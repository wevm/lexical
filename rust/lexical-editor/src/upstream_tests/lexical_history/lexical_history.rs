//! Ports of `packages/lexical-history/src/__tests__/unit/LexicalHistory.test.tsx`.
//!
//! Upstream's first tests render `<TestComposer>` with `RichTextPlugin` and
//! `HistoryPlugin` (whose delay is 1000 ms); these build the same editor from
//! `RichTextExtension` and `HistoryExtension`, mounted, as Swift does. Where
//! upstream reads a mounted editor's HTML, these read the committed document
//! ([`shape`]). The "shared HistoryState across editors" test is ported in
//! `history.rs`.

use crate::extension::{
    LexicalEditorWithDispose, build_editor_from_extensions, get_extension_dependency_from_editor,
};
use crate::history::{
    HistoryConfigOverrides, HistoryExtensionOutput, HistoryNow, HistoryStateEntry, HistoryStateRef,
    create_empty_history_state, history_extension, register_history, shared_history_extension,
};
use crate::upstream_tests::support::{set_test_root_element, shape};
use crate::{
    AnyLexicalExtensionArgument, CAN_REDO_COMMAND, CAN_UNDO_COMMAND, CLEAR_HISTORY_COMMAND,
    COMMAND_PRIORITY_CRITICAL, CreateEditorArgs, Editor, EditorFocusOptions, HISTORY_MERGE_TAG,
    LexicalError, LexicalExtension, LexicalNode, NodeBase, NodeClass, NodeRef, PointType,
    REDO_COMMAND, UNDO_COMMAND, UpdateOptions, Which, config_extension, create_node_selection,
    create_paragraph_node, create_range_selection, create_text_node, define_extension, get_root,
    select_all, set_selection,
};
use std::cell::Cell;
use std::rc::Rc;

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

/// `editor.update(fn, {discrete: true})`, on any editor.
fn update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
    editor.update(f, discrete()).unwrap();
}

/// `$createParagraphNodeWithText(text)`.
fn create_paragraph_node_with_text(text: &str) -> Result<NodeRef, LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node(text)])?;
    Ok(paragraph)
}

/// `$appendParagraph(text)`.
fn append_paragraph(text: &str) -> Result<(), LexicalError> {
    get_root().append(&[create_paragraph_node_with_text(text)?])
}

fn history_output(editor: &Editor) -> Rc<HistoryExtensionOutput> {
    get_extension_dependency_from_editor(editor, &history_extension())
        .unwrap()
        .output
        .unwrap()
}

fn history_config(overrides: HistoryConfigOverrides) -> AnyLexicalExtensionArgument {
    config_extension(&history_extension(), [overrides])
}

fn build(name: &str, dependencies: Vec<AnyLexicalExtensionArgument>) -> LexicalEditorWithDispose {
    build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap()
}

/// `CustomTextNode` (`custom-text`): a `TextNode` with a set of classes,
/// exported as `classes`.
#[derive(Clone, Debug, Default)]
struct CustomTextNode {
    base: NodeBase,
    text: crate::nodes::text::TextData,
    classes: Vec<String>,
}

impl LexicalNode for CustomTextNode {
    fn get_type(&self) -> &'static str {
        "custom-text"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn text(&self) -> Option<&crate::nodes::text::TextData> {
        Some(&self.text)
    }
    fn text_mut(&mut self) -> Option<&mut crate::nodes::text::TextData> {
        Some(&mut self.text)
    }
    fn export_json(&self, compact: bool) -> crate::json::Object {
        let mut json =
            crate::schema::export_node_json_once(crate::node::NodeClone::as_node(self), compact);
        json.insert(
            "classes",
            crate::json::Value::Array(self.classes.iter().map(|c| c.as_str().into()).collect()),
        );
        json
    }
}

/// `$createCustomTextNode(text)`.
fn create_custom_text_node(text: &str) -> NodeRef {
    let node = CustomTextNode {
        text: crate::nodes::text::TextData {
            text: text.to_owned(),
            ..Default::default()
        },
        ..CustomTextNode::default()
    };
    crate::lexical_utils::replaced(crate::lexical_utils::set_node_key(Box::new(node)))
}

/// `customTextNode.addClass(className)`.
fn add_class(node: &NodeRef, class_name: &str) {
    crate::lexical_utils::with_writable(node.get_key(), |n| {
        let n = n.as_any_mut().downcast_mut::<CustomTextNode>().unwrap();
        if !n.classes.iter().any(|c| c == class_name) {
            n.classes.push(class_name.to_owned());
        }
    });
}

/// `<Test />`: `RichTextPlugin` and `HistoryPlugin` in a `TestComposer`,
/// rendered into a container.
fn render_test() -> LexicalEditorWithDispose {
    let editor = build(
        "test",
        vec![
            crate::rich_text::rich_text_extension().into(),
            history_config(HistoryConfigOverrides {
                delay: Some(1000.0),
                ..Default::default()
            }),
        ],
    );
    set_test_root_element(&editor);
    editor
}

/// The `CAN_REDO_COMMAND` and `CAN_UNDO_COMMAND` listeners, recording
/// `(canRedo, canUndo)`.
fn register_can_flags(editor: &Editor, initial: bool) -> Rc<(Cell<bool>, Cell<bool>)> {
    let flags = Rc::new((Cell::new(initial), Cell::new(initial)));
    let heard = flags.clone();
    let _ = editor.register_command(
        &CAN_REDO_COMMAND,
        move |payload, _| {
            heard.0.set(*payload);
            Ok(false)
        },
        COMMAND_PRIORITY_CRITICAL,
    );
    let heard = flags.clone();
    let _ = editor.register_command(
        &CAN_UNDO_COMMAND,
        move |payload, _| {
            heard.1.set(*payload);
            Ok(false)
        },
        COMMAND_PRIORITY_CRITICAL,
    );
    flags
}

/// `editor.update(() => editor.dispatchCommand(UNDO_COMMAND or
/// REDO_COMMAND), {discrete: true})`.
fn dispatch_in_update(editor: &Editor, undo: bool) {
    let inner = editor.clone();
    update(editor, move || {
        let command = if undo { &UNDO_COMMAND } else { &REDO_COMMAND };
        inner.dispatch_command(command, ())?;
        Ok(())
    });
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::LexicalHistory tests > LexicalHistory after clearing
#[cfg(feature = "rich-text")]
#[test]
fn lexical_history_after_clearing() {
    let editor = render_test();
    let flags = register_can_flags(&editor, true);
    editor.dispatch_command(&CLEAR_HISTORY_COMMAND, ()).unwrap();
    assert!(!flags.0.get());
    assert!(!flags.1.get());
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::LexicalHistory tests > LexicalHistory.Redo after Quote Node
#[cfg(feature = "rich-text")]
#[test]
fn lexical_history_redo_after_quote_node() {
    let editor = render_test();
    update(&editor, || {
        let root = get_root();
        let paragraph1 = create_paragraph_node_with_text("AAA")?;
        let paragraph2 = create_paragraph_node_with_text("BBB")?;
        // The editor has one child that is an empty
        // paragraph Node.
        if let Some(child) = root.get_child_at_index(0) {
            child.replace(&paragraph1, false)?;
        }
        root.append(&[paragraph2])
    });
    let initial_json_state = editor.get_editor_state().to_json(false);

    update(&editor, || {
        let root = get_root();
        let selection = create_range_selection();
        let first_text_node = root.get_all_text_nodes()[0].clone();
        selection.point_set(Which::Anchor, first_text_node.get_key(), 0, PointType::Text)?;
        selection.point_set(Which::Focus, first_text_node.get_key(), 3, PointType::Text)?;
        set_selection(Some(selection.clone()));
        crate::selection::set_blocks_type(
            Some(&selection),
            || crate::rich_text::create_quote_node(false),
            None,
        )
    });

    let after = editor.get_editor_state().to_json(false);
    let children = after
        .get("root")
        .and_then(|r| r.get("children"))
        .and_then(|c| c.as_array())
        .unwrap();
    assert_eq!(children.len(), 2);
    assert_eq!(children[0].get("type"), Some(&"quote".into()));
    let quote_children = children[0].get("children").unwrap().as_array().unwrap();
    assert_eq!(quote_children.len(), 1);
    assert_eq!(quote_children[0].get("type"), Some(&"text".into()));
    assert_eq!(quote_children[0].get("text"), Some(&"AAA".into()));

    dispatch_in_update(&editor, true);

    assert_eq!(
        initial_json_state.to_string(),
        editor.get_editor_state().to_json(false).to_string()
    );
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::LexicalHistory tests > LexicalHistory in sequence: change, undo, redo, undo, change
#[cfg(feature = "rich-text")]
#[test]
fn lexical_history_in_sequence_change_undo_redo_undo_change() {
    let editor = render_test();
    let flags = register_can_flags(&editor, false);
    let (can_redo, can_undo) = (|| flags.0.get(), || flags.1.get());

    // focus (needs the focus to initialize)
    editor.focus(None, EditorFocusOptions::default()).unwrap();
    assert!(!can_redo());
    assert!(!can_undo());

    // change
    update(&editor, || append_paragraph("foo"));
    assert!(!can_redo());
    assert!(can_undo());

    // undo
    dispatch_in_update(&editor, true);
    assert!(can_redo());
    assert!(!can_undo());

    // redo
    dispatch_in_update(&editor, false);
    assert!(!can_redo());
    assert!(can_undo());

    // undo
    dispatch_in_update(&editor, true);
    assert!(can_redo());
    assert!(!can_undo());

    // change
    update(&editor, || append_paragraph("foo"));
    assert!(!can_redo());
    assert!(can_undo());
}

/// `createTestEditor({namespace})`.
fn create_test_editor_in(namespace: &str, parent_editor: Option<Editor>) -> Editor {
    crate::upstream_tests::support::create_test_editor(CreateEditorArgs {
        namespace: namespace.to_owned(),
        parent_editor,
        ..CreateEditorArgs::default()
    })
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::LexicalHistory tests > undoStack selection points to the same editor
#[test]
fn undo_stack_selection_points_to_the_same_editor() {
    let editor_ = create_test_editor_in("parent", None);
    let shared_history = create_empty_history_state();
    let _unregister = register_history(&editor_, &shared_history, 1000.0, None, None, None);
    update(&editor_, || get_root().append(&[create_paragraph_node()]));
    update(&editor_, || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        let node_selection = create_node_selection();
        node_selection.add(paragraph.get_key());
        set_selection(Some(node_selection));
        Ok(())
    });
    // Upstream assigns `nestedEditor._parentEditor = editor_` after the
    // nested editor's first update; a Rust editor gets its parent when it
    // is created, and nothing in that first update reads it.
    let nested_editor = create_test_editor_in("nested", Some(editor_.clone()));
    nested_editor
        .update(
            || {
                let paragraph = create_paragraph_node();
                get_root().append(std::slice::from_ref(&paragraph))?;
                paragraph.select_end()?;
                Ok(())
            },
            UpdateOptions {
                discrete: true,
                tags: vec![HISTORY_MERGE_TAG.to_owned()],
                ..UpdateOptions::default()
            },
        )
        .unwrap();
    let _unregister_nested =
        register_history(&nested_editor, &shared_history, 1000.0, None, None, None);

    update(&nested_editor, || {
        let paragraph = create_paragraph_node();
        get_root().append(std::slice::from_ref(&paragraph))?;
        paragraph.select_end()?;
        Ok(())
    });

    assert_eq!(shared_history.borrow().undo_stack.len(), 2);
    editor_.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert!(
        editor_
            .get_editor_state()
            .selection()
            .is_some_and(|s| s.is_node())
    );
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::LexicalHistory tests > Changes to TextNode leaf are detected properly #6409
#[cfg(feature = "utils")]
#[test]
fn changes_to_text_node_leaf_are_detected_properly() {
    let editor = crate::upstream_tests::support::create_test_editor(CreateEditorArgs {
        nodes: vec![NodeClass::of::<CustomTextNode>()],
        ..CreateEditorArgs::default()
    });
    let shared_history = create_empty_history_state();
    let _unregister = register_history(&editor, &shared_history, 0.0, None, None, None);
    update(&editor, || {
        let root = get_root();
        root.clear()?;
        let paragraph = create_paragraph_node();
        paragraph.append(&[create_custom_text_node("Initial text")])?;
        root.append(&[paragraph])
    });
    assert_eq!(shared_history.borrow().undo_stack.len(), 0);

    let inner = editor.clone();
    update(&editor, move || {
        // Mark dirty with no changes
        for node in get_root().get_all_text_nodes() {
            crate::lexical_utils::get_writable(node.get_key());
        }
        // Restore the editor state and ensure the history did not change
        crate::utils::restore_editor_state(&inner, &inner.get_editor_state());
        Ok(())
    });
    assert_eq!(shared_history.borrow().undo_stack.len(), 0);
    update(&editor, || {
        // Mark dirty with text change
        for node in get_root().get_all_text_nodes() {
            if node.get_type() == "custom-text" {
                node.set_text_content(&(node.get_text_content() + "!"));
            }
        }
        Ok(())
    });
    assert_eq!(shared_history.borrow().undo_stack.len(), 1);

    update(&editor, || {
        // Mark dirty with only a change to the class
        for node in get_root().get_all_text_nodes() {
            if node.get_type() == "custom-text" {
                add_class(&node, "updated");
            }
        }
        Ok(())
    });
    assert_eq!(shared_history.borrow().undo_stack.len(), 2);
}

/// `buildEditor()`: `HistoryExtension` with `{delay: 0}`.
fn build_editor() -> LexicalEditorWithDispose {
    build(
        "test",
        vec![history_config(HistoryConfigOverrides {
            delay: Some(0.0),
            ..Default::default()
        })],
    )
}

/// `buildEditorWithHistory(historyState)`.
fn build_editor_with_history(
    history_state: crate::history::SharedHistoryState,
) -> LexicalEditorWithDispose {
    build(
        "test",
        vec![history_config(HistoryConfigOverrides {
            create_initial_history_state: Some(crate::history::CreateInitialHistoryState(Rc::new(
                move |_| history_state.clone(),
            ))),
            delay: Some(0.0),
            ..Default::default()
        })],
    )
}

// Two updates are required to populate the undoStack: the first update sets
// `historyState.current`, the second pushes the previous `current` onto the
// stack and dispatches CAN_UNDO_COMMAND.
fn make_editor_with_one_undo_entry() -> LexicalEditorWithDispose {
    let editor = build_editor();
    update(&editor, || append_paragraph("first"));
    update(&editor, || append_paragraph("second"));
    editor
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > signals start as false
#[test]
fn signals_start_as_false() {
    let editor = build_editor();
    let output = history_output(&editor);
    assert!(!output.can_undo.peek());
    assert!(!output.can_redo.peek());
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > canUndo becomes true after a push, canRedo stays false
#[test]
fn can_undo_becomes_true_after_a_push_can_redo_stays_false() {
    let editor = make_editor_with_one_undo_entry();
    let output = history_output(&editor);
    assert!(output.can_undo.peek());
    assert!(!output.can_redo.peek());
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > canRedo becomes true after undo, canUndo goes false
#[test]
fn can_redo_becomes_true_after_undo_can_undo_goes_false() {
    let editor = make_editor_with_one_undo_entry();
    let output = history_output(&editor);
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert!(!output.can_undo.peek());
    assert!(output.can_redo.peek());
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > canRedo clears after redo, canUndo returns true
#[test]
fn can_redo_clears_after_redo_can_undo_returns_true() {
    let editor = make_editor_with_one_undo_entry();
    let output = history_output(&editor);
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    editor.dispatch_command(&REDO_COMMAND, ()).unwrap();
    assert!(output.can_undo.peek());
    assert!(!output.can_redo.peek());
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > signals reset to false after CLEAR_HISTORY_COMMAND
#[test]
fn signals_reset_to_false_after_clear_history_command() {
    let editor = make_editor_with_one_undo_entry();
    let output = history_output(&editor);
    assert!(output.can_undo.peek());
    editor.dispatch_command(&CLEAR_HISTORY_COMMAND, ()).unwrap();
    assert!(!output.can_undo.peek());
    assert!(!output.can_redo.peek());
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > canRedo clears when a new edit is made after undo
#[test]
fn can_redo_clears_when_a_new_edit_is_made_after_undo() {
    let editor = make_editor_with_one_undo_entry();
    let output = history_output(&editor);
    // Wrap UNDO dispatch in editor.update so that the HISTORIC_TAG from
    // undo's setEditorState does not leak into the subsequent edit.
    dispatch_in_update(&editor, true);
    assert!(output.can_redo.peek());
    update(&editor, || append_paragraph("third"));
    assert!(!output.can_redo.peek());
    assert!(output.can_undo.peek());
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > canUndo is true immediately when initialized with a non-empty undoStack
#[test]
fn can_undo_is_true_immediately_when_initialized_with_a_non_empty_undo_stack() {
    let donor = make_editor_with_one_undo_entry();
    let donor_history = history_output(&donor).history_state.peek().0;
    assert!(!donor_history.borrow().undo_stack.is_empty());

    let editor = build_editor_with_history(donor_history);
    let output = history_output(&editor);
    assert!(output.can_undo.peek());
    assert!(!output.can_redo.peek());
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > canRedo is true immediately when initialized with a non-empty redoStack
#[test]
fn can_redo_is_true_immediately_when_initialized_with_a_non_empty_redo_stack() {
    let donor = make_editor_with_one_undo_entry();
    donor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    let donor_history = history_output(&donor).history_state.peek().0;
    assert!(!donor_history.borrow().redo_stack.is_empty());

    let editor = build_editor_with_history(donor_history);
    let output = history_output(&editor);
    assert!(!output.can_undo.peek());
    assert!(output.can_redo.peek());
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension canUndo/canRedo signals > signals update when historyState signal is reassigned to a populated state
#[test]
fn signals_update_when_history_state_signal_is_reassigned_to_a_populated_state() {
    // Simulates what SharedHistoryExtension does when it inherits parent state.
    let editor = build_editor();
    let output = history_output(&editor);
    assert!(!output.can_undo.peek());

    let populated = create_empty_history_state();
    populated.borrow_mut().undo_stack.push(HistoryStateEntry {
        editor: (*editor).clone(),
        editor_state: editor.get_editor_state(),
    });

    output.history_state.set_value(HistoryStateRef(populated));
    assert!(output.can_undo.peek());
    assert!(!output.can_redo.peek());
}

/// `buildEditorWithMaxDepth(maxDepth)`.
fn build_editor_with_max_depth(max_depth: Option<usize>) -> LexicalEditorWithDispose {
    build(
        "test",
        vec![history_config(HistoryConfigOverrides {
            delay: Some(0.0),
            max_depth: Some(max_depth),
            ..Default::default()
        })],
    )
}

// Each editor.update with discrete: true is a separate history event when
// delay is 0 — the merge window is closed.  The first update populates
// historyState.current; the second pushes it onto undoStack; from then
// on every additional update adds one entry.  So `n` updates produce
// `n - 1` undoStack entries.
fn push_n_history_events(editor: &Editor, n: usize) {
    for i in 0..n {
        update(editor, move || append_paragraph(&format!("p{i}")));
    }
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension maxDepth > defaults to null (unbounded) and matches the historical behavior
#[test]
fn defaults_to_null_unbounded_and_matches_the_historical_behavior() {
    let editor = build_editor_with_max_depth(None);
    push_n_history_events(&editor, 25);
    let output = history_output(&editor);
    assert_eq!(output.max_depth.peek(), None);
    assert_eq!(output.history_state.peek().0.borrow().undo_stack.len(), 24);
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension maxDepth > caps the undoStack to maxDepth via FIFO eviction
#[test]
fn caps_the_undo_stack_to_max_depth_via_fifo_eviction() {
    let editor = build_editor_with_max_depth(Some(5));
    push_n_history_events(&editor, 20);
    let output = history_output(&editor);
    assert_eq!(output.history_state.peek().0.borrow().undo_stack.len(), 5);
    // canUndo stays true once any entry exists.
    assert!(output.can_undo.peek());
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::HistoryExtension maxDepth > reactively respects a maxDepth signal update for future events
#[test]
fn reactively_respects_a_max_depth_signal_update_for_future_events() {
    let editor = build_editor_with_max_depth(None);
    push_n_history_events(&editor, 8); // 7 entries
    let output = history_output(&editor);
    let undo_len = || output.history_state.peek().0.borrow().undo_stack.len();
    assert_eq!(undo_len(), 7);

    // Lowering maxDepth does not retroactively trim — only future pushes
    // observe the new cap, at which point the stack settles to that length.
    output.max_depth.set_value(Some(3));
    assert_eq!(undo_len(), 7);

    push_n_history_events(&editor, 5); // 5 more events trigger trimming
    assert_eq!(undo_len(), 3);
}

/// `EditorKey`: the child editor a [`ChildEditorNode`] holds, never
/// serialized (it parses to `null`).
fn editor_key() -> crate::StateConfig<Option<Rc<LexicalEditorWithDispose>>> {
    thread_local! {
        static EDITOR_KEY: crate::StateConfig<Option<Rc<LexicalEditorWithDispose>>> =
            crate::create_state_with(
                "editor",
                crate::StateValueConfig {
                    parse: Rc::new(|_| None),
                    unparse: Rc::new(|_| crate::json::Value::Null),
                    is_equal: Rc::new(|a: &Option<Rc<LexicalEditorWithDispose>>, b: &Option<Rc<LexicalEditorWithDispose>>| match (a, b) {
                        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                        (None, None) => true,
                        _ => false,
                    }),
                    reset_on_copy_node: false,
                    derived: false,
                    schema: None,
                },
            );
    }
    EDITOR_KEY.with(Clone::clone)
}

/// `ChildEditorNode` (`child-editor`): a block decorator holding a child
/// editor in its NodeState.
#[derive(Clone, Debug, Default)]
struct ChildEditorNode {
    base: NodeBase,
}

impl LexicalNode for ChildEditorNode {
    fn get_type(&self) -> &'static str {
        "child-editor"
    }
    fn base(&self) -> &NodeBase {
        &self.base
    }
    fn base_mut(&mut self) -> &mut NodeBase {
        &mut self.base
    }
    fn is_decorator(&self) -> bool {
        true
    }
    fn is_inline(&self) -> bool {
        false
    }
}

/// `childEditorNode.getOrResetEditor()`.
fn get_or_reset_editor(node: &NodeRef) -> Result<Rc<LexicalEditorWithDispose>, LexicalError> {
    if let Some(prev_editor) = crate::get_state(node, &editor_key())? {
        return Ok(prev_editor);
    }
    let editor = Rc::new(build(
        "ChildEditorNode",
        vec![
            shared_history_extension().into(),
            crate::extension::nested_editor_extension().into(),
        ],
    ));
    crate::set_state(node, &editor_key(), Some(editor.clone()))?;
    Ok(editor)
}

// upstream: lexical-history/src/__tests__/unit/LexicalHistory.test.tsx::SharedHistoryExtension > can create a parent editor
#[test]
fn shared_history_extension_can_create_a_parent_editor() {
    // `ChildEditorExtension`; its mutation listener attaches each child
    // editor to its decorator's DOM, which only upstream's HTML reads.
    let child_editor_extension = define_extension(LexicalExtension {
        nodes: vec![NodeClass::of::<ChildEditorNode>()],
        ..LexicalExtension::<(), (), ()>::new("@lexical/test/ChildEditor", ())
    });
    let step = Rc::new(Cell::new(-1.0));
    let artificial_now = move || {
        step.set(step.get() + 1.0);
        1000.0 * step.get()
    };
    let editor = build(
        "parent",
        vec![
            history_config(HistoryConfigOverrides {
                delay: Some(0.0),
                now: Some(HistoryNow(Rc::new(artificial_now))),
                ..Default::default()
            }),
            child_editor_extension.into(),
        ],
    );
    set_test_root_element(&editor);
    update(&editor, || {
        select_all()?.insert_nodes(vec![create_text_node("parent editor")])
    });
    let get_child_editor = || {
        let child = get_root().get_child_at_index(1).unwrap();
        assert_eq!(
            child.get_type(),
            "child-editor",
            "Expecting ChildEditorNode"
        );
        get_or_reset_editor(&child).unwrap()
    };
    let child_text = |editor: &Editor| {
        let child = editor.read(get_child_editor).unwrap();
        child.read(|| get_root().get_text_content()).unwrap()
    };
    assert_eq!(shape(&editor), "<paragraph>[parent editor]</paragraph>");
    update(&editor, || {
        let child = crate::create::<ChildEditorNode>();
        get_or_reset_editor(&child)?.update(
            || {
                select_all()?.insert_text("Child editor")?;
                set_selection(None);
                Ok(())
            },
            discrete(),
        )?;
        get_root().select_end()?.insert_nodes(vec![child])
    });
    // insertNodes at the end of the paragraph no longer leaves a stray empty
    // paragraph after the block decorator (#9095).
    assert_eq!(
        shape(&editor),
        "<paragraph>[parent editor]</paragraph><child-editor/>"
    );
    assert_eq!(child_text(&editor), "Child editor");
    editor
        .read(|| {
            get_child_editor().update(
                || {
                    get_root().select_end()?.insert_text(". Updated!")?;
                    set_selection(None);
                    Ok(())
                },
                discrete(),
            )
        })
        .unwrap()
        .unwrap();
    assert_eq!(
        shape(&editor),
        "<paragraph>[parent editor]</paragraph><child-editor/>"
    );
    assert_eq!(child_text(&editor), "Child editor. Updated!");
    let child_editor = editor.read(get_child_editor).unwrap();
    assert!(
        history_output(&child_editor).history_state.peek()
            == history_output(&editor).history_state.peek()
    );

    // The last change was made in the child editor, so a single undo reverts
    // it and leaves the parent alone.
    editor.dispatch_command(&UNDO_COMMAND, ()).unwrap();
    assert_eq!(child_text(&editor), "Child editor");
    assert_eq!(
        shape(&editor),
        "<paragraph>[parent editor]</paragraph><child-editor/>"
    );
    dispatch_in_update(&editor, true);
    assert_eq!(shape(&editor), "<paragraph>[parent editor]</paragraph>");
}
