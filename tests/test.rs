mod suite;

use reqwest::Client;
use solana_sdk::{compute_budget, signature::Signer, system_instruction};
use tokio::time::{sleep, Duration};

use crate::suite::{SuitePorts, TestSuite, TESTER1_PUBKEY, TESTER2_PUBKEY};

#[tokio::test]
async fn test_send_transaction() {
    let suite = TestSuite::new_local(SuitePorts::default());

    let lamports = 1111;

    let ix1 = compute_budget::ComputeBudgetInstruction::set_compute_unit_price(20_000);
    let ix2 =
        system_instruction::transfer(&suite.validator_keypair.pubkey(), &TESTER1_PUBKEY, lamports);
    let tx1 = suite
        .build_tx(
            vec![ix1, ix2],
            &[suite.validator_keypair.insecure_clone()],
            Some(&suite.validator_keypair.pubkey()),
        )
        .await;

    // or we can do
    let ix3 = system_instruction::transfer(&TESTER2_PUBKEY, &TESTER1_PUBKEY, lamports);
    let tx2 = suite
        .build_tx_with_cu_price(
            vec![ix3],
            &[suite.testers[1].insecure_clone()],
            Some(&suite.testers[1].pubkey()),
            50_000,
        )
        .await;

    // Send to p3-txn-sender
    let response = suite.p3_client.send_transaction(tx1).await;

    let result: serde_json::Value = response.json().await.unwrap();
    println!("P3 response: {}", result);

    if let Some(success_result) = result.get("result") {
        let tx_signature = success_result.as_str().unwrap();
        println!("✅ Transaction signature: {}", tx_signature);

        // Try multiple commitment levels and wait times
        let attemps = 10;
        for attempt in 1..=attemps {
            println!("🔍 Attempt {} - Checking transaction...", attempt);

            let tx_response = Client::new()
                .post("http://localhost:8899")
                .json(&serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "getTransaction",
                    "params": [
                        tx_signature,
                        {
                            "encoding": "json",
                            "commitment": "finalized",
                            "maxSupportedTransactionVersion": 0
                        }
                    ],
                    "id": 1
                }))
                .send()
                .await
                .unwrap();

            let tx_details: serde_json::Value = tx_response.json().await.unwrap();

            if let Some(tx_data) = tx_details.get("result") {
                if !tx_data.is_null() {
                    println!("✅ Found transaction!");
                    println!(
                        "Transaction data: {}",
                        serde_json::to_string_pretty(tx_data).unwrap()
                    );
                    return;
                }
            }

            // // Also try to send the same transaction to regular RPC to compare
            // if attempt == 4 {
            //     println!("🔄 Comparing with regular RPC...");
            // let rpc_response = client
            //     .post("http://localhost:8899")
            //     .json(&serde_json::json!({
            //         "jsonrpc": "2.0",
            //         "method": "sendTransaction",
            //         "params": [
            //             serialized,
            //             {"skipPreflight": true, "encoding": "base64"}
            //         ],
            //         "id": 1
            //     }))
            //     .send()
            //     .await
            //     .unwrap();

            //     let rpc_result: serde_json::Value = rpc_response.json().await.unwrap();
            //     println!("Regular RPC response: {}", rpc_result);
            // }

            sleep(Duration::from_secs(3)).await;
        }

        println!("⚠️ Transaction not found after {} attempts", attemps);
    }
}
