#!/bin/bash
# This script instantiates a voting verifier using provided params
set -e
set -u

# devnet-amplifier config
VERIFIER_CODE_ID=854
SERVICE_NAME="validators" # changes for testnet/mainnet
CHAIN_ID="devnet-amplifier"
RPC="http://devnet-amplifier.axelar.dev:26657"
SERVICE_REGISTRY_ADDR="axelar1c9fkszt5lq34vvvlat3fxj6yv7ejtqapz04e97vtc9m5z9cwnamq8zjlhz"
GOVERNANCE_ADDR="axelar1zlr7e5qf3sz7yf890rkh9tcnu87234k6k7ytd9"
AXELAR_WALLET="devnet"
WALLET_ADDR="axelar12u9hneuufhrhqpyr9h352dhrdtnz8c0z3w8rsk"
CHAIN_NAME="telcoin"
SOURCE_GATEWAY_ADDR="0xF128c84c3326727c3e155168daAa4C0156B87AD1"

# parse CLI args if given
while [[ "$#" -gt 0 ]]; do
    case $1 in 
        --wallet-addr) 
            if [[ -n "${2:-}" && ! "$2" =~ ^-- ]]; then
                WALLET_ADDR="$2" 
                shift
            else
                echo "Error: provide a value to --wallet-addr"
                exit 1
            fi
            ;;
        --src-gateway) 
            if [[ -n "$2:-}" && ! "$2" =~ ^-- ]]; then
                SOURCE_GATEWAY_ADDR="$2" 
                shift
            else
                echo "Error: provide a value to --src-gateway"
                exit 1
            fi
            ;;
        --rpc-url)
            if [[ -n "$2:-}" && ! "$2" =~ ^-- ]]; then
                RPC="$2"
                shift
            else
                echo "Must provide a value if specifying --rpc-url"
                exit 1
            fi
            ;;
    *) 
        echo "Unknown parameter passed: $1"
        exit 1
        ;;
    esac
    shift
done

echo "Using wallet address: $WALLET_ADDR"
echo "Using service name: $SERVICE_NAME"
echo "Using source chain name: $CHAIN_NAME"
echo "Using source gateway: $SOURCE_GATEWAY_ADDR"
echo "Using RPC url: $RPC"

axelard tx wasm instantiate $VERIFIER_CODE_ID \
    '{
        "governance_address": "'"$GOVERNANCE_ADDR"'",
        "service_registry_address":"'"$SERVICE_REGISTRY_ADDR"'",
        "service_name":"'"$SERVICE_NAME"'",
        "source_gateway_address":"'"$SOURCE_GATEWAY_ADDR"'",
        "voting_threshold":["1","1"],
        "block_expiry":"10",
        "confirmation_height":1,
        "source_chain":"'"$CHAIN_NAME"'",
        "rewards_address":"axelar1vaj9sfzc3z0gpel90wu4ljutncutv0wuhvvwfsh30rqxq422z89qnd989l",
        "msg_id_format":"hex_tx_hash_and_event_index",
        "address_format": "eip55"
    }' \
    --keyring-backend file \
    --from $AXELAR_WALLET \
    --gas auto --gas-adjustment 1.5 --gas-prices 0.00005uamplifier\
    --chain-id $CHAIN_ID \
    --node $RPC \
    --label test-voting-verifier \
    --admin $WALLET_ADDR
    
# Resulting voting-verifier address: axelar1cl433j3k3d4syj7wwxm8d9tlu5zqkxjrtjhpzzztk80qh8reftks9h0set