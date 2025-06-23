import * as https from "https";
import axios from "axios";
import {
  createPublicClient,
  getAddress,
  http,
  Log,
  PublicClient,
} from "viem";
import { sepolia } from "viem/chains";
import axelarAmplifierGatewayArtifact from "../../../artifacts/AxelarAmplifierGateway.json" with { type: "json" };
import * as dotenv from "dotenv";
import { createHttpsAgent, getGMPEnv, getKeystoreAccount, gmpEnv, processTargetCLIArgs, targetConfig } from "./utils.js";
dotenv.config();

/// @dev Usage example for subscribing to a target AxelarAmplifierGateway:
/// `npm run subscriber -- --target-chain telcoin-network --target-contract 0xF128c84c3326727c3e155168daAa4C0156B87AD1`

let client: PublicClient;
let httpsAgent: https.Agent;

let lastCheckedBlock: bigint;

interface ExtendedLog extends Log {
  eventName: string;
  args: {
    sender: string;
    payloadHash: string;
    destinationChain: string;
    destinationContractAddress: string;
    payload: string;
  };
}

async function main() {
  console.log("Starting up subscriber...");

  const args = process.argv.slice(2);
  processTargetCLIArgs(args);

  getGMPEnv();
  getKeystoreAccount();
  httpsAgent = createHttpsAgent(gmpEnv.crtPath!, gmpEnv.keyPath!);
  
  console.log(`Subscriber running for ${targetConfig.chain!.name}`);
  console.log(`Subscribed to ${targetConfig.contract}`);

  client = createPublicClient({
    chain: targetConfig.chain,
    transport: http(targetConfig.rpcUrl),
  });

  try {
    const currentBlock = await client.getBlockNumber();
    console.log("Current block (saved as `lastCheckedBlock`): ", currentBlock);

    const terminateSubscriber = client.watchContractEvent({
      address: getAddress(targetConfig.contract!),
      abi: axelarAmplifierGatewayArtifact.abi,
      eventName: "ContractCall",
      fromBlock: currentBlock,
      args: {},
      /*
        args: {
                destinationChain: "telcoin-network",
                destinationContractAddress: "0x07e17e17e17e17e17e17e17e17e17e17e17e17e1",
        }
        */
      onLogs: (logs) => processLogs(logs),
    });

    lastCheckedBlock = currentBlock;
  } catch (err) {
    console.error("Error monitoring events: ", err);
  }
}

async function processLogs(logs: Log[]) {
  // handle axelar's custom nomenclature for sepolia
  let sourceChain = targetConfig.chain!.name.toLowerCase();
  if (targetConfig.chain === sepolia) sourceChain = `eth-${sourceChain}`;

  const events = [];
  for (const log of logs) {
    console.log("New event: ", log);
    const txHash = log.transactionHash;
    const logIndex = log.logIndex;
    const id = `${txHash}-${logIndex}`;

    const extendedLog = log as ExtendedLog;
    const sender = extendedLog.args.sender;
    const payloadHash = extendedLog.args.payloadHash;
    const destinationChain = extendedLog.args.destinationChain;
    const destinationContractAddress =
      extendedLog.args.destinationContractAddress;
    const payload = extendedLog.args.payload;

    // construct array info for API call
    events.push({
      type: "CALL",
      eventID: id,
      message: {
        messageID: id,
        sourceChain: sourceChain,
        sourceAddress: sender,
        destinationAddress: destinationContractAddress,
        payloadHash: payloadHash,
      },
      destinationChain: destinationChain,
      payload: payload,
    });
  }

  try {
    const request = {
      events: events,
    };

    // make post request
    const response = await axios.post(
      `${gmpEnv.gmpApiUrl}/chains/${sourceChain}/events`,
      request,
      {
        headers: {
          "Content-Type": "application/json",
        },
        httpsAgent,
      }
    );

    console.log("Success: ", response.data);
  } catch (err) {
    console.error("GMP API error: ", err);
  }
}

main();
