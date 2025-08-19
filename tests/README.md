# Tests

There are several steps we need to do to get readyfor the tests

## Get test validator keypairs

When starting a test validator a fresh validator keypair is created, we need to copy that keypair and place it in `tests/keypairs/validator-keypair.json`.

We also need to get the pybkey of that kaypair, you can do so by running the following command:

```bash
solana-keygen pubkey tests/keypairs/validator-keypair.json
```

Vote account keypair is also needed, and should be done the same for: `vote-account-keypair.json`.

## Set staked PAL

Before we can send TXs to p3 ports, we need to stake PAL, currently we don't have the programs setup, so we need to manually set our address with some staked PAL

The easiest way of doing so would be to update the `update_staked_nodes` function (search in files, in `p3-quic/src/lib.rs`).

You can add the next code snippet before the first return:

```rust
let mut stakes: HashMap<Pubkey, u64> = HashMap::new();

stakes.insert(
    Pubkey::from_str_const("3wWrxQNpmGRzaVYVCCGEVLV6GMHG4Vvzza4iT79atw5A"),
    100_000_000_000,
);
stakes.insert(
    Pubkey::from_str_const("3wWrxQNpmGRzaVYVCCGEVLV6GMHG4Vvzza4iT79atw5B"),
    100_000_000_000,
);
let stakes = Arc::new(stakes);
*self.staked_nodes.write().unwrap() =
    StakedNodes::new(stakes.clone(), HashMap::default());
```

- On every fresh run of the test validator, the validator key pair is changed, we should update the pubkey in the code snippet above to match the new validator keypair.

## Adjust limits

In quic server we some limits that want to adjust to not get rate limited, specifically in `p3-quic/src/lib.rs`.

we want to adjust all `QuicServerParams` and add those values:

```rust
max_connections_per_ipaddr_per_min: 999999,
max_connections_per_peer: 999999,
```

## Update test validator config

For our tests to work, we need to set some config values on the test validator in `paladin-solana/core/src/validator.rs` in `default_for_test()` function:

```rust 
secondary_block_engine_urls: vec![
    "http://127.0.0.1:6000".to_string(),
    "http://127.0.0.1:6001".to_string(),
],
tip_manager_config: TipManagerConfig {
    funnel: None,
    rewards_split: None,
    tip_payment_program_id: pubkey!("T1pyyaTNZsKv2WcRAB8oVnk93mLJw2XzjtVYqCsaHqt"),
    tip_distribution_program_id: pubkey!(
        "4R3gSG8BpU4t19KYj8CfnbtRpnT8gtk4dvTHxVRwc2r7"
    ),
    tip_distribution_account_config: TipDistributionAccountConfig {
        merkle_root_upload_authority: Pubkey::new_unique(),
        vote_account: pubkey!("Fv9KrA41s7h4PA3QfKR3LE8abXYPv9Qb1ooVU6kdWFsV"),
        commission_bps: 10,
    },
},
```

`secondary_block_engine_urls` - List of secondary block engine URLs that are running on the same machine, this is the URLs of our p3-standalones. (notice that the main block engine URL must be provided when a test validator is started).

`tip_manager_config` - Is configuration for tips to work, tip programs URLs are of mainnet and do not change, but the `vote_account` pubkey should be the same as the one in `tests/keypairs/vote-account-keypair.json`.

## Build test validator

After adding the staked PAL, we want to build our test validator:

```bash
cargo build --bin solana-test-validator
```

## Run p3-standalones

p3-standalone is a usful tool that allows us to run a block engine helper which wraps TXs into a bundle when received on its MEV port.

To run it you should run the following command on paladin-solana repo:

```bash
cargo run --bin p3-standalone -- --rpc-servers http://127.0.0.1:8899 --websocket-servers ws://127.0.0.1:8900 --p3-addr 127.0.0.1:4821 --p3-mev-addr 127.0.0.1:4822
```

Note that the `--p3-addr` and `--p3-mev-addr` should be adjusted with every instance of the p3-standalone, so if you run multiple instances, you should change the port numbers.

Also note that the main `grpc-bind-ip` should also be changed with every new instance, the default is `5999`, for example:

```bash
cargo run --bin p3-standalone -- --rpc-servers http://127.0.0.1:8899 --websocket-servers ws://127.0.0.1:8900 --p3-addr 127.0.0.1:4823 --p3-mev-addr 127.0.0.1:4824 --grpc-bind-ip 127.0.0.1:6000
```

You can see the p3 ports were changed to `4823` and `4824`, and the grpc port was changed to `6000`.

### Mutliple p3 standalones

We sometimes want to test multiple p3-standalone instances, the easiest way of testing it, is to create a new `suite` instance with the new p3-standalone ports.

## Run test validator

We are using gRPC and because of that we need geyser enabled, currently `/etc` includes the macOS version and it successfully runs on macOS, you might need to change the config file with the lib path.

```bash
./target/debug/solana-test-validator --geyser-plugin-config {path/to/p3-txn-sender}/etc/yellowstone-geyser.json --block-engine-url http://127.0.0.1:5999
```

- NOTE - sometimes the test-validator state might get corrputed because of stopping the validator mid way, it is safe to clean the state (delete `test-ledger`) and try to run the validator again, this might fix some weird behaviors.

## Run p3-txn-sender

After you successfully started the test validator, you need to start p3-txn-sender in a separate process.

```bash
./scripts/run.sh
```

- The script should include the default environment variables, but if for some reason those are not the same for you, you will need to modify them.

## Run tests

To confirm everything is up and running smoothly, you can try to run the simple test:

```bash
cargo test --test simple -- --nocapture
```

# Logging in test-validator

Sometimes there are stuff we want to tests which are only possible using logging in test-validator

Will include all useful loggings that can help us debugging

## Received TXs

1 example to it is testing the order in which our TXs are being received.

NOTE - depending on where you send the TXs the receive place can change.

In `core/src/banking_stake/immutable_deserialized_packet.rs` the `ImmutableDeserializedPacket::new` function gives us the `sanitized_transaction` variable which contains the deserialized tx information.

We can log the TX like this:
```rust
info!(
    "PAL_TX_LOG: ImmutableDeserializedPacket::new: {:#?}",
    sanitized_transaction.clone().destruct()
);
```


## Bundle priorities

When testing bundles and their priority, we would like to log the bundle priority in the `paladin-solana/core/src/bundle_stage/bundle_storage.rs` file in `calculate_bundle_priority` function, can be logged before the return statement like this: 

```rust
info!(
    "PAL_TX_LOG: REWARDS: total_cu_cost {} \nreward_from_tx : {} \nreward_from_tips: {} \npriority: {}",
    total_cu_cost, reward_from_tx, reward_from_tips, priority
);
```
