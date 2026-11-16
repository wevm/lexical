//! Ports of `packages/lexical-extension/src/__tests__/unit/BlockIdExtension.repairTransforms.test.ts`
//! and `BlockIdExtension.nestedCopy.test.ts`.
//!
//! Upstream's regular expressions over the described document are checked
//! by shape (ids masked) and by the absence of generated or missing ids.

use super::support::{counter, paragraph};
use crate::extension::{
    BlockIdConfigOverrides, LexicalEditorWithDispose, block_id_extension,
    build_editor_from_extensions, get_block_id, node_structure_extension,
};
use crate::json::Value;
use crate::{
    COLLABORATION_TAG, Editor, InitialEditorStateType, LexicalExtension, NodeRef, UpdateOptions,
    config_extension, define_extension, get_root, parse_serialized_node,
};

fn discrete() -> UpdateOptions {
    UpdateOptions {
        discrete: true,
        ..UpdateOptions::default()
    }
}

#[cfg(feature = "rich-text")]
mod repair_transforms {
    use super::*;
    use crate::rich_text::create_quote_node;

    /// `build(repairTransforms)`.
    fn build(repair_transforms: Option<bool>) -> LexicalEditorWithDispose {
        let editor = build_editor_from_extensions([define_extension(LexicalExtension {
            initial_editor_state: Some(InitialEditorStateType::Null),
            dependencies: vec![
                crate::rich_text::rich_text_extension().into(),
                node_structure_extension().into(),
                config_extension(
                    &block_id_extension(),
                    [BlockIdConfigOverrides {
                        generate_id: Some(counter("g")),
                        repair_transforms,
                        ..Default::default()
                    }],
                ),
            ],
            ..LexicalExtension::<(), (), ()>::new("test", ())
        })
        .into()])
        .unwrap();
        editor
            .update(
                || {
                    get_root().append(&[paragraph("one")])?;
                    Ok(())
                },
                discrete(),
            )
            .unwrap();
        editor
    }

    /// `describeDoc(editor)`: each block as `type#id[children]`, text and
    /// inline nodes as quoted text.
    fn describe_doc(editor: &Editor) -> String {
        fn visit(n: &NodeRef) -> String {
            if !n.is_element() || n.is_inline() {
                return Value::from(n.get_text_content()).to_string();
            }
            let children: Vec<String> = n.get_children().iter().map(visit).collect();
            format!(
                "{}#{}[{}]",
                n.get_type(),
                get_block_id(n)
                    .unwrap()
                    .unwrap_or_else(|| "none".to_owned()),
                children.join(" ")
            )
        }
        editor
            .get_editor_state()
            .read(|| {
                get_root()
                    .get_children()
                    .iter()
                    .map(visit)
                    .collect::<Vec<_>>()
            })
            .join(" / ")
    }

    /// The description with every id replaced by `*`, after checking each
    /// is a `[\w-]+` id.
    fn masked(doc: &str) -> String {
        let mut out = String::new();
        let mut rest = doc;
        while let Some(i) = rest.find('#') {
            out.push_str(&rest[..=i]);
            let after = &rest[i + 1..];
            let len = after.len()
                - after
                    .trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                    .len();
            assert!(len > 0, "{doc}");
            out.push('*');
            rest = &after[len..];
        }
        out.push_str(rest);
        out
    }

    /// No generated (`g<n>`) or missing ids.
    fn assert_no_generated_or_missing(doc: &str) {
        assert!(!doc.contains("#none"), "{doc}");
        let mut rest = doc;
        while let Some(i) = rest.find("#g") {
            rest = &rest[i + 2..];
            assert!(!rest.starts_with(|c: char| c.is_ascii_digit()), "{doc}");
        }
    }

    /// `$appendQuoteOfParagraphs()`: a quote holding two paragraphs, which
    /// NodeStructureExtension joins.
    fn append_quote_of_paragraphs() -> Result<(), crate::LexicalError> {
        let quote = create_quote_node(false);
        quote.append(&[paragraph("a"), paragraph("b")])?;
        get_root().append(&[quote])?;
        Ok(())
    }

    fn skip_transforms() -> UpdateOptions {
        UpdateOptions {
            skip_transforms: true,
            ..discrete()
        }
    }

    // upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.repairTransforms.test.ts::BlockIdExtension repairTransforms > defaults to true: the follow-up update runs the transforms
    #[test]
    fn defaults_to_true_the_follow_up_runs_the_transforms() {
        let editor = build(None);
        editor
            .update(append_quote_of_paragraphs, skip_transforms())
            .unwrap();
        assert_eq!(
            masked(&describe_doc(&editor)),
            r#"paragraph#*["one"] / quote#*["a" "\n" "b"]"#
        );
        editor.dispose();
    }

    // upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.repairTransforms.test.ts::BlockIdExtension repairTransforms > false: the follow-up of an update applied with skipTransforms only sets ids
    #[test]
    fn false_skip_transforms_follow_up_only_sets_ids() {
        let editor = build(Some(false));
        editor
            .update(append_quote_of_paragraphs, skip_transforms())
            .unwrap();
        let doc = describe_doc(&editor);
        assert_eq!(
            masked(&doc),
            r#"paragraph#*["one"] / quote#*[paragraph#*["a"] paragraph#*["b"]]"#
        );
        assert_no_generated_or_missing(&doc);
        editor.dispose();
    }

    // upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.repairTransforms.test.ts::BlockIdExtension repairTransforms > false: the follow-up of an update tagged COLLABORATION_TAG names the blocks alike
    #[test]
    fn false_collaboration_follow_up_names_alike() {
        let (a, b) = (build(Some(true)), build(Some(false)));
        for editor in [&a, &b] {
            editor
                .update(
                    append_quote_of_paragraphs,
                    UpdateOptions {
                        tags: vec![COLLABORATION_TAG.to_owned()],
                        ..discrete()
                    },
                )
                .unwrap();
        }
        // The update ran its own transforms; the follow-up only names.
        assert_eq!(describe_doc(&b), describe_doc(&a));
        assert_no_generated_or_missing(&describe_doc(&b));
        a.dispose();
        b.dispose();
    }

    // upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.repairTransforms.test.ts::BlockIdExtension repairTransforms > false: the blocks it names get the ids the default gives them
    #[test]
    fn false_names_with_the_default_ids() {
        let (a, b) = (build(Some(true)), build(Some(false)));
        for editor in [&a, &b] {
            // Content that the structure normalization keeps as it is.
            editor
                .update(
                    || {
                        get_root().append(&[paragraph("two"), paragraph("three")])?;
                        Ok(())
                    },
                    skip_transforms(),
                )
                .unwrap();
        }
        assert_eq!(describe_doc(&b), describe_doc(&a));
        // A later local edit gets a generated id, as with the default.
        b.update(
            || {
                get_root()
                    .get_last_child()
                    .unwrap()
                    .insert_after(&paragraph("four"), true)?;
                Ok(())
            },
            discrete(),
        )
        .unwrap();
        assert!(
            describe_doc(&b).ends_with(r#"paragraph#g1["four"]"#),
            "{}",
            describe_doc(&b)
        );
        a.dispose();
        b.dispose();
    }
}

#[cfg(feature = "list")]
mod nested_copy {
    use super::*;
    use crate::create_text_node;
    use crate::list::{ListType, create_list_item_node, create_list_node};

    fn all(n: &NodeRef) -> Vec<NodeRef> {
        let mut out = vec![n.clone()];
        if n.is_element() {
            out.extend(n.get_children().iter().flat_map(all));
        }
        out
    }

    fn item(text: &str) -> NodeRef {
        let item = create_list_item_node(None);
        item.append(&[create_text_node(text)]).unwrap();
        item
    }

    fn list(list_type: ListType, items: Vec<NodeRef>) -> NodeRef {
        let list = create_list_node(list_type, 1.0);
        list.append(&items).unwrap();
        list
    }

    // upstream: lexical-extension/src/__tests__/unit/BlockIdExtension.nestedCopy.test.ts::a list copied into a nested list item (where it nests) keeps no block id
    #[test]
    fn a_list_copied_into_a_nested_item_keeps_no_block_id() {
        let editor = build_editor_from_extensions([define_extension(LexicalExtension {
            initial_editor_state: Some(InitialEditorStateType::Null),
            dependencies: vec![
                crate::list::list_extension().into(),
                config_extension(
                    &block_id_extension(),
                    [BlockIdConfigOverrides {
                        generate_id: Some(counter("g")),
                        ..Default::default()
                    }],
                ),
            ],
            ..LexicalExtension::<(), (), ()>::new("test", ())
        })
        .into()])
        .unwrap();
        editor
            .update(
                || {
                    let nested_wrapper = create_list_item_node(None);
                    nested_wrapper.append(&[list(ListType::Bullet, vec![item("nested")])])?;
                    get_root().append(&[
                        list(ListType::Number, vec![item("one"), item("two")]),
                        list(ListType::Bullet, vec![item("top"), nested_wrapper]),
                    ])?;
                    Ok(())
                },
                discrete(),
            )
            .unwrap();
        let list_id = editor
            .read(|| get_block_id(&get_root().get_first_child().unwrap()).unwrap())
            .unwrap();
        assert!(list_id.is_some());
        // A paste of the whole number list into the nested item: its JSON,
        // with the list's NodeState, parsed into a new node.
        editor
            .update(
                || {
                    let source = get_root().get_first_child_or_throw()?;
                    assert_eq!(source.get_type(), "list");
                    let copy = parse_serialized_node(&Value::Object(source.export_json(false)))?;
                    for item in source.get_children() {
                        copy.append(&[parse_serialized_node(&Value::Object(
                            item.export_json(false),
                        ))?])?;
                    }
                    let nested_list = get_root()
                        .get_last_child_or_throw()?
                        .get_last_child_or_throw()?
                        .get_first_child_or_throw()?;
                    assert!(nested_list.is_element());
                    nested_list.get_first_child_or_throw()?.append(&[copy])?;
                    Ok(())
                },
                discrete(),
            )
            .unwrap();
        editor
            .read(|| {
                let holders: Vec<NodeRef> = all(&get_root())
                    .into_iter()
                    .filter(|node| {
                        node.get_type() == "list" && get_block_id(node).unwrap() == list_id
                    })
                    .collect();
                assert_eq!(holders.len(), 1);
                assert_eq!(
                    holders[0].get_key(),
                    get_root().get_first_child().unwrap().get_key()
                );
            })
            .unwrap();
        editor.dispose();
    }
}
