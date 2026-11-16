//! `packages/lexical-file/src/fileImportExport.ts`. Upstream reads a file
//! through a DOM `<input type="file">` (`readTextFileFromSystem`) and saves
//! one through a download link (`exportBlob`); here the app passes both in,
//! as Swift's port does, since a native app has its own file panels.

use crate::json::{self, Object, Value};
use crate::{CLEAR_HISTORY_COMMAND, Editor, EditorState, LexicalError, UPSTREAM_VERSION};

/// `SerializedDocument`: a `.lexical` file's contents.
#[derive(Clone, Debug, PartialEq)]
pub struct SerializedDocument {
    /// The serialized editor state produced by `editorState.toJSON()`.
    pub editor_state: Value,
    /// The time this document was created in epoch milliseconds
    /// (`Date.now()`).
    pub last_saved: f64,
    /// The source of the document, `"Lexical"` by default.
    pub source: String,
    /// The version of Lexical that produced this document.
    pub version: String,
}

impl SerializedDocument {
    /// The document as JS's object literal has it, in its key order.
    pub fn to_json(&self) -> Value {
        let mut json = Object::new();
        json.insert("editorState", self.editor_state.clone());
        json.insert("lastSaved", self.last_saved.into());
        json.insert("source", self.source.as_str().into());
        json.insert("version", self.version.as_str().into());
        Value::Object(json)
    }
}

/// The config of [`serialized_document_from_editor_state`]: `None`, an
/// empty `source` and a `last_saved` of 0 (or NaN) are left to the
/// defaults, as JS's `||` leaves them.
#[derive(Clone, Debug, Default)]
pub struct SerializedDocumentConfig {
    pub source: Option<String>,
    pub last_saved: Option<f64>,
}

/// `Date.now()`: the current time in whole epoch milliseconds.
fn date_now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_millis() as f64)
}

/// `serializedDocumentFromEditorState(editorState, config)`: the document of
/// `editor_state`, saved at `config.last_saved` (now by default), from
/// `config.source` (`"Lexical"` by default), with `@lexical/file`'s version,
/// the upstream release the port conforms to.
pub fn serialized_document_from_editor_state(
    editor_state: &EditorState,
    config: &SerializedDocumentConfig,
) -> SerializedDocument {
    SerializedDocument {
        editor_state: editor_state.to_json(false),
        last_saved: config
            .last_saved
            .filter(|t| *t != 0.0 && !t.is_nan())
            .unwrap_or_else(date_now),
        source: config
            .source
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Lexical".to_owned()),
        version: UPSTREAM_VERSION.to_owned(),
    }
}

/// What [`editor_state_from_serialized_document`] reads: the contents of a
/// `.lexical` file, as a JSON string or already parsed.
#[derive(Clone, Copy, Debug)]
pub enum MaybeStringifiedDocument<'a> {
    Document(&'a SerializedDocument),
    String(&'a str),
}

impl<'a> From<&'a SerializedDocument> for MaybeStringifiedDocument<'a> {
    fn from(document: &'a SerializedDocument) -> Self {
        Self::Document(document)
    }
}

impl<'a> From<&'a str> for MaybeStringifiedDocument<'a> {
    fn from(text: &'a str) -> Self {
        Self::String(text)
    }
}

/// `editorStateFromSerializedDocument(editor, maybeStringifiedDocument)`:
/// parses the document's `editorState` with `editor`'s nodes, in either
/// JSON form. A document that doesn't parse, or has no `editorState`,
/// fails as `parseEditorState` does.
pub fn editor_state_from_serialized_document<'a>(
    editor: &Editor,
    maybe_stringified_document: impl Into<MaybeStringifiedDocument<'a>>,
) -> Result<EditorState, LexicalError> {
    match maybe_stringified_document.into() {
        MaybeStringifiedDocument::Document(document) => {
            editor.parse_editor_state_json(&document.editor_state)
        }
        MaybeStringifiedDocument::String(text) => {
            let json = json::parse(text).map_err(|e| LexicalError::new(e.to_string()))?;
            editor.parse_editor_state_json(json.get("editorState").unwrap_or(&Value::Undefined))
        }
    }
}

/// What reads a file the user picks (upstream's `readTextFileFromSystem`):
/// called with the file names to accept (`".lexical"`) and the callback to
/// call with the file's text, read as UTF-8. The app decides how to report
/// an error the callback returns, such as a file that isn't a document.
pub type ReadTextFileCallback = Box<dyn FnOnce(&str) -> Result<(), LexicalError>>;

