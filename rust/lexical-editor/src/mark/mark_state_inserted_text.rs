//! `registerMarkState`'s inserted-text rule, from `MarkStateExtension.ts`
//! (`$applyInsertedTextMarkIDs` and its helpers, from `$getBlock` to
//! `getSplitOrigin`), apart from the rest of it as in Swift's port
//! (`MarkStateInsertedText.swift`).
//!
//! Text is compared as JS compares it, by UTF-16 code unit: the segments,
//! their codes and the offsets are `Vec<u16>` and code-unit indices.
//!
//! JS forgets the pending edits (`forgetSoon`) at the next microtask. With
//! no event loop, the port forgets them when the update that recorded them
//! is over: the edits keep the pending editor state they were recorded in,
//! and are dropped once the editor has another (`PendingEdits::refresh`),
//! as well as at each commit.

use super::mark_state_extension::{MarkFlavor, is_markable_node, js_cmp, read_node_mark_ids};
use crate::editor::Editor;
use crate::lexical_selection::{Point, PointType, SelectionRef, get_selection};
use crate::lexical_utils::{get_node_by_key, get_node_by_key_impl};
use crate::node::NodeRef;
use crate::node_state::{get_state_erased, has_state_value, set_state};
use crate::nodes::text::create_text_node;
use crate::updates::read_editor_state;
use crate::{EditorState, LexicalError, NodeKey};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// `PendingEdits`: what the inserted-text rule needs to know of the
/// current edits besides the two editor states: the selection an insertion
/// or paste command replaces (recorded before its handler replaces it),
/// and the ids removed with `$removeMarkID`, which the rule must not give
/// back.
#[derive(Default)]
pub(super) struct PendingEdits {
    pub(super) replaced: RefCell<Option<(Point, Point)>>,
    pub(super) removed: RefCell<Vec<String>>,
    /// The pending editor state the edits were recorded in (`forgetSoon`).
    recorded_in: RefCell<Option<EditorState>>,
}

impl PendingEdits {
    /// `forgetEdits(edit)`.
    pub(super) fn forget(&self) {
        *self.replaced.borrow_mut() = None;
        self.removed.borrow_mut().clear();
        *self.recorded_in.borrow_mut() = None;
    }

    /// The microtask of `forgetSoon`: the edits of an update that is over.
    pub(super) fn refresh(&self, editor: &Editor) {
        let stale = match (&*self.recorded_in.borrow(), editor.pending()) {
            (Some(recorded), Some(pending)) => !recorded.ptr_eq(&pending),
            (Some(_), None) => true,
            (None, _) => false,
        };
        if stale {
            self.forget();
        }
    }

    /// Records an edit of the pending update, then `forgetSoon(edit)`.
    pub(super) fn record(&self, editor: &Editor, f: impl FnOnce(&Self)) {
        self.refresh(editor);
        f(self);
        *self.recorded_in.borrow_mut() = editor.pending();
    }
}

/// A set of `removed`.
trait Removed {
    fn has(&self, id: &str) -> bool;
}

impl Removed for [String] {
    fn has(&self, id: &str) -> bool {
        self.iter().any(|r| r == id)
    }
}

impl PendingEdits {
    pub(super) fn add_removed(&self, id: &str) {
        let mut removed = self.removed.borrow_mut();
        if !removed.has(id) {
            removed.push(id.to_owned());
        }
    }
}

/// `SplitOrigins`: where the pieces a flavor's inserted-text rule cut off
/// a text node in the current update came from: piece key to the flavor
/// and the key of the node it was cut from. Shared by the flavors of an
/// editor through `MarkBaseExtension`, and forgotten at each commit.
#[derive(Default)]
pub(super) struct SplitOrigins(RefCell<HashMap<NodeKey, (Rc<MarkFlavor>, NodeKey)>>);

impl SplitOrigins {
    pub(super) fn clear(&self) {
        self.0.borrow_mut().clear();
    }
}

/// `getSplitOrigin(splits, flavor, key)`: the key of the node `key` stands
/// for in `flavor`'s comparison: a piece another flavor cut off a node in
/// this update stands for that node.
fn get_split_origin(splits: &SplitOrigins, flavor: &Rc<MarkFlavor>, key: &str) -> NodeKey {
    let splits = splits.0.borrow();
    let mut key = key.to_owned();
    while let Some((other, origin)) = splits.get(&key) {
        if Rc::ptr_eq(other, flavor) {
            break;
        }
        key = origin.clone();
    }
    key
}

/// `$getBlock(node)`: the nearest ancestor of `node` (or `node` itself)
/// that is a block.
pub(super) fn get_block(node: &NodeRef) -> Option<NodeRef> {
    let mut current = Some(node.clone());
    while let Some(node) = current {
        if node.is_element() && !node.is_inline() {
            return Some(node);
        }
        current = node.get_parent();
    }
    None
}

/// A unit of a segment: a character of text.
const CHAR: u8 = 0;
/// A markable leaf that is not text.
const LEAF: u8 = 1;
/// A boundary between blocks, or around a nested block.
const BREAK: u8 = 2;
/// A node that is neither, compared by its key.
const OPAQUE: u8 = 3;

type U16s = Vec<u16>;

fn code_of(tag: u16, rest: &str) -> U16s {
    std::iter::once(tag).chain(rest.encode_utf16()).collect()
}

/// `BlockWindow`: the part of a block the comparison reads.
struct BlockWindow {
    block: NodeRef,
    from: Option<NodeRef>,
    to: Option<NodeRef>,
}

/// `Segment`.
struct Segment {
    code: U16s,
    ids: Vec<String>,
    kind: u8,
    node: NodeRef,
    text: U16s,
}

/// `Segments`.
struct Segments {
    length: usize,
    points: Vec<i64>,
    segs: Vec<Segment>,
    text: U16s,
    codes: HashMap<usize, U16s>,
}

