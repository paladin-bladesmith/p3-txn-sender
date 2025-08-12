use solana_sdk::{signer::Signer, system_instruction};

use crate::suite::{SuitePorts, TestSuite, TESTER1_PUBKEY, TESTER2_PUBKEY};

mod suite;

// Simple test where we do a simple transfer from tester 1 to tester 2
#[tokio::test]
async fn test_simple_with_cu() {
    // Generate our test suite
    let suite = TestSuite::new_local(SuitePorts::default()).await;

    // Simple transfer IX
    let transfer_amount = 1000;
    let transfer_ix =
        system_instruction::transfer(&suite.testers[0].pubkey(), &TESTER2_PUBKEY, transfer_amount);

    // Build TX with updated CU price
    let tx = suite
        .build_tx_with_cu_price(
            vec![transfer_ix],
            &[suite.testers[0].insecure_clone()],
            None,
            100_000,
        )
        .await;

    // Get balances before TX
    let before_balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;
    let before_balance_tester2 = suite.get_balance(&TESTER2_PUBKEY).await;

    // Set and confirm TX
    let sig = suite.p3_client.send_transaction(tx).await;
    let result = suite.get_transaction(&sig).await;

    // Updated balances
    let balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;
    let balance_tester2 = suite.get_balance(&TESTER2_PUBKEY).await;

    // Assert balances are correct
    assert_eq!(
        before_balance_tester1 - result.fee - transfer_amount,
        balance_tester1
    );
    assert_eq!(before_balance_tester2 + transfer_amount, balance_tester2);
}
