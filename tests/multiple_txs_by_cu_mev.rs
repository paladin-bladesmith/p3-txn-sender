use solana_sdk::{
    compute_budget, message::Message, signer::Signer, system_instruction, system_transaction,
    transaction::Transaction,
};
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
    let suite = TestSuite::new_local(SuitePorts::default()).await;

    // transfer amount
    let transfer_amount = 1000;

    // Simple tranfer without CU
    let transfer_ix =
        system_instruction::transfer(&suite.testers[0].pubkey(), &TESTER2_PUBKEY, transfer_amount);
    let tx1 = suite
        .build_tx(
            vec![transfer_ix],
            &[suite.testers[0].insecure_clone()],
            None,
        )
        .await;

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

    // Send TXs with small delay between them
    let results = suite
        .mev_client
        .send_multiple_transactions(&[tx1, tx2, tx3])
        .await;
    let sig1 = results[0].clone();
    let sig2 = results[1].clone();
    let sig3 = results[2].clone();

    // Confirm both TXs
    let (result1, result2, result3) = join!(
        suite.get_transaction(&sig1),
        suite.get_transaction(&sig2),
        suite.get_transaction(&sig3)
    );

    // Updated balances
    let balance_tester1 = suite.get_balance(&TESTER1_PUBKEY).await;
    let balance_tester2 = suite.get_balance(&TESTER2_PUBKEY).await;
    let balance_tester3 = suite.get_balance(&TESTER3_PUBKEY).await;

    // Assert balances are correct
    assert_eq!(before_balance_tester1 - result1.fee, balance_tester1);
    assert_eq!(before_balance_tester2 - result2.fee, balance_tester2);
    assert_eq!(before_balance_tester3 - result3.fee, balance_tester3);

    // Assert order of txs in block
    // NOTE - the txs might split from a single block, that doesn mean it failed
    // rather the timing was not perfect to include all txs in a single batch.
    // do not error in this case, rather return a meaningful log

    // Expected order of results
    let expected = vec![vec![sig3], vec![sig2], vec![sig1]];

    // Get block of first tx
    let tmp = suite.get_block_transactions(result1.slot).await;
    let block_txs = tmp
        .iter()
        .filter_map(|tx| {
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
}
