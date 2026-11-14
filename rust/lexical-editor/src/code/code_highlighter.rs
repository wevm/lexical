//! The highlighting loop of
//! `packages/lexical-code-prism/src/CodeHighlighterPrism.ts`, with the lazy
//! languages of `packages/lexical-code-shiki/src/CodeHighlighterShiki.ts`,
//! over any [`Tokenizer`] rather than Prism: a code block's text is
//! tokenized again when it changes, only the children that differ from the
//! new tokens are replaced, and a caret inside the block keeps its text
//! offset.

use super::code_highlight_node::{CodeHighlightNode, is_code_highlight_node};
use super::code_indentation::register_code_indentation;
use super::code_node::is_code_node;
use super::code_tokenizer::{CodeLanguageState, Tokenizer};
use super::flat_structure_utils::plainify_code_content;
use crate::constants::NODE_STATE_KEY;
use crate::error::invariant;
use crate::lexical_selection::{PointType, Which, get_selection};
use crate::node::NodeRef;
use crate::node_state::get_writable_node_state;
use crate::nodes::line_break::LineBreakNode;
use crate::nodes::tab::TabNode;
use crate::nodes::text::create_text_node;
use crate::updates::on_update;
use crate::{
    Editor, HISTORY_MERGE_TAG, LexicalError, NodeKey, Unregister, UpdateOptions, get_node_by_key,
};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

fn is_tab_node(node: &NodeRef) -> bool {
    node.get_latest().is::<TabNode>()
}

fn is_line_break_node(node: &NodeRef) -> bool {
    node.get_latest().is::<LineBreakNode>()
}

/// `language || defaultLanguage`: a code block's language never is `""`.
fn language_or_default(node: &NodeRef, tokenizer: &dyn Tokenizer) -> Option<String> {
    node.get_language()
        .or_else(|| tokenizer.default_language().map(str::to_owned))
}

/// `$copyNodeState(from, to)`: gives `to` the NodeState of `from`, flat
/// states included, keeping the state `to` has for the keys `from` doesn't
/// set.
fn copy_node_state(from: &NodeRef, to: &NodeRef) -> Result<(), LexicalError> {
    let json = match &from.get_latest().base().state {
        Some(state) => state.borrow().to_json(),
        None => return Ok(()),
    };
    if json.is_empty() {
        return Ok(());
    }
    let state = get_writable_node_state(to)?;
    let mut state = state.borrow_mut();
    for (key, value) in json.iter() {
        match value.as_object() {
            Some(nested) if key == NODE_STATE_KEY => {
                for (nested_key, nested_value) in nested.iter() {
                    state.update_from_unknown(nested_key, Rc::new(nested_value.clone()));
                }
            }
            _ => state.update_from_unknown(key, Rc::new(value.clone())),
        }
    }
    Ok(())
}

/// `TransformState`. `nodesCurrentlyHighlighting` keeps the CodeNode and
/// text transforms, which both run when a token is inserted, from
/// highlighting a block twice in one update.
#[derive(Default)]
struct TransformState {
    did_transform: bool,
    nodes_currently_highlighting: HashSet<NodeKey>,
    /// The blocks waiting for each language that is loading, so that a
    /// language is loaded once however many transforms ask for it (Swift's;
    /// Shiki's `loadCodeLanguage` loads per block).
    waiting_for_language: Vec<(String, Vec<NodeKey>)>,
}

struct Highlighter {
    editor: std::rc::Weak<crate::editor::EditorInner>,
    tokenizer: Rc<dyn Tokenizer>,
    state: RefCell<TransformState>,
}

impl Highlighter {
    /// `$textNodeTransform(editor, tokenizer, transformState, node)`.
    fn text_node_transform(self: &Rc<Self>, node: &NodeRef) -> Result<(), LexicalError> {
        // A CodeNode's children are flat, so only the parent needs checking.
        let parent_node = node.get_parent();
        if let Some(parent_node) = parent_node.filter(is_code_node) {
            self.code_node_transform(&parent_node)
        } else if is_code_highlight_node(node) {
            // A code block turned into another block: its tokens are text
            // again, keeping their NodeState (such as comment ids).
            let text = create_text_node(&node.text_data().text);
            copy_node_state(node, &text)?;
            node.replace(&text, false)?;
            Ok(())
        } else {
            Ok(())
        }
    }

