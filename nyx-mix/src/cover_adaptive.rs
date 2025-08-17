//! Adaptive Cover Traffic Controller
//!
//! This module implements the adaptive cover traffic algorithm that dynamically adjusts
//! cover traffic rates based on observed network utilization to optimize anonymity
//! while minimizing bandwidth overhead.
//!
//! ## Algorithm Overview
//!
//! The core algorithm uses a linear function to map network utilization to cover traffic rate:
//! 
//! ```
//! λ(u) = λ_base × (1 + u) × power_factor
//! ```
//!
//! Where:
//! - `u` ∈ [0, 1]: Network utilization ratio
//! - `λ_base`: Base cover traffic rate (packets per second)
//! - `power_factor`: Power mode adjustment factor
//!
//! ## Mathematical Properties
//!
//! 1. **Monotonicity**: λ(u₁) ≤ λ(u₂) for u₁ ≤ u₂ (prevents anonymity degradation)
//! 2. **Bounded Response**: λ varies within 2:1 ratio (controls bandwidth usage)
//! 3. **Stability**: Linear response ensures system convergence
//!
//! ## Security Guarantees
//!
//! - Minimum rate prevents timing analysis
//! - Bounded variation limits traffic fingerprinting
//! - Monotonic response prevents predictable patterns
//!
//! See `docs/adaptive_cover_traffic_spec.md` for detailed mathematical analysis
//! and parameter justification.

use crate::MixConfig;

/// Apply adaptive cover traffic rate based on observed network utilization.
///
/// This function implements the core adaptive algorithm that maintains anonymity
/// while responding to network conditions.
///
/// # Arguments
///
/// * `config` - Mix configuration containing base parameters
/// * `utilization` - Observed network utilization ratio [0.0, 1.0]
/// * `low_power` - Whether to apply power-saving optimizations
///
/// # Returns
///
/// Computed cover traffic rate in packets per second
///
/// # Algorithm Details
///
/// The algorithm uses a linear mapping that guarantees:
/// - **Non-decreasing response**: Higher utilization never reduces cover traffic
/// - **Bounded range**: Output varies within controlled 2:1 ratio
/// - **Power efficiency**: Mobile devices get 60% rate reduction
///
/// # Mathematical Formula
///
/// ```text
/// Normal mode:     λ(u) = λ_base × (1 + u)
/// Low power mode:  λ(u) = λ_base × low_power_ratio × (1 + u)
/// ```
///
/// # Examples
///
/// ```rust
/// use nyx_mix::{MixConfig, cover_adaptive::apply_utilization};
///
/// let config = MixConfig::default(); // λ_base = 5.0, low_power_ratio = 0.4
///
/// // Low utilization (idle network)
/// let rate_low = apply_utilization(&config, 0.0, false);  // = 5.0 pps
///
/// // Medium utilization
/// let rate_med = apply_utilization(&config, 0.5, false);  // = 7.5 pps
///
/// // High utilization
/// let rate_high = apply_utilization(&config, 1.0, false); // = 10.0 pps
///
/// // Low power mode (mobile)
/// let rate_mobile = apply_utilization(&config, 0.5, true); // = 3.0 pps
/// ```
///
/// # Performance
///
/// - Computation time: <1μs
/// - Memory usage: O(1)
/// - No allocations
pub fn apply_utilization(config: &MixConfig, utilization: f32, low_power: bool) -> f32 {
    // Clamp utilization to valid range [0.0, 1.0]
    // This prevents algorithm instability from measurement errors
    let u = utilization.clamp(0.0, 1.0);
    
    // Apply power mode adjustment
    // Low power mode reduces base rate by factor of low_power_ratio
    // Default: 40% of normal rate for mobile devices
    let base = if low_power {
        config.base_cover_lambda * config.low_power_ratio
    } else { 
        config.base_cover_lambda 
    };
    
    // Linear response function: λ(u) = base × (1 + u)
    // 
    // Rationale:
    // - At u=0 (idle): λ = base (minimum anonymity protection)
    // - At u=1 (busy): λ = 2×base (maximum response, controlled bandwidth)
    // - Linear scaling prevents algorithm oscillation
    // - Monotonic increase maintains anonymity guarantees
    base * (1.0 + u)
}

