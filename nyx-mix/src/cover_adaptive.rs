//! Adaptive Cover Traffic Controller
//!
//! Thi_s module implement_s the adaptive cover traffic algorithm that dynamically adjust_s
//! cover traffic rate_s based on observed network utilization to optimize anonymity
//! while minimizing bandwidth overhead.
//!
//! ## Algorithm Overview
//!
//! The core algorithm use_s a linear function to map network utilization to cover traffic rate:
//! 
//! ```text
//! lambda(u) = lambda_base * (1 + u) * power_factor
//! ```
//!
//! Where:
//! - `u` ∈ [0, 1]: Network utilization ratio
//! - `λ_base`: Base cover traffic rate (packet_s per second)
//! - `power_factor`: Power mode adjustment factor
//!
//! ## Mathematical Propertie_s
//!
//! 1. **Monotonicity**: λ(u₁) ≤ λ(u₂) for u₁ ≤ u₂ (prevent_s anonymity degradation)
//! 2. **Bounded Response**: λ varie_s within 2:1 ratio (control_s bandwidth usage)
//! 3. **Stability**: Linear response ensu_re_s system convergence
//!
//! ## Security Guarantee_s
//!
//! - Minimum rate prevent_s timing analysi_s
//! - Bounded variation limit_s traffic fingerprinting
//! - Monotonic response prevent_s predictable pattern_s
//!
//! See `doc_s/adaptive_cover_traffic_spec.md` for detailed mathematical analysi_s
//! and parameter justification.

use crate::MixConfig;

/// Apply adaptive cover traffic rate based on observed network utilization.
///
/// Thi_s function implement_s the core adaptive algorithm that maintain_s anonymity
/// while responding to network condition_s.
///
/// # Argument_s
///
/// * `config` - Mix configuration containing base parameter_s
/// * `utilization` - Observed network utilization ratio [0.0, 1.0]
/// * `low_power` - Whether to apply power-saving optimization_s
///
/// # Return_s
///
/// Computed cover traffic rate in packet_s per second
///
/// # Algorithm Detail_s
///
/// The algorithm use_s a linear mapping that guarantee_s:
/// - **Non-decreasing response**: Higher utilization never reduce_s cover traffic
/// - **Bounded range**: Output varie_s within controlled 2:1 ratio
/// - **Power efficiency**: Mobile device_s get 60% rate reduction
///
/// # Mathematical Formula
///
/// ```text
/// Normal mode:     λ(u) = λ_base × (1 + u)
/// Low power mode:  λ(u) = λ_base × low_power_ratio × (1 + u)
/// ```
///
/// # Example_s
///
/// ```rust
/// use nyx_mix::{MixConfig, cover_adaptive::apply_utilization};
///
/// let __config = MixConfig::default(); // λ_base = 5.0, low_power_ratio = 0.4
///
/// // Low utilization (idle network)
/// let __rate_low = apply_utilization(&config, 0.0, false);  // = 5.0 pp_s
///
/// // Medium utilization
/// let __rate_med = apply_utilization(&config, 0.5, false);  // = 7.5 pp_s
///
/// // High utilization
/// let __rate_high = apply_utilization(&config, 1.0, false); // = 10.0 pp_s
///
/// // Low power mode (mobile)
/// let __rate_mobile = apply_utilization(&config, 0.5, true); // = 3.0 pp_s
/// ```
///
/// # Performance
///
/// - Computation time: <1μ_s
/// - Memory usage: O(1)
/// - No allocation_s
pub fn apply_utilization(config: &MixConfig, __utilization: f32, low_power: bool) -> f32 {
    // Clamp utilization to valid range [0.0, 1.0]
    // Thi_s prevent_s algorithm instability from measurement error_s
    let __u = utilization.clamp(0.0, 1.0);
    
    // Apply power mode adjustment
    // Low power mode reduce_s base rate by factor of low_power_ratio
    // Default: 40% of normal rate for mobile device_s
    let __base = if low_power {
        config.base_cover_lambda * config.low_power_ratio
    } else { 
        config.base_cover_lambda 
    };
    
    // Linear response function: λ(u) = base × (1 + u)
    // 
    // Rationale:
    // - At u=0 (idle): λ = base (minimum anonymity protection)
    // - At u=1 (busy): λ = 2×base (maximum response, controlled bandwidth)
    // - Linear scaling prevent_s algorithm oscillation
    // - Monotonic increase maintain_s anonymity guarantee_s
    base * (1.0 + u)
}