    /// `$codeNodeTransform(editor, tokenizer, transformState, node)`.
    fn code_node_transform(self: &Rc<Self>, node: &NodeRef) -> Result<(), LexicalError> {
        let tokenizer = &*self.tokenizer;
        let node_key = node.get_key().to_owned();
        // A new block may have no language. A tokenizer without a default
        // leaves it unset, so Markdown round-trips a fence without an info
        // string.
        if node.get_language().is_none()
            && let Some(default_language) = tokenizer.default_language()
        {
            node.set_language(Some(default_language));
        }
        if let Some(language) = language_or_default(node, tokenizer) {
            match tokenizer.language_state(&language) {
                CodeLanguageState::Loaded => {
                    if !node.get_is_syntax_highlight_supported() {
                        node.set_is_syntax_highlight_supported(true);
                    }
                }
                CodeLanguageState::Loading => {
                    // The grammar is on its way (Shiki's `loadCodeLanguage`):
                    // the block is highlighted once it has loaded.
                    self.wait_for_language(language, node_key);
                    return Ok(());
                }
                CodeLanguageState::Unsupported => {
                    // An unknown language ("```After ") falls through to
                    // plain tokens, so the indent and line-move handlers can
                    // work on it.
                    if node.get_is_syntax_highlight_supported() {
                        node.set_is_syntax_highlight_supported(false);
                    }
                }
            }
        } else if node.get_is_syntax_highlight_supported() {
            node.set_is_syntax_highlight_supported(false);
        }

        {
            let mut state = self.state.borrow_mut();
            if !state.nodes_currently_highlighting.insert(node_key.clone()) {
                return Ok(());
            }
            if !state.did_transform {
                state.did_transform = true;
                let this = Rc::downgrade(self);
                on_update(move || {
                    if let Some(this) = this.upgrade() {
                        let mut state = this.state.borrow_mut();
                        state.did_transform = false;
                        state.nodes_currently_highlighting.clear();
                    }
                });
            }
        }

        update_and_retain_selection(&node_key, || {
            let Some(current_node) = get_node_by_key(&node_key) else {
                return Ok(false);
            };
            if !is_code_node(&current_node) || !current_node.is_attached() {
                return Ok(false);
            }
            let current_language = language_or_default(&current_node, tokenizer);
            let highlight_nodes = match &current_language {
                Some(language)
                    if tokenizer.language_state(language) != CodeLanguageState::Loaded =>
                {
                    plainify_code_content(&current_node.get_text_content())
                }
                _ => tokenizer.tokenize_code_node(&current_node, current_language.as_deref()),
            };
            let (from, to, nodes_for_replacement) =
                get_diff_range(&current_node.get_children(), &highlight_nodes);
            if from != to || !nodes_for_replacement.is_empty() {
                node.splice(from, to - from, nodes_for_replacement)?;
                return Ok(true);
            }
            Ok(false)
        })
    }

    /// Adds `node_key` to the blocks waiting for `language`, and asks the
    /// tokenizer to load it when nothing waited for it yet. Once it has
    /// loaded, an update tagged `history-merge` marks each waiting block
    /// that still has the language highlighted and dirty, so the transform
    /// highlights it (`loadCodeLanguage`'s callback).
    fn wait_for_language(self: &Rc<Self>, language: String, node_key: NodeKey) {
        {
            let mut state = self.state.borrow_mut();
            if let Some((_, keys)) = state
                .waiting_for_language
                .iter_mut()
                .find(|(l, _)| *l == language)
            {
                if !keys.contains(&node_key) {
                    keys.push(node_key);
                }
                return;
            }
            state
                .waiting_for_language
                .push((language.clone(), vec![node_key]));
        }
        let this = Rc::downgrade(self);
        let loaded = language.clone();
        self.tokenizer.load_language(
            &language,
            Box::new(move || {
                let Some(this) = this.upgrade() else {
                    return;
                };
                let keys = {
                    let mut state = this.state.borrow_mut();
                    let index = state
                        .waiting_for_language
                        .iter()
                        .position(|(l, _)| *l == loaded);
                    index.map_or_else(Vec::new, |i| state.waiting_for_language.remove(i).1)
                };
                let Some(editor) = this.editor.upgrade().map(Editor) else {
                    return;
                };
                let tokenizer = this.tokenizer.clone();
                let _ = editor.update(
                    move || {
                        for key in keys {
                            let Some(code_node) = get_node_by_key(&key).filter(is_code_node) else {
                                continue;
                            };
                            if language_or_default(&code_node, &*tokenizer).as_deref()
                                == Some(loaded.as_str())
                            {
                                code_node.set_is_syntax_highlight_supported(true);
                                code_node.mark_dirty();
                            }
                        }
                        Ok(())
                    },
                    UpdateOptions {
                        tags: vec![HISTORY_MERGE_TAG.to_owned()],
                        ..UpdateOptions::default()
                    },
                );
            }),
        );
    }
}

