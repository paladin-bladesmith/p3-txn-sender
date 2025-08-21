use std::time::Duration;

use solana_sdk::{signer::Signer, system_instruction};
use tokio::{join, time::sleep};

use crate::suite::{
    test_suite::{TESTER4_PUBKEY, TESTER5_PUBKEY},
    SuitePorts, TestSuite, TESTER1_PUBKEY, TESTER2_PUBKEY, TESTER3_PUBKEY,
};

mod suite;

// We are trying to test ordering of TXs sent to different sources.

// 1. TX with 1m CU to P3 port (highest rewards)
// 2. Bundle with 600k tips (sent to 2nd p3-standalone)
// 3. TX with 300k CU sent to mev port
// 4. Bundle with 100k tips (sent to p3-standalone)
// 5. TX with no cu or tips
//
// The sent order is 5, 4, ,3 ,2 ,1.
#[tokio::test]
async fn test_multiple_txs() {
    // Generate our test suite
    let suite = TestSuite::new_local(SuitePorts::default())
        .await
        .with_tips()
        .await;
    let suite2 = TestSuite::new_local(SuitePorts::standalone())
        .await
        .with_tips()
        .await;
    let suite3 = TestSuite::new_local(SuitePorts::standalone2())
        .await
        .with_tips()
        .await;
    let transfer_amount = 1000;

    // transfer with no cu or tip
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

    // transfer with 100k tips
    let transfer_ix =
        system_instruction::transfer(&suite.testers[1].pubkey(), &TESTER1_PUBKEY, transfer_amount);

    // Build TX with updated tips
    let tip_amount1 = 100_000;
    let tx2 = suite
        .build_tx_with_tip(
            vec![transfer_ix],
            &[suite.testers[1].insecure_clone()],
            None,
            tip_amount1,
            0,
        )
        .await;

    // transfer with 300k CU
    let transfer_ix =
        system_instruction::transfer(&suite.testers[2].pubkey(), &TESTER1_PUBKEY, transfer_amount);

    // Build TX
    let cu_price1 = 300_000;
    let tx3 = suite
        .build_tx_with_cu_price(
            vec![transfer_ix],
            &[suite.testers[2].insecure_clone()],
            None,
            cu_price1,
        )
        .await;

    let tip_amount2 = 600_000;
    let transfer_ix =
        system_instruction::transfer(&suite.testers[3].pubkey(), &TESTER1_PUBKEY, transfer_amount);
    let tx4 = suite
        .build_tx_with_tip(
            vec![transfer_ix],
            &[suite.testers[3].insecure_clone()],
            None,
            tip_amount2,
            1,
        )
        .await;

    // transfer with 1m CU
    let transfer_ix =
        system_instruction::transfer(&suite.testers[4].pubkey(), &TESTER1_PUBKEY, transfer_amount);

    // Build TX
    let cu_price2 = 1_000_000;
    let tx5 = suite
        .build_tx_with_cu_price(
            vec![transfer_ix],
            &[suite.testers[4].insecure_clone()],
            None,
            cu_price2,
        )
        .await;

    // Get balances before TX
    let before_balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;
    let before_balance_tester2 = suite.get_balance(&TESTER2_PUBKEY).await;
    let before_balance_tester3 = suite.get_balance(&TESTER3_PUBKEY).await;
    let before_balance_tester4 = suite.get_balance(&TESTER4_PUBKEY).await;
    let before_balance_tester5 = suite.get_balance(&TESTER5_PUBKEY).await;

    // Sleep for a second
    sleep(Duration::from_millis(2000)).await;

    // Order of sending doesn't matter, as long as they are sent close to each other
    // to be processed in the same slot
    let t1 = suite.p3_client.send_transaction(tx1, 1);
    let t2 = suite2.mev_client.send_transaction(tx2, 2);
    let t3 = suite.mev_client.send_transaction(tx3, 3);
    let t4 = suite3.mev_client.send_transaction(tx4, 4);
    let t5 = suite.p3_client.send_transaction(tx5, 5);
    let (sig1, sig2, sig3, sig4, sig5) = join!(t1, t2, t3, t4, t5);

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
    assert_eq!(
        before_balance_tester1 - result1.fee + transfer_amount * 4,
        balance_tester1
    );
    assert_eq!(
        before_balance_tester2 - result2.fee - tip_amount1 - transfer_amount,
        balance_tester2
    );
    assert_eq!(
        before_balance_tester3 - result3.fee - transfer_amount,
        balance_tester3
    );
    assert_eq!(
        before_balance_tester4 - result4.fee - tip_amount2 - transfer_amount,
        balance_tester4
    );
    assert_eq!(
        before_balance_tester5 - result5.fee - transfer_amount,
        balance_tester5
    );

    // Expected order of results
    let expected = vec![vec![sig4], vec![sig2], vec![sig5], vec![sig3], vec![sig1]];

    // Assert order is as expected
    suite.assert_txs_order(result1.slot, expected).await;
}
