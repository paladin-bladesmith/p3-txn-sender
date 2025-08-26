use crate::suite::{SuitePorts, TestSuite};

mod suite;

/// Run this first to setup the test ledger and fund accounts
#[tokio::test]
async fn setup() {
    // Generate our test suite
    TestSuite::new_local(SuitePorts::default())
        .await
        .with_tips()
        .await;
}
