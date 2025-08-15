use nyx_mix::{
    cmix::{verify_batch, CmixController},
    vdf,
};
use tokio::time::Duration;

/// @spec 4. cMix Integration
/// Verifies that a batch is emitted and the proof validates under expected parameters.
#[tokio::test]
async fn cmix_batch_verification() {
    let mut controller = CmixController::new(5, 20); // small batch & short delay for test
    let tx = controller.sender();
    // inject dummy packets
    for i in 0u8..3 {
        tx.send(vec![i]).await.unwrap();
    }
    // receive batch within 100ms
    let batch = tokio::time::timeout(Duration::from_millis(200), controller.recv())
        .await
        .expect("batch timeout")
        .expect("controller closed");
    assert!(verify_batch(&batch, controller.params(), None));
}
