//! Ports of `packages/lexical/src/__tests__/unit/LexicalReconciler.test.ts`,
//! its model half: the root's cached text (`__cachedText`, and the root
//! DOM's `__lexicalTextContent`, which Rust keeps as the same cache), the
//! `__textFormat` an element takes from its first text, the first text key
//! the reconciler keeps on a keyed DOM (`__lexicalFirstTextKey`, on the
//! reconciler's [`ReconciledNode`](crate::reconciler)) and the mutations it
//! reports, on editors with a no-op root element. The tests of the rendered
//! DOM are skipped in `rust/parity/skips.json`.

#![cfg(all(feature = "extension", feature = "rich-text", feature = "link"))]

use crate::extension::{LexicalEditorWithDispose, build_editor_from_extensions};
use crate::link::{LinkAttributes, create_link_node, link_extension};
use crate::nodes::element::ElementData;
use crate::nodes::line_break::create_line_break_node;
use crate::upstream_tests::support::{
    create_test_decorator_node, create_test_element_node, initialize_unit_test,
    set_test_root_element, update,
};
use crate::{
    CreateEditorArgs, Editor, EditorReadMode, LexicalExtension, LexicalNode,
    MutationListenerOptions, NodeBase, NodeClass, NodeKey, NodeMutation, NodeRef, create,
    create_paragraph_node, create_text_node, define_extension, get_node_by_key, get_root,
};
use std::cell::RefCell;
use std::rc::Rc;

const TEXT_FORMAT_BOLD: f64 = 1.0;

/// `buildEditorFromExtensions(RichTextExtension, [LinkExtension,]
/// defineExtension({name, nodes}))` on a root element.
fn build(name: &str, link: bool, nodes: Vec<NodeClass>) -> LexicalEditorWithDispose {
    let mut dependencies = vec![crate::rich_text::rich_text_extension().into()];
    if link {
        dependencies.push(link_extension().into());
    }
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        dependencies,
        nodes,
        ..LexicalExtension::<(), (), ()>::new(name, ())
    })
    .into()])
    .unwrap();
    set_test_root_element(&editor);
    editor
}

/// `createReconcilerEditor()`.
fn create_reconciler_editor() -> LexicalEditorWithDispose {
    build("reconciler-suffix-test", false, Vec::new())
}

/// `createReconcilerEditorWithLink()`.
fn create_reconciler_editor_with_link() -> LexicalEditorWithDispose {
    build("reconciler-suffix-test-with-link", true, Vec::new())
}

/// `$getRoot().__cachedText`: the text the root cached when it last
/// reconciled, if it is the committed root.
fn cached_text(editor: &Editor) -> Option<String> {
    let root = editor
        .read_with_mode(EditorReadMode::Latest, || get_root().get_latest())
        .unwrap();
    let cache = editor.0.root_text_cache.borrow();
    cache
        .as_ref()
        .filter(|(cached, _)| Rc::ptr_eq(cached, &root))
        .map(|(_, text)| text.clone())
}

/// `editor.getElementByKey(key).__lexicalFirstTextKey`.
fn first_text_key(editor: &Editor, key: &str) -> Option<NodeKey> {
    editor
        .0
        .key_to_dom_map
        .borrow()
        .get(key)
        .and_then(|dom| dom.first_text_key())
        .flatten()
}

fn discrete(editor: &Editor, f: impl FnOnce() -> Result<(), crate::LexicalError> + 'static) {
    update(editor, f);
}

fn read<R>(editor: &Editor, f: impl FnOnce() -> R) -> R {
    editor.read(f).unwrap()
}

/// `$createTextNode(text).toggleFormat(format)`.
fn formatted(text: &str, format: &str) -> NodeRef {
    let node = create_text_node(text);
    node.toggle_format(format);
    node
}

/// `$createLinkNode(url)` holding `text`.
fn link(url: &str, text: NodeRef) -> NodeRef {
    let link = create_link_node(url, &LinkAttributes::default());
    link.append(&[text]).unwrap();
    link
}

