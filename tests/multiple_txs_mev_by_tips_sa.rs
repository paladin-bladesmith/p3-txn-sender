use solana_sdk::signer::Signer;
use solana_system_interface::instruction;
use tokio::join;

use crate::suite::{SuitePorts, TestSuite, TESTER1_PUBKEY, TESTER2_PUBKEY, TESTER3_PUBKEY};

mod suite;

// Txs that are sent to the MEVport on p3-standalone are added as FIFO into a single bundle
// and sent to block engine stage, sothe sent order, should be exepcted order
#[tokio::test]
async fn test_multiple_txs() {
    // Generate our test suite
    let suite = TestSuite::new_local(SuitePorts::standalone())
        .await
        .with_tips()
        .await;

    // transfer amount
    let transfer_amount = 1000;

    // Simple tranfer without tips
    let transfer_ix =
        instruction::transfer(&suite.testers[0].pubkey(), &TESTER1_PUBKEY, transfer_amount);
    let tx1 = suite
        .build_tx(
            vec![transfer_ix],
            &[suite.testers[0].insecure_clone()],
            None,
        )
        .await;

    // transfer with 200k tips
    let transfer_ix =
        instruction::transfer(&suite.testers[1].pubkey(), &TESTER1_PUBKEY, transfer_amount);

    // Build TX with updated tips
    let tip_amount2 = 200_000;
    let tx2 = suite
        .build_tx_with_tip(
            vec![transfer_ix],
            &[suite.testers[1].insecure_clone()],
            None,
            tip_amount2,
            0,
        )
        .await;

    // transfer with 400k tips
    let transfer_ix =
        instruction::transfer(&suite.testers[2].pubkey(), &TESTER1_PUBKEY, transfer_amount);

    // Build TX with updated tips
    let tip_amount3 = 400_000;
    let tx3 = suite
        .build_tx_with_tip(
            vec![transfer_ix],
            &[suite.testers[2].insecure_clone()],
            None,
            tip_amount3,
            1,
        )
        .await;

    // Get balances before TX
    let before_balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;
    let before_balance_tester2 = suite.get_balance(&TESTER2_PUBKEY).await;
    let before_balance_tester3 = suite.get_balance(&TESTER3_PUBKEY).await;

    // Send TXs together
    let sig1 = suite.mev_client.send_transaction(tx1, 1).await;
    let sig2 = suite.mev_client.send_transaction(tx2, 2).await;
    let sig3 = suite.mev_client.send_transaction(tx3, 3).await;

    // Confirm both TXs
    let (result1, result2, result3) = join!(
        suite.get_transaction(&sig1),
        suite.get_transaction(&sig2),
        suite.get_transaction(&sig3),
    );

    // Updated balances
    let balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;
    let balance_tester2 = suite.get_balance(&TESTER2_PUBKEY).await;
    let balance_tester3 = suite.get_balance(&TESTER3_PUBKEY).await;

    // Assert balances are correct
    assert_eq!(
        before_balance_tester1 - result1.fee + transfer_amount * 2,
        balance_tester1
    );
    assert_eq!(
        before_balance_tester2 - result2.fee - tip_amount2 - transfer_amount,
        balance_tester2
    );
    assert_eq!(
        before_balance_tester3 - result3.fee - tip_amount3 - transfer_amount,
        balance_tester3
    );

    // Currently we can't have expected order because we batch all TXs into a single bundle
}
