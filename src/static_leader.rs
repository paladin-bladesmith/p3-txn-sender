use std::{
    net::{SocketAddr},
};

use solana_rpc_client_api::response::RpcContactInfo;

use crate::{
    leader_tracker::LeaderTrackerTrait,
    DEFAULT_P3_QUIC_PORT,
};

#[derive(Clone)]
pub struct StaticLeaderImpl {
    static_leader: RpcContactInfo,
}

impl StaticLeaderImpl {
    pub fn new(leader_addr: String) -> Self {
        let p3_addr = SocketAddr::new(
            leader_addr.parse().expect("Invalid IP address"),
            DEFAULT_P3_QUIC_PORT,
        );
        let static_leader = RpcContactInfo {
            pubkey: "STATIC_LEADER".to_string(),
            gossip: Some(p3_addr),
            tpu: None,
            tpu_quic: None,
            rpc: None,
            pubsub: None,
            version: None,
            feature_set: None,
            shred_version: None,
        };
        Self { static_leader }
    }
}

impl LeaderTrackerTrait for StaticLeaderImpl {
    fn get_leaders(&self) -> Vec<RpcContactInfo> {
        vec![self.static_leader.clone()]
    }
}