fn paragraph_of(children: &[NodeRef]) -> NodeRef {
    let paragraph = create_paragraph_node();
    paragraph.append(children).unwrap();
    paragraph
}

/// A paragraph per text, after clearing the root.
fn paragraphs(texts: &[&str]) -> Result<(), crate::LexicalError> {
    let root = get_root();
    root.clear()?;
    for text in texts {
        root.append(&[paragraph_of(&[create_text_node(text)])])?;
    }
    Ok(())
}

fn first_paragraph() -> NodeRef {
    let para = get_root().get_first_child_or_throw().unwrap();
    assert_eq!(para.get_type(), "paragraph", "para must be a ParagraphNode");
    para
}

fn text_of(element: &NodeRef, last: bool) -> NodeRef {
    let text = if last {
        element.get_last_child_or_throw()
    } else {
        element.get_first_child_or_throw()
    }
    .unwrap();
    assert!(text.is_text(), "text must be a TextNode");
    text
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > Cross-parent moves reuse DOM (regression #8420) > Wrapping decorator emits a single "updated" listener event and re-decorates
#[test]
fn wrapping_decorator_emits_a_single_updated_listener_event_and_re_decorates() {
    let editor = initialize_unit_test(CreateEditorArgs::default());
    let decorator_key = Rc::new(RefCell::new(String::new()));
    let key = decorator_key.clone();
    discrete(&editor, move || {
        let decorator = create_test_decorator_node();
        *key.borrow_mut() = decorator.get_key().to_owned();
        get_root().clear()?;
        get_root().append(&[paragraph_of(&[decorator])])
    });
    // `vi.spyOn(TestDecoratorNode.prototype, 'decorate')`: each call makes
    // a new value, so a call shows as a value that is not the one before.
    let key = decorator_key.borrow().clone();
    let decorated = editor.get_decorators().get(&key).cloned().unwrap();
    let events = Rc::new(RefCell::new(Vec::<(&str, NodeMutation)>::new()));
    let record = |klass: &'static str| {
        let events = events.clone();
        move |nodes: &crate::NodeMutations, _: &crate::MutationListenerPayload| {
            for (_, m) in nodes {
                events.borrow_mut().push((klass, *m));
            }
        }
    };
    let options = || MutationListenerOptions {
        skip_initialization: true,
    };
    let _decorator = editor
        .register_mutation_listener("test_decorator", record("decorator"), options())
        .unwrap();
    let _element = editor
        .register_mutation_listener("test_block", record("element"), options())
        .unwrap();
    discrete(&editor, || {
        let paragraph = get_root().get_first_child().unwrap();
        assert!(paragraph.is_element());
        let decorator = paragraph.get_first_child().unwrap();
        assert!(decorator.is_decorator());
        let wrapper = create_test_element_node();
        decorator.insert_before(&wrapper, true)?;
        wrapper.append(&[decorator])
    });
    let decorator_events: Vec<_> = events
        .borrow()
        .iter()
        .filter(|(klass, _)| *klass == "decorator")
        .copied()
        .collect();
    assert_eq!(decorator_events, [("decorator", NodeMutation::Updated)]);
    let redecorated = editor.get_decorators().get(&key).cloned().unwrap();
    assert!(!Rc::ptr_eq(&decorated, &redecorated));
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > typing at the end of the last paragraph keeps prefix DLB
#[test]
fn typing_at_the_end_of_the_last_paragraph_keeps_prefix_dlb() {
    let editor = create_reconciler_editor();
    discrete(&editor, || paragraphs(&["alpha", "beta", "gamma"]));
    assert_eq!(
        cached_text(&editor).as_deref(),
        Some("alpha\n\nbeta\n\ngamma")
    );
    discrete(&editor, || {
        let last = get_root().get_last_child_or_throw()?;
        assert_eq!(last.get_type(), "paragraph");
        let text = text_of(&last, false);
        text.set_text_content(&format!("{}!", text.get_text_content()));
        Ok(())
    });
    assert_eq!(
        cached_text(&editor).as_deref(),
        Some("alpha\n\nbeta\n\ngamma!")
    );
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > multiple contiguous dirty children at the end
#[test]
fn multiple_contiguous_dirty_children_at_the_end() {
    let editor = create_reconciler_editor();
    discrete(&editor, || paragraphs(&["a", "b", "c", "d"]));
    discrete(&editor, || {
        let root = get_root();
        let c = root.get_child_at_index(2).unwrap();
        let d = root.get_child_at_index(3).unwrap();
        text_of(&c, false).set_text_content("cc");
        text_of(&d, false).set_text_content("dd");
        Ok(())
    });
    assert_eq!(cached_text(&editor).as_deref(), Some("a\n\nb\n\ncc\n\ndd"));
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > non-contiguous dirty children take the existing fast path
#[test]
fn non_contiguous_dirty_children_take_the_existing_fast_path() {
    let editor = create_reconciler_editor();
    discrete(&editor, || paragraphs(&["x", "y", "z"]));
    discrete(&editor, || {
        let root = get_root();
        text_of(&root.get_child_at_index(0).unwrap(), false).set_text_content("xx");
        text_of(&root.get_child_at_index(2).unwrap(), false).set_text_content("zz");
        Ok(())
    });
    assert_eq!(cached_text(&editor).as_deref(), Some("xx\n\ny\n\nzz"));
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > format toggle on the last paragraph propagates to __textFormat
#[test]
fn format_toggle_on_the_last_paragraph_propagates_to_text_format() {
    let editor = create_reconciler_editor();
    discrete(&editor, || paragraphs(&["head", "foot"]));
    discrete(&editor, || {
        let last = get_root().get_last_child_or_throw()?;
        text_of(&last, false).toggle_format("bold");
        Ok(())
    });
    read(&editor, || {
        let root = get_root();
        let head = root.get_first_child_or_throw().unwrap();
        let foot = root.get_last_child_or_throw().unwrap();
        assert_eq!(head.element_data().get_text_format(), 0.0);
        assert_ne!(foot.element_data().get_text_format(), 0.0);
    });
    assert_eq!(cached_text(&editor).as_deref(), Some("head\n\nfoot"));
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > empty trailing paragraph contributes zero length
#[test]
fn empty_trailing_paragraph_contributes_zero_length() {
    let editor = create_reconciler_editor();
    discrete(&editor, || {
        get_root().clear()?;
        get_root().append(&[
            paragraph_of(&[create_text_node("hello")]),
            create_paragraph_node(),
        ])
    });
    assert_eq!(cached_text(&editor).as_deref(), Some("hello\n\n"));
    discrete(&editor, || {
        get_root()
            .get_last_child_or_throw()?
            .append(&[create_text_node("world")])
    });
    assert_eq!(cached_text(&editor).as_deref(), Some("hello\n\nworld"));
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > linebreak-bounded text nodes update suffix without extra DLB
#[test]
fn linebreak_bounded_text_nodes_update_suffix_without_extra_dlb() {
    let editor = create_reconciler_editor();
    discrete(&editor, || {
        get_root().clear()?;
        get_root().append(&[paragraph_of(&[
            create_text_node("top"),
            create_line_break_node(),
            create_text_node("bottom"),
        ])])
    });
    assert_eq!(cached_text(&editor).as_deref(), Some("top\nbottom"));
    discrete(&editor, || {
        text_of(&first_paragraph(), true).set_text_content("BOTTOM!");
        Ok(())
    });
    assert_eq!(cached_text(&editor).as_deref(), Some("top\nBOTTOM!"));
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > TextNode-direct-child suffix with length change: prefix preserved
#[test]
fn text_node_direct_child_suffix_with_length_change_prefix_preserved() {
    let editor = create_reconciler_editor();
    discrete(&editor, || {
        get_root().clear()?;
        get_root().append(&[paragraph_of(&[
            create_text_node("hello "),
            formatted("world", "bold"),
        ])])
    });
    assert_eq!(cached_text(&editor).as_deref(), Some("hello world"));
    discrete(&editor, || {
        text_of(&first_paragraph(), true).set_text_content("world!!");
        Ok(())
    });
    assert_eq!(cached_text(&editor).as_deref(), Some("hello world!!"));
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > append paragraph at end of multi-paragraph root (size+1, K=2)
#[test]
fn append_paragraph_at_end_of_multi_paragraph_root() {
    let editor = create_reconciler_editor();
    discrete(&editor, || paragraphs(&["a", "b", "c", "d", "e"]));
    assert_eq!(
        cached_text(&editor).as_deref(),
        Some("a\n\nb\n\nc\n\nd\n\ne")
    );
    discrete(&editor, || {
        get_root().append(&[paragraph_of(&[create_text_node("f")])])
    });
    assert_eq!(
        cached_text(&editor).as_deref(),
        Some("a\n\nb\n\nc\n\nd\n\ne\n\nf")
    );
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > remove last paragraph of multi-paragraph root (size-1, K=1)
#[test]
fn remove_last_paragraph_of_multi_paragraph_root() {
    let editor = create_reconciler_editor();
    discrete(&editor, || paragraphs(&["a", "b", "c", "d", "e"]));
    assert_eq!(
        cached_text(&editor).as_deref(),
        Some("a\n\nb\n\nc\n\nd\n\ne")
    );
    discrete(&editor, || {
        get_root().get_last_child_or_throw()?.remove(false)
    });
    assert_eq!(cached_text(&editor).as_deref(), Some("a\n\nb\n\nc\n\nd"));
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > sustained typing on the same paragraph stays correct (cache freshness)
#[test]
fn sustained_typing_on_the_same_paragraph_stays_correct() {
    let editor = create_reconciler_editor();
    let text_key = Rc::new(RefCell::new(String::new()));
    let key = text_key.clone();
    discrete(&editor, move || {
        paragraphs(&["alpha", "beta"])?;
        let text = create_text_node("x");
        *key.borrow_mut() = text.get_key().to_owned();
        get_root().append(&[paragraph_of(&[text])])
    });
    assert_eq!(cached_text(&editor).as_deref(), Some("alpha\n\nbeta\n\nx"));
    for next in ["xy", "xyz", "xyzz"] {
        let key = text_key.borrow().clone();
        discrete(&editor, move || {
            let text = get_node_by_key(&key).unwrap();
            assert!(text.is_text());
            text.set_text_content(next);
            Ok(())
        });
        assert_eq!(
            cached_text(&editor),
            Some(format!("alpha\n\nbeta\n\n{next}"))
        );
    }
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > sustained appends stay correct across cycles (cache freshness, size+1)
#[test]
fn sustained_appends_stay_correct_across_cycles() {
    let editor = create_reconciler_editor();
    discrete(&editor, || paragraphs(&["a", "b", "c", "d"]));
    let mut expected_text = "a\n\nb\n\nc\n\nd".to_owned();
    assert_eq!(cached_text(&editor), Some(expected_text.clone()));
    for label in ["e", "f", "g", "h", "i"] {
        discrete(&editor, move || {
            get_root().append(&[paragraph_of(&[create_text_node(label)])])
        });
        expected_text.push_str(&format!("\n\n{label}"));
        assert_eq!(cached_text(&editor), Some(expected_text.clone()));
    }
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > sustained removes stay correct across cycles (cache freshness, size-1)
#[test]
fn sustained_removes_stay_correct_across_cycles() {
    let editor = create_reconciler_editor();
    let mut remaining = vec!["a", "b", "c", "d", "e", "f", "g"];
    let all = remaining.clone();
    discrete(&editor, move || paragraphs(&all));
    assert_eq!(cached_text(&editor), Some(remaining.join("\n\n")));
    for _ in 0..3 {
        discrete(&editor, || {
            get_root().get_last_child_or_throw()?.remove(false)
        });
        remaining.pop();
        assert_eq!(cached_text(&editor), Some(remaining.join("\n\n")));
    }
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-1: root __textFormat is not propagated from descendants
#[test]
fn audit_1_root_text_format_is_not_propagated_from_descendants() {
    let editor = create_reconciler_editor();
    discrete(&editor, || {
        get_root().clear()?;
        get_root().append(&[
            create_paragraph_node(),
            create_paragraph_node(),
            create_paragraph_node(),
            paragraph_of(&[create_text_node("x")]),
        ])
    });
    assert_eq!(
        read(&editor, || get_root().element_data().get_text_format()),
        0.0
    );
    discrete(&editor, || {
        let last = get_root().get_last_child_or_throw()?;
        text_of(&last, false).toggle_format("bold");
        Ok(())
    });
    read(&editor, || {
        let root = get_root();
        let last = root.get_last_child_or_throw().unwrap();
        assert_ne!(last.element_data().get_text_format(), 0.0);
        assert_eq!(root.element_data().get_text_format(), 0.0);
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-1 control: prefix with text keeps parent __textFormat stable
#[test]
fn audit_1_control_prefix_with_text_keeps_parent_text_format_stable() {
    let editor = create_reconciler_editor();
    discrete(&editor, || paragraphs(&["a", "b", "c", "d"]));
    assert_eq!(
        read(&editor, || get_root().element_data().get_text_format()),
        0.0
    );
    discrete(&editor, || {
        let last = get_root().get_last_child_or_throw()?;
        text_of(&last, false).toggle_format("bold");
        Ok(())
    });
    read(&editor, || {
        let root = get_root();
        let last = root.get_last_child_or_throw().unwrap();
        assert_ne!(last.element_data().get_text_format(), 0.0);
        assert_eq!(root.element_data().get_text_format(), 0.0);
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-1 (non-root) control: prefix with text keeps paragraph __textFormat stable
#[test]
fn audit_1_non_root_control_prefix_with_text_keeps_paragraph_text_format_stable() {
    let editor = create_reconciler_editor();
    discrete(&editor, || {
        get_root().clear()?;
        get_root().append(&[paragraph_of(&[
            create_text_node("head"),
            create_line_break_node(),
            create_line_break_node(),
            create_text_node("tail"),
        ])])
    });
    assert_eq!(
        read(&editor, || first_paragraph()
            .element_data()
            .get_text_format()),
        0.0
    );
    discrete(&editor, || {
        text_of(&first_paragraph(), true).toggle_format("bold");
        Ok(())
    });
    assert_eq!(
        read(&editor, || first_paragraph()
            .element_data()
            .get_text_format()),
        0.0
    );
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-1 (non-root): paragraph with linebreak-only prefix leaks stale __textFormat
#[test]
fn audit_1_non_root_paragraph_with_linebreak_only_prefix_leaks_stale_text_format() {
    let editor = create_reconciler_editor();
    discrete(&editor, || {
        get_root().clear()?;
        get_root().append(&[paragraph_of(&[
            create_line_break_node(),
            create_line_break_node(),
            create_line_break_node(),
            create_text_node("x"),
        ])])
    });
    assert_eq!(
        read(&editor, || first_paragraph()
            .element_data()
            .get_text_format()),
        0.0
    );
    discrete(&editor, || {
        text_of(&first_paragraph(), true).toggle_format("bold");
        Ok(())
    });
    assert_ne!(
        read(&editor, || first_paragraph()
            .element_data()
            .get_text_format()),
        0.0
    );
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > general same-size walk caches first-text key on the parent DOM
#[test]
fn general_same_size_walk_caches_first_text_key_on_the_parent_dom() {
    let editor = create_reconciler_editor();
    discrete(&editor, || {
        get_root().clear()?;
        get_root().append(&[paragraph_of(&[
            create_text_node("a"),
            formatted("b", "bold"),
            formatted("c", "italic"),
            formatted("d", "code"),
        ])])
    });
    discrete(&editor, || {
        for child in first_paragraph().get_children() {
            assert!(child.is_text(), "child must be a TextNode");
            child.set_style("color: red");
        }
        Ok(())
    });
    read(&editor, || {
        let para = first_paragraph();
        assert_eq!(para.get_children().len(), 4);
        let first_child_key = para
            .get_first_child_or_throw()
            .unwrap()
            .get_key()
            .to_owned();
        assert_eq!(
            first_text_key(&editor, para.get_key()),
            Some(first_child_key)
        );
        assert_eq!(para.element_data().get_text_format(), 0.0);
    });
    discrete(&editor, || {
        text_of(&first_paragraph(), true).toggle_format("underline");
        Ok(())
    });
    assert_eq!(
        read(&editor, || first_paragraph()
            .element_data()
            .get_text_format()),
        0.0
    );
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > suffix walk keeps the leftmost first-text descriptor across element-child iterations
#[test]
fn suffix_walk_keeps_the_leftmost_first_text_descriptor_across_element_child_iterations() {
    let editor = create_reconciler_editor_with_link();
    discrete(&editor, || {
        get_root().clear()?;
        get_root().append(&[paragraph_of(&[
            create_line_break_node(),
            create_line_break_node(),
            create_line_break_node(),
            link("https://x.example", formatted("X", "bold")),
            link("https://y.example", formatted("Y", "italic")),
        ])])
    });
    discrete(&editor, || {
        for child in first_paragraph().get_children() {
            if child.is_element()
                && let Some(inner) = child.get_first_child().filter(NodeRef::is_text)
            {
                inner.set_style("color: red");
            }
        }
        Ok(())
    });
    assert_eq!(
        read(&editor, || first_paragraph()
            .element_data()
            .get_text_format()),
        TEXT_FORMAT_BOLD
    );
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-2: Layer 2 walk writes wrong __lexicalFirstTextKey when middle child is dirty
#[test]
fn audit_2_layer_2_walk_writes_wrong_first_text_key_when_middle_child_is_dirty() {
    let editor = create_reconciler_editor_with_link();
    discrete(&editor, || {
        get_root().clear()?;
        get_root().append(&[paragraph_of(&[
            link("https://a.example", formatted("a", "bold")),
            link("https://b.example", formatted("b", "italic")),
            link("https://c.example", formatted("c", "code")),
            link("https://d.example", formatted("d", "underline")),
        ])])
    });
    assert_eq!(
        read(&editor, || first_paragraph()
            .element_data()
            .get_text_format()),
        0.0
    );
    discrete(&editor, || {
        let link_c = first_paragraph().get_child_at_index(2).unwrap();
        assert!(link_c.is_element(), "linkC must be element");
        text_of(&link_c, false).set_style("color: red");
        Ok(())
    });
    read(&editor, || {
        let para = first_paragraph();
        let link_a = para.get_child_at_index(0).unwrap();
        assert!(link_a.is_element(), "linkA must be element");
        let text_a_key = link_a
            .get_first_child_or_throw()
            .unwrap()
            .get_key()
            .to_owned();
        assert_eq!(first_text_key(&editor, para.get_key()), Some(text_a_key));
        assert_eq!(para.element_data().get_text_format(), TEXT_FORMAT_BOLD);
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-3: $reconcileNodeChildren writes wrong cache after middle-insert
#[test]
fn audit_3_reconcile_node_children_writes_wrong_cache_after_middle_insert() {
    let editor = create_reconciler_editor_with_link();
    discrete(&editor, || {
        get_root().clear()?;
        get_root().append(&[paragraph_of(&[
            link("https://a.example", formatted("a", "bold")),
            link("https://b.example", formatted("b", "italic")),
            link("https://d.example", formatted("d", "underline")),
        ])])
    });
    discrete(&editor, || {
        let link_d = first_paragraph().get_child_at_index(2).unwrap();
        assert!(link_d.is_element(), "linkD must be element");
        link_d.insert_before(&link("https://c.example", formatted("c", "code")), true)?;
        Ok(())
    });
    read(&editor, || {
        let para = first_paragraph();
        let link_a = para.get_child_at_index(0).unwrap();
        assert!(link_a.is_element(), "linkA must be element");
        let text_a_key = link_a
            .get_first_child_or_throw()
            .unwrap()
            .get_key()
            .to_owned();
        assert_eq!(first_text_key(&editor, para.get_key()), Some(text_a_key));
    });
    editor.dispose();
}

/// AUDIT-4's `WrapperElementNode`: an inline element whose DOM wraps its
/// children's (`<main><section>`), which the model reconciler has no
/// counterpart of: its keyed DOM is where it keeps the first text.
#[derive(Clone, Debug, Default)]
struct WrapperElementNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for WrapperElementNode {
    fn get_type(&self) -> &'static str {
        "audit_wrapper"
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
    fn is_inline(&self) -> bool {
        true
    }
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-4: $bubbleChildFirstText misses cache on elements with wrapping DOM
#[test]
fn audit_4_bubble_child_first_text_misses_cache_on_elements_with_wrapping_dom() {
    let editor = build(
        "wrapper-audit",
        true,
        vec![NodeClass::of::<WrapperElementNode>()],
    );
    discrete(&editor, || {
        let wrapper = create::<WrapperElementNode>();
        wrapper.append(&[formatted("a", "bold")])?;
        get_root().clear()?;
        get_root().append(&[paragraph_of(&[
            wrapper,
            link("https://b.example", formatted("b", "italic")),
            link("https://c.example", formatted("c", "code")),
            link("https://d.example", formatted("d", "underline")),
        ])])
    });
    discrete(&editor, || {
        let link_c = first_paragraph().get_child_at_index(2).unwrap();
        assert!(link_c.is_element(), "linkC must be element");
        text_of(&link_c, false).set_style("color: red");
        Ok(())
    });
    read(&editor, || {
        let para = first_paragraph();
        let wrapper = para.get_child_at_index(0).unwrap();
        assert!(wrapper.is_element(), "wrapper must be element");
        let text_a_key = wrapper
            .get_first_child_or_throw()
            .unwrap()
            .get_key()
            .to_owned();
        assert_eq!(para.element_data().get_text_format(), TEXT_FORMAT_BOLD);
        assert_eq!(first_text_key(&editor, para.get_key()), Some(text_a_key));
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalReconciler.test.ts::LexicalReconciler > children fast path: contiguous-suffix incremental update > AUDIT-7: K=3 contiguous suffix with sizeDelta=+1 — output sentinel after helper bail
#[test]
fn audit_7_k3_contiguous_suffix_with_size_delta_plus_1() {
    let editor =
        build_editor_from_extensions([crate::rich_text::rich_text_extension().into()]).unwrap();
    set_test_root_element(&editor);
    let keys = Rc::new(RefCell::new((String::new(), String::new())));
    let k = keys.clone();
    discrete(&editor, move || {
        let root = get_root();
        root.clear()?;
        for label in ["pA", "pB", "pC", "pD"] {
            let p = paragraph_of(&[create_text_node(label)]);
            root.append(std::slice::from_ref(&p))?;
            if label == "pC" {
                k.borrow_mut().0 = p.get_key().to_owned();
            } else if label == "pD" {
                k.borrow_mut().1 = p.get_key().to_owned();
            }
        }
        Ok(())
    });
    let (para_c_key, para_d_key) = keys.borrow().clone();
    discrete(&editor, move || {
        let para_c = get_node_by_key(&para_c_key).unwrap();
        let para_d = get_node_by_key(&para_d_key).unwrap();
        assert!(para_c.get_type() == "paragraph" && para_d.get_type() == "paragraph");
        let t_c = text_of(&para_c, false);
        let t_d = text_of(&para_d, false);
        t_c.set_text_content("CHANGED_C");
        t_d.set_text_content("CHANGED_D");
        get_root().append(&[paragraph_of(&[create_text_node("pE")])])
    });
    let expected = ["pA", "pB", "CHANGED_C", "CHANGED_D", "pE"].join("\n\n");
    assert_eq!(read(&editor, || get_root().get_text_content()), expected);
    // `rootDom.__lexicalTextContent`: the root's text cache.
    assert_eq!(cached_text(&editor), Some(expected));
    editor.dispose();
}
