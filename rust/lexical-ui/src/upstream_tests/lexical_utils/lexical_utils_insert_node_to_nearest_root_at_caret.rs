//! Ports of
//! `packages/lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts`.
//!
//! Upstream runs each of its 11 scenarios under each of 8 option variants
//! (whether `$shouldSplit` splits the first and the last edge, and
//! `removeEmptyDestination`), one test per pair; here each scenario is one
//! test that runs the 8 variants and carries their 8 markers (generated
//! from the upstream ids). Upstream's only node kind is `ParagraphNode`.

#![cfg(all(feature = "extension", feature = "rich-text"))]

use crate::caret::{
    CaretDirection, TextOffset, get_child_caret, get_sibling_caret, get_text_point_caret,
};
use crate::caret_utils::{SplitAtPointCaretNextOptions, insert_node_to_nearest_root_at_caret};
use crate::extension::build_editor_from_extensions;
use crate::{
    InitialEditorStateType, LexicalError, LexicalExtension, NodeRef, create_paragraph_node,
    create_text_node, define_extension, get_root,
};
use std::rc::Rc;

/// `$serialize(node)`.
fn serialize(node: &NodeRef) -> String {
    if !node.is_element() {
        return node.get_text_content();
    }
    let children: String = node.get_children().iter().map(serialize).collect();
    format!("[{children}]")
}

/// `$rootSerialize()`.
fn root_serialize() -> String {
    get_root().get_children().iter().map(serialize).collect()
}

/// `$createLiftable('ParagraphNode', text)`.
fn create_liftable(text: &str) -> Result<NodeRef, LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(&[create_text_node(text)])?;
    Ok(paragraph)
}

fn paragraph_of(children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    let paragraph = create_paragraph_node();
    paragraph.append(children)?;
    Ok(paragraph)
}

/// `SplitFlags`.
#[derive(Clone, Copy)]
struct SplitFlags {
    first: bool,
    last: bool,
    remove_empty_destination: bool,
}

/// `variant.options`: `$shouldSplit` is a function pointer here, one per
/// pair of flags.
fn options(flags: SplitFlags) -> SplitAtPointCaretNextOptions {
    let should_split: fn(&NodeRef, &'static str) -> bool = match (flags.first, flags.last) {
        (true, true) => |_, edge| edge == "first" || edge == "last",
        (true, false) => |_, edge| edge == "first",
        (false, true) => |_, edge| edge == "last",
        (false, false) => |_, _| false,
    };
    SplitAtPointCaretNextOptions {
        should_split,
        remove_empty_destination: flags.remove_empty_destination,
        ..SplitAtPointCaretNextOptions::default()
    }
}

type Run = fn(&SplitAtPointCaretNextOptions) -> Result<(), LexicalError>;

/// `Scenario`: `$run` and `expected`.
#[derive(Clone, Copy)]
struct Scenario {
    run: Run,
    expected: fn(SplitFlags) -> String,
}

/// The expectation the four sole-child and empty scenarios share.
fn sole_expected(f: SplitFlags) -> String {
    if f.remove_empty_destination {
        "[x]".to_owned()
    } else {
        format!(
            "{}[x]{}",
            if f.first { "[]" } else { "" },
            if f.last || !f.first { "[]" } else { "" }
        )
    }
}

fn sibling_node_expected(f: SplitFlags) -> String {
    format!(
        "[a][x]{}",
        if f.last && !f.remove_empty_destination {
            "[]"
        } else {
            ""
        }
    )
}

