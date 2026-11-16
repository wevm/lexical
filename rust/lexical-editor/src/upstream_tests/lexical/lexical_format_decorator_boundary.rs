//! Ports of `packages/lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts`.

#![cfg(feature = "extension")]

use crate::constants::IS_BOLD;
use crate::extension::build_editor_from_extensions;
use crate::extension::{DecoratorTextNode, create_decorator_text_node};
use crate::upstream_tests::support::update;
use crate::{
    LexicalExtension, NodeClass, PointType, Which, create_paragraph_node, create_range_selection,
    create_text_node, define_extension, get_root, set_selection,
};

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    Partial,
    Whole,
    Multiple,
    EmptyEnd,
}

fn formatting_a_text_edge_and_decorator(backward: bool, active: bool, shape: Shape) {
    let editor = build_editor_from_extensions([define_extension(LexicalExtension {
        nodes: vec![NodeClass::of::<DecoratorTextNode>()],
        ..LexicalExtension::<(), (), ()>::new("format-decorator-boundary", ())
    })
    .into()])
    .unwrap();
    update(&editor, move || {
        let first = create_text_node("A");
        let decorator = create_decorator_text_node();
        let text = create_text_node("BC");
        let last = create_text_node("D");
        last.set_style("color: red;");
        let paragraph = create_paragraph_node();
        paragraph.append(&[first.clone(), decorator.clone(), text.clone()])?;
        let to_last = matches!(shape, Shape::Multiple | Shape::EmptyEnd);
        if to_last {
            paragraph.append(std::slice::from_ref(&last))?;
        }
        get_root().append(&[paragraph])?;
        let selection = create_range_selection();
        let (start, end) = if backward {
            (Which::Focus, Which::Anchor)
        } else {
            (Which::Anchor, Which::Focus)
        };
        selection.point_set(start, first.get_key(), 1, PointType::Text)?;
        selection.point_set(
            end,
            if to_last {
                last.get_key()
            } else {
                text.get_key()
            },
            match shape {
                Shape::EmptyEnd => 0,
                Shape::Whole => 2,
                _ => 1,
            },
            PointType::Text,
        )?;
        if active {
            set_selection(Some(selection.clone()));
        }
        assert!(decorator.is_selected(Some(&selection))?);
        selection.format_text("bold", None)?;
        assert_eq!(
            DecoratorTextNode::get_format(&decorator)?,
            f64::from(IS_BOLD)
        );
        assert_eq!(
            decorator.is_selected(Some(&selection))?,
            shape == Shape::Whole
        );
        let start_point = selection.point(start).unwrap();
        assert_eq!(
            start_point.key,
            if shape == Shape::Whole {
                first.get_key()
            } else {
                text.get_key()
            }
        );
        assert_eq!(start_point.offset, usize::from(shape == Shape::Whole));
        assert_eq!(selection.is_backward()?, backward);
        selection.format_text("bold", None)?;
        assert_eq!(text.text_data().get_format(), 0.0);
        assert_eq!(
            DecoratorTextNode::get_format(&decorator)?,
            if shape == Shape::Whole {
                0.0
            } else {
                f64::from(IS_BOLD)
            }
        );
        Ok(())
    });
    editor.dispose();
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=false, shape=partial)
#[test]
fn forward_inactive_partial() {
    formatting_a_text_edge_and_decorator(false, false, Shape::Partial);
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=false, shape=whole)
#[test]
fn forward_inactive_whole() {
    formatting_a_text_edge_and_decorator(false, false, Shape::Whole);
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=false, shape=multiple)
#[test]
fn forward_inactive_multiple() {
    formatting_a_text_edge_and_decorator(false, false, Shape::Multiple);
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=false, shape=empty-end)
#[test]
fn forward_inactive_empty_end() {
    formatting_a_text_edge_and_decorator(false, false, Shape::EmptyEnd);
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=true, shape=partial)
#[test]
fn forward_active_partial() {
    formatting_a_text_edge_and_decorator(false, true, Shape::Partial);
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=true, shape=whole)
#[test]
fn forward_active_whole() {
    formatting_a_text_edge_and_decorator(false, true, Shape::Whole);
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=true, shape=multiple)
#[test]
fn forward_active_multiple() {
    formatting_a_text_edge_and_decorator(false, true, Shape::Multiple);
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=false, active=true, shape=empty-end)
#[test]
fn forward_active_empty_end() {
    formatting_a_text_edge_and_decorator(false, true, Shape::EmptyEnd);
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=false, shape=partial)
#[test]
fn backward_inactive_partial() {
    formatting_a_text_edge_and_decorator(true, false, Shape::Partial);
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=false, shape=whole)
#[test]
fn backward_inactive_whole() {
    formatting_a_text_edge_and_decorator(true, false, Shape::Whole);
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=false, shape=multiple)
#[test]
fn backward_inactive_multiple() {
    formatting_a_text_edge_and_decorator(true, false, Shape::Multiple);
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=false, shape=empty-end)
#[test]
fn backward_inactive_empty_end() {
    formatting_a_text_edge_and_decorator(true, false, Shape::EmptyEnd);
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=true, shape=partial)
#[test]
fn backward_active_partial() {
    formatting_a_text_edge_and_decorator(true, true, Shape::Partial);
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=true, shape=whole)
#[test]
fn backward_active_whole() {
    formatting_a_text_edge_and_decorator(true, true, Shape::Whole);
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=true, shape=multiple)
#[test]
fn backward_active_multiple() {
    formatting_a_text_edge_and_decorator(true, true, Shape::Multiple);
}

// upstream: lexical/src/__tests__/unit/LexicalFormatDecoratorBoundary.test.ts::formatting a text edge and decorator (backward=true, active=true, shape=empty-end)
#[test]
fn backward_active_empty_end() {
    formatting_a_text_edge_and_decorator(true, true, Shape::EmptyEnd);
}
