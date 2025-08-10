use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    compute_budget, instruction::Instruction, message::Message, pubkey::Pubkey, signature::Keypair,
    sysvar::instructions::Instructions, transaction::Transaction,
};

use crate::suite::suite_client::SuiteClient;

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

pub struct TestSuite {
    pub rpc_client: RpcClient,
    pub p3_client: SuiteClient,
    pub mev_client: SuiteClient,
    pub validator_keypair: Keypair,
    pub testers: [Keypair; 3],
}

pub const TESTER1_PUBKEY: Pubkey =
    Pubkey::from_str_const("7gt41ih9Q3CBB6gUwj2xQFBEd72MNSFMBFv8rHhrYr9E");
pub const TESTER2_PUBKEY: Pubkey =
    Pubkey::from_str_const("2YmebjD5Y2fTDBrF4s4DoNPFyKMJLV6ftYzUXuibrU4h");
pub const TESTER3_PUBKEY: Pubkey =
    Pubkey::from_str_const("9Hcmomr84nehtwEj13KDNfbSLSeNSvzKtEAw3HCMyccr");

impl TestSuite {
    /// Creates new suite for local testing
    pub fn new_local(ports: SuitePorts) -> Self {
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
        }
    }

    /// TODO: Make sure our setup is ready.
    /// Checks if clients are up and running, and all keypairs have some balance for tests
    pub async fn check_setup(self) -> Self {
        self.rpc_client.get_health().await.unwrap();
        self
    }

    pub async fn build_tx(
        &self,
        ixs: Vec<Instruction>,
        from: &[Keypair],
        payer: Option<&Pubkey>,
    ) -> Transaction {
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
        let cu_ix = compute_budget::ComputeBudgetInstruction::set_compute_unit_price(cu_price);
        ixs.insert(0, cu_ix);
        let message = Message::new(&ixs, payer);
        Transaction::new(from, message, self.get_latest_blockhash().await)
    }

    pub async fn get_latest_blockhash(&self) -> solana_sdk::hash::Hash {
        self.rpc_client.get_latest_blockhash().await.unwrap()
    }
}