/// `Flattened`.
struct Flattened {
    codes: Vec<U16s>,
    kinds: Vec<u8>,
    ids: Vec<Vec<String>>,
    nodes: Vec<NodeRef>,
    offsets: Vec<usize>,
    points: Vec<i64>,
}

/// `$getReplacedRange(selection)`: the start and end of a range selection
/// that is not collapsed.
pub(super) fn get_replaced_range(
    selection: Option<&SelectionRef>,
) -> Result<Option<(Point, Point)>, LexicalError> {
    let Some(selection) = selection else {
        return Ok(None);
    };
    let Some(range) = selection.range().filter(|r| !r.is_collapsed()) else {
        return Ok(None);
    };
    Ok(Some(if selection.is_backward()? {
        (range.focus, range.anchor)
    } else {
        (range.anchor, range.focus)
    }))
}

/// `COMPOSITION_PLACEHOLDERS`: what an input method may put in place of the
/// selection when a composition starts.
const COMPOSITION_PLACEHOLDERS: [&str; 3] = ["", "\u{200b}", "\u{a0}"];

/// `NON_TEXT_UNIT`.
const NON_TEXT_UNIT: u16 = 0xfffc;

/// `$childrenText(block)`.
fn children_text(block: &NodeRef) -> U16s {
    let mut text = Vec::new();
    for child in block.get_children() {
        if child.is_element() {
            if child.is_inline() {
                text.extend(children_text(&child));
            } else {
                text.extend(child.get_text_content().encode_utf16());
            }
        } else if child.is_text() {
            text.extend(child.get_text_content().encode_utf16());
        } else {
            text.push(NON_TEXT_UNIT);
        }
    }
    text
}

/// `$unitSize(node)`.
fn unit_size(node: &NodeRef) -> i64 {
    if node.is_text() {
        node.get_text_content_size() as i64
    } else {
        1
    }
}

/// `$offsetInBlock(block, point)`: the offset of `point` in the text of
/// `block`'s inline content (`$childrenText`); -1 when a nested block comes
/// first.
fn offset_in_block(block: &NodeRef, point: &Point) -> i64 {
    fn add(node: &NodeRef, offset: &mut i64) -> bool {
        if node.is_element() {
            if !node.is_inline() {
                return false;
            }
            return node.get_children().iter().all(|child| add(child, offset));
        }
        *offset += unit_size(node);
        true
    }
    fn walk(element: &NodeRef, point: &Point, offset: &mut i64) -> bool {
        if point.point_type == PointType::Element && point.key == element.get_key() {
            let children = element.get_children();
            return children
                .iter()
                .take(point.offset)
                .all(|child| add(child, offset));
        }
        let mut child = element.get_first_child();
        while let Some(c) = child {
            if c.is_text() && c.get_key() == point.key {
                *offset += point.offset as i64;
                return true;
            }
            if c.is_element() {
                if !c.is_inline() {
                    *offset = -1;
                    return true;
                }
                if walk(&c, point, offset) {
                    return true;
                }
            } else {
                *offset += unit_size(&c);
            }
            child = c.get_next_sibling();
        }
        false
    }
    let mut offset = 0;
    if walk(block, point, &mut offset) {
        offset
    } else {
        -1
    }
}

/// `$getCompositionReplacedRange(editor, base, started)`: the selection a
/// composition started with, when its first update replaced it with one of
/// the `COMPOSITION_PLACEHOLDERS` and put the caret after it (or selected
/// it).
pub(super) fn get_composition_replaced_range(
    editor: &Editor,
    base: &EditorState,
    started: &EditorState,
) -> Result<Option<(Point, Point)>, LexicalError> {
    struct Before {
        block: NodeKey,
        end: i64,
        range: (Point, Point),
        start: i64,
        text: U16s,
    }
    let before = read_editor_state(Some(editor), base, || {
        let Some(range) = get_replaced_range(get_selection().as_ref())? else {
            return Ok::<_, LexicalError>(None);
        };
        let start_node = get_node_by_key(&range.0.key);
        let end_node = get_node_by_key(&range.1.key);
        let block = start_node.as_ref().and_then(get_block);
        let (Some(block), Some(end_node)) = (block, end_node) else {
            return Ok(None);
        };
        if get_block(&end_node).as_ref() != Some(&block) {
            return Ok(None);
        }
        let start = offset_in_block(&block, &range.0);
        let end = offset_in_block(&block, &range.1);
        Ok(if start < 0 || end < start {
            None
        } else {
            Some(Before {
                block: block.get_key().to_owned(),
                end,
                range,
                start,
                text: children_text(&block),
            })
        })
    })?;
    let Some(before) = before else {
        return Ok(None);
    };
    Ok(read_editor_state(Some(editor), started, || {
        let range = get_selection().and_then(|s| s.range())?;
        let block = get_node_by_key(&before.block).filter(|b| b.is_element() && b.is_attached())?;
        let anchor = offset_in_block(&block, &range.anchor);
        let focus = offset_in_block(&block, &range.focus);
        let text = children_text(&block);
        let (start, end) = (before.start as usize, before.end as usize);
        let head = before.text.get(..start)?;
        let tail = before.text.get(end..)?;
        let selected = before.text.get(start..end)?;
        for placeholder in COMPOSITION_PLACEHOLDERS {
            let placeholder: U16s = placeholder.encode_utf16().collect();
            // A selection that already was the placeholder isn't replaced
            if !placeholder.is_empty() && placeholder == selected {
                continue;
            }
            let end = before.start + placeholder.len() as i64;
            let expected: U16s = [head, &placeholder, tail].concat();
            if text == expected
                && anchor.max(focus) == end
                && (anchor.min(focus) == end || anchor.min(focus) == before.start)
            {
                return Some(before.range.clone());
            }
        }
        None
    }))
}

