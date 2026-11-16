//! `@lexical/file` (the `file` feature), from `packages/lexical-file/src`:
//! the `.lexical` document format ([`serialized_document_from_editor_state`],
//! [`editor_state_from_serialized_document`]) and importing and exporting
//! files ([`import_file`], [`export_file`]), where the app picks and saves
//! the file, as upstream's DOM file input and download link do.

mod file_import_export;

pub use file_import_export::{
    ExportFileConfig, MaybeStringifiedDocument, ReadTextFileCallback, SerializedDocument,
    SerializedDocumentConfig, editor_state_from_serialized_document, export_file, import_file,
    serialized_document_from_editor_state,
};
