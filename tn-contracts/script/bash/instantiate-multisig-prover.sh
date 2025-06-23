#!/bin/bash
# This script instantiates a multisig prover using provided params
set -e
set -u

# devnet-amplifier config
PROVER_CODE_ID=855
CHAIN_ID="devnet-amplifier"
SERVICE_NAME="validators"
RPC="http://devnet-amplifier.axelar.dev:26657" 
GOVERNANCE_ADDR="axelar1zlr7e5qf3sz7yf890rkh9tcnu87234k6k7ytd9"
MULTISIG_ADDR="axelar19jxy26z0qnnspa45y5nru0l5rmy9d637z5km2ndjxthfxf5qaswst9290r"
COORDINATOR_ADDR="axelar1m2498n4h2tskcsmssjnzswl5e6eflmqnh487ds47yxyu6y5h4zuqr9zk4g"
SERVICE_REGISTRY_ADDR="axelar1c9fkszt5lq34vvvlat3fxj6yv7ejtqapz04e97vtc9m5z9cwnamq8zjlhz"

# telcoin-specific devnet config
WALLET_ADDR="axelar12u9hneuufhrhqpyr9h352dhrdtnz8c0z3w8rsk"
CHAIN_NAME="telcoin"
VOTING_VERIFIER_ADDR="axelar1cl433j3k3d4syj7wwxm8d9tlu5zqkxjrtjhpzzztk80qh8reftks9h0set"
INTERNAL_GATEWAY_ADDR="axelar1r2s8ye304vtyhfgajljdjj6pcpeya7jwdn9tgw8wful83uy2stnqk4x7ya"

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
        --verifier-addr) 
            if [[ -n "${2:-}" && ! "$2" =~ ^-- ]]; then
                VOTING_VERIFIER_ADDR="$2" 
                shift
            else
                echo "Error: provide a value to --verifier-addr"
                exit 1
            fi
            ;;
        --gateway-addr) 
            if [[ -n "${2:-}" && ! "$2" =~ ^-- ]]; then
                INTERNAL_GATEWAY_ADDR="$2" 
                shift 
            else
                echo "Error: provide a value to --gateway-addr"
                exit 1
            fi
            ;;
        --rpc-url)
            if [[ -n "${2:-}" && ! "$2" =~ ^-- ]]; then
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

# using cast, derive domain separator from `${chainName}_${myWalletAddress}_${proverCodeId}`
input_string="${CHAIN_NAME}_${WALLET_ADDR}_${PROVER_CODE_ID}"
# will be `0x6d4588172fd6e74880710f7559d18e79e26e75c17f7bbf1b2307bda750a46671`
domain_separator=$(cast keccak "$input_string")
domain_separator_unprefixed=${domain_separator#0x}

echo "Using wallet address: $WALLET_ADDR"
echo "Using service name: $SERVICE_NAME"
echo "Using chain name: $CHAIN_NAME"
echo "Using voting verifier: $VOTING_VERIFIER_ADDR"
echo "Using internal gateway: $INTERNAL_GATEWAY_ADDR"
echo "Using domain separator: $domain_separator"
echo "Using RPC url: $RPC"

axelard tx wasm instantiate $PROVER_CODE_ID \
    '{
        "admin_address": "'"$WALLET_ADDR"'",
        "governance_address": "'"$GOVERNANCE_ADDR"'",
        "gateway_address": "'"$INTERNAL_GATEWAY_ADDR"'",
        "multisig_address": "'"$MULTISIG_ADDR"'",
        "coordinator_address":"'"$COORDINATOR_ADDR"'",
        "service_registry_address":"'"$SERVICE_REGISTRY_ADDR"'",
        "voting_verifier_address": "'"$VOTING_VERIFIER_ADDR"'",
        "signing_threshold": ["1","1"],
        "service_name": "'"$SERVICE_NAME"'",
        "chain_name":"'"$CHAIN_NAME"'",
        "verifier_set_diff_threshold": 1,
        "encoder": "abi",
        "key_type": "ecdsa",
        "domain_separator": "'"$domain_separator_unprefixed"'"
    }' \
    --keyring-backend file \
    --from $AXELAR_WALLET \
    --gas auto --gas-adjustment 1.5 --gas-prices 0.00005uamplifier \
    --chain-id $CHAIN_ID \
    --node $RPC \
    --label test-prover  \
    --admin $WALLET_ADDR

# Resulting multisig-prover address: axelar16pqdlnmmrvw4egnlf4nqw8ytvpzqwdec7hfl262yspayjyaxcuys8q2r3l