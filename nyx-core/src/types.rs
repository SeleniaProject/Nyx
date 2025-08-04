/// 256-bit node identifier (blake3(pubkey) prefix).
pub type NodeId = [u8; 32]; 

/// Path identifier for multipath data plane (0-255)
pub type PathId = u8;

/// Connection identifier for tracking streams
pub type ConnectionId = u64;

/// Maximum number of concurrent paths in multipath mode
pub const MAX_PATHS: usize = 8;

/// Minimum number of hops in dynamic hop routing
pub const MIN_HOPS: u8 = 3;

/// Maximum number of hops in dynamic hop routing  
pub const MAX_HOPS: u8 = 7;

/// Default number of hops (fallback)
pub const DEFAULT_HOPS: u8 = 5; 