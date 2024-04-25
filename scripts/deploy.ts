import { parseArgs } from "util";

import { HostChain } from "./client";
import { deployConfig } from "./config";
import { readContractFileBytes } from "./utils";

function artifact(name: string): string {
  return `${__dirname}/../artifacts/${name}.wasm`
}

const config = deployConfig();

const { values } = parseArgs({
  args: Bun.argv,
  options: {
    contract: {
      type: 'string',
    },
    msg: {
      type: 'string',
    },
    code: {
      type: 'string',
    },
    funds: {
      type: 'string',
    },
    store_only: {
      type: 'boolean',
      default: false,
    },
  },
  strict: true,
  allowPositionals: true,
});

if (!values.contract || !values.msg) {
  throw Error("Usage: bun run deploy.ts --contract <contract-package> --msg <init-msg-json>");
}

const wasmBytes: Uint8Array = await readContractFileBytes(artifact(values.contract));

const disconnectedHostChain = await HostChain.create(
  config.HOST_CHAIN_PREFIX,
  config.WALLET_MNEMONIC,
  config.HOST_CHAIN_GAS_PRICE,
);

const hostChain = await disconnectedHostChain.connect(config.HOST_CHAIN_RPC);

let admin = await hostChain.accountAddress();

let codeId = 0;

if (values.code) 
{
  codeId = +values.code;
  console.log(`using provided code id: ${codeId}`);
} 
else 
{
  console.log(`uploading contract byte-code`);
  const [storedCode] = await hostChain.uploadWasm(wasmBytes);

  codeId = storedCode;
  console.log(`contract code id: ${codeId}`);

  if (values.store_only) {
    process.exit(0);
  }
}

const funds = values.funds ? +values.funds : undefined;

const [contract] = await hostChain.initContract(
  codeId, 
  JSON.parse(values.msg), 
  values.contract,
  funds,
  admin
);

console.log(`contract deployed: ${contract.address}`);
