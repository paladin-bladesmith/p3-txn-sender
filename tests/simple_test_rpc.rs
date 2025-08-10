use solana_sdk::{signer::Signer, system_transaction};

#[tokio::test]
async fn simple_transfer() {
    let rpc_client =
        solana_client::nonblocking::rpc_client::RpcClient::new("http://localhost:8899".to_string());

    let keypair = solana_sdk::signature::read_keypair_file("validator-keypair.json")
        .expect("Failed to read keypair");
    let rec_keypair =
        solana_sdk::signature::read_keypair_file("recipient.json").expect("Failed to read keypair");

    let to_pubkey = rec_keypair.pubkey();
    let lamports = 1111;

    let recent_blockhash = rpc_client.get_latest_blockhash().await.unwrap();

    let transaction =
        system_transaction::transfer(&keypair, &to_pubkey, lamports, recent_blockhash);

    let serialized = base64::encode(bincode::serialize(&transaction).unwrap());
    println!("Serialized transaction: {}", serialized);

    let client = reqwest::Client::new();
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
