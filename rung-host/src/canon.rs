//! Canonical serialization: every byte the host makes is deterministic.
//!
//! - object keys are sorted (recursively);
//! - numbers use one formatting: integers exactly, floats as the shortest
//!   round-trip form (`serde_json`'s), after [`fixed`] where a value is the
//!   host's own arithmetic;
//! - no whitespace;
//! - no timestamps or ids are added here; a caller that puts one in a value
//!   chose to.
//!
//! The same value gives the same bytes in every process (gate G-l pins it
//! across two runs).

use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// Canonical bytes of `v`.
pub fn bytes(v: &Value) -> Vec<u8> {
    let mut out = Vec::with_capacity(128);
    write(v, &mut out);
    out
}

/// Canonical text of `v`.
pub fn string(v: &Value) -> String {
    // `write` only emits UTF-8 (JSON text).
    String::from_utf8(bytes(v)).unwrap_or_default()
}

/// Canonical bytes of anything serializable.
pub fn of<T: Serialize + ?Sized>(t: &T) -> Vec<u8> {
    bytes(&serde_json::to_value(t).unwrap_or(Value::Null))
}

fn write(v: &Value, out: &mut Vec<u8>) {
    match v {
        Value::Object(map) => {
            out.push(b'{');
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_str(k, out);
                out.push(b':');
                write(&map[k], out);
            }
            out.push(b'}');
        }
        Value::Array(items) => {
            out.push(b'[');
            for (i, x) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write(x, out);
            }
            out.push(b']');
        }
        Value::String(s) => write_str(s, out),
        // Null, bool and numbers: serde_json's text is already canonical.
        other => out.extend_from_slice(other.to_string().as_bytes()),
    }
}

fn write_str(s: &str, out: &mut Vec<u8>) {
    out.extend_from_slice(Value::String(s.to_string()).to_string().as_bytes());
}

/// `x` rounded to six decimals: the host's fixed float form.
pub fn fixed(x: f64) -> f64 {
    if !x.is_finite() {
        return 0.0;
    }
    (x * 1e6).round() / 1e6
}

/// Lowercase hex SHA-256 of `b`, first 16 bytes (32 hex chars).
pub fn hash(b: &[u8]) -> String {
    let d = Sha256::digest(b);
    d.iter().take(16).map(|x| format!("{x:02x}")).collect()
}

/// [`hash`] of the canonical bytes of `v`.
pub fn hash_value(v: &Value) -> String {
    hash(&bytes(v))
}

/// The host's token estimate for `n` bytes: one token per four bytes,
/// rounded up. A deterministic stand-in for a tokenizer; the provider's
/// own count is what `llm.call` records.
pub fn tokens(n: usize) -> usize {
    n.div_ceil(4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keys_are_sorted_at_every_depth() {
        let a = json!({"b": 1, "a": {"z": [1, {"y": 2, "x": 3}], "c": null}});
        assert_eq!(
            string(&a),
            r#"{"a":{"c":null,"z":[1,{"x":3,"y":2}]},"b":1}"#
        );
    }

    #[test]
    fn floats_have_one_form() {
        assert_eq!(string(&json!(fixed(0.1 + 0.2))), "0.3");
        assert_eq!(string(&json!(1.5e-7)), "1.5e-7");
        assert_eq!(fixed(f64::NAN), 0.0);
    }

    #[test]
    fn strings_are_escaped() {
        assert_eq!(string(&json!("a\"b\n")), r#""a\"b\n""#);
    }
}
