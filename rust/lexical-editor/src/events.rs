//! The DOM's `DataTransfer`, which Lexical's clipboard functions read and
//! write. A headless editor has no DOM, so the port has its own, as Swift's
//! does (`swift/Sources/Lexical/LexicalEvents.swift`); only what the ported
//! clipboard functions use is here.

use std::cell::RefCell;

/// `DataTransfer`: the data of a clipboard operation, by format (a MIME
/// type), in the order each format was first set.
///
/// It is shared as a DOM `DataTransfer` is, so [`set_data`](Self::set_data)
/// takes `&self`: a copy fills the one the platform's clipboard event
/// carries, and the caller reads it afterwards.
#[derive(Debug, Default)]
pub struct DataTransfer {
    entries: RefCell<Vec<(String, String)>>,
}

impl DataTransfer {
    /// `new DataTransfer()`: one with no data.
    pub fn new() -> Self {
        Self::default()
    }

    /// The format a type names: lowercased, with `text` read as
    /// `text/plain` and `url` as `text/uri-list`, as browsers do.
    fn format(format: &str) -> String {
        let format = format.to_lowercase();
        match format.as_str() {
            "text" => "text/plain".to_owned(),
            "url" => "text/uri-list".to_owned(),
            _ => format,
        }
    }

    /// `types`: the formats set so far, in order.
    pub fn types(&self) -> Vec<String> {
        self.entries
            .borrow()
            .iter()
            .map(|(t, _)| t.clone())
            .collect()
    }

    /// `getData(format)`: the data for `format`, or `""`.
    pub fn get_data(&self, format: &str) -> String {
        let format = Self::format(format);
        self.entries
            .borrow()
            .iter()
            .find(|(t, _)| *t == format)
            .map(|(_, data)| data.clone())
            .unwrap_or_default()
    }

    /// `setData(format, data)`: sets the data for `format`, keeping its
    /// place if it was set before.
    pub fn set_data(&self, format: &str, data: &str) {
        let format = Self::format(format);
        let mut entries = self.entries.borrow_mut();
        match entries.iter_mut().find(|(t, _)| *t == format) {
            Some(entry) => entry.1 = data.to_owned(),
            None => entries.push((format, data.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_by_format_in_order() {
        let data_transfer = DataTransfer::new();
        data_transfer.set_data("text/html", "");
        data_transfer.set_data("Text", "plain");
        data_transfer.set_data("text/html", "<b>x</b>");
        assert_eq!(data_transfer.types(), ["text/html", "text/plain"]);
        assert_eq!(data_transfer.get_data("text/plain"), "plain");
        assert_eq!(data_transfer.get_data("TEXT/HTML"), "<b>x</b>");
        assert_eq!(data_transfer.get_data("URL"), "");
    }
}
