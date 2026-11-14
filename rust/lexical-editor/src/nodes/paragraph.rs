//! `nodes/LexicalParagraphNode.ts`.

use crate::LexicalError;
use crate::generated_json::{
    export_compact_element_node, export_element_node, update_element_node,
};
use crate::json::{Object, Value};
use crate::lexical_selection::SelectionRef;
use crate::node::{LexicalNode, NodeBase, NodeRef};
use crate::nodes::element::ElementData;
use crate::selection_deletion::is_js_whitespace;

/// `$createParagraphNode()`.
pub fn create_paragraph_node() -> NodeRef {
    crate::lexical_utils::set_node_key(Box::new(ParagraphNode::default()))
}

#[derive(Clone, Debug, Default)]
pub struct ParagraphNode {
    base: NodeBase,
    element: ElementData,
}

impl LexicalNode for ParagraphNode {
    fn get_type(&self) -> &'static str {
        "paragraph"
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

    fn export_json(&self, compact: bool) -> Object {
        let mut json = if compact {
            export_compact_element_node(self)
        } else {
            export_element_node(self)
        };
        // Provide backwards compatible values, see #7971.
        let absent = |json: &Object, key: &str| json.get(key).is_none_or(Value::is_undefined);
        if absent(&json, "textFormat") || absent(&json, "textStyle") {
            let first_text_node = NodeRef::new(self.base.key.clone())
                .get_children()
                .into_iter()
                .map(|child| child.get_latest())
                .find(|child| child.text().is_some());
            let (text_format, text_style) = match &first_text_node {
                Some(text) => {
                    let text = text.text().expect("a text node");
                    (text.format, text.style.clone())
                }
                None => (self.element.text_format, self.element.text_style.clone()),
            };
            if !compact || text_format != 0.0 {
                json.insert("textFormat", text_format.into());
            }
            if !compact || !text_style.is_empty() {
                json.insert("textStyle", text_style.into());
            }
        }
        json
    }

    fn update_from_json(&mut self, json: &Object) {
        update_element_node(&mut self.element, json);
    }

    /// `ParagraphNode.insertNewAfter`: a paragraph with the selection's
    /// format and style as its text format and style, and this paragraph's
    /// direction, alignment and style.
    fn insert_new_after(
        &self,
        this: &NodeRef,
        selection: &SelectionRef,
        restore_selection: bool,
    ) -> Result<Option<NodeRef>, LexicalError> {
        let (format, style) = selection
            .with_range(|r| (r.format, r.style.clone()))
            .unwrap_or_default();
        let new_element = create_paragraph_node();
        new_element.set_text_format(format);
        new_element.set_text_style(&style);
        new_element.set_direction(self.element.get_direction());
        new_element.set_element_format(self.element.get_format_type());
        new_element.set_element_style(self.element.get_style());
        this.insert_after(&new_element, restore_selection)?;
        Ok(Some(new_element))
    }

    /// `ParagraphNode.collapseAtStart()`: an empty (trimmed) paragraph is
    /// removed as long as there is a sibling to go to. Every child has to
    /// be blank text: a non-text child is content even with no text.
    fn collapse_at_start(&self, this: &NodeRef, _: &SelectionRef) -> Result<bool, LexicalError> {
        if this
            .get_children()
            .iter()
            .all(|child| child.is_text() && child.get_text_content().chars().all(is_js_whitespace))
        {
            if this.get_next_sibling().is_some() {
                this.select_next(None, None)?;
                this.remove(false)?;
                return Ok(true);
            }
            if this.get_previous_sibling().is_some() {
                this.select_previous(None, None)?;
                this.remove(false)?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `ParagraphNode.extractWithChild(child, selection, destination)`: a
    /// paragraph with an alignment, an indent or a style travels as a block
    /// when the selection covers all of its text, as those live nowhere
    /// else.
    fn extract_with_child(
        &self,
        this: &NodeRef,
        _child: &NodeRef,
        selection: Option<&SelectionRef>,
        _destination: &str,
    ) -> Result<bool, LexicalError> {
        let Some(range) = selection.and_then(SelectionRef::range) else {
            return Ok(false);
        };
        if self.element.get_format_type().is_empty()
            && self.element.get_indent() == 0.0
            && self.element.get_style().is_empty()
        {
            return Ok(false);
        }
        if crate::selection_deletion::is_block_fully_selected(this, &range)? {
            let text_content = this.get_text_content();
            return Ok(!text_content.is_empty()
                && selection.expect("a range selection").get_text_content()? == text_content);
        }
        Ok(false)
    }

    /// `ParagraphNode.createDOM(config)`: a `<p>` with the theme's
    /// `paragraph` class names.
    #[cfg(feature = "html")]
    fn create_dom(
        &self,
        _this: &NodeRef,
        config: &crate::EditorConfig,
        _editor: &crate::Editor,
    ) -> Result<crate::dom::DomNode, LexicalError> {
        let dom = crate::lexical_dom::get_document().create_element("p");
        if let Some(class_names) =
            crate::lexical_dom::get_cached_class_name_array(&config.theme, "paragraph")
            && !class_names.is_empty()
        {
            dom.class_list().add(&class_names);
        }
        Ok(dom)
    }

    /// `ParagraphNode.exportDOM(editor)`: `ElementNode`'s, with a `<br>` in
    /// an empty paragraph and the alignment as `text-align`.
    #[cfg(feature = "html")]
    fn export_dom(
        &self,
        this: &NodeRef,
        editor: &crate::Editor,
    ) -> Result<crate::lexical_dom::DomExportOutput, LexicalError> {
        let output = crate::nodes::element::export_dom(self, this, editor)?;
        if let Some(element) = output.element.as_ref().filter(|e| e.is_element()) {
            if this.is_empty() {
                element.append_child(&crate::lexical_dom::get_document().create_element("br"));
            }
            let format_type = self.element.get_format_type();
            if !format_type.is_empty() {
                element.style().set_property("text-align", format_type, "");
            }
        }
        Ok(crate::lexical_dom::DomExportOutput {
            element: output.element,
            ..Default::default()
        })
    }

    /// `ParagraphNode`'s `importDOM`: `<p>`.
    #[cfg(feature = "html")]
    fn import_dom(&self) -> Option<crate::lexical_dom::DomConversionMap> {
        Some(vec![(
            "p".to_owned(),
            crate::lexical_dom::importer(0, convert_paragraph_element),
        )])
    }
}

/// `$convertParagraphElement(element)`: a paragraph with the element's
/// alignment (from `text-align`, else the legacy `align` attribute),
/// indent and direction.
#[cfg(feature = "html")]
fn convert_paragraph_element(
    element: &crate::dom::DomNode,
) -> Result<Option<crate::lexical_dom::DomConversionOutput>, LexicalError> {
    use crate::lexical_dom::{
        is_element_format_type, set_direction_from_dom, set_format_from_dom,
        set_node_indent_from_dom,
    };
    let node = create_paragraph_node();
    set_format_from_dom(&node, element);
    set_node_indent_from_dom(element, &node)?;
    if node.element_data().get_format_type().is_empty()
        && let Some(align) = element.get_attribute("align")
        && is_element_format_type(&align)
    {
        node.set_element_format(&align);
    }
    set_direction_from_dom(&node, element);
    Ok(Some(crate::lexical_dom::DomConversionOutput::node(Some(
        node,
    ))))
}
