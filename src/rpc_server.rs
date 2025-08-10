use std::{fmt::Debug, sync::Arc, time::Instant};

use cadence_macros::{statsd_count, statsd_time};
use jsonrpsee::{
    core::{async_trait, RpcResult},
    proc_macros::rpc,
    types::ErrorObjectOwned,
};
use serde::Deserialize;
use solana_rpc_client_api::config::RpcSendTransactionConfig;
use solana_sdk::transaction::VersionedTransaction;
use solana_transaction_status::UiTransactionEncoding;

use crate::{
    errors::invalid_request,
    transaction_store::{TransactionData, TransactionStore},
    txn_sender::TxnSender,
    vendor::solana_rpc::decode_and_deserialize,
};

#[repr(u16)]
#[derive(Deserialize, Clone, Debug)]
pub enum SendPorts {
    P3 = 4819,
    MEV = 4820,
}

// jsonrpsee does not make it easy to access http data,
// so creating this optional param to pass in metadata
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
pub struct OptionalRequestMetadata {
    pub api_key: Option<String>,
    pub send_port: Option<SendPorts>,
}

impl OptionalRequestMetadata {
    pub fn unwrap_or_default(self) -> RequestMetadata {
        let api_key = self.api_key.unwrap_or("none".to_string());
        let send_port = self.send_port.unwrap_or(SendPorts::P3) as u16;

        RequestMetadata { api_key, send_port }
    }
}

// jsonrpsee does not make it easy to access http data,
// so creating this optional param to pass in metadata
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
pub struct RequestMetadata {
    pub api_key: String,
    pub send_port: u16,
}

impl Default for RequestMetadata {
    fn default() -> Self {
        Self {
            api_key: "none".to_string(),
            send_port: SendPorts::P3 as u16,
        }
    }
}

#[rpc(server)]
pub trait AtlasTxnSender {
    #[method(name = "health")]
    async fn health(&self) -> String;
    #[method(name = "sendTransaction")]
    async fn send_transaction(
        &self,
        txn: String,
        params: RpcSendTransactionConfig,
        request_metadata: Option<OptionalRequestMetadata>,
    ) -> RpcResult<String>;
}

pub struct AtlasTxnSenderImpl {
    txn_sender: Arc<dyn TxnSender>,
    transaction_store: Arc<dyn TransactionStore>,
    max_txn_send_retries: usize,
}

impl AtlasTxnSenderImpl {
    pub fn new(
        txn_sender: Arc<dyn TxnSender>,
        transaction_store: Arc<dyn TransactionStore>,
        max_txn_send_retries: usize,
    ) -> Self {
        Self {
            txn_sender,
            max_txn_send_retries,
            transaction_store,
        }
    }
}

#[async_trait]
impl AtlasTxnSenderServer for AtlasTxnSenderImpl {
    async fn health(&self) -> String {
        "ok".to_string()
    }
    async fn send_transaction(
        &self,
        txn: String,
        params: RpcSendTransactionConfig,
        request_metadata: Option<OptionalRequestMetadata>,
    ) -> RpcResult<String> {
        let sent_at = Instant::now();
        let request_metadata = request_metadata
            .map(|m| m.unwrap_or_default())
            .unwrap_or_default();
        let RequestMetadata { api_key, .. } = request_metadata.clone();
        statsd_count!("send_transaction", 1, "api_key" => &api_key);
        validate_send_transaction_params(&params)?;
        let start = Instant::now();
        let encoding = params.encoding.unwrap_or(UiTransactionEncoding::Base58);
        let binary_encoding = encoding.into_binary_encoding().ok_or_else(|| {
            invalid_request(&format!(
                "unsupported encoding: {encoding}. Supported encodings: base58, base64"
            ))
        })?;
        let (wire_transaction, versioned_transaction) =
            match decode_and_deserialize::<VersionedTransaction>(txn, binary_encoding) {
                Ok((wire_transaction, versioned_transaction)) => {
                    (wire_transaction, versioned_transaction)
                }
                Err(e) => {
                    return Err(invalid_request(&e.to_string()));
                }
            };
        let signature = versioned_transaction.signatures[0].to_string();
        if self.transaction_store.has_signature(&signature) {
            statsd_count!("duplicate_transaction", 1, "api_key" => &api_key);
            return Ok(signature);
        }
        let transaction = TransactionData {
            wire_transaction,
            versioned_transaction,
            sent_at,
            retry_count: 0,
            max_retries: std::cmp::min(
                self.max_txn_send_retries,
                params.max_retries.unwrap_or(self.max_txn_send_retries),
            ),
            request_metadata,
        };
        self.txn_sender.send_transaction(transaction);
        statsd_time!(
            "send_transaction_time",
            start.elapsed(),
            "api_key" => &api_key
        );
        Ok(signature)
    }
}

fn validate_send_transaction_params(
    params: &RpcSendTransactionConfig,
) -> Result<(), ErrorObjectOwned> {
    if !params.skip_preflight {
        return Err(invalid_request("running preflight check is not supported"));
    }
    Ok(())
}
