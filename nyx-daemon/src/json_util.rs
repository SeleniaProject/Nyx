#![forbid(unsafe_code)]
// Centralized JSON utilities with optional SIMD acceleration (feature = "simd").
// Some helpers may remain unused in certain feature sets; suppress dead_code noise.
#![allow(dead_code)]

use serde::{Serialize, de::DeserializeOwned};

#[inline]
pub fn decode_from_str<T: DeserializeOwned>(s: &str) -> Result<T, String> {
    #[cfg(feature = "simd")]
    {
        // simd-json requires &mut str for zero-copy; we clone minimally here to keep API simple.
        let mut owned = s.to_owned();
        simd_json::serde::from_str::<T>(&mut owned).map_err(|e| e.to_string())
    }
    #[cfg(not(feature = "simd"))]
    {
        serde_json::from_str::<T>(s).map_err(|e| e.to_string())
    }
}

#[inline]
pub fn decode_from_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, String> {
    #[cfg(feature = "simd")]
    {
        let mut owned = bytes.to_vec();
        simd_json::serde::from_slice::<T>(&mut owned).map_err(|e| e.to_string())
    }
    #[cfg(not(feature = "simd"))]
    {
        serde_json::from_slice::<T>(bytes).map_err(|e| e.to_string())
    }
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
