//! Ports of `packages/lexical/src/__tests__/unit/LexicalEditorEmptyState.test.ts`.
//! Upstream mocks `devInvariant` to run its production arm; only the test
//! that never reaches it is ported (rust/parity/skips.json has the other).

use crate::{CreateEditorArgs, RootElement, RootElementRef, create_editor, get_root};
use std::rc::Rc;

struct TestRoot;
impl RootElement for TestRoot {}

// upstream: lexical/src/__tests__/unit/LexicalEditorEmptyState.test.ts::setEditorState with an empty editor state (production arm) > leaves a non-empty editor state untouched
#[test]
fn leaves_a_non_empty_editor_state_untouched() {
    let editor = create_editor(CreateEditorArgs::default());
    editor
        .set_root_element(Some(RootElementRef(Rc::new(TestRoot))))
        .unwrap();

    let state = editor
        .parse_editor_state(
            r#"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"hello","type":"text","version":1}],"direction":null,"format":"","indent":0,"type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#,
        )
        .unwrap();
    // `onError` is not called.
    editor.set_editor_state(&state, None).unwrap();

    editor
        .read(|| {
            // No stray paragraph appended alongside the parsed content.
            assert_eq!(get_root().get_children_size(), 1);
            assert_eq!(get_root().get_text_content(), "hello");
        })
        .unwrap();
}
