//! `ClipboardImportExtension.ts` (`@experimental` upstream): the stacks of
//! importers, one per MIME type, that `$insertDataTransferForRichText`
//! runs in priority order until one takes the data. Without the
//! `extension` feature (or an editor built without
//! `ClipboardImportExtension`) the default stacks run.
//!
//! JS passes the HTML through a Trusted Types policy (`trustHTML`) first;
//! outside a browser there is none, so the string is parsed as is.

use super::{LEXICAL_MIME_TYPE, generate_nodes_from_serialized_nodes, insert_generated_nodes};
use crate::events::DataTransfer;
use crate::json::{self, Value};
use crate::lexical_selection::{SelectionRef, get_selection, set_selection};
use crate::nodes::tab::create_tab_node;
use crate::selection_insert_nodes::{RawTextVisitor, tokenize_raw_text};
use crate::{Editor, LexicalError};
use std::cmp::Ordering;
use std::rc::Rc;

/// The `$next` of an [`ImportMimeTypeFunction`]: runs the handler
/// registered before it, with other data or another selection where they
/// are `Some`, and returns whether it took the data (`false` below the
/// bottom of the stack).
pub type ImportMimeTypeNext<'a> =
    &'a dyn Fn(Option<&str>, Option<&SelectionRef>) -> Result<bool, LexicalError>;

/// `ImportMimeTypeFunction`: a handler in one MIME type's stack. It gets
/// the (non-empty) data, the selection to insert at, `$next` and the whole
/// `DataTransfer`, and returns whether it took the data.
pub type ImportMimeTypeFunction = Rc<
    dyn Fn(
        &str,
        &SelectionRef,
        ImportMimeTypeNext<'_>,
        &DataTransfer,
    ) -> Result<bool, LexicalError>,
>;

/// `ImportMimeTypeConfig`: a stack per MIME type, in the order the types
/// were first added; the last function of a stack runs first.
pub type ImportMimeTypeConfig = Vec<(String, Vec<ImportMimeTypeFunction>)>;

/// `ImportMimeTypePriority`: weights by MIME type, lower first. Types
/// without one run after the weighted ones, in code unit order.
pub type ImportMimeTypePriority = Vec<(String, f64)>;

/// `ClipboardImportConfig`.
#[derive(Clone)]
pub struct ClipboardImportConfig {
    /// `$importMimeType`: a contribution's functions go on top of each
    /// type's stack (`[...prev, ...override]`).
    pub import_mime_type: ImportMimeTypeConfig,
    /// Merged key by key: a contribution's weight replaces the type's.
    pub priority: ImportMimeTypePriority,
}

impl Default for ClipboardImportConfig {
    fn default() -> Self {
        Self {
            import_mime_type: default_import_mime_type(),
            priority: default_import_mime_type_priority(),
        }
    }
}

