use solana_sdk::{signer::Signer, system_instruction, system_transaction};

use crate::suite::{
    test_suite::{TESTER4_PUBKEY, TESTER5_PUBKEY},
    SuitePorts, TestSuite, TESTER1_PUBKEY, TESTER2_PUBKEY, TESTER3_PUBKEY,
};

mod suite;

// Simple tests where we send 1 TX to a port and make sure it works
// Can be run using `cargo test --test simple -- --nocapture --exact simple_p3`

#[tokio::test]
async fn setup() {
    // Generate our test suite
    let suite = TestSuite::new_local(SuitePorts::default()).await;
}

/// Send simple transfer TX to regular p3 port
#[tokio::test]
async fn simple_p3() {
    // Generate our test suite
    let suite = TestSuite::new_local(SuitePorts::default()).await;

    // Simple transfer TX
    let transfer_amount = 1000;
    let tx = system_transaction::transfer(
        &suite.testers[0],
        &TESTER1_PUBKEY,
        transfer_amount,
        suite.get_latest_blockhash().await,
    );

    // Get balances before TX
    let before_balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;

    // Set and confirm TX
    let sig = suite.p3_client.send_transaction(tx, 1).await;
    let result = suite.get_transaction(&sig).await;

    // Updated balances
    let balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;

    // Assert balances are correct
    assert_eq!(
        before_balance_tester1 - result.fee,
        balance_tester1
    );
}

/// Simple TX with tips sent to mev port
#[tokio::test]
async fn simple_mev() {
    // Generate our test suite
    let suite = TestSuite::new_local(SuitePorts::default())
        .await
        .with_tips()
        .await;

    // Simple transfer TX
    let transfer_amount = 1000;
    let ix =
        system_instruction::transfer(&suite.testers[1].pubkey(), &TESTER2_PUBKEY, transfer_amount);

    let tip_amount = 100_000;
    let tx = suite
        .build_tx_with_tip(
            vec![ix],
            &[suite.testers[1].insecure_clone()],
            None,
            tip_amount,
            0,
        )
        .await;

    // Get balances before TX
    let before_balance_tester1 = suite.get_balance(&TESTER2_PUBKEY).await;

    // Set and confirm TX
    let sig = suite.mev_client.send_transaction(tx, 1).await;
    let result = suite.get_transaction(&sig).await;

    // Updated balances
    let balance_tester1 = suite.get_balance(&TESTER2_PUBKEY).await;

    // Assert balances are correct
    assert_eq!(
        before_balance_tester1 - result.fee - tip_amount,
        balance_tester1
    );
}

/// Simple TX to p3 port with updated CU
#[tokio::test]
async fn simple_with_cu() {
    // Generate our test suite
    let suite = TestSuite::new_local(SuitePorts::default()).await;

    // Simple transfer IX
    let transfer_amount = 1000;
    let transfer_ix =
        system_instruction::transfer(&suite.testers[2].pubkey(), &TESTER3_PUBKEY, transfer_amount);

    // Build TX with updated CU price
    let cu_price = 100_000;
    let tx = suite
        .build_tx_with_cu_price(
            vec![transfer_ix],
            &[suite.testers[2].insecure_clone()],
            None,
            cu_price,
        )
        .await;

    // Get balances before TX
    let before_balance_tester1 = suite.get_balance(&TESTER3_PUBKEY).await;

    // Set and confirm TX
    let sig = suite.p3_client.send_transaction(tx, 1).await;
    let result = suite.get_transaction(&sig).await;

    // Updated balances
    let balance_tester1 = suite.get_balance(&TESTER3_PUBKEY).await;

    // Assert balances are correct
    assert_eq!(
        before_balance_tester1 - result.fee,
        balance_tester1
    );
}

/// Confirm the Standalone BE is working
#[tokio::test]
async fn simple_standalone() {
    // Generate our test suite
    let suite = TestSuite::new_local(SuitePorts::standalone()).await;

    // Simple transfer TX
    let transfer_amount = 1000;
    let tx = system_transaction::transfer(
        &suite.testers[3],
        &TESTER4_PUBKEY,
        transfer_amount,
        suite.get_latest_blockhash().await,
    );

    // Get balances before TX
    let before_balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;

    // Set and confirm TX
    let sig = suite.mev_client.send_transaction(tx, 1).await;
    let result = suite.get_transaction(&sig).await;

    // Updated balances
    let balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;

    // Assert balances are correct
    assert_eq!(
        before_balance_tester1 - result.fee,
        balance_tester1
    );
}
