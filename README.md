# telcoin-network-cantina-audit

Telcoin Network is the EVM layer 1 blockchain for the Telcoin Platform. Unlike Ethereum, consensus uses Narhwal and Bullshark to create local DAGs. The protocol is designed so only GSMA full-member mobile network operators (MNOs) validate the blocks to earn rewards. Telcoin (TEL) exists as an ERC-20 on Ethereum. An initial balance for block rewards will be bridged before genesis through Axelar. Once the transaction is approved by Axelar, the protocol will execute the transaction to obtain state changes and include the updated state in Genesis.

Prize distribution and scoring
Total Prize Pool: $110,000

Additional pay for dedicated Cantina researcher: $30,000

$10,000 of the prize pot is reserved for Low Severity findings. These reports are judged based on quality and reviewers are then ranked from 1st to 5th for the purpose of prize allocation.

1st: $4,000
2nd: $2,500
3rd: $1,500
4th: $1,000
5th: $1,000
Early Submission Incentive To make sure the Telcoin launch is completed on schedule, researchers are incentivized to submit High/Medium severity findings early, ie: as soon as one is found. The first valid submission will be rewarded an additional 20% reward, in comparison to its subsequent duplicates.

The finding must identify the root cause, highest valid impact and describe the finding with all the necessary details to consider it valid.
Please note that low quality or vague submissions or submissions that could be subject to interpretations will not be considered for the additional reward.
The escalation process will not apply for these rewards and there will be no discussion for these rewards. The decision made by the Judges/Telcoin team on these rewards will be final.
Example: If a finding has 5 duplicates.
Using regular each of the duplicates would get $2000 each
With the current incentive of 20%. The earliest valid submission gets $2307.72, and the rest of the duplicates get $1923.07 each.
Scoring described in the competition scoring page.

Findings Severities described in detail on our docs page.

Scope
Repository:
Protocol (rust): https://github.com/telcoin-association/telcoin-network
Commit hash: 5ab5cb350d5a607db30a63623c753ee557fd387c
Smart Contracts (solidity): https://github.com/telcoin-association/tn-contracts
Commit hash: 37c3ea99551ff7affa79b5591379ea66abe0041a
Files:
Telcoin Network
bin/telcoin-network/src
crates/config
crates/consensus
crates/engine
crates/batch-builder
crates/batch-validator
crates/execution/tn-rpc
crates/network-libp2p
crates/network-types
crates/node
crates/state-sync
crates/storage
crates/tn-reth
crates/tn-utils
crates/types
TN Contracts
src/consensus
src/interfaces
src/InterchainTEL.sol
src/recoverable-wrapper
src/WTEL.sol
Build Instructions
Telcoin-network
This is a fairly standard Rust project using Cargo so standard cargo commands will work like:

build
test
Etc
There is also a Makefile that is used similar to a script to encapsulate some more complex commands. Use make help to see a list of the commands. The cargo test command will run a subset of tests, basically the slower running tests need to be run separately. See the github CI file ./.github/workflows/pr.yml and ./etc/test-and-attest.sh for some examples of commands run as part of the CI process. Note that test-and-attest.sh can only really work for a telcoin developer (it requires credentials) but includes some test (like restarts) that may be useful. To run the slower restart tests you can use cargo test test_restarts -- --ignored. Note that make attest calls the test-and-attest script and won't work without credentials. To run a local testnet there is a script ./etc/local-testnet.sh. This will compile the release version of telcoin-network (can take some time), generate genesis and start four validators and an observer node (five processes total). It will print the endpoints for the RPCs that can be used with foundry or other ETH tooling (these will provide the standard eth, web3 and net namespaces as well as a custom tn namespace). This is the best way to start up some nodes to test against. These nodes are configured for fast block times (a second or so), see the script to slow them down. It will create a directory ./local-validators/ that will contain the log files for each node (.log files) as well as all the configuration files and databases for each node (a dir per node). You can start and stop (use killall or something similar) with the same data dir or can delete the ./local-validators/ dir to start fresh. The command to start up a test network for the first time (this will work for later starts as well but the dev account can only be funded at genesis) is etc/local-testnet.sh --dev-funds 0x3DCc9a6f3A71F0A6C8C659c65558321c374E917a --start. You will want to use an account with –dev-funds that you have the key to. This account will be funded with a huge amount of TEL for testing and is how you can fund other accounts/contracts. This script can be hacked to provide more validators, observers etc. This is probably the best way to get a testnet up to work with and outlines the genesis ceremony to do so.

tn-contracts
This repository does not use Foundry git submodules due to dependencies that do not properly support them. Instead of the lib directory, all dependencies are kept in node_modules

Requires Node version >= 18, which can be installed like so: nvm install 18
And then install using npm, note that yarn will throw an error because the yarn package manager has removed Circle Research's RecoverableWrapper. npm install
To build the smart contracts: forge b
To run the smart contract tests, which will run for a bit to fuzz thoroughly, use: forge test
The fork tests will require you to add a Sepolia and Telcoin-Network RPC url to the .env file.

Basic POC Test
Mandatory POC rule does NOT apply for this competition but must be provided upon request.

Telcoin Network Example: https://github.com/Telcoin-Association/telcoin-network/blob/dc5a212b00b36069f5dcf2fe291e055fa2ab2971/crates/config/src/genesis.rs#L346-L390

Tn-contracts example: https://github.com/Telcoin-Association/tn-contracts/blob/a96328fa3a73d4d39af27b2c7a9ce6b6b21e5094/test/ITS/InterchainTELTest.t.sol#L44

Out of Scope
Expected behaviors such as trusted/untrusted roles and/or any accepted risks
Telcoin Association verifies all validators. The validators are known and permissioned. The protocol will launch without slashing enabled during an alpha-mainnet phase. During this time, governance may manually apply slashes or revoke NFT access for validators to participate.
Batches of transactions are created from independent transaction pools. These batches are validated by the committee and included in the final consensus block. When the engine executes the consensus block (ConsensusOutput), each consensus batch is executed as a single EVM block. The protocol ignores duplicate transactions during EVM execution.
Outbound bridging transactions have a 7-day wait period. After this period, the user can submit a second transaction to bridge off Telcoin Network.
LightChaser report
https://gist.github.com/ChaseTheLight01/26fa7a86dfc8fd959a85079bae12ad65

Findings Report
## Summary
A short summary of the issue, keep it brief.

## Finding Description
A more detailed explanation of the issue. Poorly written or incorrect findings may result in rejection and a decrease of reputation score.

Describe which security guarantees it breaks and how it breaks them. If this bug does not automatically happen, showcase how a malicious input would propagate through the system to the part of the code where the issue occurs.

## Impact Explanation
Elaborate on why you've chosen a particular impact assessment.

## Likelihood Explanation
Explain how likely this is to occur and why.

## Proof of Concept
A proof of concept is normally required for Critical, High and Medium Submissions for reviewers under 80 reputation points. Please check the competition page for more details, otherwise your submission may be rejected by the judges.

## Recommendation
How can the issue be fixed or solved. Preferably, you can also add a snippet of the fixed code here.
