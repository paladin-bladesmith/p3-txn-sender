use std::{future::IntoFuture, time::Duration};

use reqwest::Client;
use solana_sdk::transaction::Transaction;

pub struct SuiteClient {
    _client: Client,
    client_url: String,
    send_port: u16,
}

impl SuiteClient {
    pub fn new(client_url: String, send_port: u16) -> Self {
        Self {
            _client: Client::new(),
            client_url,
            send_port,
        }
    }

    /// Sends single transaction to this port
    pub async fn send_transaction(&self, tx: Transaction) -> String {
        send_transaction(
            self._client.clone(),
            self.client_url.clone(),
            self.send_port,
            tx,
            0,
        )
        .await
    }

    /// Sends multiple transactions to the same port with a small delay
    pub async fn send_multiple_transactions(&self, txs: &[Transaction]) -> Vec<String> {
        let mut handles = Vec::with_capacity(txs.len());

        for (i, tx) in txs.iter().enumerate() {
            handles.push(tokio::spawn(send_transaction(
                self._client.clone(),
                self.client_url.clone(),
                self.send_port.clone(),
                tx.clone(),
                i as u8,
            )));
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            results.push(handle.await.unwrap())
        }

        results
    }
}

/// Helper function
async fn send_transaction(
    client: Client,
    client_url: String,
    port: u16,
    tx: Transaction,
    id: u8,
) -> String {
    let serialized = base64::encode(bincode::serialize(&tx).unwrap());

    let res = client
        .post(client_url)
        .json(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "sendTransaction",
            "params": [
                serialized,
                {"skipPreflight": true, "encoding": "base64"},
                {"sendPort": port},
            ],
            "id": 1
        }))
        .send()
        .await
        .unwrap();

    let result = res.json::<serde_json::Value>().await.unwrap();
    if let Some(success_result) = result.get("result") {
        let tx_signature = success_result.as_str().unwrap().to_string();
        println!("✅ Transaction id {}, signature: {}", id, tx_signature);
        tx_signature
    } else {
        panic!("TX failed: {}", result.to_string())
    }
}
