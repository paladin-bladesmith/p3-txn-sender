use solana_sdk::{signer::Signer, system_instruction, system_transaction};
use tokio::join;

use crate::suite::{SuitePorts, TestSuite, TESTER1_PUBKEY, TESTER2_PUBKEY, TESTER3_PUBKEY};

mod suite;

// Test with multiple TXs sent to the validator
// We send the same amount from tester 1 to 2, then from 2 to 3, and then from 3 back to 1.
// At the end of the test, everyone should have same amount of funds, but minus the paid fees
#[tokio::test]
async fn test_multiple_txs() {
    // Generate our test suite
    let suite = TestSuite::new_local(SuitePorts::default()).await;

    // transfer amount
    let transfer_amount = 1000;

    // Simple tranfer without CU
    let tx1 = system_transaction::transfer(
        &suite.testers[0],
        &TESTER2_PUBKEY,
        transfer_amount,
        suite.get_latest_blockhash().await,
    );

    // transfer with 10k CU price
    let transfer_ix =
        system_instruction::transfer(&suite.testers[1].pubkey(), &TESTER3_PUBKEY, transfer_amount);

    // Build TX with updated CU price
    let tx2 = suite
        .build_tx_with_cu_price(
            vec![transfer_ix],
            &[suite.testers[1].insecure_clone()],
            None,
            10_000,
        )
        .await;

    // transfer with 30k CU price
    let transfer_ix =
        system_instruction::transfer(&suite.testers[2].pubkey(), &TESTER1_PUBKEY, transfer_amount);

    // Build TX with updated CU price
    let tx3 = suite
        .build_tx_with_cu_price(
            vec![transfer_ix],
            &[suite.testers[2].insecure_clone()],
            None,
            30_000,
        )
        .await;

    // Get balances before TX
    let before_balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;
    let before_balance_tester2 = suite.get_balance(&TESTER2_PUBKEY).await;
    let before_balance_tester3 = suite.get_balance(&TESTER3_PUBKEY).await;

    // Send both TXs with small delay between them
    let results = suite
        .p3_client
        .send_multiple_transactions(&[tx1, tx2, tx3])
        .await;
    let sig1 = results[0].clone();
    let sig2 = results[1].clone();
    let sig3 = results[2].clone();

    // Confirm both TXs
    let ((fee1, _), (fee2, _), (fee3, _)) = join!(
        suite.get_transaction(sig1),
        suite.get_transaction(sig2),
        suite.get_transaction(sig3)
    );

    // Updated balances
    let balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;
    let balance_tester2 = suite.get_balance(&TESTER2_PUBKEY).await;
    let balance_tester3 = suite.get_balance(&TESTER3_PUBKEY).await;

    // Assert balances are correct
    assert_eq!(before_balance_tester1 - fee1, balance_tester1);
    assert_eq!(before_balance_tester2 - fee2, balance_tester2);
    assert_eq!(before_balance_tester3 - fee3, balance_tester3);
}
