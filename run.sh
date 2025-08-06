export RPC_URL=http://localhost:8899
export GRPC_URL=http://localhost:10000
export STATIC_IP=127.0.0.1
export IDENTITY_KEYPAIR_FILE=validator-keypair.json
# MNEMONIC="milk inflict tuna shuffle bunker dinner impulse normal increase identify observe inject"

cargo run --release
