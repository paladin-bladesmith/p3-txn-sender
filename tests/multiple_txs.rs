use solana_sdk::{signer::Signer, system_instruction, system_transaction};
use tokio::join;

use crate::suite::{SuitePorts, TestSuite, TESTER1_PUBKEY, TESTER2_PUBKEY, TESTER3_PUBKEY};

mod suite;

// Simple test where we do a simple transfer from tester 1 to tester 2
#[tokio::test]
async fn test_multiple_txs() {
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

    // Another transfer
    let tx2 = system_transaction::transfer(
        &suite.testers[1],
        &TESTER3_PUBKEY,
        transfer_amount,
        suite.get_latest_blockhash().await,
    );

    // Get balances before TX
    let before_balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;
    let before_balance_tester2 = suite.get_balance(&TESTER2_PUBKEY).await;
    let before_balance_tester3 = suite.get_balance(&TESTER3_PUBKEY).await;

    // Send both TXs
    let (sig1, sig2) = join!(
        suite.p3_client.send_transaction(tx),
        suite.p3_client.send_transaction(tx2)
    );

    // Confirm both TXs
    let ((fee1, _), (fee2, _)) = join!(suite.get_transaction(sig1), suite.get_transaction(sig2));

    // Updated balances
    let balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;
    let balance_tester2 = suite.get_balance(&TESTER2_PUBKEY).await;
    let balance_tester3 = suite.get_balance(&TESTER3_PUBKEY).await;

    // Assert balances are correct
    assert_eq!(
        before_balance_tester1 - fee1 - transfer_amount,
        balance_tester1
    );
    assert_eq!(before_balance_tester2 - fee2, balance_tester2);
    assert_eq!(before_balance_tester3 + transfer_amount, balance_tester3);
}
