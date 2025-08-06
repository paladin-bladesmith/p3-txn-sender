use solana_sdk::{
    signature::{Keypair, Signer},
    system_transaction,
    pubkey::Pubkey,
};
use std::str::FromStr;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_send_transaction() {
    let rpc_client = solana_client::nonblocking::rpc_client::RpcClient::new("http://localhost:8899".to_string());
    
    let keypair = solana_sdk::signature::read_keypair_file("validator-keypair.json")
        .expect("Failed to read keypair");
    let rec_keypair = solana_sdk::signature::read_keypair_file("recipient.json")
        .expect("Failed to read keypair");
    
    let to_pubkey = rec_keypair.pubkey();
    let lamports = 1000;
    
    let recent_blockhash = rpc_client.get_latest_blockhash().await.unwrap();
    
    let transaction = system_transaction::transfer(
        &keypair,
        &to_pubkey,
        lamports,
        recent_blockhash,
    );
    
    let serialized = base64::encode(bincode::serialize(&transaction).unwrap());
    println!("Serialized transaction: {}", serialized);
    
    // Send to p3-txn-sender
    let client = reqwest::Client::new();
    let response = client
        .post("http://localhost:4040")
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "sendTransaction",
            "params": [
                serialized,
                {"skipPreflight": true, "encoding": "base64"},
                null
            ],
            "id": 1
        }))
        .send()
        .await
        .unwrap();
    
    let result: serde_json::Value = response.json().await.unwrap();
    println!("P3 response: {}", result);
    
    if let Some(success_result) = result.get("result") {
        let tx_signature = success_result.as_str().unwrap();
        println!("✅ Transaction signature: {}", tx_signature);
        
        // Try multiple commitment levels and wait times
        for attempt in 1..=5 {
            println!("🔍 Attempt {} - Checking transaction...", attempt);
            
            // Try different commitment levels
            for commitment in ["processed", "confirmed", "finalized"] {
                let tx_response = client
                    .post("http://localhost:8899")
                    .json(&serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": "getTransaction",
                        "params": [
                            tx_signature,
                            {
                                "encoding": "json",
                                "commitment": commitment,
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
                        println!("✅ Found transaction with {} commitment!", commitment);
                        println!("Transaction data: {}", serde_json::to_string_pretty(tx_data).unwrap());
                        return;
                    }
                }
            }
            
            // Also try to send the same transaction to regular RPC to compare
            if attempt == 1 {
                println!("🔄 Comparing with regular RPC...");
                let rpc_response = client
                    .post("http://localhost:8899")
                    .json(&serde_json::json!({
                        "jsonrpc": "2.0",
                        "method": "sendTransaction",
                        "params": [
                            serialized,
                            {"skipPreflight": true, "encoding": "base64"}
                        ],
                        "id": 1
                    }))
                    .send()
                    .await
                    .unwrap();
                    
                let rpc_result: serde_json::Value = rpc_response.json().await.unwrap();
                println!("Regular RPC response: {}", rpc_result);
            }
            
            sleep(Duration::from_secs(3)).await;
        }
        
        println!("⚠️ Transaction not found after 5 attempts");
    }
}
