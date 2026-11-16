//! `internal/positionalBlockId.ts`: positional block ids, the ids
//! `BlockIdExtension` gives the blocks that a document stored without ids
//! leaves unnamed. They are derived from the document and the block's place
//! in it, so every reader and every replica of the same document derives the
//! same ones. The derivation is part of the stored format: changing anything
//! here changes the ids of every document that is read without ids.

use crate::json::{Object, Value};
use crate::serialized_export::{export_node_json, with_compact_export};
use crate::slot::{get_slot, get_slot_names};
use crate::{NodeRef, get_root};

/// `ID_ALPHABET`: the 64 characters of ids, in base64url order (RFC 4648,
/// section 5): one character per 6 bits.
pub(crate) const ID_ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// `ID_LENGTH`: characters in a generated or positional id: 126 bits.
pub(crate) const ID_LENGTH: usize = 21;

/// SHA-256 (FIPS 180-4) round constants: the first 32 bits of the
/// fractional parts of the cube roots of the first 64 primes.
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Runs the compression function over one 64-byte block.
fn compress(h: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for (t, word) in w.iter_mut().take(16).enumerate() {
        let i = t * 4;
        *word = u32::from_be_bytes([block[i], block[i + 1], block[i + 2], block[i + 3]]);
    }
    for t in 16..64 {
        let x = w[t - 15];
        let y = w[t - 2];
        let s0 = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
        let s1 = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
        w[t] = w[t - 16]
            .wrapping_add(s0)
            .wrapping_add(w[t - 7])
            .wrapping_add(s1);
    }
    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = *h;
    for t in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let t1 = hh
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[t])
            .wrapping_add(w[t]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let t2 = s0.wrapping_add(maj);
        hh = g;
        g = f;
        f = e;
        e = d.wrapping_add(t1);
        d = c;
        c = b;
        b = a;
        a = t1.wrapping_add(t2);
    }
    for (word, value) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
        *word = word.wrapping_add(value);
    }
}

/// The SHA-256 state words after hashing `message`.
fn sha256_words(message: &[u8]) -> [u32; 8] {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let full = message.len() - message.len() % 64;
    for block in message[..full].as_chunks::<64>().0 {
        compress(&mut h, block);
    }
    // The rest of the message, a 1 bit, zeros, and the 64-bit length.
    let rest = &message[full..];
    let mut buffer = [0u8; 128];
    let end = if rest.len() < 56 { 64 } else { 128 };
    buffer[..rest.len()].copy_from_slice(rest);
    buffer[rest.len()] = 0x80;
    let bits = (message.len() as u64).wrapping_mul(8);
    buffer[end - 8..end].copy_from_slice(&bits.to_be_bytes());
    for block in buffer[..end].as_chunks::<64>().0 {
        compress(&mut h, block);
    }
    h
}

/// `sha256(message)`: the SHA-256 digest of `message`.
pub(crate) fn sha256(message: &[u8]) -> [u8; 32] {
    let h = sha256_words(message);
    let mut digest = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        digest[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

/// A deep copy of `value` whose objects have their keys inserted sorted, in
/// UTF-16 code unit order (`Object.keys(record).sort()`).
fn sort_keys(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(sort_keys).collect()),
        Value::Object(record) => {
            let mut keys: Vec<&str> = record.keys().collect();
            keys.sort_by(|a, b| crate::utf16::cmp(a, b));
            let mut sorted = Object::new();
            for key in keys {
                sorted.insert(
                    key,
                    sort_keys(record.get(key).expect("a key of the record")),
                );
            }
            Value::Object(sorted)
        }
        _ => value.clone(),
    }
}

/// `canonicalJSON(value)`: `JSON.stringify` (no whitespace) of a deep copy
/// of `value` in which the keys of every object were inserted in UTF-16 code
/// unit order. An object then writes the keys that are array indices first,
/// in numeric order, then the others in that order.
pub(crate) fn canonical_json(value: &Value) -> String {
    sort_keys(value).to_string()
}

