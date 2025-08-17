#![forbid(unsafe_code)]

/// 双方向の符号化冗長率(送信/受信)を持つ簡易アダプタ
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Redundancy { pub tx: f32, pub rx: f32 }

impl Redundancy {
	pub fn clamp(self) -> Self {
		let c = |v: f32| v.clamp(0.0, 0.9);
		Redundancy { tx: c(self.tx), rx: c(self.rx) }
	}
}

/// RTT(ms)と観測損失率(loss:0..1)から簡易に冗長率を調整するスタブ。
pub fn adaptive_raptorq_redundancy(rtt_ms: u32, loss: f32, prev: Redundancy) -> Redundancy {
	let mut tx = prev.tx;
	let mut rx = prev.rx;
	// 損失が大きいほど増やす
	let bump = (loss * 0.5).clamp(0.0, 0.3);
	tx += bump;
	rx += bump * 0.8;
	// RTTが大きいと復元のため受信側重視
	if rtt_ms > 100 { rx += 0.05; }
	Redundancy { tx, rx }.clamp()
}
