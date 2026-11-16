//! Ports of `packages/lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts`.
//!
//! Prism's grammars aren't ported (rust/PARITY.md, `@lexical/code-core`),
//! so the highlighter runs over a [`PlainTokenizer`] defaulting to
//! `javascript`, as `code_highlighter_extension`'s default does; the tests
//! check text, selections and indentation, which don't depend on the token
//! types. Both tables are generated from the upstream source:
//! `suite` (each scenario in both directions) and `OUTDENT_SCENARIOS` of
//! `lexical-code-core/src/__tests__/outdentTestUtils.ts` (without
//! `tabSize=2.5`, which a `usize` can't hold; rust/parity/skips.json).

#![cfg(all(feature = "extension", feature = "rich-text"))]

use super::super::support::{initialize_unit_test, update};
use crate::code::{
    CodeIndentConfigOverrides, PlainTokenizer, code_highlighter_extension, code_indent_extension,
    create_code_node, register_code_highlighting,
};
use crate::extension::{
    CanIndentPredicate, build_editor_from_extensions, default_can_indent, register_tab_indentation,
    signal, tab_indentation_extension,
};
use crate::{
    CaretDirection, CreateEditorArgs, Editor, INDENT_CONTENT_COMMAND, KEY_TAB_COMMAND,
    KeyboardEvent, LexicalExtension, NodeRef, OUTDENT_CONTENT_COMMAND, PointType, Which,
    caret_range_from_selection, config_extension, create_range_selection, define_extension,
    get_caret_range_in_direction, get_child_caret, get_collapsed_caret_range, get_root,
    get_selection, get_sibling_caret, get_text_point_caret, set_selection_from_caret_range,
};
use std::cell::RefCell;
use std::rc::Rc;

/// The `[before, after, operation]` scenarios of `suite`.
const SUITE: &[(&str, &str, &str)] = &[
    (">|", ">-|", "tab"),
    (">|", ">-|", "indent"),
    (">|", ">|", "outdent"),
    (">|line1", ">-|line1", "tab"),
    (">|line1", ">-|line1", "indent"),
    (">|line1", ">|line1", "outdent"),
    (">-li|ne1", ">-li-|ne1", "tab"),
    (">-li|ne1", ">--li|ne1", "indent"),
    (">-li|ne1", ">li|ne1", "outdent"),
    (">|-line1|", ">|--line1|", "tab"),
    (">|-line1|", ">|--line1|", "indent"),
    (">|-line1|", ">|line1|", "outdent"),
    (">-li|ne1|", ">-li-|", "tab"),
    (">-li|ne1|", ">--li|ne1|", "indent"),
    (">-li|ne1|", ">li|ne1|", "outdent"),
    (">|word1| word2", ">-| word2", "tab"),
    (">|line1>|", ">|-line1>|", "tab"),
    (">|line1>|", ">|-line1>|", "indent"),
    (">|line1>|line2", ">|-line1>|line2", "tab"),
    (">|line1>|line2", ">|-line1>|line2", "indent"),
    (">line1|>line2|", ">-line1|>-line2|", "tab"),
    (">line1|>line2|", ">-line1|>-line2|", "indent"),
    (">|>li|ne1", ">|>-li|ne1", "tab"),
    (">|>li|ne1", ">|>-li|ne1", "indent"),
    (">|>|line1", ">|->|line1", "tab"),
    (">|>|line1", ">|->|line1", "indent"),
    (">|>>|line1", ">|>>|line1", "tab"),
    (">|>>|line1", ">|>>|line1", "indent"),
    (">|line1>li|ne2", ">|-line1>-li|ne2", "tab"),
    (">|line1>li|ne2", ">|-line1>-li|ne2", "indent"),
    (">|-line1>li|ne2", ">|line1>li|ne2", "outdent"),
    (">li|ne1>li|ne2", ">-li|ne1>-li|ne2", "tab"),
    (">li|ne1>li|ne2", ">-li|ne1>-li|ne2", "indent"),
    (">-li|ne1>>-li|ne2", ">--li|ne1>>--li|ne2", "tab"),
    (">-li|ne1>>-li|ne2", ">--li|ne1>>--li|ne2", "indent"),
    (">-li|ne1>>-li|ne2", ">li|ne1>>li|ne2", "outdent"),
    (">|-line1>->-line2|", ">|--line1>-->--line2|", "tab"),
];

/// `getRawTextWithSelection(input)`.
fn raw_text_with_selection(input: &str) -> String {
    input
        .strip_prefix('>')
        .unwrap_or(input)
        .replace('>', "\n")
        .replace('-', "\t")
}

/// `.replaceAll('\t', '-').replaceAll('\n', '>')`.
fn visible(text: &str) -> String {
    text.replace('\t', "-").replace('\n', ">")
}

