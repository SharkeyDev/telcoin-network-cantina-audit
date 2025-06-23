#!/bin/bash
# This script funds a rewards pool; which can be for either a verifier or multisig
set -e
set -u

# devnet-amplifier config
CHAIN_NAME="telcoin"
CHAIN_ID="devnet-amplifier"
REWARDS_CONTRACT_ADDR="axelar1vaj9sfzc3z0gpel90wu4ljutncutv0wuhvvwfsh30rqxq422z89qnd989l"
VOTING_VERIFIER_OR_MULTISIG_CONTRACT_ADDR=""
RPC="http://devnet-amplifier.axelar.dev:26657" 
AMOUNT="1000uamplifier" # (1000000 = 1 AXL)
# telcoin-specific devnet config
AXELAR_WALLET="devnet"

VERIFIER_FLAG=false
MULTISIG_FLAG=false

# parse CLI args if given
while [[ "$#" -gt 0 ]]; do
    case $1 in
        # specify --verifier to fund a verifier pool
        --verifier)
            VERIFIER_FLAG=true
            if [[ -n "${2:-}" && ! "$2" =~ ^-- ]]; then
                VOTING_VERIFIER_OR_MULTISIG_CONTRACT_ADDR="$2"
                shift
            else
                echo "Error: provide a value to --verifier"
                exit 1
            fi
            ;;
        # specify --multisig to fund a multisig pool
        --multisig) 
            MULTISIG_FLAG=true
            if [[ -n "${2:-}" && ! "$2" =~ ^-- ]]; then
                VOTING_VERIFIER_OR_MULTISIG_CONTRACT_ADDR="$2"
                shift
            else
                echo "Error: provide a value to --multisig"
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
        --amount)
            if [[ -n "${2:-}" && ! "$2" =~ ^-- ]]; then
                AMOUNT="$2"
                shift
            else
                echo "Must provide a value if specifying --amount"
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

# ensure either --verifier or --multisig is provided and not both
if [[ "$VERIFIER_FLAG" == true && "$MULTISIG_FLAG" == true ]]; then
    echo "Error: script can only fund one pool type at a time."
    exit 1
fi
if [[ "$VERIFIER_FLAG" == false && "$MULTISIG_FLAG" == false ]]; then
    echo "Error: must specify verifier or multisig pool type."
    exit 1
fi

echo "Using target reward pool address: $VOTING_VERIFIER_OR_MULTISIG_CONTRACT_ADDR"
echo "Using native token amount: $AMOUNT"
echo "Using RPC url: $RPC"

axelard tx wasm execute $REWARDS_CONTRACT_ADDR \
    '{
        "add_rewards":
            {
                "pool_id":
                    {
                        "chain_name":"'"$CHAIN_NAME"'",
                        "contract":"'"$VOTING_VERIFIER_OR_MULTISIG_CONTRACT_ADDR"'"
                    }
            }
    }' \
    --amount $AMOUNT \
    --keyring-backend file \
    --chain-id $CHAIN_ID \
    --from $AXELAR_WALLET \
    --gas auto --gas-adjustment 1.5 --gas-prices 0.007uamplifier \
    --node $RPC