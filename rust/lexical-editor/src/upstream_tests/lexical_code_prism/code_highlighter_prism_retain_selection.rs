//! Ports of `packages/lexical-code-prism/src/__tests__/unit/CodeHighlighterPrismRetainSelection.test.ts`.
//!
//! `registerCodeHighlighting(editor)` highlights with Prism; here with
//! [`TestTokenizer`] (see `support`). The `test.each` table is one test.

use super::super::support::{initialize_unit_test, update};
use super::support::TestTokenizer;
use crate::code::{create_code_node, register_code_highlighting};
use crate::{CreateEditorArgs, PointType, create_text_node, get_root, get_selection};
use std::rc::Rc;

// upstream: lexical-code-prism/src/__tests__/unit/CodeHighlighterPrismRetainSelection.test.ts::CodeHighlighterPrism $updateAndRetainSelection > retains a non-negative offset for code content "\nfoo" (#8943)
// upstream: lexical-code-prism/src/__tests__/unit/CodeHighlighterPrismRetainSelection.test.ts::CodeHighlighterPrism $updateAndRetainSelection > retains a non-negative offset for code content "\n\nfoo" (#8943)
// upstream: lexical-code-prism/src/__tests__/unit/CodeHighlighterPrismRetainSelection.test.ts::CodeHighlighterPrism $updateAndRetainSelection > retains a non-negative offset for code content "foo\nbar" (#8943)
#[test]
fn retains_a_non_negative_offset_for_code_content() {
    for (code_text, expected_text) in [
        ("\nfoo", "X\nfoo"),
        ("\n\nfoo", "X\n\nfoo"),
        ("foo\nbar", "Xfoo\nbar"),
    ] {
        let editor = initialize_unit_test(CreateEditorArgs {
            nodes: crate::code::code_nodes(),
            ..CreateEditorArgs::default()
        });
        let _unregister =
            register_code_highlighting(&editor, Rc::new(TestTokenizer::default())).unwrap();
        update(&editor, move || {
            let code = create_code_node(Some("javascript"), None);
            let root = get_root();
            root.clear()?;
            root.append(std::slice::from_ref(&code))?;
            code.append(&[create_text_node(code_text)])?;
            // Caret at the very start of the code block, while its content
            // is still un-flattened, so the transform has to restore it.
            code.select(Some(0), Some(0))?;
            Ok(())
        });
        editor
            .read(|| {
                let selection = get_selection().and_then(|s| s.range());
                let selection = selection.expect("a range selection");
                for point in [&selection.anchor, &selection.focus] {
                    if point.point_type == PointType::Text {
                        let size = crate::get_node_by_key(&point.key)
                            .unwrap()
                            .get_text_content_size();
                        assert!(point.offset <= size, "{code_text:?}");
                    }
                }
            })
            .unwrap();
        update(&editor, || {
            get_selection()
                .filter(crate::SelectionRef::is_range)
                .expect("a range selection")
                .insert_text("X")
        });
        assert_eq!(
            editor
                .read(|| get_root().get_first_child().unwrap().get_text_content())
                .unwrap(),
            expected_text
        );
    }
}