/// Index of a char in UTF-16 code units (the scenarios are ASCII).
fn utf16_index(s: &str, byte: usize) -> usize {
    crate::utf16::len(&s[..byte])
}

/// One scenario in one direction.
fn run_tabs_scenario(before: &'static str, after: &'static str, op: &'static str, forwards: bool) {
    let editor = initialize_unit_test(CreateEditorArgs {
        nodes: crate::code::code_nodes(),
        ..CreateEditorArgs::default()
    });
    let _ = crate::rich_text::register_rich_text(&editor);
    let _ = register_tab_indentation(
        &editor,
        signal(None).as_readonly(),
        signal(CanIndentPredicate(Rc::new(default_can_indent))).as_readonly(),
    );
    let _ = register_code_highlighting(
        &editor,
        Rc::new(PlainTokenizer {
            default_language: Some("javascript".to_owned()),
        }),
    )
    .unwrap();

    // Fill the editor with the scenario's input text.
    let code_slot: Rc<RefCell<Option<NodeRef>>> = Rc::default();
    let slot = code_slot.clone();
    update(&editor, move || {
        let code_node = create_code_node(None, None);
        get_root().append(std::slice::from_ref(&code_node))?;
        code_node.select_start()?;
        get_selection()
            .unwrap()
            .insert_raw_text(&raw_text_with_selection(before).replace('|', ""))?;
        *slot.borrow_mut() = Some(code_node);
        Ok(())
    });
    let code_node = code_slot.take().unwrap();

    // A RangeSelection matching the `|`s of the input, then the command.
    let (code, ed) = (code_node.clone(), editor.clone());
    update(&editor, move || {
        let selection = create_range_selection();
        let code_key = code.get_key().to_owned();
        if code.get_children_size() == 0 {
            selection.point_set(Which::Anchor, &code_key, 0, PointType::Element)?;
            selection.point_set(Which::Focus, &code_key, 0, PointType::Element)?;
        } else {
            let raw_text = raw_text_with_selection(before);
            let sel_first = utf16_index(&raw_text, raw_text.find('|').unwrap());
            let mut sel_last = utf16_index(&raw_text, raw_text.rfind('|').unwrap());
            if sel_last != sel_first {
                sel_last -= 1;
            }
            let mut matching = code.get_first_child();
            let mut parent_index = 0;
            let mut offset = 0;
            while let Some(m) = matching
                .clone()
                .filter(|m| offset + m.get_text_content_size() < sel_first)
            {
                offset += m.get_text_content_size();
                matching = m.get_next_sibling();
                parent_index += 1;
            }
            match &matching {
                None => selection.point_set(Which::Anchor, &code_key, 0, PointType::Element)?,
                Some(m) if m.get_type() != "linebreak" => selection.point_set(
                    Which::Anchor,
                    m.get_key(),
                    sel_first - offset,
                    PointType::Text,
                )?,
                Some(_) => selection.point_set(
                    Which::Anchor,
                    &code_key,
                    parent_index,
                    PointType::Element,
                )?,
            }
            while let Some(m) = matching
                .clone()
                .filter(|m| offset + m.get_text_content_size() < sel_last)
            {
                offset += m.get_text_content_size();
                matching = m.get_next_sibling();
                parent_index += 1;
            }
            match &matching {
                None => selection.point_set(Which::Focus, &code_key, 0, PointType::Element)?,
                Some(m) if m.get_type() != "linebreak" => selection.point_set(
                    Which::Focus,
                    m.get_key(),
                    sel_last - offset,
                    PointType::Text,
                )?,
                Some(_) => selection.point_set(
                    Which::Focus,
                    &code_key,
                    parent_index + 1,
                    PointType::Element,
                )?,
            }
        }
        set_selection_from_caret_range(&get_caret_range_in_direction(
            &caret_range_from_selection(&selection.range().unwrap())?,
            if forwards {
                CaretDirection::Next
            } else {
                CaretDirection::Previous
            },
        )?)?;
        match op {
            "indent" => ed.dispatch_command(&INDENT_CONTENT_COMMAND, ())?,
            "outdent" => ed.dispatch_command(&OUTDENT_CONTENT_COMMAND, ())?,
            // `tabKeyboardEvent()`.
            _ => ed.dispatch_command(
                &KEY_TAB_COMMAND,
                Rc::new(KeyboardEvent {
                    key: "Tab".to_owned(),
                    ..KeyboardEvent::default()
                }),
            )?,
        };
        Ok(())
    });

    editor
        .read(|| {
            let output = after.replace('|', "");
            let selected_output = match (after.find('|'), after.rfind('|')) {
                (Some(first), Some(last)) => after[first..=last].replace('|', ""),
                _ => String::new(),
            };
            let context = format!("{op}: {before} => {after} (forwards: {forwards})");
            assert_eq!(
                format!(">{}", visible(&code_node.get_text_content())),
                output,
                "{context}"
            );
            let selection = get_selection().unwrap();
            assert_eq!(
                visible(&selection.get_text_content().unwrap()),
                selected_output,
                "{context}"
            );
            let is_backward = selection.is_backward().unwrap();
            if forwards {
                assert!(!is_backward, "{context}");
            } else if after.split('|').count() == 3 {
                assert!(is_backward, "{context}");
            }
        })
        .unwrap();
}

// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >| => >-| (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >| => >-| (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >| => >-| (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >| => >-| (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >| => >| (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >| => >| (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1 => >-|line1 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1 => >-|line1 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1 => >-|line1 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1 => >-|line1 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >|line1 => >|line1 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >|line1 => >|line1 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >-li|ne1 => >-li-|ne1 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >-li|ne1 => >-li-|ne1 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >-li|ne1 => >--li|ne1 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >-li|ne1 => >--li|ne1 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >-li|ne1 => >li|ne1 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >-li|ne1 => >li|ne1 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|-line1| => >|--line1| (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|-line1| => >|--line1| (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|-line1| => >|--line1| (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|-line1| => >|--line1| (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >|-line1| => >|line1| (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >|-line1| => >|line1| (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >-li|ne1| => >-li-| (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >-li|ne1| => >-li-| (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >-li|ne1| => >--li|ne1| (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >-li|ne1| => >--li|ne1| (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >-li|ne1| => >li|ne1| (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >-li|ne1| => >li|ne1| (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|word1| word2 => >-| word2 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|word1| word2 => >-| word2 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1>| => >|-line1>| (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1>| => >|-line1>| (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1>| => >|-line1>| (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1>| => >|-line1>| (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1>|line2 => >|-line1>|line2 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1>|line2 => >|-line1>|line2 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1>|line2 => >|-line1>|line2 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1>|line2 => >|-line1>|line2 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >line1|>line2| => >-line1|>-line2| (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >line1|>line2| => >-line1|>-line2| (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >line1|>line2| => >-line1|>-line2| (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >line1|>line2| => >-line1|>-line2| (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|>li|ne1 => >|>-li|ne1 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|>li|ne1 => >|>-li|ne1 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|>li|ne1 => >|>-li|ne1 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|>li|ne1 => >|>-li|ne1 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|>|line1 => >|->|line1 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|>|line1 => >|->|line1 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|>|line1 => >|->|line1 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|>|line1 => >|->|line1 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|>>|line1 => >|>>|line1 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|>>|line1 => >|>>|line1 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|>>|line1 => >|>>|line1 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|>>|line1 => >|>>|line1 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1>li|ne2 => >|-line1>-li|ne2 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|line1>li|ne2 => >|-line1>-li|ne2 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1>li|ne2 => >|-line1>-li|ne2 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >|line1>li|ne2 => >|-line1>-li|ne2 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >|-line1>li|ne2 => >|line1>li|ne2 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >|-line1>li|ne2 => >|line1>li|ne2 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >li|ne1>li|ne2 => >-li|ne1>-li|ne2 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >li|ne1>li|ne2 => >-li|ne1>-li|ne2 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >li|ne1>li|ne2 => >-li|ne1>-li|ne2 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >li|ne1>li|ne2 => >-li|ne1>-li|ne2 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >-li|ne1>>-li|ne2 => >--li|ne1>>--li|ne2 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >-li|ne1>>-li|ne2 => >--li|ne1>>--li|ne2 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >-li|ne1>>-li|ne2 => >--li|ne1>>--li|ne2 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing indent: >-li|ne1>>-li|ne2 => >--li|ne1>>--li|ne2 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >-li|ne1>>-li|ne2 => >li|ne1>>li|ne2 (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing outdent: >-li|ne1>>-li|ne2 => >li|ne1>>li|ne2 (backwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|-line1>->-line2| => >|--line1>-->--line2| (forwards)
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > Tabs > testing tab: >|-line1>->-line2| => >|--line1>-->--line2| (backwards)
#[test]
fn tabs() {
    assert_eq!(SUITE.len(), 37);
    for &(before, after, op) in SUITE {
        for forwards in [true, false] {
            run_tabs_scenario(before, after, op, forwards);
        }
    }
}

/// An outdent scenario: `(rawText, cursorOffset, tabSize, expectedText,
/// expectedCursor)`.
type OutdentScenario = (
    &'static str,
    usize,
    Option<usize>,
    &'static str,
    Option<usize>,
);