/// `$collectSegments(flavor, windows, points, $isUnchangedBlock)`.
fn collect_segments(
    flavor: &MarkFlavor,
    windows: &[BlockWindow],
    points: &[Point],
    is_unchanged_block: &dyn Fn(&NodeRef) -> bool,
) -> Result<Segments, LexicalError> {
    struct Collector<'a> {
        flavor: &'a MarkFlavor,
        points: &'a [Point],
        is_unchanged_block: &'a dyn Fn(&NodeRef) -> bool,
        out: Segments,
        window_start: usize,
        after_nested: bool,
    }
    impl Collector<'_> {
        fn add(&mut self, node: &NodeRef, kind: u8, code: U16s, text: U16s, ids: Vec<String>) {
            if self.after_nested {
                self.after_nested = false;
                if kind != BREAK {
                    self.separate(node);
                }
            }
            let unit = if kind == CHAR {
                text.clone()
            } else {
                self.out.codes.insert(self.out.length, code.clone());
                vec![0xffff]
            };
            self.out.segs.push(Segment {
                code,
                ids,
                kind,
                node: node.clone(),
                text,
            });
            self.out.length += unit.len();
            self.out.text.extend(unit);
        }

        /// A break before a nested block's content, or after it.
        fn separate(&mut self, node: &NodeRef) {
            let last = (self.out.segs.len() > self.window_start)
                .then(|| self.out.segs.last())
                .flatten();
            if last.is_some_and(|last| last.kind != BREAK) {
                self.add(node, BREAK, vec![0], Vec::new(), Vec::new());
            }
        }

        fn locate_element_points(&mut self, key: &str, index: usize) {
            for (i, point) in self.points.iter().enumerate() {
                if point.point_type == PointType::Element
                    && point.key == key
                    && point.offset == index
                {
                    self.out.points[i] = self.out.length as i64;
                }
            }
        }

        fn visit_children(
            &mut self,
            element: &NodeRef,
            from: Option<&NodeRef>,
            to: Option<&NodeRef>,
        ) -> Result<(), LexicalError> {
            let key = element.get_key().to_owned();
            // The index of the child, to locate element points at
            let mut index: Option<usize> = self
                .points
                .iter()
                .any(|p| p.point_type == PointType::Element && p.key == key)
                .then(|| from.and_then(NodeRef::get_index_within_parent).unwrap_or(0));
            let mut child = match from {
                Some(from) => Some(from.clone()),
                None => element.get_first_child(),
            };
            while let Some(c) = child {
                if let Some(i) = index {
                    self.locate_element_points(&key, i);
                    index = Some(i + 1);
                }
                self.visit_node(&c)?;
                if to == Some(&c) {
                    break;
                }
                child = c.get_next_sibling();
            }
            if let Some(i) = index {
                self.locate_element_points(&key, i);
            }
            Ok(())
        }

        fn visit_node(&mut self, node: &NodeRef) -> Result<(), LexicalError> {
            let key = node.get_key().to_owned();
            if node.is_text() && is_markable_node(self.flavor, node) {
                let text: U16s = node.get_text_content().encode_utf16().collect();
                for (i, point) in self.points.iter().enumerate() {
                    if point.point_type == PointType::Text
                        && point.key == key
                        && point.offset <= text.len()
                    {
                        self.out.points[i] = (self.out.length + point.offset) as i64;
                    }
                }
                let ids = read_node_mark_ids(self.flavor, node)?;
                self.add(node, CHAR, Vec::new(), text, ids);
            } else if is_markable_node(self.flavor, node) {
                let ids = read_node_mark_ids(self.flavor, node)?;
                self.add(node, LEAF, code_of(2, &key), Vec::new(), ids);
            } else if node.is_element() && node.is_inline() {
                self.visit_children(node, None, None)?;
            } else if node.is_element() && !(self.is_unchanged_block)(node) {
                // A changed nested block (a list item's nested list...) is
                // compared through, between breaks
                self.separate(node);
                self.visit_children(node, None, None)?;
                self.after_nested = true;
            } else {
                self.add(node, OPAQUE, code_of(1, &key), Vec::new(), Vec::new());
            }
            Ok(())
        }
    }
    let mut collector = Collector {
        flavor,
        points,
        is_unchanged_block,
        out: Segments {
            length: 0,
            points: vec![-1; points.len()],
            segs: Vec::new(),
            text: Vec::new(),
            codes: HashMap::new(),
        },
        window_start: 0,
        after_nested: false,
    };
    for (i, window) in windows.iter().enumerate() {
        collector.after_nested = false;
        if i > 0 {
            collector.add(&window.block, BREAK, vec![0], Vec::new(), Vec::new());
        }
        collector.window_start = collector.out.segs.len();
        collector.visit_children(&window.block, window.from.as_ref(), window.to.as_ref())?;
    }
    Ok(collector.out)
}

fn is_high_surrogate(code: u16) -> bool {
    (0xd800..=0xdbff).contains(&code)
}

fn is_low_surrogate(code: u16) -> bool {
    (0xdc00..=0xdfff).contains(&code)
}

/// `sameEnd(a, b, limit, fromEnd)`: how many units the two sides share at
/// their start (or end), up to `limit`, without splitting a surrogate pair.
fn same_end(a: &Segments, b: &Segments, limit: i64, from_end: bool) -> usize {
    let at = |side: &Segments, k: usize| {
        if from_end {
            side.length.checked_sub(1 + k)
        } else {
            Some(k)
        }
    };
    let unit = |side: &Segments, k: usize| at(side, k).and_then(|i| side.text.get(i).copied());
    let mut k = 0;
    while (k as i64) < limit {
        let code = unit(a, k);
        if code.is_none()
            || code != unit(b, k)
            || (code == Some(0xffff)
                && at(a, k).and_then(|i| a.codes.get(&i)) != at(b, k).and_then(|i| b.codes.get(&i)))
        {
            break;
        }
        k += 1;
    }
    let last = if k > 0 {
        unit(a, k - 1).unwrap_or(0)
    } else {
        0
    };
    if (from_end && is_low_surrogate(last)) || (!from_end && is_high_surrogate(last)) {
        k - 1
    } else {
        k
    }
}

