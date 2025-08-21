use std::time::Duration;

use solana_sdk::{signer::Signer, system_instruction};
use tokio::{join, time::sleep};

use crate::suite::{SuitePorts, TestSuite, TESTER1_PUBKEY, TESTER2_PUBKEY, TESTER3_PUBKEY};

mod suite;

// We are testing multiple BEs, so we send a single TX to each BE (3) with its own tip
// Exepcting 3 bundles to be included, ordered by the tip amount
#[tokio::test]
async fn test_multiple_txs() {
    // Generate our test suite
    let suite = TestSuite::new_local(SuitePorts::standalone())
        .await
        .with_tips()
        .await;
    let suite2 = TestSuite::new_local(SuitePorts::standalone2())
        .await
        .with_tips()
        .await;
    let suite3 = TestSuite::new_local(SuitePorts::standalone3())
        .await
        .with_tips()
        .await;
    let transfer_amount = 1000;

    // transfer with no tip
    let transfer_ix =
        system_instruction::transfer(&suite.testers[0].pubkey(), &TESTER1_PUBKEY, transfer_amount);

    // Build TX
    let tx1 = suite
        .build_tx(
            vec![transfer_ix],
            &[suite.testers[0].insecure_clone()],
            None,
        )
        .await;

    // transfer with 300k tips
    let transfer_ix =
        system_instruction::transfer(&suite.testers[1].pubkey(), &TESTER1_PUBKEY, transfer_amount);

    // Build TX with updated tips
    let tip_amount2 = 300_000;
    let tx2 = suite
        .build_tx_with_tip(
            vec![transfer_ix],
            &[suite.testers[1].insecure_clone()],
            None,
            tip_amount2,
            0,
        )
        .await;

    let tip_amount3 = 600_000;
    let transfer_ix =
        system_instruction::transfer(&suite.testers[2].pubkey(), &TESTER1_PUBKEY, transfer_amount);
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

    // Sleep for a second
    sleep(Duration::from_millis(1000)).await;

    // Order of sending doesn't matter, as long as they are sent close to each other
    // to be processed in the same slot
    let t1 = suite.mev_client.send_transaction(tx1, 1);
    let t2 = suite2.mev_client.send_transaction(tx2, 2);
    let t3 = suite3.mev_client.send_transaction(tx3, 3);
    let (sig1, sig2, sig3) = join!(t1, t2, t3);

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

    // Expected order of results
    let expected = vec![vec![sig3], vec![sig2], vec![sig1]];

    // Assert order is as expected
    suite.assert_txs_order(result1.slot, expected).await;

    // NOTE- important check you should do is to make sure the logs of validator shows that
    // it processed the correct amount of bundles, there are 3 standalones running, so we expect 3 bundles
    // `processing 3 bundles` should appear under DEBUG logs
}
