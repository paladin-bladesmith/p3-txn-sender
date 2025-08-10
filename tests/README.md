# Tests

There are several steps we need to do to get readyfor the tests

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

* In case you change the validator keypair, make sure to change the first pubkey here as well.

## Build test validator

After adding the staked PAL, we want to build our test validator:

```bash
cargo build --bin solana-test-validator
```

## Run test validator

We are using gRPC and because of that we need geyser enabled, currently `/etc` includes the macOS version and it successfully runs on macOS, you might need to change the config file with the lib path. 

```bash
./target/debug/solana-test-validator --geyser-plugin-config {path/to/p3-txn-sender}/etc/yellowstone-geyser.json
```

* NOTE - sometimes the test-validator state might get corrputed because of stopping the validator mid way, it is safe to clean the state (delete `test-ledger`) and try to run the validator again, this might fix some weird behaviors.

## Run p3-txn-sender

After you successfully started the test validator, you need to start p3-txn-sender in a separate process.

```bash
./scripts/run.sh
```

* The script should include the default environment variables, but if for some reason those are not the same for you, you will need to modify them.

## Run tests

To confirm everything is up and running smoothly, you can try to run the simple test:

```bash
cargo test --test simple -- --nocapture
```
