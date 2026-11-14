//! `internal/blockHandles.ts`: block handles, the shortest prefix, at least
//! [`MIN_BLOCK_HANDLE_LENGTH`] characters long, of a hash of a block's id
//! that no other id's hash of the document starts with, like a short git
//! hash. Hashing first makes handles short and typo-safe whatever the ids
//! look like. Handles are derived from the ids of one document and never
//! stored. These helpers are pure: `BlockIdExtension` feeds them the ids and
//! decides which blocks are live.

use crate::ordered::OrderedMap;
use std::cmp::Ordering;

/// `MIN_BLOCK_HANDLE_LENGTH` (`@experimental`): the shortest handle.
pub const MIN_BLOCK_HANDLE_LENGTH: usize = 4;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// `HANDLE_KEY_LENGTH`: the length of a hash key, and so the longest handle:
/// 64 bits in base64url. A longer string is looked up as a whole id.
pub(crate) const HANDLE_KEY_LENGTH: usize = 11;

/// `handleKey(id)`: the hash key of an id: 64 bits of a fast
/// non-cryptographic hash (cyrb53's mixing, kept to 64 bits), in base64url,
/// most significant bits first. `Math.imul` is a wrapping 32-bit multiply.
pub(crate) fn handle_key(id: &str) -> String {
    let mut h1: u32 = 0xdeadbeef;
    let mut h2: u32 = 0x41c6ce57;
    for c in id.encode_utf16() {
        h1 = (h1 ^ u32::from(c)).wrapping_mul(2654435761);
        h2 = (h2 ^ u32::from(c)).wrapping_mul(1597334677);
    }
    h1 = (h1 ^ (h1 >> 16)).wrapping_mul(2246822507);
    h1 ^= (h2 ^ (h2 >> 13)).wrapping_mul(3266489909);
    h2 = (h2 ^ (h2 >> 16)).wrapping_mul(2246822507);
    h2 ^= (h1 ^ (h1 >> 13)).wrapping_mul(3266489909);
    // h2 then h1, 6 bits at a time: 10 whole characters and 4 bits.
    let mut key = String::with_capacity(HANDLE_KEY_LENGTH);
    let mut bit: i32 = 0;
    while bit < 64 {
        let value = if bit + 6 <= 32 {
            h2 >> (26 - bit)
        } else if bit >= 32 {
            let shift = 26 - (bit - 32);
            if shift >= 0 {
                h1 >> shift
            } else {
                h1 << -shift
            }
        } else {
            let low = bit + 6 - 32;
            (h2 << low) | (h1 >> (32 - low))
        };
        key.push(ALPHABET[(value & 63) as usize] as char);
        bit += 6;
    }
    key
}

/// `HandleTable`: the handles of a set of ids.
pub(crate) struct HandleTable {
    /// The distinct hash keys, in ascending UTF-16 code unit order.
    pub(crate) sorted: Vec<String>,
    /// Hash key → the ids that have it (one, unless the hash collides).
    pub(crate) ids_of_key: OrderedMap<Vec<String>>,
    /// Id → its handle.
    pub(crate) handles: OrderedMap<String>,
}

fn common_prefix_length(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}

/// The handle of `key` between its neighbours in sorted order: long enough
/// to differ from both, so from every other key. Hash keys are ASCII.
fn handle_between(key: &str, prev: Option<&str>, next: Option<&str>) -> String {
    let mut length = MIN_BLOCK_HANDLE_LENGTH;
    if let Some(prev) = prev {
        length = length.max(common_prefix_length(key, prev) + 1);
    }
    if let Some(next) = next {
        length = length.max(common_prefix_length(key, next) + 1);
    }
    key[..length.min(key.len())].to_owned()
}

/// The first index of `sorted` whose key is not below `value`.
fn lower_bound(sorted: &[String], value: &str) -> usize {
    sorted.partition_point(|key| crate::utf16::cmp(key, value) == Ordering::Less)
}

/// `buildHandleTable(ids)`: the handles of `ids` (duplicates allowed), in
/// O(n log n): after sorting their hash keys, only a key's neighbours can
/// share a longer prefix with it than any other key.
pub(crate) fn build_handle_table<'a>(ids: impl IntoIterator<Item = &'a str>) -> HandleTable {
    let mut ids_of_key: OrderedMap<Vec<String>> = OrderedMap::new();
    let mut seen = crate::ordered::OrderedSet::new();
    for id in ids {
        if seen.contains(id) {
            continue;
        }
        seen.add(id);
        let key = handle_key(id);
        match ids_of_key.get_mut(&key) {
            Some(list) => list.push(id.to_owned()),
            None => ids_of_key.set(&key, vec![id.to_owned()]),
        }
    }
    let mut sorted: Vec<String> = ids_of_key.keys().map(str::to_owned).collect();
    sorted.sort_by(|a, b| crate::utf16::cmp(a, b));
    let mut handles = OrderedMap::new();
    for (i, key) in sorted.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| sorted[j].as_str());
        let next = sorted.get(i + 1).map(String::as_str);
        let handle = handle_between(key, prev, next);
        for id in ids_of_key.get(key).expect("a key of the table") {
            handles.set(id, handle.clone());
        }
    }
    HandleTable {
        sorted,
        ids_of_key,
        handles,
    }
}

/// `handleInTable(table, id)`: the handle of `id` against the ids of
/// `table`, whether or not `id` is one of them.
pub(crate) fn handle_in_table(table: &HandleTable, id: &str) -> String {
    if let Some(known) = table.handles.get(id) {
        return known.clone();
    }
    let key = handle_key(id);
    let sorted = &table.sorted;
    let i = lower_bound(sorted, &key);
    let at = sorted.get(i).map(String::as_str);
    let next = if at == Some(key.as_str()) {
        sorted.get(i + 1).map(String::as_str)
    } else {
        at
    };
    let prev = i.checked_sub(1).map(|j| sorted[j].as_str());
    handle_between(&key, prev, next)
}

/// `idsWithPrefix(table, prefix)`: the ids of `table` whose hash key starts
/// with `prefix`, by key.
pub(crate) fn ids_with_prefix(table: &HandleTable, prefix: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut i = lower_bound(&table.sorted, prefix);
    while i < table.sorted.len() && table.sorted[i].starts_with(prefix) {
        result.extend(
            table
                .ids_of_key
                .get(&table.sorted[i])
                .expect("a key of the table")
                .iter()
                .cloned(),
        );
        i += 1;
    }
    result
}
