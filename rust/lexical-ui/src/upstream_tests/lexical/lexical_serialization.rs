//! Ports of `packages/lexical/src/__tests__/unit/LexicalSerialization.test.ts`.

#![cfg(all(
    feature = "rich-text",
    feature = "list",
    feature = "link",
    feature = "code",
    feature = "table"
))]

use crate::code::{CodeHighlightNode, CodeNode, create_code_highlight_node, create_code_node};
use crate::json::parse;
use crate::link::{LinkAttributes, create_link_node};
use crate::list::{ListType, create_list_item_node, create_list_node};
use crate::rich_text::{HeadingTagType, create_heading_node, create_quote_node};
use crate::table::{
    InsertTableCommandPayloadHeaders, create_table_node_with_dimensions, table_nodes,
};
use crate::upstream_tests::support::{initialize_unit_test, json_to_equal, update};
use crate::{
    CreateEditorArgs, LexicalError, NodeClass, NodeRef, create_paragraph_node, create_text_node,
    get_root,
};

/// `node.append(...children)`, returning the node.
fn append(node: NodeRef, children: &[NodeRef]) -> Result<NodeRef, LexicalError> {
    node.append(children)?;
    Ok(node)
}

/// `$createTextNode(text).toggleFormat(format)`.
fn formatted(text: &str, format: &str) -> NodeRef {
    let node = create_text_node(text);
    node.toggle_format(format);
    node
}

/// A list item of a text, a link of `url` holding `link_text`, and a text.
fn link_item(
    before: &str,
    url: &str,
    link_text: &str,
    after: &str,
) -> Result<NodeRef, LexicalError> {
    append(
        create_list_item_node(None),
        &[
            create_text_node(before),
            append(
                create_link_node(url, &LinkAttributes::default()),
                &[create_text_node(link_text)],
            )?,
            create_text_node(after),
        ],
    )
}

/// `$createEditorContent()`.
fn create_editor_content() -> Result<(), LexicalError> {
    let root = get_root();
    if root.get_first_child().is_some() {
        return Ok(());
    }
    root.append(&[append(
        create_heading_node(HeadingTagType::H1),
        &[create_text_node("Welcome to the playground")],
    )?])?;
    root.append(&[append(
        create_quote_node(false),
        &[create_text_node(concat!(
            "In case you were wondering what the black box at the bottom is \u{2013} it's the debug view, showing the current state of the editor. ",
            "You can disable it by pressing on the settings control in the bottom-left of your screen and toggling the debug view setting."
        ))],
    )?])?;
    root.append(&[append(
        create_paragraph_node(),
        &[
            create_text_node("The playground is a demo environment built with "),
            formatted("@lexical/react", "code"),
            create_text_node("."),
            create_text_node(" Try typing in "),
            formatted("some text", "bold"),
            create_text_node(" with "),
            formatted("different", "italic"),
            create_text_node(" formats."),
        ],
    )?])?;
    root.append(&[append(
        create_paragraph_node(),
        &[create_text_node(
            "Make sure to check out the various plugins in the toolbar. You can also use #hashtags or @-mentions too!",
        )],
    )?])?;
    root.append(&[append(
        create_paragraph_node(),
        &[create_text_node(
            "If you'd like to find out more about Lexical, you can:",
        )],
    )?])?;
    root.append(&[append(
        create_list_node(ListType::Bullet, 1.0),
        &[
            link_item(
                "Visit the ",
                "https://lexical.dev/",
                "Lexical website",
                " for documentation and more information.",
            )?,
            link_item(
                "Check out the code on our ",
                "https://github.com/meta/lexical",
                "GitHub repository",
                ".",
            )?,
            link_item(
                "Playground code can be found ",
                "https://github.com/meta/lexical/tree/main/packages/lexical-playground",
                "here",
                ".",
            )?,
            link_item(
                "Join our ",
                "https://discord.com/invite/KmG4wQnnD9",
                "Discord Server",
                " and chat with the team.",
            )?,
        ],
    )?])?;
    root.append(&[append(
        create_paragraph_node(),
        &[create_text_node(
            "Lastly, we're constantly adding cool new features to this playground. So make sure you check back here when you next get a chance :).",
        )],
    )?])?;
    root.append(&[append(
        create_code_node(Some("javascript"), None),
        &[create_code_highlight_node(
            "const lexical = \"awesome\"",
            None,
        )],
    )?])?;
    root.append(&[create_table_node_with_dimensions(
        5,
        5,
        InsertTableCommandPayloadHeaders::Both(true),
    )?])
}

