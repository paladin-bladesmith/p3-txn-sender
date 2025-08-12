use cadence_macros::{statsd_count, statsd_gauge, statsd_time};
use solana_client::connection_cache::ConnectionCache;
use solana_connection_cache::nonblocking::client_connection::ClientConnection;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    runtime::{Builder, Runtime},
    time::{sleep, timeout},
};
use tonic::async_trait;
use tracing::{error, info, warn};

use crate::{
    leader_tracker::{LeaderTracker, LeaderTrackerTrait},
    rpc_server::RequestMetadata,
    solana_rpc::SolanaRpc,
    transaction_store::{get_signature, TransactionData, TransactionStore},
};

const MAX_TIMEOUT_SEND_DATA: Duration = Duration::from_millis(500);
const MAX_TIMEOUT_SEND_DATA_BATCH: Duration = Duration::from_millis(500);
const SEND_TXN_RETRIES: usize = 10;

#[async_trait]
pub trait TxnSender: Send + Sync {
    fn send_transaction(&self, txn: TransactionData);
}

pub struct TxnSenderImpl {
    leader_tracker: Arc<LeaderTracker>,
    transaction_store: Arc<dyn TransactionStore>,
    connection_cache: Arc<ConnectionCache>,
    solana_rpc: Arc<dyn SolanaRpc>,
    txn_sender_runtime: Arc<Runtime>,
    txn_send_retry_interval_seconds: usize,
    max_retry_queue_size: Option<usize>,
}

impl TxnSenderImpl {
    pub fn new(
        leader_tracker: Arc<LeaderTracker>,
        transaction_store: Arc<dyn TransactionStore>,
        connection_cache: Arc<ConnectionCache>,
        solana_rpc: Arc<dyn SolanaRpc>,
        txn_sender_threads: usize,
        txn_send_retry_interval_seconds: usize,
        max_retry_queue_size: Option<usize>,
    ) -> Self {
        let txn_sender_runtime = Builder::new_multi_thread()
            .worker_threads(txn_sender_threads)
            .enable_all()
            .build()
            .unwrap();
        let txn_sender = Self {
            leader_tracker,
            transaction_store,
            connection_cache,
            solana_rpc,
            txn_sender_runtime: Arc::new(txn_sender_runtime),
            txn_send_retry_interval_seconds,
            max_retry_queue_size,
        };
        txn_sender.retry_transactions();
        txn_sender
    }

    fn retry_transactions(&self) {
        let leader_tracker = self.leader_tracker.clone();
        let transaction_store = self.transaction_store.clone();
        let connection_cache = self.connection_cache.clone();
        let txn_sender_runtime = self.txn_sender_runtime.clone();
        let txn_send_retry_interval_seconds = self.txn_send_retry_interval_seconds;
        let max_retry_queue_size = self.max_retry_queue_size;
        tokio::spawn(async move {
            loop {
                let mut transactions_reached_max_retries = vec![];
                let transaction_map = transaction_store.get_transactions();
                let queue_length = transaction_map.len();
                statsd_gauge!("transaction_retry_queue_length", queue_length as u64);

                // Shed transactions by retry_count, if necessary.
                if let Some(max_size) = max_retry_queue_size {
                    if queue_length > max_size {
                        warn!(
                            "Transaction retry queue length is over the limit of {}: {}. Load shedding transactions with highest retry count.",
                            max_size,
                            queue_length
                        );
                        let mut transactions: Vec<(String, TransactionData)> = transaction_map
                            .iter()
                            .map(|x| (x.key().to_owned(), x.value().to_owned()))
                            .collect();
                        transactions.sort_by(|(_, a), (_, b)| a.retry_count.cmp(&b.retry_count));
                        let transactions_to_remove = transactions[(max_size + 1)..].to_vec();
                        for (signature, _) in transactions_to_remove {
                            transaction_store.remove_transaction(signature.clone());
                            transaction_map.remove(&signature);
                        }
                        let records_dropped = queue_length - max_size;
                        statsd_gauge!("transactions_retry_queue_dropped", records_dropped as u64);
                    }
                }

                let mut wire_transactions = vec![];
                for mut transaction_data in transaction_map.iter_mut() {
                    wire_transactions.push((
                        transaction_data.request_metadata.send_port,
                        transaction_data.wire_transaction.clone(),
                    ));
                    if transaction_data.retry_count >= transaction_data.max_retries {
                        transactions_reached_max_retries
                            .push(get_signature(&transaction_data).unwrap());
                    } else {
                        transaction_data.retry_count += 1;
                    }
                }
                for (send_port, wire_transaction) in wire_transactions.iter() {
                    for (leader_num, leader) in leader_tracker.get_leaders().iter().enumerate() {
                        let connection_cache = connection_cache.clone();
                        let sent_at = Instant::now();
                        let leader = Arc::new(leader.clone());
                        let mut socket_addr = leader.gossip.unwrap();
                        info!("p3 port is: {}", send_port);
                        socket_addr.set_port(*send_port);
                        let wire_transaction = wire_transaction.clone();
                        txn_sender_runtime.spawn(async move {
                        // retry unless its a timeout
                        for i in 0..SEND_TXN_RETRIES {
                            let conn = connection_cache
                                .get_nonblocking_connection(&socket_addr);
                            if let Ok(result) = timeout(MAX_TIMEOUT_SEND_DATA_BATCH, conn.send_data(&wire_transaction)).await {
                                if let Err(e) = result {
                                    if i == SEND_TXN_RETRIES-1 {
                                        error!(
                                            retry = "true",
                                            "Failed to send transaction batch to {:?}: {}",
                                            leader, e
                                        );
                                        statsd_count!("transaction_send_error", 1, "retry" => "true", "last_attempt" => "true");
                                    } else {
                                        statsd_count!("transaction_send_error", 1, "retry" => "true", "last_attempt" => "false");
                                    }
                                } else {
                                    let leader_num_str = leader_num.to_string();
                                    statsd_time!(
                                        "transaction_received_by_leader",
                                        sent_at.elapsed(), "leader_num" => &leader_num_str, "api_key" => "not_applicable", "retry" => "true");
                                    return;
                                }
                            } else {
                                // Note: This is far too frequent to log. It will fill the disks on the host and cost too much on DD.
                                statsd_count!("transaction_send_timeout", 1);
                            }
                        }
                    });
                    }
                }
                // remove transactions that reached max retries
                for signature in transactions_reached_max_retries {
                    let _ = transaction_store.remove_transaction(signature);
                    statsd_count!("transactions_reached_max_retries", 1);
                }
                sleep(Duration::from_secs(txn_send_retry_interval_seconds as u64)).await;
            }
        });
    }

