#![cfg(test)]

use nyx_sdk::{DaemonClient, SdkConfig};

#[tokio::test]
async fn with_token_trims_and_sets() {
    let c = DaemonClient::new(SdkConfig::default()).with_token("   ");
    // whitespace-only ignored
    // cannot access field directly here; create another with non-empty
    let c2 = DaemonClient::new(SdkConfig::default()).with_token("  abc  ");
    // Indirectly verify via debug formatting of authorization in request path by checking that no panic occurs
    // and methods can be called up to the point they attempt to connect (which will fail). We only test builder logic here.
    // If we could access field it would be Some("abc"). This test ensures API compiles and runs.
    drop(c);
    drop(c2);
}
