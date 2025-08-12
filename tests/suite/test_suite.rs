use std::{str::FromStr, time::Duration};

use reqwest::Client;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    compute_budget,
    instruction::Instruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::Transaction,
};
use solana_transaction_status::UiTransactionEncoding;
use tokio::time::sleep;

use crate::suite::suite_client::SuiteClient;

pub const VALIDATOR_PUBKEY: Pubkey =
    Pubkey::from_str_const("3wWrxQNpmGRzaVYVCCGEVLV6GMHG4Vvzza4iT79atw5A");
pub const TESTER1_PUBKEY: Pubkey =
    Pubkey::from_str_const("7gt41ih9Q3CBB6gUwj2xQFBEd72MNSFMBFv8rHhrYr9E");
pub const TESTER2_PUBKEY: Pubkey =
    Pubkey::from_str_const("2YmebjD5Y2fTDBrF4s4DoNPFyKMJLV6ftYzUXuibrU4h");
pub const TESTER3_PUBKEY: Pubkey =
    Pubkey::from_str_const("9Hcmomr84nehtwEj13KDNfbSLSeNSvzKtEAw3HCMyccr");

pub struct SuitePorts {
    rpc: u16,
    sender: u16,
    p3: u16,
    mev: u16,
}

impl Default for SuitePorts {
    fn default() -> Self {
        Self {
            rpc: 8899,
            sender: 4040,
            p3: 4819,
            mev: 4820,
        }
    }
}

pub struct TxResponse {
    pub fee: u64,
    pub cu_consumed: u64,
    pub slot: u64,
}

pub struct TestSuite {
    pub rpc_client: RpcClient,
    pub p3_client: SuiteClient,
    pub mev_client: SuiteClient,
    pub validator_keypair: Keypair,
    pub testers: [Keypair; 3],
    base_url: String,
    ports: SuitePorts,
}

impl TestSuite {
    /// Creates new suite for local testing
    pub async fn new_local(ports: SuitePorts) -> Self {
        let url = "http://127.0.0.1";
        let rpc_client = solana_client::nonblocking::rpc_client::RpcClient::new(format!(
            "{}:{}",
            url, ports.rpc
        ));

        let validator_keypair =
            solana_sdk::signature::read_keypair_file("tests/keypairs/validator-keypair.json")
                .expect("Failed to read validator-keypair");

        // Set 3 testers (should be enough for all tests)
        let tester1_keypair =
            solana_sdk::signature::read_keypair_file("tests/keypairs/tester1.json")
                .expect("Failed to read tester1");
        let tester2_keypair =
            solana_sdk::signature::read_keypair_file("tests/keypairs/tester2.json")
                .expect("Failed to read tester2");
        let tester3_keypair =
            solana_sdk::signature::read_keypair_file("tests/keypairs/tester3.json")
                .expect("Failed to read tester3");

        let client_url = format!("{}:{}", url, ports.sender);
        let p3_client = SuiteClient::new(client_url.clone(), ports.p3);
        let mev_client = SuiteClient::new(client_url, ports.mev);

        Self {
            rpc_client,
            p3_client,
            mev_client,
            validator_keypair,
            testers: [tester1_keypair, tester2_keypair, tester3_keypair],
            base_url: url.to_string(),
            ports,
        }
        .check_setup()
        .await
    }

    /// Make sure our setup is ready.
    /// Checks if clients are up and running, and all keypairs have some balance for tests
    async fn check_setup(self) -> Self {
        // Confirm our validator RPC is running
        self.rpc_client.get_health().await.unwrap();

        // Confirm our sender is running
        let client = Client::new();
        let res = client
            .post(&format!("{}:{}", self.base_url, self.ports.sender))
            .json(&serde_json::json!({
                "jsonrpc": "2.0",
                "method": "health",
                "params": null,
                "id": 1
            }))
            .send()
            .await
            .unwrap()
            .status();
        assert!(res.is_success(), "Sender is not running");

        // Confirm we have SOL for all our keypairs (validator and 3 testers)
        // Helper function to airdrop incase balance is 0
        let validate_balance = async |key: &Pubkey| {
            let bal = self.get_balance(key).await;

            // airdrop if balance is 0
            if bal <= 1_000_000_000 {
                println!("Balance is low for {}", key);
                let sig = self.request_airdrop(key, 2_000_000_000_000).await;

                // Confirm airdrop finalized
                self.get_transaction(&sig).await;
            }
        };

        validate_balance(&self.validator_keypair.pubkey()).await;
        validate_balance(&self.testers[0].pubkey()).await;
        validate_balance(&self.testers[1].pubkey()).await;
        validate_balance(&self.testers[2].pubkey()).await;

        self
    }

    pub async fn build_tx(
        &self,
        mut ixs: Vec<Instruction>,
        from: &[Keypair],
        payer: Option<&Pubkey>,
    ) -> Transaction {
        // Set cu limit because of unknown bug
        let cu_limit_ix = compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(500_000);
        ixs.insert(0, cu_limit_ix);

        let message = Message::new(&ixs, payer);
        Transaction::new(from, message, self.get_latest_blockhash().await)
    }

    pub async fn build_tx_with_cu_price(
        &self,
        mut ixs: Vec<Instruction>,
        from: &[Keypair],
        payer: Option<&Pubkey>,
        cu_price: u64,
    ) -> Transaction {
        if cu_price > 0 {
            let cu_ix = compute_budget::ComputeBudgetInstruction::set_compute_unit_price(cu_price);
            ixs.insert(0, cu_ix);
        }

        // We need to set limit because of some unknown "maybe bug" in the ordering of TXs without it
        let cu_limit_ix = compute_budget::ComputeBudgetInstruction::set_compute_unit_limit(500_000);
        ixs.insert(0, cu_limit_ix);

        let message = Message::new(&ixs, payer);
        Transaction::new(from, message, self.get_latest_blockhash().await)
    }

    pub async fn get_latest_blockhash(&self) -> solana_sdk::hash::Hash {
        self.rpc_client.get_latest_blockhash().await.unwrap()
    }

    pub async fn get_balance(&self, key: &Pubkey) -> u64 {
        self.rpc_client.get_balance(key).await.unwrap()
    }

    /// airdrop some SOL to address
    pub async fn request_airdrop(&self, key: &Pubkey, amount: u64) -> String {
        self.rpc_client
            .request_airdrop(key, amount)
            .await
            .unwrap()
            .to_string()
    }

    /// Rpc query the transaction
    /// returns (fee paid, cu consumed)
    pub async fn get_transaction(&self, sig: &str) -> TxResponse {
        let sig = Signature::from_str(sig).unwrap();

        println!("🕐 Attemping to get transaction");
        let mut result = None;
        for _ in 0..10 {
            match self
                .rpc_client
                .get_transaction(&sig, UiTransactionEncoding::Json)
                .await
            {
                Ok(res) => {
                    result = Some(res);
                    break;
                }
                Err(_) => sleep(Duration::from_secs(3)).await,
            }
        }

        if let Some(result) = result {
            let res = result.transaction.meta.unwrap();

            TxResponse {
                fee: res.fee,
                cu_consumed: res.compute_units_consumed.unwrap_or(0),
                slot: result.slot,
            }
        } else {
            panic!("❌ Failed getting the transaction");
        }
    }

    pub async fn get_block_transactions(
        &self,
        slot: u64,
    ) -> Vec<solana_transaction_status::EncodedTransactionWithStatusMeta> {
        self.rpc_client.get_block(slot).await.unwrap().transactions
    }
}
