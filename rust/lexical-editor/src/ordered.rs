//! Insertion-ordered collections with JS `Map` and `Set` semantics: setting
//! an existing key keeps its position, a deleted key that is added again goes
//! last, and iterating by index (`get(i)` for `i = 0, 1, ...`) visits entries
//! added during the iteration, as `for...of` over a JS `Set` does. The update
//! cycle depends on that: normalizing a dirty text node can dirty others,
//! which the same pass then visits.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// A hasher for node keys and other short strings the editor makes itself
/// (FxHash's mixing, eight bytes at a time). The standard library's
/// SipHash guards against keys chosen to collide, which these are not, and
/// costs more than the rest of a lookup: an update looks a node up by its
/// key many times for each node it touches.
#[derive(Clone, Copy, Default)]
pub(crate) struct KeyHasher(u64);

impl Hasher for KeyHasher {
    fn write(&mut self, bytes: &[u8]) {
        let mut word = 0u64;
        let mut i = 0;
        while i < bytes.len() {
            word |= u64::from(bytes[i]) << ((i % 8) * 8);
            i += 1;
            if i % 8 == 0 || i == bytes.len() {
                self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
                word = 0;
            }
        }
    }

    fn write_u8(&mut self, byte: u8) {
        self.0 = (self.0.rotate_left(5) ^ u64::from(byte)).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }

    fn finish(&self) -> u64 {
        // The multiplication mixes into the high bits, and the table picks
        // a bucket by the low ones.
        self.0.rotate_left(26)
    }
}

/// A `HashMap` keyed with [`KeyHasher`].
pub(crate) type KeyMap<K, V> = HashMap<K, V, BuildHasherDefault<KeyHasher>>;

#[derive(Clone, Debug)]
pub(crate) struct OrderedMap<V> {
    entries: Vec<Option<(String, V)>>,
    index: KeyMap<String, usize>,
}

impl<V> Default for OrderedMap<V> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            index: KeyMap::default(),
        }
    }
}

impl<V> OrderedMap<V> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn len(&self) -> usize {
        self.index.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    pub(crate) fn contains(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    /// `map.get(key)`.
    pub(crate) fn get(&self, key: &str) -> Option<&V> {
        let &i = self.index.get(key)?;
        self.entries[i].as_ref().map(|(_, v)| v)
    }

    pub(crate) fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        let &i = self.index.get(key)?;
        self.entries[i].as_mut().map(|(_, v)| v)
    }

    #[cfg(feature = "extension")]
    pub(crate) fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }

    /// `map.set(key, value)`.
    pub(crate) fn set(&mut self, key: &str, value: V) {
        if let Some(&i) = self.index.get(key) {
            self.entries[i].as_mut().expect("indexed").1 = value;
            return;
        }
        self.index.insert(key.to_owned(), self.entries.len());
        self.entries.push(Some((key.to_owned(), value)));
    }

    /// `map.delete(key)`.
    pub(crate) fn delete(&mut self, key: &str) -> Option<V> {
        let i = self.index.remove(key)?;
        self.entries[i].take().map(|(_, v)| v)
    }

    /// The entry at position `i` of the iteration order, skipping deleted
    /// entries; `None` past the end. Positions stay valid while entries are
    /// added.
    pub(crate) fn entry_at(&self, i: &mut usize) -> Option<(&str, &V)> {
        while *i < self.entries.len() {
            if let Some((k, v)) = &self.entries[*i] {
                return Some((k, v));
            }
            *i += 1;
        }
        None
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.entries
            .iter()
            .filter_map(|e| e.as_ref().map(|(k, v)| (k.as_str(), v)))
    }

    pub(crate) fn keys(&self) -> impl Iterator<Item = &str> {
        self.iter().map(|(k, _)| k)
    }
}

/// A JS `Set` of strings.
#[derive(Clone, Debug, Default)]
pub(crate) struct OrderedSet(OrderedMap<()>);

impl OrderedSet {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn contains(&self, key: &str) -> bool {
        self.0.contains(key)
    }

    pub(crate) fn add(&mut self, key: &str) {
        self.0.set(key, ());
    }

    pub(crate) fn delete(&mut self, key: &str) -> bool {
        self.0.delete(key).is_some()
    }

    /// See [`OrderedMap::entry_at`].
    pub(crate) fn key_at(&self, i: &mut usize) -> Option<&str> {
        self.0.entry_at(i).map(|(k, _)| k)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.keys()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterates_like_a_js_set() {
        // const s = new Set(['a', 'b', 'c']); s.delete('b'); s.add('a');
        // const seen = []; for (const k of s) { seen.push(k); if (k === 'a') s.add('d'); }
        let mut s = OrderedSet::new();
        for k in ["a", "b", "c"] {
            s.add(k);
        }
        s.delete("b");
        s.add("a");
        let mut seen = Vec::new();
        let mut i = 0;
        while let Some(k) = s.key_at(&mut i).map(str::to_owned) {
            if k == "a" {
                s.add("d");
            }
            seen.push(k);
            i += 1;
        }
        assert_eq!(seen, ["a", "c", "d"]);
        // A deleted key added again goes last.
        s.delete("a");
        s.add("a");
        assert_eq!(s.iter().collect::<Vec<_>>(), ["c", "d", "a"]);
    }
}