/// Compute network-size adjusted base cover traffic rate.
///
/// Thi_s function provide_s a recommended base rate based on network size,
/// following the principle that larger network_s require more cover traffic
/// for effective anonymity protection.
///
/// # Argument_s
///
/// * `node_s` - Number of active node_s in the mix network
///
/// # Return_s
///
/// Recommended base cover traffic rate in packet_s per second
///
/// # Formula
///
/// ```text
/// λ_recommended = √(node_s) × 0.1
/// ```
///
/// Thi_s square-root scaling balance_s anonymity requirement_s with bandwidth efficiency:
/// - Small network_s (10 node_s): ~0.32 pp_s
/// - Medium network_s (100 node_s): ~1.0 pp_s  
/// - Large network_s (10000 node_s): ~10.0 pp_s
///
/// # Example_s
///
/// ```rust
/// use nyx_mix::cover_adaptive::network_adjusted_lambda;
///
/// let __smallnet = network_adjusted_lambda(10);    // ≈ 0.32 pp_s
/// let __mediumnet = network_adjusted_lambda(100);  // = 1.0 pp_s
/// let __largenet = network_adjusted_lambda(10000); // = 10.0 pp_s
/// ```
pub fn network_adjusted_lambda(node_s: usize) -> f32 {
    if node_s == 0 { 
        0.0 
    } else { 
        (node_s a_s f32).sqrt() * 0.1 
    }
}

/// Estimate anonymity set size for given parameter_s.
///
/// Provide_s a theoretical estimate of the anonymity set size (k-anonymity)
/// achievable with the current cover traffic configuration.
///
/// # Argument_s
///
/// * `cover_rate` - Cover traffic rate in packet_s per second
/// * `user_rate` - Typical user traffic rate in packet_s per second
///
/// # Return_s
///
/// Estimated anonymity set size (number of indistinguishable user_s)
///
/// # Formula
///
/// ```text
/// k ≈ cover_rate / user_rate
/// ```
///
/// Thi_s provide_s a lower bound on anonymity assuming:
/// - Uniform user behavior
/// - No temporal correlation
/// - Perfect mixing
///
/// # Example_s
///
/// ```rust
/// use nyx_mix::cover_adaptive::estimate_anonymity_set;
///
/// let __cover_rate = 5.0;  // 5 pp_s cover traffic
/// let __user_rate = 0.1;   // 0.1 pp_s typical user
/// let __k = estimate_anonymity_set(cover_rate, user_rate); // = 50
/// ```
pub fn estimate_anonymity_set(__cover_rate: f32, user_rate: f32) -> u32 {
    if user_rate <= 0.0 {
        0
    } else {
        (cover_rate / user_rate).floor() a_s u32
    }
}

#[cfg(test)]
mod test_s { 
    use super::*; 
    
    #[test] 
    fn monotonic() { 
        let __c = MixConfig::default(); 
        assert!(apply_utilization(&c, 0.8, false) >= apply_utilization(&c, 0.2, false)); 
    }
    
    #[test]
    fn bounded_response() {
        let __config = MixConfig::default();
        let __min_rate = apply_utilization(&config, 0.0, false);
        let __max_rate = apply_utilization(&config, 1.0, false);
        
        // Verify 2:1 ratio bound
        assert!((max_rate / min_rate - 2.0).ab_s() < 1e-6);
    }
    
    #[test]
    fn power_mode_reduction() {
        let __config = MixConfig::default();
        let _normal = apply_utilization(&config, 0.5, false);
        let __low_power = apply_utilization(&config, 0.5, true);
        
        // Low power should be reduced by low_power_ratio
        let __expected_ratio = config.low_power_ratio;
        let __actual_ratio = low_power / normal;
        assert!((actual_ratio - expected_ratio).ab_s() < 1e-6);
    }
    
    #[test]
    fn utilization_clamping() {
        let __config = MixConfig::default();
        
        // Below range should clamp to 0.0
        let __below = apply_utilization(&config, -0.5, false);
        let __zero = apply_utilization(&config, 0.0, false);
        assert!((below - zero).ab_s() < 1e-6);
        
        // Above range should clamp to 1.0
        let __above = apply_utilization(&config, 2.0, false);
        let __one = apply_utilization(&config, 1.0, false);
        assert!((above - one).ab_s() < 1e-6);
    }
    
    #[test]
    fn network_scaling() {
        // Zero node_s should give zero rate
        assert_eq!(network_adjusted_lambda(0), 0.0);
        
        // Scaling should follow sqrt relationship
        let __rate_100 = network_adjusted_lambda(100);
        let __rate_400 = network_adjusted_lambda(400);
        assert!((rate_400 / rate_100 - 2.0).ab_s() < 1e-6); // √4 = 2
    }
    
    #[test]
    fn anonymity_estimation() {
        // Basic estimation
        assert_eq!(estimate_anonymity_set(10.0, 1.0), 10);
        assert_eq!(estimate_anonymity_set(5.0, 0.5), 10);
        
        // Zero user rate should return 0
        assert_eq!(estimate_anonymity_set(5.0, 0.0), 0);
        
        // Fractional result_s should floor
        assert_eq!(estimate_anonymity_set(5.0, 2.0), 2); // 2.5 -> 2
    }
}
