//! What `HTMLTableCellElement` and `HTMLTableRowElement` answer in jsdom
//! (`colSpan`, `rowSpan`, `cellIndex`, `rowIndex`), which the table nodes'
//! DOM import reads. The crate's DOM has one node type, so these read the
//! attributes and the tree as jsdom's reflections do.

use crate::dom::DomNode;

/// HTML's rules for parsing non-negative integers: leading ASCII
/// whitespace, an optional sign, then digits; `None` without digits or for
/// a negative value.
fn parse_non_negative_integer(value: &str) -> Option<f64> {
    let s = value.trim_start_matches([' ', '\t', '\n', '\x0c', '\r']);
    let (negative, s) = match s.as_bytes().first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let digits: &str = &s[..s.bytes().take_while(u8::is_ascii_digit).count()];
    if digits.is_empty() {
        return None;
    }
    let n: f64 = digits.parse().ok()?;
    if negative && n != 0.0 {
        return None;
    }
    Some(n)
}

/// `cell.colSpan`: the `colspan` attribute, from 1 to 1000; 1 without one.
pub(crate) fn col_span(cell: &DomNode) -> f64 {
    match cell
        .get_attribute("colspan")
        .as_deref()
        .and_then(parse_non_negative_integer)
    {
        Some(n) if n < 1.0 => 1.0,
        Some(n) => n.min(1000.0),
        None => 1.0,
    }
}

/// `cell.rowSpan`: the `rowspan` attribute, from 0 to 65534; 1 without one.
pub(crate) fn row_span(cell: &DomNode) -> f64 {
    match cell
        .get_attribute("rowspan")
        .as_deref()
        .and_then(parse_non_negative_integer)
    {
        Some(n) => n.min(65534.0),
        None => 1.0,
    }
}

/// What `element.colSpan = n` and `element.rowSpan = n` write: the number
/// as an `unsigned long`, or 1 when it is above 2147483647.
pub(crate) fn unsigned_long(n: f64) -> u64 {
    let n = if n.is_finite() { n.trunc() } else { 0.0 };
    let n = n.rem_euclid(4_294_967_296.0) as u64;
    if n <= 2_147_483_647 { n } else { 1 }
}

fn is_named(node: &DomNode, name: &str) -> bool {
    node.is_element() && node.local_name().eq_ignore_ascii_case(name)
}

/// `cell.cellIndex`: the cell's index among the `td` and `th` children of
/// the nearest `tr` around it, -1 outside a row.
pub(crate) fn cell_index(cell: &DomNode) -> i64 {
    let mut node = Some(cell.clone());
    while let Some(n) = node {
        if is_named(&n, "tr") {
            return n
                .children()
                .iter()
                .filter(|c| is_named(c, "td") || is_named(c, "th"))
                .position(|c| c == cell)
                .map_or(-1, |i| i as i64);
        }
        node = n.parent_node();
    }
    -1
}

/// `row.rowIndex`: the row's index in its table's `rows` (the `thead`
/// rows, then the `tbody` rows and the table's own, then the `tfoot`
/// rows), -1 outside a table.
pub(crate) fn row_index(row: &DomNode) -> i64 {
    let Some(parent) = row.parent_element() else {
        return -1;
    };
    let table = if ["thead", "tbody", "tfoot"]
        .iter()
        .any(|name| is_named(&parent, name))
    {
        match parent.parent_element() {
            Some(table) => table,
            None => return -1,
        }
    } else {
        parent
    };
    if !is_named(&table, "table") {
        return -1;
    }
    let (mut head, mut body, mut foot) = (Vec::new(), Vec::new(), Vec::new());
    for child in table.children() {
        let rows = |section: &DomNode| -> Vec<DomNode> {
            section
                .children()
                .into_iter()
                .filter(|r| is_named(r, "tr"))
                .collect()
        };
        if is_named(&child, "thead") {
            head.extend(rows(&child));
        } else if is_named(&child, "tbody") {
            body.extend(rows(&child));
        } else if is_named(&child, "tfoot") {
            foot.extend(rows(&child));
        } else if is_named(&child, "tr") {
            body.push(child);
        }
    }
    head.into_iter()
        .chain(body)
        .chain(foot)
        .position(|r| &r == row)
        .map_or(-1, |i| i as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spans_as_jsdom_does() {
        for (value, expected) in [
            ("2", Some(2.0)),
            ("  +3x", Some(3.0)),
            ("-0", Some(0.0)),
            ("-1", None),
            ("", None),
            ("x", None),
        ] {
            assert_eq!(parse_non_negative_integer(value), expected, "{value:?}");
        }
        assert_eq!(unsigned_long(3.7), 3);
        assert_eq!(unsigned_long(-1.0), 1);
    }
}