/// `$nodeJSON(node)`: a node's JSON with its children and slots, without its
/// block id (and without a `$` that held only that).
fn node_json(node: &NodeRef) -> Object {
    let mut json = export_node_json(&*node.get_latest());
    if let Some(Value::Object(state)) = json.get("$")
        && state.contains_key("blockId")
    {
        let mut rest = state.clone();
        rest.remove("blockId");
        if rest.is_empty() {
            json.remove("$");
        } else {
            json.insert("$", Value::Object(rest));
        }
    }
    if node.is_element() {
        let children = node
            .get_children()
            .iter()
            .map(|child| Value::Object(node_json(child)))
            .collect();
        json.insert("children", Value::Array(children));
    }
    let names = get_slot_names(node);
    if !names.is_empty() {
        let mut slots = Object::new();
        for name in &names {
            if let Some(slot) = get_slot(node, name) {
                slots.insert(name.as_str(), Value::Object(node_json(&slot)));
            }
        }
        json.insert("$slots", Value::Object(slots));
    }
    json
}

/// `$canonicalDocument()`: the active document as the canonical JSON its
/// key is hashed from: the compact export of the root, without any
/// `blockId`, in [`canonical_json`].
pub(crate) fn canonical_document() -> String {
    with_compact_export(true, || {
        canonical_json(&Value::Object(node_json(&get_root())))
    })
}

/// `$documentKey()`: the key of the active document: SHA-256 of the UTF-8
/// bytes of [`canonical_document`]. Ids are left out, so naming blocks does
/// not change it.
pub(crate) fn document_key() -> [u8; 32] {
    sha256(canonical_document().as_bytes())
}

/// An id from the SHA-256 of `input`: the first 21 characters (126 bits)
/// of the digest's unpadded base64url encoding.
fn id_from_digest_of(input: &[u8]) -> String {
    let h = sha256_words(input);
    let mut id = String::with_capacity(ID_LENGTH);
    let mut bit = 0usize;
    while id.len() < ID_LENGTH {
        let word = bit >> 5;
        let offset = (bit & 31) as u32;
        let pair = if offset <= 26 {
            h[word] >> (26 - offset)
        } else {
            (h[word] << (offset - 26)) | (h[word + 1] >> (58 - offset))
        };
        id.push(ID_ALPHABET[(pair & 63) as usize] as char);
        bit += 6;
    }
    id
}

/// `derivePositionalId(documentKey, place)`: the positional id of the block
/// at `place` in the document with key `document_key`: the first 21
/// characters of the unpadded base64url encoding of
/// SHA-256(`document_key` ‖ UTF-8(`place`)). `place` is the block's path
/// among identified blocks, its indices joined by `.` (`"3"`, `"0.2"`), with
/// `#n` appended for the n-th retry after a collision.
pub(crate) fn derive_positional_id(document_key: &[u8; 32], place: &str) -> String {
    let mut input = document_key.to_vec();
    input.extend_from_slice(place.as_bytes());
    id_from_digest_of(&input)
}

/// `deriveDuplicateId(id, ordinal)`: the id of the k-th later holder of the
/// repeated id `id`: the first 21 characters of the unpadded base64url
/// encoding of SHA-256(UTF-8(`"dup"` + U+0000 + `id` + U+0000 + `ordinal`)).
pub(crate) fn derive_duplicate_id(id: &str, ordinal: &str) -> String {
    id_from_digest_of(format!("dup\u{0}{id}\u{0}{ordinal}").as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// FIPS 180-4's examples, and messages around the padding boundaries.
    #[test]
    fn sha256_matches_known_digests() {
        assert_eq!(
            hex(&sha256(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha256(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        assert_eq!(
            hex(&sha256(&[b'a'; 64])),
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"
        );
    }
}