const SCENARIOS: [Scenario; 11] = [
    // sole child, caret on node (previous) — caret.origin === node
    Scenario {
        run: |options| {
            let x = create_liftable("x")?;
            get_root().append(&[paragraph_of(std::slice::from_ref(&x))?])?;
            insert_node_to_nearest_root_at_caret(
                &x,
                &get_sibling_caret(&x, CaretDirection::Previous),
                options,
            )?;
            Ok(())
        },
        expected: sole_expected,
    },
    // sole child, caret on node (next) — caret.origin === node
    Scenario {
        run: |options| {
            let x = create_liftable("x")?;
            get_root().append(&[paragraph_of(std::slice::from_ref(&x))?])?;
            insert_node_to_nearest_root_at_caret(
                &x,
                &get_sibling_caret(&x, CaretDirection::Next),
                options,
            )?;
            Ok(())
        },
        expected: sole_expected,
    },
    // empty, caret on paragraph (next)
    Scenario {
        run: |options| {
            let x = create_liftable("x")?;
            let p = create_paragraph_node();
            get_root().append(std::slice::from_ref(&p))?;
            let caret = get_child_caret(&p, CaretDirection::Next).expect("an element");
            insert_node_to_nearest_root_at_caret(&x, &caret, options)?;
            Ok(())
        },
        expected: sole_expected,
    },
    // empty, caret on paragraph (previous)
    Scenario {
        run: |options| {
            let x = create_liftable("x")?;
            let p = create_paragraph_node();
            get_root().append(std::slice::from_ref(&p))?;
            let caret = get_child_caret(&p, CaretDirection::Previous).expect("an element");
            insert_node_to_nearest_root_at_caret(&x, &caret, options)?;
            Ok(())
        },
        expected: sole_expected,
    },
    // caret on root (next)
    Scenario {
        run: |options| {
            let x = create_liftable("x")?;
            let caret = get_child_caret(&get_root(), CaretDirection::Next).expect("an element");
            insert_node_to_nearest_root_at_caret(&x, &caret, options)?;
            Ok(())
        },
        expected: |_| "[x]".to_owned(),
    },
    // sibling before, caret on node (next) — caret.origin === node
    Scenario {
        run: |options| {
            let x = create_liftable("x")?;
            get_root().append(&[paragraph_of(&[create_text_node("a"), x.clone()])?])?;
            insert_node_to_nearest_root_at_caret(
                &x,
                &get_sibling_caret(&x, CaretDirection::Next),
                options,
            )?;
            Ok(())
        },
        expected: sibling_node_expected,
    },
    // sibling before, caret on node (previous) — caret.origin === node
    Scenario {
        run: |options| {
            let x = create_liftable("x")?;
            get_root().append(&[paragraph_of(&[create_text_node("a"), x.clone()])?])?;
            insert_node_to_nearest_root_at_caret(
                &x,
                &get_sibling_caret(&x, CaretDirection::Previous),
                options,
            )?;
            Ok(())
        },
        expected: sibling_node_expected,
    },
    // sibling before, caret on sibling (previous)
    Scenario {
        run: |options| {
            let x = create_liftable("x")?;
            let sib = create_text_node("a");
            get_root().append(&[paragraph_of(&[sib.clone(), x.clone()])?])?;
            insert_node_to_nearest_root_at_caret(
                &x,
                &get_sibling_caret(&sib, CaretDirection::Previous),
                options,
            )?;
            Ok(())
        },
        expected: |f| format!("{}[x][a]", if f.first { "[]" } else { "" }),
    },
    // sibling before, caret on text start (next)
    Scenario {
        run: |options| {
            let x = create_liftable("x")?;
            let sib = create_text_node("ab");
            get_root().append(&[paragraph_of(&[sib.clone(), x.clone()])?])?;
            insert_node_to_nearest_root_at_caret(
                &x,
                &get_text_point_caret(&sib, CaretDirection::Next, 0)?,
                options,
            )?;
            Ok(())
        },
        expected: |f| format!("{}[x][ab]", if f.first { "[]" } else { "" }),
    },
    // sibling before, caret on text middle (next)
    Scenario {
        run: |options| {
            let x = create_liftable("x")?;
            let sib = create_text_node("ab");
            get_root().append(&[paragraph_of(&[sib.clone(), x.clone()])?])?;
            insert_node_to_nearest_root_at_caret(
                &x,
                &get_text_point_caret(&sib, CaretDirection::Next, 1)?,
                options,
            )?;
            Ok(())
        },
        expected: |_| "[a][x][b]".to_owned(),
    },
    // sibling before, caret on text end (next)
    Scenario {
        run: |options| {
            let x = create_liftable("x")?;
            let sib = create_text_node("ab");
            get_root().append(&[paragraph_of(&[sib.clone(), x.clone()])?])?;
            insert_node_to_nearest_root_at_caret(
                &x,
                &get_text_point_caret(
                    &sib,
                    CaretDirection::Next,
                    TextOffset::End(CaretDirection::Next),
                )?,
                options,
            )?;
            Ok(())
        },
        expected: |f| {
            format!(
                "[ab][x]{}",
                if !f.remove_empty_destination && f.last {
                    "[]"
                } else {
                    ""
                }
            )
        },
    },
];

