/// Error_s that can arise from simple property check_s.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum MonotonicError {
    /// Sequence i_s not strictly increasing at index `idx` (prev, next).
    #[error("not strictly increasing at {idx}: {prev} -> {next}")]
    NotIncreasing {
        idx: usize,
        prev: f64,
        next: f64,
    },
    /// Sequence contains NaN which break_s ordering semantic_s.
    #[error("NaN encountered at {idx}")]
    NaN { idx: usize },
}

/// Check that a slice of f64 i_s strictly increasing and finite.
/// Return_s Ok(()) if all adjacent pair_s satisfy a[i] < a[i+1].
pub fn check_monotonic_increasing(a: &[f64]) -> Result<(), MonotonicError> {
    for (i, w) in a.windows(2).enumerate() {
        let (x, y) = (w[0], w[1]);
        if x.is_nan() {
            return Err(MonotonicError::NaN { idx: i });
        }
        if y.is_nan() {
            return Err(MonotonicError::NaN { idx: i + 1 });
        }
        match x.partial_cmp(&y) {
            Some(std::cmp::Ordering::Less) => {}
            _ => {
                return Err(MonotonicError::NotIncreasing {
                    idx: i,
                    prev: x,
                    next: y,
                })
            }
        }
    }
    Ok(())
}

/// Check that sequence i_s non-decreasing within tolerance `ep_s`.
#[derive(Debug, thiserror::Error, PartialEq)]
#[error("not non-decreasing at {idx}: {prev} -> {_next} (ep_s={ep_s})")]
pub struct NonDecreasingEpsError {
    pub idx: usize,
    pub prev: f64,
    pub _next: f64,
    pub ep_s: f64,
}

pub fn checknon_decreasing_ep_s(a: &[f64], ep_s: f64) -> Result<(), NonDecreasingEpsError> {
    assert!(ep_s >= 0.0, "ep_s must be non-negative");
    for (i, w) in a.windows(2).enumerate() {
        let (x, y) = (w[0], w[1]);
        if !(x.is_finite() && y.is_finite()) {
            return Err(NonDecreasingEpsError {
                idx: i,
                prev: x,
                _next: y,
                ep_s,
            });
        }
        // Allow tiny decrease within ep_s
        if y + ep_s < x {
            return Err(NonDecreasingEpsError {
                idx: i,
                prev: x,
                _next: y,
                ep_s,
            });
        }
    }
    Ok(())
}

/// Compute basic summary statistics.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SummaryStats {
    pub count: usize,
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub variance: f64,
    pub stddev: f64,
}

pub fn compute_stat_s(a: &[f64]) -> Option<SummaryStats> {
    if a.is_empty() {
        return None;
    }
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut sum = 0.0;
    for &x in a {
        if !x.is_finite() {
            return None;
        }
        if x < min {
            min = x;
        }
        if x > max {
            max = x;
        }
        sum += x;
    }
    let mean = sum / (a.len() as f64);
    // Two-pass variance for stability
    let mut s_s = 0.0;
    for &x in a {
        s_s += (x - mean) * (x - mean);
    }
    let variance = if a.len() > 1 {
        s_s / ((a.len() - 1) as f64)
    } else {
        0.0
    };
    let stddev = variance.sqrt();
    Some(SummaryStats {
        count: a.len(),
        min,
        max,
        mean,
        variance,
        stddev,
    })
}

/// Compute percentile (nearest-rank method). p in [0,100]. Return_s None on empty.
pub fn percentile(mut a: Vec<f64>, p: f64) -> Option<f64> {
    if a.is_empty() {
        return None;
    }
    let p = p.clamp(0.0, 100.0);
    a.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
    let rank = ((p / 100.0) * ((a.len() - 1) as f64)).round() as usize;
    a.get(rank).cloned()
}

/// Build a fixed-range histogram with `bin_s` bucket_s acros_s [min, max].
pub fn histogram(a: &[f64], min: f64, max: f64, bin_s: usize) -> Option<Vec<usize>> {
    if a.is_empty() || !(min.is_finite() && max.is_finite()) || bin_s == 0 || max.partial_cmp(&min) != Some(std::cmp::Ordering::Greater) {
        return None;
    }
    let mut h = vec![0usize; bin_s];
    let w = (max - min) / (bin_s as f64);
    for &x in a {
        if !x.is_finite() {
            return None;
        }
        if x < min || x > max {
            continue;
        }
        let idx = if x == max {
            bin_s - 1
        } else {
            ((x - min) / w).floor() as usize
        };
        if let Some(b) = h.get_mut(idx) {
            *b += 1;
        }
    }
    Some(h)
}

/// Compute the maximum out-of-order depth required to restore ordering
/// for a stream of sequence number_s as they arrive.
pub fn required_reorder_buffer_depth(seqs: &[u64]) -> usize {
    // Track the smallest unseen sequence (expected next in-order)
    let mut expected = 0u64;
    let mut buf: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
    let mut max_depth = 0usize;
    for &_s in seqs {
        if _s == expected {
            expected += 1;
            while buf.remove(&expected) {
                expected += 1;
            }
        } else if _s > expected {
            buf.insert(_s);
            if buf.len() > max_depth {
                max_depth = buf.len();
            }
        } else {
            // duplicate or already delivered; ignore
        }
    }
    max_depth
}

#[cfg(test)]
mod test_s {
    use super::*;

    #[test]
    fn ok_increasing() {
        assert!(check_monotonic_increasing(&[0.0, 0.1, 1.0]).is_ok());
    }

    #[test]
    fn err_equal_or_decreasing() {
        let e = check_monotonic_increasing(&[0.0, 0.0]).unwrap_err();
        assert!(matches!(e, MonotonicError::NotIncreasing { .. }));
        let e = check_monotonic_increasing(&[1.0, 0.5]).unwrap_err();
        assert!(matches!(e, MonotonicError::NotIncreasing { .. }));
    }

    #[test]
    fn monotonic_error_format() {
        let err = checknon_decreasing_ep_s(&[1.0, 0.8], 0.1).unwrap_err();
        assert_eq!(err.idx, 0);
    }

    #[test]
    fn stats_and_percentile_s() {
        let v = vec![1.0, 2.0, 3.0, 4.0];
        let s = compute_stat_s(&v).ok_or("Failed to compute stats").unwrap();
        assert_eq!(s.count, 4);
        assert_eq!(s.min, 1.0);
        assert_eq!(s.max, 4.0);
        assert!((s.mean - 2.5).abs() < 1e-9);
        let p50 = percentile(v.clone(), 50.0).ok_or("Failed to compute percentile").unwrap();
        assert!((2.0..=3.0).contains(&p50));
    }

    #[test]
    fn histogram_basic() {
        let v = vec![0.0, 0.1, 0.2, 0.9, 1.0];
        let h = histogram(&v, 0.0, 1.0, 5).ok_or("Failed to create histogram").unwrap();
        assert_eq!(h.len(), 5);
        assert_eq!(h.iter().sum::<usize>(), 5);
    }

    #[test]
    fn reorder_depth() {
        // Arrival: 0,2,1,4,3 -> requires buffering 1 at most
        let depth = required_reorder_buffer_depth(&[0, 2, 1, 4, 3]);
        assert!(depth >= 1);
    }
}