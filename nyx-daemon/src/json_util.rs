#![forbid(unsafe_code)]
// Centralized JSON utilities with optional SIMD acceleration (feature = "simd").
// Some helpers may remain unused in certain feature sets; suppress dead_code noise.
#![allow(dead_code)]

use serde::{Serialize, de::DeserializeOwned};

#[inline]
pub fn decode_from_str<T: DeserializeOwned>(s: &str) -> Result<T, String> {
    // NOTE: simd-json's from_str is unsafe in 0.13.x. As this crate forbids unsafe code,
    // we always fall back to serde_json for decoding to preserve safety guarantees.
    serde_json::from_str::<T>(s).map_err(|e| e.to_string())
}

#[inline]
pub fn decode_from_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    // See note above: keep decoding on serde_json to avoid unsafe.
    serde_json::from_slice::<T>(bytes).map_err(|e| e.to_string())
}

#[inline]
pub fn encode_to_vec<T: Serialize>(v: &T) -> Result<Vec<u8>, String> {
    #[cfg(feature = "simd")]
    {
        let s = simd_json::to_string(v).map_err(|e| e.to_string())?;
        Ok(s.into_bytes())
    }
    #[cfg(not(feature = "simd"))]
    {
        serde_json::to_vec(v).map_err(|e| e.to_string())
    }
}

#[inline]
pub fn encode_line<T: Serialize>(v: &T) -> Result<Vec<u8>, String> {
    let mut buf = encode_to_vec(v)?;
    buf.push(b'\n');
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct Simple {
        a: u32,
        s: String,
    }

    #[test]
    fn roundtrip_string_map() {
        let value = serde_json::json!({"k": "v", "n": 123});
        let bytes = encode_to_vec(&value).expect("encode");
        let decoded: serde_json::Value = decode_from_slice(&bytes).expect("decode");
        assert_eq!(decoded, value);
    }

    #[test]
    fn roundtrip_struct() {
        let v = Simple { a: 7, s: "ok".into() };
        let line = encode_line(&v).expect("encode_line");
        assert!(line.ends_with(b"\n"));
        let decoded: Simple = decode_from_slice(&line[..line.len()-1]).expect("decode");
        assert_eq!(decoded, v);
    }
}
