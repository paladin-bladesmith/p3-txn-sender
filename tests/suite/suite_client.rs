use reqwest::{Client, Response};
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

    pub async fn send_transaction(&self, tx: Transaction) -> Response {
        let serialized = base64::encode(bincode::serialize(&tx).unwrap());

        self._client
            .post(&self.client_url)
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "sendTransaction",
                "params": [
                    serialized,
                    {"skipPreflight": true, "encoding": "base64"},
                    {"send_port": self.send_port.to_string()},
                ],
                "id": 1
            }))
            .send()
            .await
            .unwrap()
    }
}