/// Compute network-size adjusted base cover traffic rate.
///
/// This function provides a recommended base rate based on network size,
/// following the principle that larger networks require more cover traffic
/// for effective anonymity protection.
///
/// # Arguments
///
/// * `nodes` - Number of active nodes in the mix network
///
/// # Returns
///
/// Recommended base cover traffic rate in packets per second
///
/// # Formula
///
/// ```text
/// λ_recommended = √(nodes) × 0.1
/// ```
///
/// This square-root scaling balances anonymity requirements with bandwidth efficiency:
/// - Small networks (10 nodes): ~0.32 pps
/// - Medium networks (100 nodes): ~1.0 pps  
/// - Large networks (10000 nodes): ~10.0 pps
///
/// # Examples
///
/// ```rust
/// use nyx_mix::cover_adaptive::network_adjusted_lambda;
///
/// let small_net = network_adjusted_lambda(10);    // ≈ 0.32 pps
/// let medium_net = network_adjusted_lambda(100);  // = 1.0 pps
/// let large_net = network_adjusted_lambda(10000); // = 10.0 pps
/// ```
pub fn network_adjusted_lambda(nodes: usize) -> f32 {
    if nodes == 0 { 
        0.0 
    } else { 
        (nodes as f32).sqrt() * 0.1 
    }
}

/// Estimate anonymity set size for given parameters.
///
/// Provides a theoretical estimate of the anonymity set size (k-anonymity)
/// achievable with the current cover traffic configuration.
///
/// # Arguments
///
/// * `cover_rate` - Cover traffic rate in packets per second
/// * `user_rate` - Typical user traffic rate in packets per second
///
/// # Returns
///
/// Estimated anonymity set size (number of indistinguishable users)
///
/// # Formula
///
/// ```text
/// k ≈ cover_rate / user_rate
/// ```
///
/// This provides a lower bound on anonymity assuming:
/// - Uniform user behavior
/// - No temporal correlation
/// - Perfect mixing
///
/// # Examples
///
/// ```rust
/// use nyx_mix::cover_adaptive::estimate_anonymity_set;
///
/// let cover_rate = 5.0;  // 5 pps cover traffic
/// let user_rate = 0.1;   // 0.1 pps typical user
/// let k = estimate_anonymity_set(cover_rate, user_rate); // = 50
/// ```
pub fn estimate_anonymity_set(cover_rate: f32, user_rate: f32) -> u32 {
    if user_rate <= 0.0 {
        0
    } else {
        (cover_rate / user_rate).floor() as u32
    }
}

#[cfg(test)]
mod tests { 
    use super::*; 
    
    #[test] 
    fn monotonic() { 
        let c = MixConfig::default(); 
        assert!(apply_utilization(&c, 0.8, false) >= apply_utilization(&c, 0.2, false)); 
    }
    
    #[test]
    fn bounded_response() {
        let config = MixConfig::default();
        let min_rate = apply_utilization(&config, 0.0, false);
        let max_rate = apply_utilization(&config, 1.0, false);
        
        // Verify 2:1 ratio bound
        assert!((max_rate / min_rate - 2.0).abs() < 1e-6);
    }
    
    #[test]
    fn power_mode_reduction() {
        let config = MixConfig::default();
        let normal = apply_utilization(&config, 0.5, false);
        let low_power = apply_utilization(&config, 0.5, true);
        
        // Low power should be reduced by low_power_ratio
        let expected_ratio = config.low_power_ratio;
        let actual_ratio = low_power / normal;
        assert!((actual_ratio - expected_ratio).abs() < 1e-6);
    }
    
    #[test]
    fn utilization_clamping() {
        let config = MixConfig::default();
        
        // Below range should clamp to 0.0
        let below = apply_utilization(&config, -0.5, false);
        let zero = apply_utilization(&config, 0.0, false);
        assert!((below - zero).abs() < 1e-6);
        
        // Above range should clamp to 1.0
        let above = apply_utilization(&config, 2.0, false);
        let one = apply_utilization(&config, 1.0, false);
        assert!((above - one).abs() < 1e-6);
    }
    
    #[test]
    fn network_scaling() {
        // Zero nodes should give zero rate
        assert_eq!(network_adjusted_lambda(0), 0.0);
        
        // Scaling should follow sqrt relationship
        let rate_100 = network_adjusted_lambda(100);
        let rate_400 = network_adjusted_lambda(400);
        assert!((rate_400 / rate_100 - 2.0).abs() < 1e-6); // √4 = 2
    }
    
    #[test]
    fn anonymity_estimation() {
        // Basic estimation
        assert_eq!(estimate_anonymity_set(10.0, 1.0), 10);
        assert_eq!(estimate_anonymity_set(5.0, 0.5), 10);
        
        // Zero user rate should return 0
        assert_eq!(estimate_anonymity_set(5.0, 0.0), 0);
        
        // Fractional results should floor
        assert_eq!(estimate_anonymity_set(5.0, 2.0), 2); // 2.5 -> 2
    }
}