/// `segmentEnds(side)`: where each segment ends, in units.
fn segment_ends(side: &Segments) -> Vec<i64> {
    let mut position = 0i64;
    side.segs
        .iter()
        .map(|seg| {
            position += if seg.kind == CHAR {
                seg.text.len() as i64
            } else {
                1
            };
            position
        })
        .collect()
}

/// `flattenSide(side, other, prefix, suffix)`: one unit per character
/// between the shared prefix and suffix, and one per run of the prefix and
/// suffix (cut where either side has a segment end), each with a code that
/// tells the two sides' units apart.
fn flatten_side(side: &Segments, other: &Segments, prefix: usize, suffix: usize) -> Flattened {
    let mut out = Flattened {
        codes: Vec::new(),
        ids: Vec::new(),
        kinds: Vec::new(),
        nodes: Vec::new(),
        offsets: Vec::new(),
        points: vec![-1; side.points.len()],
    };
    let (length, other_length) = (side.length as i64, other.length as i64);
    let (prefix, suffix) = (prefix as i64, suffix as i64);
    let suffix_start = length - suffix;
    let mut cuts: Vec<i64> = segment_ends(side);
    for end in segment_ends(other) {
        if end < prefix {
            cuts.push(end);
        } else if end > other_length - suffix {
            cuts.push(end - other_length + length);
        }
    }
    cuts.push(prefix);
    cuts.push(suffix_start);
    cuts.sort_unstable();
    cuts.dedup();
    let mut cut = 0;
    let mut starts: Vec<i64> = Vec::new();
    let mut position = 0i64;
    for seg in &side.segs {
        if seg.kind != CHAR {
            out.codes.push(seg.code.clone());
            out.kinds.push(seg.kind);
            out.nodes.push(seg.node.clone());
            out.offsets.push(0);
            out.ids.push(seg.ids.clone());
            starts.push(position);
            position += 1;
            continue;
        }
        let text_len = seg.text.len() as i64;
        let mut offset = 0i64;
        while offset < text_len {
            let at = position + offset;
            let (code, next) = if at < prefix || at >= suffix_start {
                while cuts.get(cut).is_some_and(|&c| c <= at) {
                    cut += 1;
                }
                let end = cuts
                    .get(cut)
                    .copied()
                    .unwrap_or(i64::MAX)
                    .min(position + text_len);
                let code = if at < prefix {
                    code_of(3, &at.to_string())
                } else {
                    code_of(4, &(length - at).to_string())
                };
                (code, end - position)
            } else {
                let o = offset as usize;
                let size = if is_high_surrogate(seg.text[o])
                    && seg.text.get(o + 1).copied().is_some_and(is_low_surrogate)
                {
                    2
                } else {
                    1
                };
                (seg.text[o..o + size].to_vec(), offset + size as i64)
            };
            out.codes.push(code);
            out.kinds.push(CHAR);
            out.nodes.push(seg.node.clone());
            out.offsets.push(offset as usize);
            out.ids.push(seg.ids.clone());
            starts.push(at);
            offset = next;
        }
        position += text_len;
    }
    for (i, &point) in side.points.iter().enumerate() {
        if point >= 0 {
            out.points[i] = starts.iter().take_while(|&&start| start < point).count() as i64;
        }
    }
    out
}

/// `intersect(a, b)`.
fn intersect(a: &[String], b: &[String]) -> Vec<String> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    a.iter().filter(|id| b.contains(id)).cloned().collect()
}

/// `without(a, removed)`.
fn without(a: &[String], removed: &[String]) -> Vec<String> {
    a.iter().filter(|id| !removed.has(id)).cloned().collect()
}

/// `union(a, b)`: sorted, without duplicates.
fn union(a: &[String], b: &[String]) -> Vec<String> {
    if b.is_empty() {
        return a.to_vec();
    }
    if a.is_empty() {
        return b.to_vec();
    }
    let mut ids: Vec<String> = Vec::new();
    for id in a.iter().chain(b) {
        if !ids.contains(id) {
            ids.push(id.clone());
        }
    }
    ids.sort_by(|x, y| js_cmp(x, y));
    ids
}

/// `MATCH_BUDGET`: the largest table `matchUnits` fills.
const MATCH_BUDGET: usize = 250_000;

/// `matchUnits(a, a0, a1, b, b0, b1)`: a longest common subsequence of the
/// two ranges of codes, as index pairs; `None` past the budget.
fn match_units(
    a: &[U16s],
    a0: usize,
    a1: usize,
    b: &[U16s],
    b0: usize,
    b1: usize,
) -> Option<Vec<(usize, usize)>> {
    let n = a1.saturating_sub(a0);
    let m = b1.saturating_sub(b0);
    if n == 0 || m == 0 {
        return Some(Vec::new());
    }
    if n * m > MATCH_BUDGET {
        return None;
    }
    let width = m + 1;
    let mut lengths = vec![0u32; (n + 1) * width];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lengths[i * width + j] = if a[a0 + i] == b[b0 + j] {
                lengths[(i + 1) * width + j + 1] + 1
            } else {
                lengths[(i + 1) * width + j].max(lengths[i * width + j + 1])
            };
        }
    }
    let mut pairs = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if a[a0 + i] == b[b0 + j] {
            pairs.push((a0 + i, b0 + j));
            i += 1;
            j += 1;
        } else if lengths[(i + 1) * width + j] >= lengths[i * width + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    Some(pairs)
}

/// `$hasMarkIDsState(flavor, node)`: whether the node's state holds the
/// flavor's key at all (set, or read from JSON), which a node split off or
/// copied from another carries over.
fn has_mark_ids_state(flavor: &MarkFlavor, node: &NodeRef) -> bool {
    has_state_value(&*node.get_latest(), &flavor.state.erased())
}

/// `$createTextNode('').constructor === node.constructor`: like JS, this
/// makes (and leaves to the garbage collector) a text node.
fn is_created_text_class(node: &NodeRef) -> bool {
    create_text_node("").get_type() == node.get_type()
}

