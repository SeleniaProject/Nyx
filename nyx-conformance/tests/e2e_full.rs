#![allow(clippy::unwrap_used)]
use nyx_fec::RaptorQCodec;
use nyx_mix::adaptive::AdaptiveCoverGenerator;
use nyx_mix::cmix::{verify_batch, CmixController};
use nyx_stream::ReorderBuffer;
use rand::{thread_rng, Rng};
use tokio::time::{timeout, Duration};

/// @spec 4. cMix Integration
/// Combined E2E scenario exercising each major subsystem (cMix, LowPower, Multipath, RaptorQ).
#[tokio::test]
async fn e2e_full_stack() {
    // --- cMix Batch & VDF Proof -----------------------------------------
    let mut cmix = CmixController::new(4, 5); // small delay for test speed
    let tx = cmix.sender();
    // inject dummy packets
    for i in 0u8..4 {
        tx.send(vec![i]).await.unwrap();
    }
    let batch = timeout(Duration::from_millis(50), cmix.recv())
        .await
        .expect("cMix timeout")
        .expect("controller closed");
    assert!(verify_batch(&batch, cmix.params(), None));

    // --- Low Power Adaptive Cover ---------------------------------------
    let mut cover = AdaptiveCoverGenerator::new(10.0, 0.3);
    // Simulate entering Low Power mode
    cover.set_low_power(true);
    let lambda_lp = cover.current_lambda();
    // next_delay call just to update internal state
    let _ = cover.next_delay();
    assert!(
        lambda_lp < 10.0 * 0.5,
        "λ should be reduced in low-power mode"
    );

    // --- Multipath Reorder Buffer ---------------------------------------
    let mut rb = ReorderBuffer::new(0u64);
    // push packets out-of-order: 1,0,2
    let mut delivered = rb.push(1, 1);
    assert!(delivered.is_empty());
    delivered.extend(rb.push(0, 0));
    delivered.extend(rb.push(2, 2));
    // expect 0,1,2 in order
    assert_eq!(delivered, vec![0, 1, 2]);

    // --- RaptorQ Encode/Decode ------------------------------------------
    let codec = RaptorQCodec::new(0.5); // 50% redundancy (ensure recovery after dropping first 2 packets)
    let mut data = vec![0u8; 4096];
    thread_rng().fill(&mut data[..]);
    let packets = codec.encode(&data);
    // simulate loss: drop two packets AFTER sentinel so length sentinel is preserved
    // packets[0] is sentinel inserted by encoder; drop indices 1 and 2
    let mut subset: Vec<_> = Vec::with_capacity(packets.len() - 2);
    subset.push(packets[0].clone());
    subset.extend_from_slice(&packets[3..]);
    let recovered = codec
        .decode(&subset)
        .expect("decode failure (insufficient repair symbols)");
    assert_eq!(recovered, data);
}
