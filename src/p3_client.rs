use std::{net::SocketAddr, sync::Arc, time::Duration};
use solana_client::connection_cache::ConnectionCache;
use solana_client::nonblocking::tpu_connection::TpuConnection;
use tokio::time::timeout;
use tracing::{debug, error, warn};

pub struct P3Handler {
    p3_addr: SocketAddr,
    connection_cache: Arc<ConnectionCache>,
    timeout_duration: Duration,
}

impl P3Handler {
    pub fn new(p3_addr: SocketAddr, connection_cache: Arc<ConnectionCache>) -> Self {
        Self {
            p3_addr,
            connection_cache,
            timeout_duration: Duration::from_millis(500),
        }
    }

    pub async fn send_transaction(&self, wire_transaction: &[u8]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Sending transaction to P3 address: {}", self.p3_addr);
        
        let conn = self.connection_cache.get_nonblocking_connection(&self.p3_addr);
        let send_future = conn.send_data(wire_transaction);
        
        match timeout(self.timeout_duration, send_future).await {
            Ok(Ok(())) => {
                debug!("Successfully sent {} bytes to P3 address {}", wire_transaction.len(), self.p3_addr);
                Ok(())
            }
            Ok(Err(e)) => {
                error!("Failed to send to P3 address {}: {}", self.p3_addr, e);
                Err(Box::new(e))
            }
            Err(_) => {
                warn!("Timeout sending to P3 address {}", self.p3_addr);
                Err("P3 send timeout".into())
            }
        }
    }

    pub fn get_address(&self) -> SocketAddr {
        self.p3_addr
    }
}
