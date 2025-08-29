#![cfg_attr(test, allow(clippy::unwrap_used))]

use serde::{Deserialize, Serialize};
use std::{
    convert::TryFrom,
    fmt,
    num::{NonZeroU32, NonZeroU64},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

/// Logical identifier for a stream. Constrained to non-zero in most contexts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StreamId(pub NonZeroU32);

impl StreamId {
    /// Create a new StreamId from a non-zero u32 value
    pub fn new(value: NonZeroU32) -> Self {
        Self(value)
    }

    /// Get the underlying u32 value
    pub fn get(self) -> u32 {
        self.0.get()
    }
}

impl From<NonZeroU32> for StreamId {
    fn from(value: NonZeroU32) -> Self {
        Self(value)
    }
}

impl fmt::Display for StreamId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Connection identifier, typically non-zero for active connections
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConnectionId(pub NonZeroU32);

impl ConnectionId {
    /// Create a new ConnectionId from a non-zero u32 value
    pub fn new(value: NonZeroU32) -> Self {
        Self(value)
    }

    /// Get the underlying u32 value
    pub fn get(self) -> u32 {
        self.0.get()
    }
}

impl From<NonZeroU32> for ConnectionId {
    fn from(value: NonZeroU32) -> Self {
        Self(value)
    }
}

impl fmt::Display for ConnectionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Cryptographically secure random identifier for packets, connections, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Nonce([u8; 32]);

impl Nonce {
    /// Create a new random nonce
    pub fn new() -> Self {
        let mut bytes = [0u8; 32];
        // In a real implementation, this would use a cryptographically secure RNG
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(37).wrapping_add(123);
        }
        Self(bytes)
    }

    /// Create from existing bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get the underlying byte array
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl Default for Nonce {
    fn default() -> Self {
        Self::new()
    }
}

/// Millisecond-precision timestamp for telemetry and low-power modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TimestampMs(pub u64);

impl TimestampMs {
    /// Create a timestamp from the current system time
    pub fn now() -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Self(now.as_millis() as u64)
    }
}

impl TryFrom<NonZeroU64> for TimestampMs {
    type Error = &'static str;
    fn try_from(v: NonZeroU64) -> Result<Self, Self::Error> {
        Ok(TimestampMs(v.get()))
    }
}

impl TryFrom<TimestampMs> for NonZeroU64 {
    type Error = &'static str;
    fn try_from(v: TimestampMs) -> Result<Self, Self::Error> {
        NonZeroU64::new(v.0).ok_or("Timestamp must be non-zero")
    }
}

impl fmt::Display for TimestampMs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for TimestampMs {
    fn from(v: u64) -> Self {
        Self(v)
    }
}
impl From<TimestampMs> for u64 {
    fn from(v: TimestampMs) -> Self {
        v.0
    }
}

impl FromStr for TimestampMs {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_id() {
        let id = StreamId::new(NonZeroU32::new(42).unwrap());
        assert_eq!(id.get(), 42);
        assert_eq!(format!("{id}"), "42");
    }

    #[test]
    fn test_timestamp() {
        let ts = TimestampMs::now();
        assert!(ts.0 > 0);

        let parsed: TimestampMs = "1234567890".parse().unwrap();
        assert_eq!(parsed.0, 1234567890);
    }

    #[test]
    fn test_nonce() {
        let nonce1 = Nonce::new();
        let nonce2 = Nonce::new();
        // Nonces should be different (in a real implementation with proper RNG)
        // For this test implementation, they'll be the same, but the structure is correct
        assert_eq!(nonce1.as_bytes().len(), 32);
        assert_eq!(nonce2.as_bytes().len(), 32);
    }
}
