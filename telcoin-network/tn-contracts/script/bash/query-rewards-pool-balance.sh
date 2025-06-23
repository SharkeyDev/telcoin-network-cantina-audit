#!/bin/bash
# This script queries a Telcoin-Network rewards pool's balance
set -e
set -u

# devnet-amplifier config
CHAIN_NAME="telcoin"
REWARDS_CONTRACT_ADDR="axelar1vaj9sfzc3z0gpel90wu4ljutncutv0wuhvvwfsh30rqxq422z89qnd989l"
VOTING_VERIFIER_OR_MULTISIG_CONTRACT_ADDR=""
RPC="http://devnet-amplifier.axelar.dev:26657"

VERIFIER_FLAG=false
MULTISIG_FLAG=false

# parse CLI args if given
while [[ "$#" -gt 0 ]]; do
    case $1 in
        # specify --verifier to query a verifier pool
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
        # specify --multisig to query a multisig pool
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
    *)
        echo "Unknown parameter passed: $1"
        exit 1
        ;;
    esac
    shift
done

# ensure either --verifier or --multisig is provided and not both
if [[ "$VERIFIER_FLAG" == true && "$MULTISIG_FLAG" == true ]]; then
    echo "Error: script can only query one pool type at a time."
    exit 1
fi
if [[ "$VERIFIER_FLAG" == false && "$MULTISIG_FLAG" == false ]]; then
    echo "Error: must specify verifier or multisig pool type."
    exit 1
fi

echo "Using target reward pool address: $VOTING_VERIFIER_OR_MULTISIG_CONTRACT_ADDR"
echo "Using RPC url: $RPC"

axelard q wasm contract-state smart $REWARDS_CONTRACT_ADDR \
    '{
        "rewards_pool":
            {
                "pool_id":
                    {
                        "chain_name":"'"$CHAIN_NAME"'",
                        "contract":"'"$VOTING_VERIFIER_OR_MULTISIG_CONTRACT_ADDR"'"
                    }
            }
    }' \
    --node $RPC