/// `DEFAULT_IMPORT_MIME_TYPE_PRIORITY`: `application/x-lexical-editor`
/// (0), `text/html` (10), `text/plain` (20), `text/uri-list` (30).
pub fn default_import_mime_type_priority() -> ImportMimeTypePriority {
    [
        (LEXICAL_MIME_TYPE, 0.0),
        ("text/html", 10.0),
        ("text/plain", 20.0),
        ("text/uri-list", 30.0),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
}

/// `DEFAULT_IMPORT_MIME_TYPE`: the Lexical JSON importer, the HTML
/// importer (with the `html` feature), and the plain-text importer for
/// both `text/plain` and `text/uri-list` (drops of a URI behave like text).
pub fn default_import_mime_type() -> ImportMimeTypeConfig {
    let plain_text: ImportMimeTypeFunction = Rc::new(default_plain_text_importer);
    let mut config: ImportMimeTypeConfig = vec![(
        LEXICAL_MIME_TYPE.to_owned(),
        vec![Rc::new(default_lexical_editor_importer) as ImportMimeTypeFunction],
    )];
    #[cfg(feature = "html")]
    config.push((
        "text/html".to_owned(),
        vec![Rc::new(default_html_importer) as ImportMimeTypeFunction],
    ));
    config.push(("text/plain".to_owned(), vec![plain_text.clone()]));
    config.push(("text/uri-list".to_owned(), vec![plain_text]));
    config
}

fn active_editor() -> Result<Editor, LexicalError> {
    crate::updates::get_active_editor()
        .ok_or_else(|| LexicalError::new("Unable to find an active editor."))
}

/// `$defaultLexicalEditorImporter`: parses the JSON and, when it names the
/// editor's namespace and has a `nodes` array, inserts its nodes. Anything
/// that fails (bad JSON, an unknown node type, an insertion that throws) is
/// caught, as JS catches it (and logs it), and so is a payload of another
/// editor: each leaves the data to `$next`.
fn default_lexical_editor_importer(
    data: &str,
    selection: &SelectionRef,
    next: ImportMimeTypeNext<'_>,
    _: &DataTransfer,
) -> Result<bool, LexicalError> {
    let run = || -> Result<bool, LexicalError> {
        let editor = active_editor()?;
        let Ok(payload) = json::parse(data) else {
            return Ok(false);
        };
        let namespace_matches =
            payload.get("namespace").and_then(Value::as_str) == Some(editor.get_namespace());
        let Some(serialized_nodes) = payload.get("nodes").and_then(Value::as_array) else {
            return Ok(false);
        };
        if !namespace_matches {
            return Ok(false);
        }
        let nodes = generate_nodes_from_serialized_nodes(serialized_nodes)?;
        insert_generated_nodes(&editor, nodes, selection)?;
        Ok(true)
    };
    if run().unwrap_or(false) {
        return Ok(true);
    }
    next(None, None)
}

/// `$defaultHtmlImporter`: parses the HTML and inserts the nodes of the
/// legacy `$generateNodesFromDOM` (the nodes' `importDOM`). An error is
/// caught, as JS catches it (and logs it), and leaves the data to `$next`.
#[cfg(feature = "html")]
fn default_html_importer(
    data: &str,
    selection: &SelectionRef,
    next: ImportMimeTypeNext<'_>,
    _: &DataTransfer,
) -> Result<bool, LexicalError> {
    let run = || -> Result<(), LexicalError> {
        let editor = active_editor()?;
        let dom = crate::dom::parse_from_string(data);
        let nodes = crate::html::generate_nodes_from_dom(&editor, &dom)?;
        insert_generated_nodes(&editor, nodes, selection)
    };
    match run() {
        Ok(()) => Ok(true),
        Err(_) => next(None, None),
    }
}

/// `$defaultPlainTextImporter`: on a range selection, the text token by
/// token (`tokenizeRawText`), each `\n` as `insertParagraph`, each tab as a
/// tab node and the rest as `insertText`, each on the editor's selection as
/// the one before leaves it (after making `selection` the editor's); on a
/// node selection, `insertRawText`, which does nothing.
fn default_plain_text_importer(
    data: &str,
    selection: &SelectionRef,
    _: ImportMimeTypeNext<'_>,
    _: &DataTransfer,
) -> Result<bool, LexicalError> {
    if !selection.is_range() {
        selection.insert_raw_text(data)?;
        return Ok(true);
    }
    // Each insertion below reports the caret position that follows it by
    // writing to the editor's selection, so the loop re-reads it between
    // tokens. Honoring a caller-supplied selection means promoting it to
    // the editor's selection first.
    if !get_selection().is_some_and(|current| current.ptr_eq(selection)) {
        set_selection(Some(selection.clone()));
    }
    /// The tokens' insertions; JS stops at the first that throws.
    struct Insert(Result<(), LexicalError>);
    impl Insert {
        fn with_current_range(
            &mut self,
            f: impl FnOnce(&SelectionRef) -> Result<(), LexicalError>,
        ) {
            if self.0.is_ok()
                && let Some(current) = get_selection().filter(SelectionRef::is_range)
            {
                self.0 = f(&current);
            }
        }
    }
    impl RawTextVisitor for Insert {
        fn linebreak(&mut self) {
            self.with_current_range(|cur| cur.insert_paragraph().map(|_| ()));
        }
        fn tab(&mut self) {
            self.with_current_range(|cur| cur.insert_nodes(vec![create_tab_node()]));
        }
        fn text(&mut self, text: &str) {
            self.with_current_range(|cur| cur.insert_text(text));
        }
    }
    let mut insert = Insert(Ok(()));
    tokenize_raw_text(data, &mut insert);
    insert.0.map(|_| true)
}

/// `ClipboardImportOutput`: the merged config, with
/// [`insert_data_transfer`](Self::insert_data_transfer).
pub struct ClipboardImportOutput {
    pub import_mime_type: ImportMimeTypeConfig,
    pub priority: ImportMimeTypePriority,
}

impl ClipboardImportOutput {
    /// `$insertDataTransfer(dataTransfer, selection)`: runs the stack of
    /// each MIME type with data in `data_transfer`, in priority order,
    /// until one takes it; whether one did. `text/html` equal to the
    /// `text/plain` is skipped (iOS Safari's autocorrect pastes such a
    /// payload).
    pub fn insert_data_transfer(
        &self,
        data_transfer: &DataTransfer,
        selection: &SelectionRef,
    ) -> Result<bool, LexicalError> {
        let plain_string = data_transfer.get_data("text/plain");
        for mime in self.ordered_mime_types() {
            let data = data_transfer.get_data(mime);
            if data.is_empty() || (mime == "text/html" && data == plain_string) {
                continue;
            }
            let fns = self
                .import_mime_type
                .iter()
                .find(|(k, _)| k == mime)
                .map(|(_, fns)| &fns[..])
                .unwrap_or_default();
            if call_import_mime_type_function_stack(fns, &data, selection, data_transfer)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `orderedMimeTypes(config)`: the types with a stack, by weight (a
    /// stable sort, as JS's), the unweighted last in code unit order.
    fn ordered_mime_types(&self) -> Vec<&str> {
        let weight = |mime: &str| {
            self.priority
                .iter()
                .find(|(k, _)| k == mime)
                .map(|(_, w)| *w)
        };
        let mut mimes: Vec<&str> = self
            .import_mime_type
            .iter()
            .map(|(k, _)| k.as_str())
            .collect();
        mimes.sort_by(|a, b| match (weight(a), weight(b)) {
            (None, None) => a.encode_utf16().cmp(b.encode_utf16()),
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(wa), Some(wb)) => wa.partial_cmp(&wb).unwrap_or(Ordering::Equal),
        });
        mimes
    }
}

/// `$callImportMimeTypeFunctionStack(fns, data, selection, dataTransfer)`:
/// the last function first, each `$next` calling the one below it.
fn call_import_mime_type_function_stack(
    fns: &[ImportMimeTypeFunction],
    data: &str,
    selection: &SelectionRef,
    data_transfer: &DataTransfer,
) -> Result<bool, LexicalError> {
    fn call_at(
        fns: &[ImportMimeTypeFunction],
        i: usize,
        data: &str,
        selection: &SelectionRef,
        data_transfer: &DataTransfer,
    ) -> Result<bool, LexicalError> {
        if i == 0 {
            return Ok(false);
        }
        let next = |next_data: Option<&str>, next_selection: Option<&SelectionRef>| {
            call_at(
                fns,
                i - 1,
                next_data.unwrap_or(data),
                next_selection.unwrap_or(selection),
                data_transfer,
            )
        };
        (fns[i - 1])(data, selection, &next, data_transfer)
    }
    call_at(fns, fns.len(), data, selection, data_transfer)
}

/// `$getImportOutput()` (`@internal` upstream): the active editor's
/// `ClipboardImportExtension` output, or the defaults without it.
pub fn get_import_output() -> Result<Rc<ClipboardImportOutput>, LexicalError> {
    #[cfg(feature = "extension")]
    if let Some(output) =
        crate::extension::get_peer_dependency::<ClipboardImportConfig, ClipboardImportOutput, ()>(
            extensions::CLIPBOARD_IMPORT_EXTENSION_NAME,
        )?
        .and_then(|dep| dep.output)
    {
        return Ok(output);
    }
    let ClipboardImportConfig {
        import_mime_type,
        priority,
    } = ClipboardImportConfig::default();
    Ok(Rc::new(ClipboardImportOutput {
        import_mime_type,
        priority,
    }))
}

#[cfg(feature = "extension")]
pub use extensions::*;

#[cfg(feature = "extension")]
mod extensions {
    use super::*;
    use crate::extension_core::{
        ExtensionBuildState, ExtensionConfigBase, LexicalExtension, define_extension,
    };

    pub(super) const CLIPBOARD_IMPORT_EXTENSION_NAME: &str = "@lexical/clipboard/Import";

    /// `Partial<ClipboardImportConfig>`.
    #[derive(Clone, Default)]
    pub struct ClipboardImportConfigOverrides {
        pub import_mime_type: Option<ImportMimeTypeConfig>,
        pub priority: Option<ImportMimeTypePriority>,
    }

    impl ExtensionConfigBase for ClipboardImportConfig {
        type Partial = ClipboardImportConfigOverrides;

        fn shallow_merge(&self, overrides: &ClipboardImportConfigOverrides) -> Self {
            ClipboardImportConfig {
                import_mime_type: overrides
                    .import_mime_type
                    .clone()
                    .unwrap_or_else(|| self.import_mime_type.clone()),
                priority: overrides
                    .priority
                    .clone()
                    .unwrap_or_else(|| self.priority.clone()),
            }
        }
    }

    /// `ClipboardImportExtension`.
    pub type ClipboardImportExtensionType =
        LexicalExtension<ClipboardImportConfig, ClipboardImportOutput, ()>;

    fn make_clipboard_import() -> Rc<ClipboardImportExtensionType> {
        define_extension(LexicalExtension {
            build: Some(Rc::new(
                |_: &Editor, config: &ClipboardImportConfig, _: &ExtensionBuildState<()>| {
                    Ok(ClipboardImportOutput {
                        import_mime_type: config.import_mime_type.clone(),
                        priority: config.priority.clone(),
                    })
                },
            )),
            merge_config: Some(Rc::new(
                |config: &ClipboardImportConfig, partial: &ClipboardImportConfigOverrides| {
                    let mut merged = config.shallow_merge(partial);
                    if let Some(import_mime_type) = &partial.import_mime_type {
                        merged.import_mime_type = config.import_mime_type.clone();
                        crate::clipboard::append_stacks(
                            &mut merged.import_mime_type,
                            import_mime_type,
                        );
                    }
                    if let Some(priority) = &partial.priority {
                        // A key in `partial` replaces the same key (an
                        // extension can rerank a built-in type), and new
                        // keys are added.
                        merged.priority = config.priority.clone();
                        for (k, v) in priority {
                            match merged.priority.iter_mut().find(|(key, _)| key == k) {
                                Some(entry) => entry.1 = *v,
                                None => merged.priority.push((k.clone(), *v)),
                            }
                        }
                    }
                    merged
                },
            )),
            ..LexicalExtension::new(
                CLIPBOARD_IMPORT_EXTENSION_NAME,
                ClipboardImportConfig::default(),
            )
        })
    }

    #[cfg(feature = "html")]
    fn make_clipboard_dom_import() -> Rc<LexicalExtension> {
        use crate::html::import::{
            GenerateNodesFromDomOptions, ImportSourceKind, SharedDataTransfer, context_value,
            core_import_extension, generate_nodes_from_dom_via_extension, import_source,
            import_source_data_transfer,
        };
        let html_importer: ImportMimeTypeFunction =
            Rc::new(|html, selection, _next, data_transfer| {
                let dom = crate::dom::parse_from_string(html);
                // The stack gets the `DataTransfer` by reference, and the
                // context keeps its values, so it gets a copy.
                let copy = DataTransfer::new();
                for format in data_transfer.types() {
                    copy.set_data(&format, &data_transfer.get_data(&format));
                }
                let nodes = generate_nodes_from_dom_via_extension(
                    &dom,
                    Some(&GenerateNodesFromDomOptions {
                        context: Some(vec![
                            context_value(&import_source(), ImportSourceKind::Paste),
                            context_value(
                                &import_source_data_transfer(),
                                Some(SharedDataTransfer(Rc::new(copy))),
                            ),
                        ]),
                        ..GenerateNodesFromDomOptions::default()
                    }),
                )?;
                insert_generated_nodes(&active_editor()?, nodes, selection)?;
                Ok(true)
            });
        define_extension(LexicalExtension {
            dependencies: vec![
                core_import_extension().into(),
                crate::extension_core::config_extension(
                    &clipboard_import_extension(),
                    [ClipboardImportConfigOverrides {
                        import_mime_type: Some(vec![("text/html".to_owned(), vec![html_importer])]),
                        ..ClipboardImportConfigOverrides::default()
                    }],
                ),
            ],
            ..LexicalExtension::new("@lexical/clipboard/DOMImport", ())
        })
    }

    thread_local! {
        static CLIPBOARD_IMPORT_EXTENSION: Rc<ClipboardImportExtensionType> =
            make_clipboard_import();
        #[cfg(feature = "html")]
        static CLIPBOARD_DOM_IMPORT_EXTENSION: Rc<LexicalExtension> = make_clipboard_dom_import();
    }

    /// `ClipboardImportExtension` (`@experimental` upstream): the import
    /// direction of `GetClipboardDataExtension`, a stack of
    /// [`ImportMimeTypeFunction`]s per MIME type with priority weights. Its
    /// output is what [`get_import_output`] returns.
    pub fn clipboard_import_extension() -> Rc<ClipboardImportExtensionType> {
        CLIPBOARD_IMPORT_EXTENSION.with(Rc::clone)
    }

    /// `ClipboardDOMImportExtension` (`@experimental` upstream): `text/html`
    /// through `DOMImportExtension`'s rules (with `CoreImportExtension`)
    /// rather than the nodes' `importDOM`, with the `paste` import source and
    /// the `DataTransfer` in the import context.
    #[cfg(feature = "html")]
    pub fn clipboard_dom_import_extension() -> Rc<LexicalExtension> {
        CLIPBOARD_DOM_IMPORT_EXTENSION.with(Rc::clone)
    }
}
