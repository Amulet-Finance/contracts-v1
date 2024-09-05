import { parseArgs } from "util";

import { HostChain } from "./client";
import { deployConfig } from "./config";
import { readContractFileBytes } from "./utils";

const { values } = parseArgs({
  args: Bun.argv,
  options: {
    contract: {
      type: "string",
    },
    msg: {
      type: "string",
    },
    code: {
      type: "string",
    },
    funds: {
      type: "string",
    },
    label: {
      type: "string",
    },
    store_only: {
      type: "boolean",
      default: false,
    },
    full_path: {
      type: "boolean",
      default: false,
    },
    mainnet: {
      type: "boolean",
      default: false,
    },
  },
  strict: true,
  allowPositionals: true,
});

const config = deployConfig(values.mainnet || false);

const disconnectedHostChain = await HostChain.create(
  config.HOST_CHAIN_PREFIX,
  config.WALLET_MNEMONIC,
  config.HOST_CHAIN_GAS_PRICE,
);

const hostChain = await disconnectedHostChain.connect(config.HOST_CHAIN_RPC);

const admin = await hostChain.accountAddress();

const funds = values.funds ? +values.funds : null;

async function deploy(codeId: number, label: string) {
  if (!values.msg) {
    throw Error("--msg <contract init msg json> flag required");
  }

  console.log(values.msg);

  const [contract] = await hostChain.initContract(
    codeId,
    JSON.parse(values.msg),
    label,
    funds,
    admin,
  );

  console.log(`contract deployed: ${contract.address}`);
}

if (values.code) {
  if (!values.contract) {
    console.log("ignoring --contract flag because --code is specified");
  }

  if (!values.label) {
    throw Error("--label <label> flag required because --code is specified");
  }

  await deploy(+values.code, values.label);

  process.exit(0);
}

if (!values.contract) {
  throw Error("--contract <contract package or path> flag required");
}

const wasmPath = values.full_path
  ? values.contract
  : `${__dirname}/../artifacts/${values.contract}.wasm`;

const wasmBytes: Uint8Array = await readContractFileBytes(wasmPath);

console.log(`uploading contract byte-code`);

const [storedCode] = await hostChain.uploadWasm(wasmBytes);

console.log(`contract code id: ${storedCode}`);

if (values.store_only) {
  process.exit(0);
}

if (values.full_path && !values.label) {
  throw Error("--label <label> flag required because --full_path is specified");
}

const label = values.label || values.contract;

await deploy(storedCode, label);