/// `OUTDENT_SCENARIOS`.
const OUTDENT_SCENARIOS: &[OutdentScenario] = &[
    ("  hello", 2, Some(2), "hello", None),
    ("    hello", 4, Some(2), "  hello", Some(2)),
    (" hello", 1, Some(2), "hello", None),
    ("    hello", 4, Some(4), "hello", None),
    ("  hello", 2, Some(4), "hello", None),
    ("hello", 0, Some(2), "hello", None),
    ("\t  hello", 1, Some(2), "  hello", None),
    ("  hello", 2, None, "  hello", None),
    ("    hello", 4, Some(0), "    hello", None),
];

/// `$runOutdentScenario(editor, rawText, cursorOffset)`: `(text, cursor)`.
fn run_outdent_scenario(
    editor: &Editor,
    raw_text: &'static str,
    cursor_offset: usize,
) -> (String, usize) {
    let code_slot: Rc<RefCell<Option<NodeRef>>> = Rc::default();
    let slot = code_slot.clone();
    update(editor, move || {
        let code_node = create_code_node(None, None);
        let root = get_root();
        root.clear()?;
        root.append(std::slice::from_ref(&code_node))?;
        code_node.select_start()?.insert_raw_text(raw_text)?;
        *slot.borrow_mut() = Some(code_node);
        Ok(())
    });
    let code_node = code_slot.take().unwrap();
    let (code, ed) = (code_node.clone(), editor.clone());
    update(editor, move || {
        let mut matching = code.get_first_child();
        let mut offset = 0;
        while let Some(m) = matching
            .clone()
            .filter(|m| offset + m.get_text_content_size() < cursor_offset)
        {
            offset += m.get_text_content_size();
            matching = m.get_next_sibling();
        }
        assert!(
            matching
                .as_ref()
                .is_none_or(|m| m.get_type() == "linebreak" || m.is_text())
        );
        let point = match &matching {
            None => get_child_caret(&code, CaretDirection::Next).unwrap(),
            Some(m) if m.get_type() != "linebreak" => {
                get_text_point_caret(m, CaretDirection::Next, cursor_offset - offset)?
            }
            Some(m) => get_sibling_caret(m, CaretDirection::Next),
        };
        set_selection_from_caret_range(&get_collapsed_caret_range(point))?;
        ed.dispatch_command(&OUTDENT_CONTENT_COMMAND, ())?;
        Ok(())
    });
    editor
        .read(|| {
            let selection = get_selection().unwrap().range().unwrap();
            assert_eq!(selection.focus.point_type, PointType::Text);
            let mut cursor = 0;
            let mut node = code_node.get_first_child();
            while let Some(n) = node.filter(|n| n.is_text() || n.get_type() == "linebreak") {
                if selection.focus.key == n.get_key() {
                    cursor += selection.focus.offset;
                    break;
                }
                cursor += n.get_text_content_size();
                node = n.get_next_sibling();
            }
            (code_node.get_text_content(), cursor)
        })
        .unwrap()
}

// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > tabSize=2 strips two leading spaces
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > tabSize=2 strips one indent level from …
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > tabSize=2 best-effort: line with one le…
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > tabSize=4 strips four leading spaces
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > tabSize=4 best-effort: two leading spac…
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > no-op when line has no leading whitespa…
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > TabNode at the start takes precedence o…
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > tabSize=undefined preserves existing Ta…
// upstream: lexical-code-prism/src/__tests__/unit/LexicalCodeNodeTabs.test.ts::LexicalCodeNode tests > tabSize (#8410): outdent space-indented lines > tabSize=0 is rejected by the guard (non…
#[test]
fn tab_size_outdent_space_indented_lines() {
    assert_eq!(OUTDENT_SCENARIOS.len(), 9);
    for &(raw_text, cursor_offset, tab_size, expected_text, expected_cursor) in OUTDENT_SCENARIOS {
        let editor = build_editor_from_extensions([define_extension(LexicalExtension {
            dependencies: vec![
                crate::rich_text::rich_text_extension().into(),
                tab_indentation_extension().into(),
                code_highlighter_extension().into(),
                config_extension(
                    &code_indent_extension(),
                    [CodeIndentConfigOverrides {
                        tab_size: Some(tab_size),
                        ..Default::default()
                    }],
                ),
            ],
            ..LexicalExtension::<(), (), ()>::new("prism-outdent", ())
        })
        .into()])
        .unwrap();
        let (text, cursor) = run_outdent_scenario(&editor, raw_text, cursor_offset);
        assert_eq!(text, expected_text, "{raw_text:?} tabSize {tab_size:?}");
        if let Some(expected_cursor) = expected_cursor {
            assert_eq!(cursor, expected_cursor, "{raw_text:?} tabSize {tab_size:?}");
        }
    }
}