/// `$splitsIntoOwnClass(node)`: whether `splitText` makes pieces of the
/// node's own class (a token or segmented node, or a subclass that is not
/// the text class, is not split for ids).
fn splits_into_own_class(node: &NodeRef) -> bool {
    let mode = node.get_latest().text().map(|t| t.get_mode());
    mode == Some("normal") && (node.is_simple_text() || is_created_text_class(node))
}

/// `$setUnitIDs(flavor, flat, ids, start, end, splits)`: gives the units
/// `start..end` of one node their ids, splitting a text node where they
/// change (or, when it can't be split, giving it all of them).
fn set_unit_ids(
    flavor: &Rc<MarkFlavor>,
    flat: &Flattened,
    ids: &[Vec<String>],
    start: usize,
    end: usize,
    splits: &SplitOrigins,
) -> Result<(), LexicalError> {
    let state = &flavor.state;
    let node = &flat.nodes[start];
    if !node.is_text() {
        set_state(node, state, ids[start].clone())?;
        return Ok(());
    }
    let offsets: Vec<i64> = (start + 1..end)
        .filter(|&i| !state.is_equal(&ids[i], &ids[i - 1]))
        .map(|i| flat.offsets[i] as i64)
        .collect();
    if !offsets.is_empty() && !splits_into_own_class(node) {
        let mut all = Vec::new();
        for unit in &ids[start..end] {
            all = union(&all, unit);
        }
        if !state.is_equal(&read_node_mark_ids(flavor, node)?, &all) {
            set_state(node, state, all)?;
        }
        return Ok(());
    }
    let parts = if offsets.is_empty() {
        vec![node.clone()]
    } else {
        node.split_text(&offsets)?
    };
    // The pieces stand for the node in the comparisons of the other flavors
    for part in parts.iter().skip(1) {
        splits.0.borrow_mut().insert(
            part.get_key().to_owned(),
            (flavor.clone(), node.get_key().to_owned()),
        );
    }
    let mut i = start;
    for part in &parts {
        let Some(part_ids) = ids.get(i) else {
            break;
        };
        if !state.is_equal(&read_node_mark_ids(flavor, part)?, part_ids) {
            set_state(part, state, part_ids.clone())?;
        }
        // The next unit whose ids differ starts the next piece
        loop {
            i += 1;
            if !(i < end && state.is_equal(&ids[i], &ids[i - 1])) {
                break;
            }
        }
    }
    Ok(())
}

/// `getInsertedRange(prevText, text, caret)`: the code units of `text`
/// that are not in `prev_text`: after the longest shared prefix, and up to
/// the caret when the rest of the text is `prev_text`'s end, or else the
/// longest shared suffix.
fn get_inserted_range(prev_text: &[u16], text: &[u16], caret: Option<usize>) -> (usize, usize) {
    let mut end: Option<usize> = None;
    let mut max_start = prev_text.len().min(text.len());
    if let Some(caret) = caret.filter(|&c| c <= text.len())
        && prev_text.ends_with(&text[caret..])
    {
        end = Some(caret);
        max_start = caret.min(prev_text.len() - (text.len() - caret));
    }
    let mut start = 0;
    while start < max_start && prev_text[start] == text[start] {
        start += 1;
    }
    if start > 0 && is_high_surrogate(text[start - 1]) {
        start -= 1;
    }
    let end = end.unwrap_or_else(|| {
        let mut suffix = 0;
        let max_suffix = prev_text.len().min(text.len()) - start;
        while suffix < max_suffix
            && prev_text[prev_text.len() - 1 - suffix] == text[text.len() - 1 - suffix]
        {
            suffix += 1;
        }
        let mut end = text.len() - suffix;
        if end < text.len() && is_low_surrogate(text[end]) {
            end += 1;
        }
        end
    });
    (start, end)
}

/// `prevNodes.get(node.getKey()) === node`: the node is the version
/// `prev_nodes` has.
fn is_same_node(prev_nodes: &crate::editor_state::NodeMap, node: &NodeRef) -> bool {
    prev_nodes
        .get(node.get_key())
        .is_some_and(|prev| Rc::ptr_eq(prev, &node.get_latest()))
}