/// `$updateAndRetainSelection(nodeKey, updateFn)`: runs `update_fn`, and
/// when it changed the block, puts a caret that was in it back at the same
/// text offset.
fn update_and_retain_selection(
    node_key: &str,
    update_fn: impl FnOnce() -> Result<bool, LexicalError>,
) -> Result<(), LexicalError> {
    let Some(node) = get_node_by_key(node_key) else {
        return Ok(());
    };
    if !is_code_node(&node) || !node.is_attached() {
        return Ok(());
    }
    // Only a range selection inside this block is retained: restoring one
    // elsewhere would drag the caret into the block.
    let Some(selection) = get_selection().filter(|s| s.range().is_some()) else {
        update_fn()?;
        return Ok(());
    };
    let anchor = selection.point(Which::Anchor).expect("a range selection");
    let anchor_node = anchor.get_node()?;
    if anchor_node != node && !node.is_parent_of(&anchor_node) {
        update_fn()?;
        return Ok(());
    }
    let anchor_offset = anchor.offset;
    let is_new_line_anchor = anchor.point_type == PointType::Element
        && anchor_offset
            .checked_sub(1)
            .and_then(|i| node.get_child_at_index(i))
            .is_some_and(|child| is_line_break_node(&child));
    let mut text_offset = 0;
    // The text before the anchor (the text of its node's previous
    // siblings), and the anchor's own offset.
    if !is_new_line_anchor {
        text_offset = anchor_offset
            + anchor_node
                .get_previous_siblings()
                .iter()
                .map(NodeRef::get_text_content_size)
                .sum::<usize>();
    }

    if !update_fn()? {
        return Ok(());
    }

    // Only a line break puts the anchor on the element.
    if is_new_line_anchor {
        let anchor = selection.point(Which::Anchor).expect("a range selection");
        anchor
            .get_node()?
            .select(Some(anchor_offset), Some(anchor_offset))?;
        return Ok(());
    }
    // Walk to the text offset. A LineBreakNode takes one unit but holds no
    // text point, so an offset that lands on one is an element point.
    let children = node.get_children();
    for (index, child) in children.iter().enumerate() {
        if child.is_text() {
            let text_content_size = child.get_text_content_size();
            if text_content_size >= text_offset {
                child.select(Some(text_offset), Some(text_offset))?;
                return Ok(());
            }
            text_offset -= text_content_size;
        } else if is_line_break_node(child) {
            if text_offset == 0 {
                node.select(Some(index), Some(index))?;
                return Ok(());
            }
            text_offset -= 1;
        }
    }
    node.select(Some(children.len()), Some(children.len()))?;
    Ok(())
}

/// `getDiffRange(prevNodes, nextNodes)`: the range `from..to` of
/// `prev_nodes` to replace with the part of `next_nodes` that differs, so
/// that the children the new tokens match are kept.
fn get_diff_range<'a>(
    prev_nodes: &[NodeRef],
    next_nodes: &'a [NodeRef],
) -> (usize, usize, &'a [NodeRef]) {
    let mut leading_match = 0;
    while leading_match < prev_nodes.len() {
        match next_nodes.get(leading_match) {
            Some(next) if is_equal(&prev_nodes[leading_match], next) => leading_match += 1,
            _ => break,
        }
    }
    let prev_nodes_length = prev_nodes.len();
    let next_nodes_length = next_nodes.len();
    let max_trailing_match = prev_nodes_length.min(next_nodes_length) - leading_match;
    let mut trailing_match = 0;
    while trailing_match < max_trailing_match {
        trailing_match += 1;
        if !is_equal(
            &prev_nodes[prev_nodes_length - trailing_match],
            &next_nodes[next_nodes_length - trailing_match],
        ) {
            trailing_match -= 1;
            break;
        }
    }
    (
        leading_match,
        prev_nodes_length - trailing_match,
        &next_nodes[leading_match..next_nodes_length - trailing_match],
    )
}

