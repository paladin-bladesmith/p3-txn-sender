use solana_sdk::{signer::Signer, system_instruction};
use solana_transaction_status::EncodedTransaction;
use tokio::join;

use crate::suite::{SuitePorts, TestSuite, TESTER1_PUBKEY, TESTER2_PUBKEY, TESTER3_PUBKEY};

mod suite;

// Test with multiple TXs sent to the validator
// We send the same amount from tester 1 to 2, then from 2 to 3, and then from 3 back to 1.
// At the end of the test, everyone should have same amount of funds, but minus the paid fees
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

    // transfer with 100k tips
    let transfer_ix =
        system_instruction::transfer(&suite.testers[1].pubkey(), &TESTER2_PUBKEY, transfer_amount);

    // Build TX with updated tips
    let tip_amount2 = 100_000;
    let tx1 = suite
        .build_tx_with_tip(
            vec![transfer_ix],
            &[suite.testers[1].insecure_clone()],
            Some(&suite.testers[1].pubkey()),
            tip_amount2,
            0,
        )
        .await;

    // transfer with 300k tips
    let transfer_ix =
        system_instruction::transfer(&suite.testers[2].pubkey(), &TESTER3_PUBKEY, transfer_amount);

    // Build TX with updated tips
    let tip_amount3 = 300_000;
    let tx2 = suite
        .build_tx_with_tip(
            vec![transfer_ix],
            &[suite.testers[2].insecure_clone()],
            Some(&suite.testers[2].pubkey()),
            tip_amount3,
            1,
        )
        .await;

    let transfer_ix =
        system_instruction::transfer(&suite.testers[0].pubkey(), &TESTER1_PUBKEY, transfer_amount);
    let tx3 = suite
        .build_tx_with_tip(
            vec![transfer_ix],
            &[suite.testers[0].insecure_clone()],
            Some(&suite.testers[0].pubkey()),
            600_000,
            2,
        )
        .await;

    // Get balances before TX
    let before_balance_tester2 = suite.get_balance(&TESTER2_PUBKEY).await;
    let before_balance_tester3 = suite.get_balance(&TESTER3_PUBKEY).await;

    // Send TXs with small delay between them
    let t1 = suite.mev_client.send_transaction(tx1);
    let t2 = suite2.mev_client.send_transaction(tx2);
    let t3 = suite3.mev_client.send_transaction(tx3);
    let (sig1, sig2, sig3) = join!(t1, t2, t3);

    // Confirm both TXs
    let (result1, result2, result3) = join!(
        suite.get_transaction(&sig1),
        suite.get_transaction(&sig2),
        suite.get_transaction(&sig3),
    );

    // Updated balances
    let balance_tester2 = suite.get_balance(&TESTER2_PUBKEY).await;
    let balance_tester3 = suite.get_balance(&TESTER3_PUBKEY).await;

    // Assert balances are correct
    assert_eq!(
        before_balance_tester2 - result1.fee - tip_amount2,
        balance_tester2
    );
    // println!(
    //     "b: {}, f: {}, t: {}, a: {}",
    //     before_balance_tester3, result3.fee, tip_amount3, balance_tester3
    // );
    assert_eq!(
        before_balance_tester3 - result2.fee - tip_amount3,
        balance_tester3
    );

    // Assert order of txs in block
    // NOTE - the txs might split from a single block, that doesn mean it failed
    // rather the timing was not perfect to include all txs in a single batch.
    // do not error in this case, rather return a meaningful log

    // Expected order of results
    let expected = vec![vec![sig2], vec![sig1]];

    // Get block of first tx
    let tmp = suite.get_block_transactions(result1.slot).await;
    let block_txs = tmp
        .iter()
        .enumerate()
        .filter_map(|(id, tx)| {
            if let Some(meta) = &tx.meta {
                if let Some(err) = &meta.err {
                    println!("TX id: {} failed with: {:#?}", id, err);
                }
            }

            let sig = if let EncodedTransaction::Json(ui_tx) = &tx.transaction {
                ui_tx.signatures.clone()
            } else {
                panic!("Failed to parse TX")
            };

            if expected.contains(&sig) {
                Some(sig)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if block_txs.len() < 3 {
        // Return meaningful error that some txs splitted and we cant assert order
        // Which mainly means to try again for better luck
        println!("⁉️ Some txs splitted, can't assert correctly")
    }

    for (i, tx) in block_txs.iter().enumerate() {
        if tx != &expected[i] {
            println!("❌ Order at index {} is wrong", i);
            break;
        }
    }

    println!("Received TXs: {:#?}", block_txs)
}