/// `$applyInsertedTextMarkIDs(flavor, editor, prevState, replaced, removed,
/// splits, replacedForSure)`: the inserted-text rule. Run in the root
/// transform, it compares the block of a collapsed selection (with the
/// blocks the update split it into or joined to it) with `prev_state`, unit
/// by unit, and gives the units that are not in `prev_state`:
///
/// - inside a marked run (between two units that share ids), those ids;
/// - at the edge of a run, none of them;
/// - in place of a replaced selection (`replaced`), the ids all of it had,
///   too;
/// - their own ids, when they were inserted with them (a paste), unless
///   they were split off an entity with the ids it had.
///
/// The ids in `removed` are not given back. Units that were in
/// `prev_state` keep theirs, and a text node that ends up with ids that
/// change along it is split.
pub(super) fn apply_inserted_text_mark_ids(
    flavor: &Rc<MarkFlavor>,
    editor: &Editor,
    prev_state: &EditorState,
    replaced: Option<&(Point, Point)>,
    removed: &[String],
    splits: &SplitOrigins,
    replaced_for_sure: bool,
) -> Result<(), LexicalError> {
    let Some(range) = get_selection()
        .and_then(|s| s.range())
        .filter(|r| r.is_collapsed())
    else {
        return Ok(());
    };
    let caret = range.anchor.clone();
    let caret_node = caret.get_node()?;
    let Some(block) = get_block(&caret_node) else {
        return Ok(());
    };
    let prev_nodes = prev_state.0.node_map.borrow();
    let prev_nodes = &*prev_nodes;
    let in_prev = |key: &str| prev_nodes.contains_key(key);
    // An inline element whose children are all as they were, with a unit
    fn is_unchanged_inline(prev_nodes: &crate::editor_state::NodeMap, element: &NodeRef) -> bool {
        let mut has_units = false;
        let mut child = element.get_first_child();
        while let Some(c) = child {
            if !is_same_node(prev_nodes, &c) {
                return false;
            }
            if c.is_element() && c.is_inline() {
                if !is_unchanged_inline(prev_nodes, &c) {
                    return false;
                }
                has_units = true;
            } else if !c.is_text() || c.get_text_content_size() > 0 {
                has_units = true;
            }
            child = c.get_next_sibling();
        }
        has_units
    }
    // A sibling of the edit that is as it was: the comparison's context
    let is_context = |node: &NodeRef| -> bool {
        if !is_same_node(prev_nodes, node) {
            return false;
        }
        if node.is_text() {
            return node.get_text_content_size() > 0;
        }
        !(node.is_element() && node.is_inline()) || is_unchanged_inline(prev_nodes, node)
    };
    let expand = |start: Option<NodeRef>, previous: bool| -> Option<NodeRef> {
        let mut node = start;
        while let Some(n) = node.clone() {
            if is_context(&n) {
                break;
            }
            node = if previous {
                n.get_previous_sibling()
            } else {
                n.get_next_sibling()
            };
        }
        node
    };
    // The caret's siblings in its block, or those of its ancestor there
    let (left, right) = if caret.point_type == PointType::Element && caret.key == block.get_key() {
        let right = block.get_child_at_index(caret.offset);
        let left = match &right {
            Some(right) => right.get_previous_sibling(),
            None => block.get_last_child(),
        };
        (left, right)
    } else {
        let mut top = caret_node.clone();
        while let Some(parent) = top.get_parent().filter(|p| *p != block) {
            top = parent;
        }
        (top.get_previous_sibling(), top.get_next_sibling())
    };
    let from = expand(left, true);
    let to = expand(right, false);
    // The blocks the update made of the caret's block in prev_state: from
    // the nearest one before it that was there, to the next one that was
    // there and still starts as it did
    let is_next_boundary = |node: &NodeRef| -> bool {
        if !in_prev(node.get_key()) {
            return false;
        }
        let first_child = if node.is_element() {
            node.get_first_child()
        } else {
            None
        };
        is_same_node(prev_nodes, node)
            || first_child.is_none_or(|first_child| is_same_node(prev_nodes, &first_child))
    };
    let parent = block.get_parent();
    let mut first = Some(block.clone());
    let mut next = None;
    if parent.is_some() {
        while let Some(f) = first.clone() {
            if in_prev(f.get_key()) {
                break;
            }
            first = f.get_previous_sibling();
        }
        next = block.get_next_sibling();
        while let Some(n) = next.clone() {
            if is_next_boundary(&n) {
                break;
            }
            next = n.get_next_sibling();
        }
    } else if !in_prev(block.get_key()) {
        return Ok(());
    }
    let mut windows: Vec<BlockWindow> = Vec::new();
    let mut current = match (&first, &parent) {
        (Some(first), _) => Some(first.clone()),
        (None, Some(parent)) => parent.get_first_child(),
        (None, None) => None,
    };
    while let Some(c) = current.clone() {
        if next.as_ref() == Some(&c) {
            break;
        }
        if !c.is_element() {
            return Ok(());
        }
        let is_caret_block = c == block;
        let window_from = if windows.is_empty() && first.is_some() {
            if is_caret_block {
                from.clone()
            } else {
                expand(c.get_last_child(), true)
            }
        } else {
            None
        };
        windows.push(BlockWindow {
            block: c.clone(),
            from: window_from,
            to: if is_caret_block { to.clone() } else { None },
        });
        if (is_caret_block && to.is_some()) || parent.is_none() {
            // The context after the caret ends the comparison
            break;
        }
        current = c.get_next_sibling();
    }
    // @internal access: the dirty elements and the pending state, which a
    // block that did not change at all is the same in
    let pending = editor.pending();
    let is_unchanged_block = |node: &NodeRef| -> bool {
        let key = node.get_key();
        let Some(prev_node) = prev_nodes.get(key) else {
            return false;
        };
        pending
            .as_ref()
            .and_then(|pending| get_node_by_key_impl(pending, key))
            .is_some_and(|next| Rc::ptr_eq(prev_node, &next))
            && !editor.0.dirty.borrow().elements.contains(key)
    };
    let after_segments = collect_segments(
        flavor,
        &windows,
        std::slice::from_ref(&caret),
        &is_unchanged_block,
    )?;
    let first_key = first.as_ref().map(|f| f.get_key().to_owned());
    let next_key = next.as_ref().map(|n| n.get_key().to_owned());
    let parent_key = parent.as_ref().map(|p| p.get_key().to_owned());
    let block_key = block.get_key().to_owned();
    let from_key = windows[0].from.as_ref().map(|f| f.get_key().to_owned());
    let to_key = windows
        .last()
        .and_then(|w| w.to.as_ref())
        .map(|t| t.get_key().to_owned());
    let replaced_points: Vec<Point> = replaced
        .map(|(start, end)| vec![start.clone(), end.clone()])
        .unwrap_or_default();
    let before_segments = read_editor_state(Some(editor), prev_state, || {
        let mut prev_windows: Vec<BlockWindow> = Vec::new();
        let mut current = match &first_key {
            Some(first_key) => {
                let current = get_node_by_key(first_key);
                let prev_parent = current.as_ref().and_then(NodeRef::get_parent);
                if prev_parent.as_ref().map(NodeRef::get_key) != parent_key.as_deref() {
                    return Ok(None);
                }
                current
            }
            None => {
                let Some(prev_parent) = parent_key
                    .as_deref()
                    .and_then(get_node_by_key)
                    .filter(NodeRef::is_element)
                else {
                    return Ok(None);
                };
                prev_parent.get_first_child()
            }
        };
        let mut reached_to = false;
        while let Some(c) = current.clone() {
            if next_key.as_deref() == Some(c.get_key()) {
                break;
            }
            if !c.is_element() {
                return Ok(None);
            }
            let mut window = BlockWindow {
                block: c.clone(),
                from: None,
                to: None,
            };
            if prev_windows.is_empty()
                && let Some(from_key) = &from_key
            {
                // The context before the edit is where it was
                window.from = get_node_by_key(from_key);
                if window.from.as_ref().and_then(NodeRef::get_parent).as_ref() != Some(&c) {
                    return Ok(None);
                }
            }
            if let Some(to_key) = to_key.as_deref().filter(|_| c.get_key() == block_key) {
                window.to = get_node_by_key(to_key);
                if window.to.as_ref().and_then(NodeRef::get_parent).as_ref() != Some(&c) {
                    return Ok(None);
                }
                prev_windows.push(window);
                reached_to = true;
                break;
            }
            prev_windows.push(window);
            if parent_key.is_none() {
                break;
            }
            current = c.get_next_sibling();
        }
        if if to_key.is_some() {
            !reached_to
        } else {
            next_key.is_some() && current.is_none()
        } {
            return Ok(None);
        }
        collect_segments(flavor, &prev_windows, &replaced_points, &is_unchanged_block).map(Some)
    })?;
    let caret_at = after_segments.points[0];
    let Some(before_segments) = before_segments else {
        return Ok(());
    };
    if caret_at < 0 {
        return Ok(());
    }
    // The shared prefix ends at the caret, and the shared suffix starts
    // there; neither crosses the replaced selection
    let (after_length, before_length) =
        (after_segments.length as i64, before_segments.length as i64);
    let mut suffix_limit = after_length - caret_at;
    let mut prefix_limit = caret_at.min(before_length - suffix_limit).max(0);
    let replaced_start = before_segments.points.first().copied().unwrap_or(-1);
    let replaced_end = before_segments.points.get(1).copied().unwrap_or(-1);
    if replaced_start >= 0 && replaced_end >= 0 {
        prefix_limit = prefix_limit.min(replaced_start);
        suffix_limit = suffix_limit.min(before_length - replaced_end);
    }
    let prefix = same_end(&after_segments, &before_segments, prefix_limit, false);
    let suffix = same_end(
        &after_segments,
        &before_segments,
        suffix_limit.min(after_length.min(before_length) - prefix as i64),
        true,
    );
    let after = flatten_side(&after_segments, &before_segments, prefix, suffix);
    let before = flatten_side(&before_segments, &after_segments, prefix, suffix);
    let c = after.points[0] as usize;
    let n = after.codes.len();
    let m = before.codes.len();
    // The units after the caret are the end of prev_state's
    let k = n - c;
    if k > m {
        return Ok(());
    }
    for i in 1..=k {
        if after.codes[n - i] != before.codes[m - i] {
            return Ok(());
        }
    }
    let e = m - k;
    let mut s = 0;
    while s < c && s < e && after.codes[s] == before.codes[s] {
        s += 1;
    }
    // @internal access: the text nodes normalization merged, which a new
    // node merged into an old one makes new text of
    let normalized: HashSet<NodeKey> = editor
        .0
        .dirty
        .borrow()
        .normalized
        .iter()
        .map(str::to_owned)
        .collect();
    let absorbed_new = normalized
        .iter()
        .any(|key| !in_prev(key) && get_node_by_key(key).is_none_or(|node| !node.is_attached()));
    // Text that was not in prev_state (or that another flavor's split cut
    // off text that was not)
    let is_new_text = |node: &NodeRef| -> bool {
        let key = get_split_origin(splits, flavor, node.get_key());
        !in_prev(&key) || (absorbed_new && normalized.contains(&key))
    };
    // A selection replaced with new text and leaves
    let mut replaced_selection = false;
    if replaced.is_some()
        && let [start, end] = before.points[..]
        && start >= 0
        && end > start
        && end as usize == e
        && start as usize <= s
    {
        let start = start as usize;
        replaced_selection = true;
        let mut i = start;
        while i < c && replaced_selection && !replaced_for_sure {
            replaced_selection = after.kinds[i] > LEAF || is_new_text(&after.nodes[i]);
            i += 1;
        }
        if replaced_selection {
            s = start;
        }
    }
    // The nearest sibling before a new node that was in prev_state
    let origins: RefCell<HashMap<NodeKey, Option<NodeRef>>> = RefCell::default();
    let get_origin = |node: &NodeRef| -> Option<NodeRef> {
        let mut keys: Vec<NodeKey> = Vec::new();
        let mut origin = node.get_previous_sibling();
        while let Some(o) = origin.clone() {
            if in_prev(o.get_key()) {
                break;
            }
            if let Some(known) = origins.borrow().get(o.get_key()) {
                origin = known.clone();
                break;
            }
            keys.push(o.get_key().to_owned());
            origin = o.get_previous_sibling();
        }
        for key in keys {
            origins.borrow_mut().insert(key, origin.clone());
        }
        origin
    };
    // New text split off an entity (a hashtag turned into text) with the
    // ids it had: they are the entity's, not the text's own
    let split_off: RefCell<HashMap<NodeKey, bool>> = RefCell::default();
    let erased = flavor.state.erased();
    let is_split_off_entity = |node: &NodeRef| -> Result<bool, LexicalError> {
        let key = node.get_key().to_owned();
        if let Some(&result) = split_off.borrow().get(&key) {
            return Ok(result);
        }
        let origin = if node.is_text() && !in_prev(&key) {
            get_origin(node)
        } else {
            None
        };
        let result = match origin {
            Some(origin) if origin.is_text() && origin.get_type() != node.get_type() => {
                // The same ids value, as splitText copies it
                Rc::ptr_eq(
                    &get_state_erased(&*origin.get_latest(), &erased)?,
                    &get_state_erased(&*node.get_latest(), &erased)?,
                ) && is_created_text_class(node)
            }
            _ => false,
        };
        split_off.borrow_mut().insert(key, result);
        Ok(result)
    };
    let mut ids = after.ids.clone();
    // A new unit without ids next to one of prev_state with ids takes
    // them: the text the update moved or re-created
    let keep = |ids: &mut Vec<Vec<String>>, i: usize, j: usize| {
        if after.kinds[i] == CHAR && ids[i].is_empty() && !before.ids[j].is_empty() {
            let node = &after.nodes[i];
            let key = node.get_key();
            if key != before.nodes[j].get_key()
                && ((!in_prev(key) && !has_mark_ids_state(flavor, node))
                    || (absorbed_new && normalized.contains(key)))
            {
                ids[i] = without(&before.ids[j], removed);
            }
        }
    };
    for i in 0..s {
        keep(&mut ids, i, i);
    }
    for i in c..n {
        keep(&mut ids, i, i - c + e);
    }
    // The gaps between the units the two sides share, between the prefix
    // and the caret
    let mut gaps: Vec<(usize, usize, usize, usize)> = Vec::new();
    let pairs = if replaced_selection {
        Some(Vec::new())
    } else {
        match_units(&after.codes, s, c, &before.codes, s, e)
    };
    let (mut i0, mut j0) = (s, s);
    for (i, j) in pairs.unwrap_or_default() {
        if i > i0 || j > j0 {
            gaps.push((i0, i, j0, j));
        }
        keep(&mut ids, i, j);
        i0 = i + 1;
        j0 = j + 1;
    }
    if c > i0 || e > j0 {
        gaps.push((i0, c, j0, e));
    }
    let unit_ids = |ids: &[Vec<String>], i: Option<usize>| -> Vec<String> {
        match i {
            Some(i) if i < n && after.kinds[i] <= LEAF => ids[i].clone(),
            _ => Vec::new(),
        }
    };
    // The nodes of prev_state's side, whose text the comparison has seen
    let compared: HashSet<&str> = before.nodes.iter().map(NodeRef::get_key).collect();
    // A unit of a node that was in prev_state and kept its text, or that is
    // outside what was inserted into it
    let inserted_ranges: RefCell<HashMap<NodeKey, (usize, usize)>> = RefCell::default();
    let is_kept_unit = |i: usize| -> bool {
        let node = &after.nodes[i];
        let Some(prev_node) = prev_nodes.get(node.get_key()) else {
            return false;
        };
        let latest = node.get_latest();
        let (Some(text), Some(prev_text)) = (latest.text(), prev_node.text()) else {
            return true;
        };
        let prev_text = prev_text.get_text();
        if prev_text == text.get_text() {
            return true;
        }
        if compared.contains(node.get_key()) {
            return false;
        }
        let key = node.get_key().to_owned();
        let range = *inserted_ranges.borrow_mut().entry(key).or_insert_with(|| {
            let caret_offset = (caret.point_type == PointType::Text && caret.key == node.get_key())
                .then_some(caret.offset);
            let prev: U16s = prev_text.encode_utf16().collect();
            let next: U16s = text.get_text().encode_utf16().collect();
            get_inserted_range(&prev, &next, caret_offset)
        });
        after.offsets[i] < range.0 || after.offsets[i] >= range.1
    };
    for (g0, g1, r0, r1) in gaps {
        if g1 == g0 {
            continue;
        }
        let kept: Vec<bool> = (g0..g1)
            .map(|i| after.kinds[i] <= LEAF && is_kept_unit(i))
            .collect();
        // The ids all of the replaced units had
        let mut replaced_ids: Option<Vec<String>> = None;
        for j in r0..r1 {
            if before.kinds[j] != BREAK {
                let unit = if before.kinds[j] <= LEAF {
                    before.ids[j].clone()
                } else {
                    Vec::new()
                };
                replaced_ids = Some(match replaced_ids {
                    None => unit,
                    Some(replaced_ids) => intersect(&replaced_ids, &unit),
                });
            }
        }
        // Each run of inserted units gets the ids its neighbors share
        let mut start = g0;
        while start < g1 {
            if kept[start - g0] {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < g1 && !kept[end - g0] {
                end += 1;
            }
            let mut shared = intersect(
                &unit_ids(&ids, start.checked_sub(1)),
                &unit_ids(&ids, Some(end)),
            );
            if let Some(replaced_ids) = &replaced_ids {
                shared = union(&shared, &without(replaced_ids, removed));
            }
            for i in start..end {
                if after.kinds[i] <= LEAF {
                    // Pasted content keeps its own ids
                    let node = &after.nodes[i];
                    let own = if is_new_text(node)
                        && !after.ids[i].is_empty()
                        && !is_split_off_entity(node)?
                    {
                        after.ids[i].clone()
                    } else {
                        Vec::new()
                    };
                    ids[i] = union(&own, &shared);
                }
            }
            start = end;
        }
    }
    let mut i = 0;
    while i < n {
        let mut end = i + 1;
        while end < n && after.nodes[end] == after.nodes[i] {
            end += 1;
        }
        if (i..end)
            .any(|u| after.kinds[u] <= LEAF && !flavor.state.is_equal(&ids[u], &after.ids[u]))
        {
            set_unit_ids(flavor, &after, &ids, i, end, splits)?;
        }
        i = end;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> U16s {
        s.encode_utf16().collect()
    }

    #[test]
    fn inserted_range_up_to_the_caret() {
        assert_eq!(get_inserted_range(&u("abc"), &u("abxc"), Some(3)), (2, 3));
        // Repeated letters: the caret decides which was typed
        assert_eq!(get_inserted_range(&u("aa"), &u("aaa"), Some(1)), (0, 1));
        assert_eq!(get_inserted_range(&u("aa"), &u("aaa"), None), (2, 3));
        // A surrogate pair is not split
        assert_eq!(
            get_inserted_range(&u("a\u{1f600}"), &u("a\u{1f601}\u{1f600}"), None),
            (1, 3)
        );
    }

    #[test]
    fn units_match_as_a_longest_common_subsequence() {
        let a: Vec<U16s> = ["a", "b", "c"].map(u).to_vec();
        let b: Vec<U16s> = ["a", "c"].map(u).to_vec();
        assert_eq!(match_units(&a, 0, 3, &b, 0, 2), Some(vec![(0, 0), (2, 1)]));
    }
}
