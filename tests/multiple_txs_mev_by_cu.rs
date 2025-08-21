use solana_sdk::{signer::Signer, system_instruction};
use tokio::join;

use crate::suite::{
    test_suite::{TESTER4_PUBKEY, TESTER5_PUBKEY},
    SuitePorts, TestSuite, TESTER1_PUBKEY, TESTER2_PUBKEY, TESTER3_PUBKEY,
};

mod suite;

// Test with multiple TXs sent to the validator
// We send the same amount from tester 1 to 2, then from 2 to 3, and then from 3 back to 1.
// At the end of the test, everyone should have same amount of funds, but minus the paid fees
// All TXS are sent to the mev port with updated CU prices
#[tokio::test]
async fn test_multiple_txs() {
    // Generate our test suite
    let suite = TestSuite::new_local(SuitePorts::default()).await;

    // transfer amount
    let transfer_amount = 1000;

    // Simple tranfer without CU
    let transfer_ix =
        system_instruction::transfer(&suite.testers[0].pubkey(), &TESTER1_PUBKEY, transfer_amount);
    let tx1 = suite
        .build_tx(
            vec![transfer_ix],
            &[suite.testers[0].insecure_clone()],
            None,
        )
        .await;

    // transfer with 100k CU price
    let transfer_ix =
        system_instruction::transfer(&suite.testers[1].pubkey(), &TESTER1_PUBKEY, transfer_amount);

    // Build TX with updated CU price
    let tx2 = suite
        .build_tx_with_cu_price(
            vec![transfer_ix],
            &[suite.testers[1].insecure_clone()],
            None,
            100_000,
        )
        .await;

    // transfer with 300k CU price
    let transfer_ix =
        system_instruction::transfer(&suite.testers[2].pubkey(), &TESTER1_PUBKEY, transfer_amount);

    // Build TX with updated CU price
    let tx3 = suite
        .build_tx_with_cu_price(
            vec![transfer_ix],
            &[suite.testers[2].insecure_clone()],
            None,
            300_000,
        )
        .await;

    // transfer with 300k CU price
    let transfer_ix =
        system_instruction::transfer(&suite.testers[3].pubkey(), &TESTER1_PUBKEY, transfer_amount);

    // Build TX with updated CU price
    let tx4 = suite
        .build_tx_with_cu_price(
            vec![transfer_ix],
            &[suite.testers[3].insecure_clone()],
            None,
            600_000,
        )
        .await;

    // transfer with 300k CU price
    let transfer_ix =
        system_instruction::transfer(&suite.testers[4].pubkey(), &TESTER1_PUBKEY, transfer_amount);

    // Build TX with updated CU price
    let tx5 = suite
        .build_tx_with_cu_price(
            vec![transfer_ix],
            &[suite.testers[4].insecure_clone()],
            None,
            1_000_000,
        )
        .await;

    // Get balances before TX
    let before_balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;
    let before_balance_tester2 = suite.get_balance(&TESTER2_PUBKEY).await;
    let before_balance_tester3 = suite.get_balance(&TESTER3_PUBKEY).await;
    let before_balance_tester4 = suite.get_balance(&TESTER4_PUBKEY).await;
    let before_balance_tester5 = suite.get_balance(&TESTER5_PUBKEY).await;

    // Send TXs with small delay between them
    let results = suite
        .mev_client
        .send_multiple_transactions(&[tx1, tx2, tx3, tx4, tx5])
        .await;
    let sig1 = results[0].clone();
    let sig2 = results[1].clone();
    let sig3 = results[2].clone();
    let sig4 = results[3].clone();
    let sig5 = results[4].clone();

    // Confirm both TXs
    let (result1, result2, result3, result4, result5) = join!(
        suite.get_transaction(&sig1),
        suite.get_transaction(&sig2),
        suite.get_transaction(&sig3),
        suite.get_transaction(&sig4),
        suite.get_transaction(&sig5),
    );

    // Updated balances
    let balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;
    let balance_tester2 = suite.get_balance(&TESTER2_PUBKEY).await;
    let balance_tester3 = suite.get_balance(&TESTER3_PUBKEY).await;
    let balance_tester4 = suite.get_balance(&TESTER4_PUBKEY).await;
    let balance_tester5 = suite.get_balance(&TESTER5_PUBKEY).await;

    // Assert balances are correct
    assert_eq!(before_balance_tester1 - result1.fee + transfer_amount * 4, balance_tester1);
    assert_eq!(before_balance_tester2 - result2.fee - transfer_amount, balance_tester2);
    assert_eq!(before_balance_tester3 - result3.fee - transfer_amount, balance_tester3);
    assert_eq!(before_balance_tester4 - result4.fee - transfer_amount, balance_tester4);
    assert_eq!(before_balance_tester5 - result5.fee - transfer_amount, balance_tester5);

    // Expected order of results
    let expected = vec![vec![sig5], vec![sig4], vec![sig3], vec![sig2], vec![sig1]];

    // Assert order is as expected
    suite.assert_txs_order(result1.slot, expected).await;
}