    fn track_transaction(&self, transaction_data: &TransactionData) {
        let signature = get_signature(transaction_data);
        if signature.is_none() {
            return;
        }
        let signature = signature.unwrap();
        self.transaction_store
            .add_transaction(transaction_data.clone());
        let solana_rpc = self.solana_rpc.clone();
        let RequestMetadata { api_key, .. } = transaction_data.request_metadata.clone();
        self.txn_sender_runtime.spawn(async move {
            let confirmed_at = solana_rpc.confirm_transaction(signature.clone()).await;

            // Collect metrics
            // We separate the retry metrics to reduce the cardinality with API key and price.
            if confirmed_at.is_some() {
                statsd_count!("transactions_landed_by_key", 1, "api_key" => &api_key);
                "true"
            } else {
                statsd_count!("transactions_not_landed_by_key", 1, "api_key" => &api_key);
                "false"
            };
        });
    }
}

#[async_trait]
impl TxnSender for TxnSenderImpl {
    fn send_transaction(&self, transaction_data: TransactionData) {
        self.track_transaction(&transaction_data);
        let RequestMetadata { api_key, send_port } = transaction_data.request_metadata.clone();
        let mut leader_num = 0;
        for leader in self.leader_tracker.get_leaders() {
            if leader.gossip.is_none() {
                error!("leader {:?} has no gossip", leader);
                continue;
            }
            let connection_cache = self.connection_cache.clone();
            let wire_transaction = transaction_data.wire_transaction.clone();
            let api_key = api_key.clone();
            self.txn_sender_runtime.spawn(async move {
                let mut socket_addr = leader.gossip.unwrap();
                socket_addr.set_port(send_port);
                info!("p3 port send is: {}", send_port);

                for i in 0..SEND_TXN_RETRIES {
                    let conn =
                        connection_cache.get_nonblocking_connection(&socket_addr);
                        let e = conn.server_addr();
                        error!("___{:?}", e);

                    if let Ok(result) = timeout(MAX_TIMEOUT_SEND_DATA, conn.send_data(&wire_transaction)).await {
                            if let Err(e) = result {
                                if i == SEND_TXN_RETRIES-1 {
                                    error!(
                                        retry = "false",
                                        "Failed to send transaction to {:?}: {}",
                                        leader, e
                                    );
                                    statsd_count!("transaction_send_error", 1, "retry" => "false", "last_attempt" => "true");
                                } else {
                                    statsd_count!("transaction_send_error", 1, "retry" => "false", "last_attempt" => "false");
                                }
                        } else {
                            let leader_num_str = leader_num.to_string();
                            info!("Data sent!");
                            statsd_time!(
                                "transaction_received_by_leader",
                                transaction_data.sent_at.elapsed(), "leader_num" => &leader_num_str, "api_key" => &api_key, "retry" => "false");
                            return;
                        }
                    } else {
                        // Note: This is far too frequent to log. It will fill the disks on the host and cost too much on DD.
                        statsd_count!("transaction_send_timeout", 1);
                    }
                }
            });
            leader_num += 1;
        }
    }
}
