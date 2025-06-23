# Verifier instructions for Telcoin-Network bridging

Running a verifier on Axelar Network constitutes running an instance of tofnd and of ampd in tandem. These services perform Axelar GMP message verification and sign transactions representing votes which are submitted to Axelar Network as part of Telcoin-Network's bridging flow.

### Note: This document's instructions detail the process to run a TN Verifier for testing on devnet and testnet by the Telcoin Network team. For mainnet, Axelar Network already has an existing set of verifiers who will run verifiers alongside a TN NVV client. Thus this document is not relevant to mainnet deployment

## Running a TOFND instance

Download the tofnd binary depending on machine architecture from the [latest release tag](https://github.com/axelarnetwork/tofnd/releases)

Create a default mnemonic and configuration in ~/.tofnd/, then back it up and delete it.

```bash
~/Downloads/tofnd*-v1.0.1 -m create
mv ~/.tofnd/export ~/.tofnd/export-new-location
```

Create an alias or symlink to the `tofnd` binary in your `.bashrc`. Be sure to specify the correct file name which may be a different architecture or later version than v1.0.1.

```bash
echo "alias tofnd=~/Downloads/tofnd-linux-amd64-v1.0.1" >> ~/.bashrc
source ~/.bashrc
```

Now run tofnd. This can be done with the alias or with the binary directly:

```bash
tofnd
./tofnd-linux-amd64-v1.0.1 -m existing
```

## Running an AMPD instance

### Obtaining the ampd binary

Download the ampd binary depending on machine architecture from the [latest release tag](https://github.com/axelarnetwork/axelar-amplifier/releases)

Add ampd to your PATH by adding an alias to ampd at the end of the .bashrc file on your machine and then reload the file to apply all changes:

```bash
echo "alias ampd=~/Downloads/ampd-linux-amd64-v1.2.0" >> ~/.bashrc
source ~/.bashrc
```

Replace ampd-linux-amd64-v1.2.0 with the correct ampd binary if needed

Now you can run ampd, for example:

```bash
ampd --version
```

### Configure ampd for Telcoin-Network and a source chain (eg Sepolia)

Ampd relies on a config file with handler contract declarations for each chain. This config file is located at `~/.ampd/config.toml`

Below is an example of the `~/.ampd/config.toml` config toml declaring handlers for Sepolia and Telcoin-Network using public RPC endpoints.

```bash
# JSON-RPC URL of Axelar node
tm_jsonrpc="http://devnet-amplifier.axelar.dev:26657"
# gRPC URL of Axelar node
tm_grpc="tcp://devnet-amplifier.axelar.dev:9090"
# max blockchain events to queue. Will error if set too low
event_buffer_cap=10000
# the /status endpoint bind address, often port 3000 i.e "0.0.0.0:3000"
health_check_bind_addr="0.0.0.0:3000"

[service_registry]
# address of service registry
cosmwasm_contract="axelar1c9fkszt5lq34vvvlat3fxj6yv7ejtqapz04e97vtc9m5z9cwnamq8zjlhz"

[broadcast]
# max gas for a transaction. Transactions can contain multiple votes and signatures
batch_gas_limit="20000000"
# how often to broadcast transactions
broadcast_interval="1s"
# chain id of Axelar network to connect to
chain_id="devnet-amplifier"
# gas adjustment to use when broadcasting
gas_adjustment="2"
# gas price with denom, i.e. "0.007uaxl"
gas_price="0.00005uamplifier"
# max messages to queue when broadcasting
queue_cap="1000"
# how often to query for transaction inclusion in a block
tx_fetch_interval="1000ms"
# how many times to query for transaction inclusion in a block before failing
tx_fetch_max_retries="15"

[tofnd_config]
batch_gas_limit="10000000"
# uid of key used for signing transactions
key_uid="axelar"
# metadata, should just be set to ampd
party_uid="ampd"
# url of tofnd
url="http://127.0.0.1:50051"

# multisig handler. This handler is used for all supported chains.
[[handlers]]
# address of multisig contract
cosmwasm_contract="axelar19jxy26z0qnnspa45y5nru0l5rmy9d637z5km2ndjxthfxf5qaswst9290r"
type="MultisigSigner"

# Eth-Sepolia EvmMsgVerifier handler declaration.
[[handlers]]
chain_name="eth-sepolia"
# URL of JSON-RPC endpoint for external chain
chain_rpc_url="https://rpc.ankr.com/eth_sepolia"
# verifier contract address
cosmwasm_contract="axelar16swl5eh3vspnyzddwltuf93hc65nwju7498u0zyq33u4cj8rxy4sgxx30m"
# handler type. Could be EvmMsgVerifier | SuiMsgVerifier
type="EvmMsgVerifier"
# if the chain supports the finalized tag via RPC API, use RPCFinalizedBlock, else use ConfirmationHeight
chain_finalization="RPCFinalizedBlock"

# Eth-Sepolia EvmVerifierSetVerifier handler declaration.
[[handlers]]
chain_name="eth-sepolia"
chain_rpc_url="https://rpc.ankr.com/eth_sepolia"
cosmwasm_contract="axelar16swl5eh3vspnyzddwltuf93hc65nwju7498u0zyq33u4cj8rxy4sgxx30m"
type="EvmVerifierSetVerifier"

# Telcoin-Network EvmMsgVerifier handler declaration
[[handlers]]
chain_name="telcoin-devnet"
# URL of JSON-RPC endpoint for external chain
chain_rpc_url="https://adiri.tel"
# verifier contract address
cosmwasm_contract="axelar1kdzmvkjtvu8cct0gzzqdj8jyd6yvlcswauu73ccmvcl0w429xcxqdqst4p"
# handler type; TN is EVM
type="EvmMsgVerifier"
# TN supports the finalized tag via RPC API; use RPCFinalizedBlock
chain_finalization="RPCFinalizedBlock"

# Telcoin-Network EvmVerifierSetVerifier handler declaration
[[handlers]]
chain_name="telcoin-devnet"
# URL of JSON-RPC endpoint for external chain
chain_rpc_url="https://adiri.tel"
# verifier contract address
cosmwasm_contract="axelar1kdzmvkjtvu8cct0gzzqdj8jyd6yvlcswauu73ccmvcl0w429xcxqdqst4p"
# handler type; TN is EVM
type="EvmVerifierSetVerifier"
```

### Fund the Verifier associated with the ampd instance

To determine the verifier address associated with the ampd instance we've configure thus far, run:

`ampd verifier-address`

##### For reference, the Telcoin Network verifier for devnet is `axelar1t055c4qmplk8dwfaqf55dnm29ddg75rjh4jlle`. This verifier is the sole verifier for both the voting verifier and multisig prover contracts associated with Telcoin Network GMP messages on Axelar devnet-amplifier.

After determining the verifier address, fund it for gas purposes. This can be done using the Axelar faucet or with a transaction on devnet:

`axelard tx bank send wallet axelar1t055c4qmplk8dwfaqf55dnm29ddg75rjh4jlle 100uamplifier --node http://devnet-amplifier.axelar.dev:26657`

To query the verifier's balance to ensure it has been funded:

`axelard q bank balances axelar1t055c4qmplk8dwfaqf55dnm29ddg75rjh4jlle --node http://devnet-amplifier.axelar.dev:26657`

### Submit the Verifier onboarding form

Once funded, submit [the Amplifier Verifier onboarding form](https://docs.google.com/forms/d/e/1FAIpQLSfQQhk292yT9j8sJF5ARRIE8PpI3LjuFc8rr7xZW7posSLtJA/viewform) for whitelisting

### Bond the Verifier

Verifiers must post a bond; note that the bond amount varies between devnet-amplifier, testnet, and mainnet. Remember that there is an existing network of mainnet verifiers who have already posted this bond.

Note that each network possesses a corresponding "service name" which is the terminology that `ampd` recognizes, where:

- devnet-amplifier is called "validators"
- testnet is called "amplifier"

#### For devnet-amplifier ie "validators", the bond amount is 100 uamplifier (equivalent to the faucet distribution amount):

`ampd bond-verifier validators 100 uamplifier`

[Here is the bonding transaction for the TN <> devnet-amplifier verifier](https://devnet-amplifier.axelarscan.io/tx/8822B3CBDDAC6F83B80E748DDF05BC7F6F66A14B54C73FD44327EC841C1F098F)

#### For testnet ie "amplifier", the bond amount is 100k uAXL:

`ampd bond-verifier amplifier 100000000000 uaxl`

#### For mainnet, the bond amount is 500k uAXL:

`ampd bond-verifier amplifier 50000000000 uaxl`

### Register the ampd instance's public key

This step registers the ampd instances public key

`ampd register-public-key ecdsa`

[Here is the public key registration transaction for the TN <> devnet-amplifier verifier](https://devnet-amplifier.axelarscan.io/tx/8CBC0F77B0C3A1E1BE6C4DBE9505BC21CC582429CBCBFBC90B77DEE2DFCB019B)

### Register support for specified chains,

Use `ampd register-chain-support` to register support for specific chains, in this case Telcoin-Network and Sepolia. Be sure to pass in the correct "service name"!

`ampd register-chain-support validators eth-sepolia ethereum-sepolia telcoin-network`

#### Important: any chain included in the `register-chain-support` command must have associated handler declarations in the `~/.ampd/config.toml` file above

### Run the `ampd` daemon

Once the ampd verifier instance has been whitelisted and authorized in accordance with the form submitted above, the `ampd` daemon can be run (alongside `tofnd`) to begin verifying GMP messages which are submitted to the GMP API.

While running, the verifier instance will monitor the voting verifier and multisig prover contracts and submit transactions to vote on GMP message veracity upon detecting new polls.

To determine veracity, ampd performs an RPC call to the endpoint specified in the `~/.ampd/config.toml` to confirm that the GMP message being voted on (ie received by the voting verifier) was indeed emitted on the source chain's external gateway contract.

As such, GMP message veracity is dependent on the source chain's finality properties. Put another way, a GMP message is unequivocally valid if it was emitted in the external gateway contract's `ContractCall` event as part of a block that has reached finality. This is because once a chain has come to consensus on a block, the block's execution is irreversible, immutable, and permanent on every node in the network.

To learn what constitutes finality properties on the source chain, ampd performs an RPC call to the standard `RPCFinalizedBlock`.