/// `isEqual(nodeA, nodeB)`: CodeHighlightNodes of the same text and type,
/// tabs and line breaks. Any other text node differs, so it becomes a
/// token.
fn is_equal(node_a: &NodeRef, node_b: &NodeRef) -> bool {
    let (a, b) = (node_a.get_latest(), node_b.get_latest());
    if let (Some(a), Some(b)) = (
        a.downcast_ref::<CodeHighlightNode>(),
        b.downcast_ref::<CodeHighlightNode>(),
    ) {
        return a.text.text == b.text.text && a.highlight_type == b.highlight_type;
    }
    (is_tab_node(node_a) && is_tab_node(node_b))
        || (is_line_break_node(node_a) && is_line_break_node(node_b))
}

/// `registerHighlightingOnly(editor, tokenizer)`: the highlighting
/// transforms of `CodeNode`, `TextNode` and `CodeHighlightNode`, without
/// the indent handlers of [`register_code_indentation`] and without
/// `registerCodeGutter`, which writes to the DOM. Returns what unregisters
/// them.
pub fn register_highlighting_only(editor: &Editor, tokenizer: Rc<dyn Tokenizer>) -> Unregister {
    let highlighter = Rc::new(Highlighter {
        editor: editor.weak(),
        tokenizer,
        state: RefCell::default(),
    });
    let code = highlighter.clone();
    let text = highlighter.clone();
    let code_highlight = highlighter;
    super::merge(vec![
        editor.register_node_transform("code", move |node| code.code_node_transform(node)),
        editor.register_node_transform("text", move |node| text.text_node_transform(node)),
        editor.register_node_transform("code-highlight", move |node| {
            code_highlight.text_node_transform(node)
        }),
    ])
}

/// `registerCodeHighlighting(editor, tokenizer)`: highlighting with
/// `tokenizer` and the indent handlers of [`register_code_indentation`]
/// (without a `tabSize`), for code that doesn't use
/// `code_highlighter_extension`. Fails
/// when the editor lacks `CodeNode` or `CodeHighlightNode`. Returns what
/// unregisters both.
pub fn register_code_highlighting(
    editor: &Editor,
    tokenizer: Rc<dyn Tokenizer>,
) -> Result<Unregister, LexicalError> {
    let nodes = editor.registered_nodes();
    invariant!(
        nodes.contains_key("code") && nodes.contains_key("code-highlight"),
        "CodeHighlightPlugin: CodeNode or CodeHighlightNode not registered on editor"
    );
    Ok(super::merge(vec![
        register_highlighting_only(editor, tokenizer),
        register_code_indentation(editor, None),
    ]))
}

#[cfg(feature = "extension")]
pub use extension::*;

#[cfg(feature = "extension")]
mod extension {
    use super::super::code_extension::{code_extension, code_indent_extension};
    use super::super::code_node::DEFAULT_CODE_LANGUAGE;
    use super::super::code_tokenizer::{PlainTokenizer, Tokenizer};
    use super::register_highlighting_only;
    use crate::Editor;
    use crate::extension::named_signals_config;
    use crate::extension::signals::effect;
    use crate::extension_core::{
        ExtensionBuildState, ExtensionRegisterState, LexicalExtension, define_extension,
    };
    use std::rc::Rc;

    /// A [`Tokenizer`] in a config, compared by identity, as JS compares
    /// objects.
    #[derive(Clone)]
    pub struct TokenizerRef(pub Rc<dyn Tokenizer>);

    impl PartialEq for TokenizerRef {
        fn eq(&self, other: &Self) -> bool {
            Rc::ptr_eq(&self.0, &other.0)
        }
    }

    named_signals_config! {
        /// `CodePrismConfig` / `CodeShikiConfig`.
        pub struct CodeHighlighterConfig;
        /// `Partial<CodePrismConfig>`.
        pub struct CodeHighlighterConfigOverrides;
        /// `namedSignals(CodePrismConfig)`: the extension's output.
        pub struct CodeHighlighterOutput {
            /// `disabled`: the highlighter isn't registered. Flipping the
            /// signal switches it on or off without rebuilding the editor.
            disabled: bool,
            /// `tokenizer`: the engine that tokenizes code.
            tokenizer: TokenizerRef,
        }
    }