const EXPECTED_STRINGIFIED_EDITOR_STATE: &str = r#"{"root":{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Welcome to the playground","type":"text","version":1}],"direction":null,"format":"","indent":0,"type":"heading","version":1,"tag":"h1"},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"In case you were wondering what the black box at the bottom is – it's the debug view, showing the current state of the editor. You can disable it by pressing on the settings control in the bottom-left of your screen and toggling the debug view setting.","type":"text","version":1}],"direction":null,"format":"","indent":0,"type":"quote","version":1},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"The playground is a demo environment built with ","type":"text","version":1},{"detail":0,"format":16,"mode":"normal","style":"","text":"@lexical/react","type":"text","version":1},{"detail":0,"format":0,"mode":"normal","style":"","text":". Try typing in ","type":"text","version":1},{"detail":0,"format":1,"mode":"normal","style":"","text":"some text","type":"text","version":1},{"detail":0,"format":0,"mode":"normal","style":"","text":" with ","type":"text","version":1},{"detail":0,"format":2,"mode":"normal","style":"","text":"different","type":"text","version":1},{"detail":0,"format":0,"mode":"normal","style":"","text":" formats.","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Make sure to check out the various plugins in the toolbar. You can also use #hashtags or @-mentions too!","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"If you'd like to find out more about Lexical, you can:","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1},{"children":[{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Visit the ","type":"text","version":1},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Lexical website","type":"text","version":1}],"direction":null,"format":"","indent":0,"type":"link","version":1,"rel":null,"target":null,"title":null,"url":"https://lexical.dev/"},{"detail":0,"format":0,"mode":"normal","style":"","text":" for documentation and more information.","type":"text","version":1}],"direction":null,"format":"","indent":0,"type":"listitem","version":1,"value":1},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Check out the code on our ","type":"text","version":1},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"GitHub repository","type":"text","version":1}],"direction":null,"format":"","indent":0,"type":"link","version":1,"rel":null,"target":null,"title":null,"url":"https://github.com/meta/lexical"},{"detail":0,"format":0,"mode":"normal","style":"","text":".","type":"text","version":1}],"direction":null,"format":"","indent":0,"type":"listitem","version":1,"value":2},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Playground code can be found ","type":"text","version":1},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"here","type":"text","version":1}],"direction":null,"format":"","indent":0,"type":"link","version":1,"rel":null,"target":null,"title":null,"url":"https://github.com/meta/lexical/tree/main/packages/lexical-playground"},{"detail":0,"format":0,"mode":"normal","style":"","text":".","type":"text","version":1}],"direction":null,"format":"","indent":0,"type":"listitem","version":1,"value":3},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Join our ","type":"text","version":1},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Discord Server","type":"text","version":1}],"direction":null,"format":"","indent":0,"type":"link","version":1,"rel":null,"target":null,"title":null,"url":"https://discord.com/invite/KmG4wQnnD9"},{"detail":0,"format":0,"mode":"normal","style":"","text":" and chat with the team.","type":"text","version":1}],"direction":null,"format":"","indent":0,"type":"listitem","version":1,"value":4}],"direction":null,"format":"","indent":0,"type":"list","version":1,"listType":"bullet","start":1,"tag":"ul"},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"Lastly, we're constantly adding cool new features to this playground. So make sure you check back here when you next get a chance :).","type":"text","version":1}],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1},{"children":[{"detail":0,"format":0,"mode":"normal","style":"","text":"const lexical = \"awesome\"","type":"code-highlight","version":1}],"direction":null,"format":"","indent":0,"type":"code","version":1,"language":"javascript"},{"children":[{"children":[{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":3,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":1,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":1,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":1,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":1,"rowSpan":1}],"direction":null,"format":"","indent":0,"type":"tablerow","version":1},{"children":[{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":2,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1}],"direction":null,"format":"","indent":0,"type":"tablerow","version":1},{"children":[{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":2,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1}],"direction":null,"format":"","indent":0,"type":"tablerow","version":1},{"children":[{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":2,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1}],"direction":null,"format":"","indent":0,"type":"tablerow","version":1},{"children":[{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":2,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1},{"children":[{"children":[],"direction":null,"format":"","indent":0,"textFormat":0,"textStyle":"","type":"paragraph","version":1}],"direction":null,"format":"","indent":0,"type":"tablecell","version":1,"backgroundColor":null,"colSpan":1,"headerState":0,"rowSpan":1}],"direction":null,"format":"","indent":0,"type":"tablerow","version":1}],"direction":null,"format":"","indent":0,"type":"table","version":1}],"direction":null,"format":"","indent":0,"type":"root","version":1}}"#;

// upstream: lexical/src/__tests__/unit/LexicalSerialization.test.ts::LexicalSerialization tests > serializes and deserializes from JSON
#[test]
fn serializes_and_deserializes_from_json() {
    let mut nodes = vec![
        NodeClass::of::<CodeNode>(),
        NodeClass::of::<CodeHighlightNode>(),
    ];
    nodes.extend(table_nodes());
    let editor = initialize_unit_test(CreateEditorArgs {
        nodes,
        ..CreateEditorArgs::default()
    });
    update(&editor, create_editor_content);
    let stringified_editor_state = editor.get_editor_state().to_json(false).to_string();
    let expected = parse(EXPECTED_STRINGIFIED_EDITOR_STATE).unwrap();
    assert!(
        json_to_equal(&parse(&stringified_editor_state).unwrap(), &expected),
        "{stringified_editor_state}"
    );
    // Check that this JSON round-trips
    let editor_state = editor
        .parse_editor_state(&stringified_editor_state)
        .unwrap();
    let other_stringified_editor_state = editor_state.to_json(false).to_string();
    assert!(
        json_to_equal(&parse(&other_stringified_editor_state).unwrap(), &expected),
        "{other_stringified_editor_state}"
    );
}