/// Runs `scenario` under each of upstream's `optionVariants`, in its order.
fn check_all_variants(scenario: Scenario) {
    for first in [true, false] {
        for last in [true, false] {
            for remove_empty_destination in [true, false] {
                let flags = SplitFlags {
                    first,
                    last,
                    remove_empty_destination,
                };
                let editor = build_editor_from_extensions([define_extension(LexicalExtension {
                    initial_editor_state: Some(InitialEditorStateType::Update(Rc::new(
                        move |_| (scenario.run)(&options(flags)),
                    ))),
                    dependencies: vec![crate::rich_text::rich_text_extension().into()],
                    ..LexicalExtension::<(), (), ()>::new("[insert-to-nearest-root]", ())
                })
                .into()])
                .unwrap();
                assert_eq!(
                    editor.read(root_serialize).unwrap(),
                    (scenario.expected)(flags),
                    "first: {first}, last: {last}, removeEmptyDestination: {remove_empty_destination}"
                );
            }
        }
    }
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sole child, caret on node (previous) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sole child, caret on node (previous) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sole child, caret on node (previous) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sole child, caret on node (previous) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sole child, caret on node (previous) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sole child, caret on node (previous) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sole child, caret on node (previous) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sole child, caret on node (previous) — caret.origin === node
#[test]
fn sole_child_caret_on_node_previous() {
    check_all_variants(SCENARIOS[0]);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sole child, caret on node (next) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sole child, caret on node (next) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sole child, caret on node (next) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sole child, caret on node (next) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sole child, caret on node (next) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sole child, caret on node (next) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sole child, caret on node (next) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sole child, caret on node (next) — caret.origin === node
#[test]
fn sole_child_caret_on_node_next() {
    check_all_variants(SCENARIOS[1]);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > empty, caret on paragraph (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > empty, caret on paragraph (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > empty, caret on paragraph (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > empty, caret on paragraph (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > empty, caret on paragraph (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > empty, caret on paragraph (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > empty, caret on paragraph (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > empty, caret on paragraph (next)
#[test]
fn empty_caret_on_paragraph_next() {
    check_all_variants(SCENARIOS[2]);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > empty, caret on paragraph (previous)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > empty, caret on paragraph (previous)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > empty, caret on paragraph (previous)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > empty, caret on paragraph (previous)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > empty, caret on paragraph (previous)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > empty, caret on paragraph (previous)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > empty, caret on paragraph (previous)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > empty, caret on paragraph (previous)
#[test]
fn empty_caret_on_paragraph_previous() {
    check_all_variants(SCENARIOS[3]);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > caret on root (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > caret on root (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > caret on root (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > caret on root (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > caret on root (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > caret on root (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > caret on root (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > caret on root (next)
#[test]
fn caret_on_root_next() {
    check_all_variants(SCENARIOS[4]);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sibling before, caret on node (next) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sibling before, caret on node (next) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sibling before, caret on node (next) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sibling before, caret on node (next) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sibling before, caret on node (next) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sibling before, caret on node (next) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sibling before, caret on node (next) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sibling before, caret on node (next) — caret.origin === node
#[test]
fn sibling_before_caret_on_node_next() {
    check_all_variants(SCENARIOS[5]);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sibling before, caret on node (previous) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sibling before, caret on node (previous) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sibling before, caret on node (previous) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sibling before, caret on node (previous) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sibling before, caret on node (previous) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sibling before, caret on node (previous) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sibling before, caret on node (previous) — caret.origin === node
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sibling before, caret on node (previous) — caret.origin === node
#[test]
fn sibling_before_caret_on_node_previous() {
    check_all_variants(SCENARIOS[6]);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sibling before, caret on sibling (previous)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sibling before, caret on sibling (previous)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sibling before, caret on sibling (previous)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sibling before, caret on sibling (previous)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sibling before, caret on sibling (previous)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sibling before, caret on sibling (previous)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sibling before, caret on sibling (previous)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sibling before, caret on sibling (previous)
#[test]
fn sibling_before_caret_on_sibling_previous() {
    check_all_variants(SCENARIOS[7]);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sibling before, caret on text start (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sibling before, caret on text start (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sibling before, caret on text start (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sibling before, caret on text start (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sibling before, caret on text start (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sibling before, caret on text start (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sibling before, caret on text start (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sibling before, caret on text start (next)
#[test]
fn sibling_before_caret_on_text_start_next() {
    check_all_variants(SCENARIOS[8]);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sibling before, caret on text middle (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sibling before, caret on text middle (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sibling before, caret on text middle (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sibling before, caret on text middle (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sibling before, caret on text middle (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sibling before, caret on text middle (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sibling before, caret on text middle (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sibling before, caret on text middle (next)
#[test]
fn sibling_before_caret_on_text_middle_next() {
    check_all_variants(SCENARIOS[9]);
}

// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last removeEmptyDestination > sibling before, caret on text end (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first last > sibling before, caret on text end (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first removeEmptyDestination > sibling before, caret on text end (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with first > sibling before, caret on text end (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last removeEmptyDestination > sibling before, caret on text end (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with last > sibling before, caret on text end (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with removeEmptyDestination > sibling before, caret on text end (next)
// upstream: lexical-utils/src/__tests__/unit/LexicalUtilsInsertNodeToNearestRootAtCaret.test.ts::$insertNodeToNearestRootAtCaret edge cases > ParagraphNode with no-splits-or-remove > sibling before, caret on text end (next)
#[test]
fn sibling_before_caret_on_text_end_next() {
    check_all_variants(SCENARIOS[10]);
}