    /// `CodePrismExtension`, over the configured tokenizer.
    pub type CodeHighlighterExtensionType =
        LexicalExtension<CodeHighlighterConfig, CodeHighlighterOutput, ()>;

    fn make_code_highlighter_extension() -> Rc<CodeHighlighterExtensionType> {
        define_extension(LexicalExtension {
            build: Some(Rc::new(
                |_: &Editor, config: &CodeHighlighterConfig, _: &ExtensionBuildState<()>| {
                    Ok(CodeHighlighterOutput::of(config))
                },
            )),
            dependencies: vec![code_extension().into(), code_indent_extension().into()],
            register: Some(Rc::new(
                |editor: &Editor,
                 _: &CodeHighlighterConfig,
                 state: &ExtensionRegisterState<(), CodeHighlighterOutput>| {
                    let stores = state
                        .get_output()
                        .expect("CodeHighlighterExtension has a build");
                    let editor = editor.weak();
                    Ok(effect(move || {
                        if stores.disabled.value() {
                            return None;
                        }
                        let tokenizer = stores.tokenizer.value();
                        let editor = editor.upgrade().map(Editor)?;
                        Some(register_highlighting_only(&editor, tokenizer.0))
                    }))
                },
            )),
            // Swift's name: the extension is neither Prism's nor Shiki's.
            ..LexicalExtension::new(
                "@lexical/code-highlighter",
                CodeHighlighterConfig {
                    disabled: false,
                    tokenizer: TokenizerRef(Rc::new(PlainTokenizer {
                        default_language: Some(DEFAULT_CODE_LANGUAGE.to_owned()),
                    })),
                },
            )
        })
    }

    thread_local! {
        static CODE_HIGHLIGHTER_EXTENSION: Rc<CodeHighlighterExtensionType> =
            make_code_highlighter_extension();
    }

    /// `CodePrismExtension`, over the configured tokenizer (by default a
    /// [`PlainTokenizer`] defaulting to `javascript`, since Prism isn't
    /// ported): highlights code blocks with
    /// [`register_highlighting_only`], unless `disabled`.
    /// [`code_extension`] registers the nodes, and [`code_indent_extension`]
    /// brings the indent handlers.
    pub fn code_highlighter_extension() -> Rc<CodeHighlighterExtensionType> {
        CODE_HIGHLIGHTER_EXTENSION.with(Rc::clone)
    }
}

#[cfg(test)]
mod tests {
    use super::super::code_node::create_code_node;
    use super::super::code_nodes;
    use super::super::code_tokenizer::{CodeToken, Tokenizer};
    use super::*;
    use crate::{CreateEditorArgs, create_editor, get_root};
    use std::cell::Cell;

    /// A tokenizer whose one language, `lazy`, loads when it is asked to,
    /// as Shiki's grammars do, and makes the whole block one `lazy` token.
    #[derive(Default)]
    struct LazyTokenizer {
        loaded: Cell<bool>,
        loads: Cell<usize>,
        on_ready: RefCell<Option<Box<dyn FnOnce()>>>,
    }

    impl Tokenizer for LazyTokenizer {
        fn default_language(&self) -> Option<&str> {
            Some("lazy")
        }
        fn language_state(&self, language: &str) -> CodeLanguageState {
            match (language, self.loaded.get()) {
                ("lazy", true) => CodeLanguageState::Loaded,
                ("lazy", false) => CodeLanguageState::Loading,
                _ => CodeLanguageState::Unsupported,
            }
        }
        fn tokenize(&self, code: &str, _language: &str) -> Vec<CodeToken> {
            vec![CodeToken::token("lazy", code)]
        }
        fn load_language(&self, _language: &str, on_ready: Box<dyn FnOnce()>) {
            self.loads.set(self.loads.get() + 1);
            *self.on_ready.borrow_mut() = Some(on_ready);
        }
    }

    fn update(editor: &Editor, f: impl FnOnce() -> Result<(), LexicalError> + 'static) {
        editor
            .update(
                f,
                UpdateOptions {
                    discrete: true,
                    ..UpdateOptions::default()
                },
            )
            .unwrap();
    }

    /// A child's type, text and highlight type.
    type Child = (String, String, Option<String>);

