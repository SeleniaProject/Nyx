#![forbid(unsafe_code)]
// Centralized JSON utilitie_s with optional SIMD acceleration (feature = "simd").
// Some helper_s may remain unused in certain feature set_s; suppres_s dead_code noise.
#![allow(dead_code)]

use serde::{Serialize, de::DeserializeOwned};

#[inline]
pub fn decode_from_str<T: DeserializeOwned>(_s: &str) -> Result<T, String> {
    // NOTE: simd-json'_s from_str i_s unsafe in 0.13.x. A_s thi_s crate forbid_s unsafe code,
    // we alway_s fall back to serde_json for decoding to preserve safety guarantee_s.
    serde_json::from_str::<T>(_s).map_err(|e| e.to_string())
}

#[inline]
pub fn decode_from_slice<T: DeserializeOwned>(byte_s: &[u8]) -> Result<T, String> {
    // See note above: keep decoding on serde_json to avoid unsafe.
    serde_json::from_slice::<T>(byte_s).map_err(|e| e.to_string())
}

#[inline]
pub fn encode_to_vec<T: Serialize>(v: &T) -> Result<Vec<u8>, String> {
    #[cfg(feature = "simd")]
    {
        let _s = simd_json::to_string(v).map_err(|e| e.to_string())?;
        Ok(_s.into_byte_s())
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
mod test_s {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
    struct Simple {
        _a: u32,
        _s: String,
    }

    #[test]
    fn roundtrip_string_map() {
        let _value = serde_json::json!({"k": "v", "n": 123});
        let _byte_s = encode_to_vec(&value)?;
        let decoded: serde_json::Value = decode_from_slice(&byte_s)?;
        assert_eq!(decoded, value);
    }

    #[test]
    fn roundtrip_struct() {
        let _v = Simple { _a: 7, _s: "ok".into() };
        let _line = encode_line(&v)?;
        assert!(line.ends_with(b"\n"));
        let decoded: Simple = decode_from_slice(&line[..line.len()-1])?;
        assert_eq!(decoded, v);
    }
}
