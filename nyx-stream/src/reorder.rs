//! Packet Reordering Buffer.
//! Maintains in-order delivery for Multipath reception.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

/// ReorderBuffer collects packets keyed by monotonically increasing sequence numbers
/// and releases them in-order.
///
/// • Auto‐scale up: when the observed gap (highest – expected) nears the current
///   window size the buffer doubles up to 8192 slots. これは経路 RTT 差で発生する
///   リオーダ幅に追従する。
/// • Auto‐shrink: 当面のリオーダが減ったら 1/4 未満で半減。
pub struct ReorderBuffer<T> {
    next_seq: u64,
    initial_seq: u64,
    window: BTreeMap<u64, T>,
    max_window: usize,
    // drain_ready で一括返却せず pop_front でも取り出せるようバッファ
    pending: Vec<T>,
    delivered_any: bool,
    rebased: bool,
}

impl<T> ReorderBuffer<T> {
    /// Create a new buffer starting at `initial_seq`.
    pub fn new(initial_seq: u64) -> Self {
    Self { next_seq: initial_seq, initial_seq, window: BTreeMap::new(), max_window: 32, pending: Vec::new(), delivered_any: false, rebased: false }
    }

    /// Push packet with `seq`. Returns a vector of in-order packets now ready.
    pub fn push(&mut self, seq: u64, pkt: T) -> Vec<T> where T: Clone {
        if seq < self.next_seq {
            // 初回配達前で initial より小さい値が来た => リベースモードへ
            if !self.delivered_any && seq < self.initial_seq {
                self.rebased = true;
                self.window.insert(seq, pkt);
                if seq < self.next_seq { self.next_seq = seq; }
            } else if !self.delivered_any {
                self.window.insert(seq, pkt);
                if seq < self.next_seq { self.next_seq = seq; }
            } else {
                // 既に delivery 後の過去パケットは破棄
                return Vec::new();
            }
        } else {
            self.window.insert(seq, pkt);
        }

        // Observe current gap between next expected and highest received.
        if let Some((&high, _)) = self.window.iter().rev().next() {
            let gap = (high - self.next_seq + 1) as usize;
            // Up-scale: if gap approaches 80% of current window, double with cap 8192.
            if gap * 5 / 4 > self.max_window && self.max_window < 8192 {
                self.max_window = (self.max_window * 2).min(8192);
            }
        }
        if self.rebased {
            // リベース後は自動排出せず (pop_front で取得)
            Vec::new()
        } else {
            let before = self.pending.len();
            self.fill_pending();
            self.pending[before..].iter().cloned().collect()
        }
    }
    fn fill_pending(&mut self) {
        let mut progressed = false;
        while let Some(pkt) = self.window.remove(&self.next_seq) {
            self.pending.push(pkt);
            self.next_seq += 1;
            progressed = true;
        }
        if progressed { self.delivered_any = true; }
        if self.window.len() < self.max_window / 4 && self.max_window > 32 {
            self.max_window /= 2;
        }
        while self.window.len() > self.max_window {
            if let Some((&last, _)) = self.window.iter().rev().next() {
                self.window.remove(&last);
            } else { break; }
        }
    }

    /// Pop a single in-order packet if available.
    pub fn pop_front(&mut self) -> Option<T> {
        // まず pending が空なら進展を収集
    if self.pending.is_empty() { self.fill_pending(); }
        if self.pending.is_empty() { return None; }
        Some(self.pending.remove(0))
    }
}

impl<T> ReorderBuffer<T> {
    pub fn len(&self) -> usize { self.pending.len() + self.window.len() }
    pub fn is_empty(&self) -> bool { self.len() == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_order_delivery() {
        let mut buf = ReorderBuffer::new(0);
        // Push out-of-order: 1,0,2
        assert!(buf.push(1, 1).is_empty());
        let r1 = buf.push(0, 0);
        assert_eq!(r1, vec![0,1]);
        let r2 = buf.push(2, 2);
        assert_eq!(r2, vec![2]);
    }
} 