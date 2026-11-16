//! Ports of `packages/lexical-hashtag/src/__tests__/unit/LexicalHashtagNode.test.ts`.

use crate::hashtag::{HashtagNode, create_hashtag_node};
use crate::upstream_tests::support::{initialize_unit_test, update};
use crate::{CreateEditorArgs, NodeClass};

// upstream: lexical-hashtag/src/__tests__/unit/LexicalHashtagNode.test.ts::LexicalHashtagNode tests > HashtagNode.exportJSON() should return and object conforming to the expected schema
#[test]
fn hashtag_node_export_json_conforms_to_the_expected_schema() {
    let editor = initialize_unit_test(CreateEditorArgs {
        nodes: vec![NodeClass::of::<HashtagNode>()],
        ..CreateEditorArgs::default()
    });
    update(&editor, || {
        let node = create_hashtag_node("therickestrickofall");
        // If you broke this test, you changed the public interface of a
        // serialized Lexical Core Node. Please ensure the correct adapter
        // logic is in place in the corresponding importJSON  method
        // to accommodate these changes.
        assert_eq!(
            crate::json::Value::Object(node.export_json(false)).to_string(),
            r#"{"detail":0,"format":0,"mode":"normal","style":"","text":"therickestrickofall","type":"hashtag","version":1}"#
        );
        Ok(())
    });
}
