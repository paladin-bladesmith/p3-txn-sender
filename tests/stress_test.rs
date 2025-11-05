use std::time::Duration;

use solana_sdk::signer::Signer;
use solana_system_interface::instruction;
use tokio::{join, time::sleep};

use crate::suite::{
    test_suite::{TESTER4_PUBKEY, TESTER5_PUBKEY},
    SuitePorts, TestSuite, TESTER1_PUBKEY
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

    let validator_bal = suite
        .rpc_client
        .get_balance(&suite.validator_keypair.pubkey())
        .await
        .unwrap();

    let mut txs = vec![];
    for x in 0..1000 {
        // transfer with no cu or tip
        let transfer_ix = instruction::transfer(
            &TESTER1_PUBKEY,
            &suite.testers[(x % 4) + 1].pubkey(),
            transfer_amount + x as u64,
        );

        // Build TX
        let tx = suite
            .build_tx(
                vec![transfer_ix],
                &[suite.testers[0].insecure_clone()],
                None,
            )
            .await;

        txs.push(tx);
    }

    // Sleep for a second
    sleep(Duration::from_millis(2000)).await;

    let res = suite.p3_client.send_multiple_transactions(&txs).await;

    suite.get_transaction(&res[0]).await;
    println!("Tx: {:?}", res[0]);
}