    /// Whether each block is highlighted, and its children.
    fn blocks(editor: &Editor) -> Vec<(bool, Vec<Child>)> {
        editor
            .read(|| {
                get_root()
                    .get_children()
                    .iter()
                    .map(|block| {
                        let children = block
                            .get_children()
                            .iter()
                            .map(|c| {
                                (
                                    c.get_type().to_owned(),
                                    c.get_text_content(),
                                    c.get_highlight_type(),
                                )
                            })
                            .collect();
                        (block.get_is_syntax_highlight_supported(), children)
                    })
                    .collect()
            })
            .unwrap()
    }

    /// A language that is loading leaves its blocks alone; once it has
    /// loaded (once, for both blocks), they are highlighted.
    #[test]
    fn a_loading_language_highlights_once_loaded() {
        let editor = create_editor(CreateEditorArgs {
            nodes: code_nodes(),
            ..CreateEditorArgs::default()
        });
        let tokenizer = Rc::new(LazyTokenizer::default());
        let _unregister = register_code_highlighting(&editor, tokenizer.clone()).unwrap();
        update(&editor, || {
            for text in ["a", "b"] {
                let code = create_code_node(None, None);
                code.append(&[create_text_node(text)])?;
                get_root().append(&[code])?;
            }
            Ok(())
        });
        let plain = |text: &str| vec![("text".to_owned(), text.to_owned(), None)];
        assert_eq!(
            blocks(&editor),
            vec![(false, plain("a")), (false, plain("b"))]
        );
        assert_eq!(tokenizer.loads.get(), 1);
        tokenizer.loaded.set(true);
        let on_ready = tokenizer.on_ready.borrow_mut().take().unwrap();
        on_ready();
        editor.read(|| ()).unwrap();
        let lazy = |text: &str| {
            vec![(
                "code-highlight".to_owned(),
                text.to_owned(),
                Some("lazy".to_owned()),
            )]
        };
        assert_eq!(blocks(&editor), vec![(true, lazy("a")), (true, lazy("b"))]);
    }

    /// `register_code_highlighting` needs the code nodes.
    #[test]
    fn registering_without_the_code_nodes_fails() {
        let editor = create_editor(CreateEditorArgs::default());
        let error = register_code_highlighting(&editor, Rc::new(LazyTokenizer::default()))
            .err()
            .unwrap();
        assert_eq!(
            error.message(),
            "CodeHighlightPlugin: CodeNode or CodeHighlightNode not registered on editor"
        );
    }

    /// `CodePrismExtension`'s `tokenizer` highlights, and its `disabled`
    /// signal unregisters the highlighter.
    #[cfg(feature = "extension")]
    #[test]
    fn the_extension_highlights_unless_disabled() {
        use crate::extension::{
            build_editor_from_extensions, get_extension_dependency_from_editor,
        };
        use crate::extension_core::config_extension;

        struct Words;
        impl Tokenizer for Words {
            fn default_language(&self) -> Option<&str> {
                Some("words")
            }
            fn language_state(&self, _language: &str) -> CodeLanguageState {
                CodeLanguageState::Loaded
            }
            fn tokenize(&self, code: &str, _language: &str) -> Vec<CodeToken> {
                vec![CodeToken::token("word", code)]
            }
        }
        let built = build_editor_from_extensions(vec![config_extension(
            &code_highlighter_extension(),
            [CodeHighlighterConfigOverrides {
                tokenizer: Some(TokenizerRef(Rc::new(Words))),
                ..CodeHighlighterConfigOverrides::default()
            }],
        )])
        .unwrap();
        let editor = built.editor().clone();
        editor.read(|| ()).unwrap();
        update(&editor, || {
            let code = create_code_node(None, None);
            code.append(&[create_text_node("a")])?;
            get_root().clear()?;
            get_root().append(&[code])
        });
        let word = (
            "code-highlight".to_owned(),
            "a".to_owned(),
            Some("word".to_owned()),
        );
        assert_eq!(blocks(&editor), vec![(true, vec![word.clone()])]);
        let output = get_extension_dependency_from_editor(&editor, &code_highlighter_extension())
            .unwrap()
            .output
            .unwrap();
        output.disabled.set_value(true);
        update(&editor, || {
            let code = get_root().get_first_child().unwrap();
            code.append(&[create_text_node("b")])
        });
        let text = ("text".to_owned(), "b".to_owned(), None);
        assert_eq!(blocks(&editor), vec![(true, vec![word, text])]);
    }
}