/// `importFile(editor)`: lets the user pick a `.lexical` file through
/// `read_text_file_from_system`, then sets the editor's state to it and
/// clears its history (`CLEAR_HISTORY_COMMAND`).
pub fn import_file(
    editor: &Editor,
    read_text_file_from_system: impl FnOnce(&str, ReadTextFileCallback),
) {
    let editor = editor.clone();
    read_text_file_from_system(
        ".lexical",
        Box::new(move |text| {
            editor
                .set_editor_state(&editor_state_from_serialized_document(&editor, text)?, None)?;
            editor.dispatch_command(&CLEAR_HISTORY_COMMAND, ())?;
            Ok(())
        }),
    )
}

/// The config of [`export_file`]: `None` or empty values are left to the
/// defaults.
#[derive(Clone, Debug, Default)]
pub struct ExportFileConfig {
    /// The file's name without `.lexical`; the time saved
    /// (`toISOString()`) by default.
    pub file_name: Option<String>,
    /// The document's source, `"Lexical"` by default.
    pub source: Option<String>,
}

/// `exportFile(editor, config)`: the editor's state as a `.lexical`
/// document saved now, passed to `export_blob` (upstream's `exportBlob`,
/// which downloads it) as `JSON.stringify` writes it, with the file name
/// `<fileName>.lexical`.
pub fn export_file(
    editor: &Editor,
    config: &ExportFileConfig,
    export_blob: impl FnOnce(&str, &str),
) {
    let now = date_now();
    let serialized_document = serialized_document_from_editor_state(
        &editor.get_editor_state(),
        &SerializedDocumentConfig {
            source: config.source.clone(),
            last_saved: Some(now),
        },
    );
    let file_name = config
        .file_name
        .clone()
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| to_iso_string(now));
    export_blob(
        &serialized_document.to_json().to_string(),
        &format!("{file_name}.lexical"),
    );
}

/// `new Date(time).toISOString()` for a time from 1970 to 9999:
/// `YYYY-MM-DDTHH:mm:ss.sssZ` in UTC.
fn to_iso_string(time: f64) -> String {
    let ms = time as i64;
    let days = ms.div_euclid(86_400_000);
    let in_day = ms.rem_euclid(86_400_000);
    // `civil_from_days` (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        in_day / 3_600_000,
        in_day / 60_000 % 60,
        in_day / 1000 % 60,
        in_day % 1000,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CreateEditorArgs, create_editor, get_root};
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn to_iso_string_matches_js() {
        // `new Date(t).toISOString()` in Node.
        for (time, iso) in [
            (1762401787158.0, "2025-11-06T04:03:07.158Z"),
            (0.0, "1970-01-01T00:00:00.000Z"),
            (951782400000.0, "2000-02-29T00:00:00.000Z"),
            (4102444799999.0, "2099-12-31T23:59:59.999Z"),
            (1.5, "1970-01-01T00:00:00.001Z"),
        ] {
            assert_eq!(to_iso_string(time), iso);
        }
    }

    #[test]
    fn export_then_import_a_file() {
        let editor = create_editor(CreateEditorArgs::default());
        let state = editor
            .parse_editor_state(r#"{"root":{"type":"root","children":[{"type":"paragraph","children":[{"type":"text","text":"Hi"}]}]}}"#)
            .unwrap();
        editor.set_editor_state(&state, None).unwrap();
        let saved = Rc::new(RefCell::new(None));
        let into = saved.clone();
        export_file(
            &editor,
            &ExportFileConfig {
                file_name: Some("notes".to_owned()),
                source: None,
            },
            move |text, name| *into.borrow_mut() = Some((text.to_owned(), name.to_owned())),
        );
        let (text, name) = saved.borrow_mut().take().unwrap();
        assert_eq!(name, "notes.lexical");
        let document = json::parse(&text).unwrap();
        assert_eq!(
            document.get("source").and_then(Value::as_str),
            Some("Lexical")
        );
        assert_eq!(
            document.get("version").and_then(Value::as_str),
            Some(UPSTREAM_VERSION)
        );

        let other = create_editor(CreateEditorArgs::default());
        import_file(&other, |accept, callback| {
            assert_eq!(accept, ".lexical");
            callback(&text).unwrap();
        });
        assert_eq!(other.read(|| get_root().get_text_content()).unwrap(), "Hi");
    }
}
