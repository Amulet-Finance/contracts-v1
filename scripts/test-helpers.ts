import { expect } from "bun:test";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { Coin, DirectSecp256k1HdWallet, coin } from "@cosmjs/proto-signing";
import {
  BankExtension,
  MsgTransferEncodeObject,
  SigningStargateClient,
  StakingExtension,
  QueryClient as StargateQueryClient,
  StdFee,
  calculateFee,
  setupBankExtension,
  setupStakingExtension,
} from "@cosmjs/stargate";
import { Tendermint37Client } from "@cosmjs/tendermint-rpc";

export type Wallet = DirectSecp256k1HdWallet;
export type QueryClient = StakingExtension & BankExtension;
export type HostClient = SigningCosmWasmClient;
export type RemoteClient = SigningStargateClient;

export function createFee(suite: ITestSuite, amount: number): StdFee {
  const price = suite.getHostGasPrices();
  return calculateFee(amount, price);
}

export function toBeWithinN(n: number, actual: any, expected: any) {
  expect(BigInt(actual)).toBeLessThanOrEqual(BigInt(expected) + BigInt(n));
  expect(BigInt(actual)).toBeGreaterThanOrEqual(BigInt(expected) - BigInt(n));
}

export async function createQueryClient(rpc: string): Promise<QueryClient> {
  const tmClient = await Tendermint37Client.connect(`http://${rpc}`);
  const qClient = StargateQueryClient.withExtensions(
    tmClient,
    setupStakingExtension,
    setupBankExtension,
  );
  return qClient;
}

export async function createWallet(
  mnemonic: string,
  prefix: string,
): Promise<Wallet> {
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
    prefix,
  });
  return wallet;
}

export async function createRemoteWallet(
  suite: ITestSuite,
  wallet_id: string,
): Promise<Wallet> {
  const mnemonic = suite.getWalletMnemonics()[wallet_id];
  const prefix = suite.getRemotePrefix();
  return createWallet(mnemonic, prefix);
}

export async function createHostWallet(
  suite: ITestSuite,
  wallet_id: string,
): Promise<Wallet> {
  const mnemonic = suite.getWalletMnemonics()[wallet_id];
  const prefix = suite.getHostPrefix();
  return createWallet(mnemonic, prefix);
}

export async function createRemoteClient(
  suite: ITestSuite,
  wallet: Wallet,
): Promise<RemoteClient> {
  const rpc = suite.getRemoteRpc();
  const client = await SigningStargateClient.connectWithSigner(
    `http://${rpc}`,
    wallet,
  );
  return client;
}

export async function createHostClient(
  suite: ITestSuite,
  wallet: Wallet,
): Promise<HostClient> {
  const rpc = suite.getHostRpc();
  const client = await SigningCosmWasmClient.connectWithSigner(
    `http://${rpc}`,
    wallet,
  );
  return client;
}

export async function ibcTransfer(
  suite: ITestSuite,
  client: SigningStargateClient,
  token: Coin,
  sender: string,
  receiver: string,
): Promise<void> {
  const timeoutTimestamp: bigint = BigInt(
    (Date.now() + 5 * 60 * 60 * 1000) * 1e6,
  );

  const typeUrl = "/ibc.applications.transfer.v1.MsgTransfer";

  const transferMsg: MsgTransferEncodeObject = {
    typeUrl,
    value: {
      sender,
      receiver,
      sourcePort: "transfer",
      sourceChannel: "channel-0",
      token,
      timeoutTimestamp,
    },
  };

  const price = suite.getRemoteGasPrices();
  const gas = calculateFee(500_000, price);

  await client.signAndBroadcast(sender, [transferMsg], gas);
}

export async function initGenericLstVault(
  suite: ITestSuite,
  client: HostClient,
  codeId: number,
  creator: string,
  lst_redemption_rate_oracle: string,
  lst_denom: string,
  lst_decimals: number,
  underlying_decimals: number,
): Promise<string> {
  const initMsg = {
    lst_redemption_rate_oracle,
    lst_denom,
    lst_decimals,
    underlying_decimals,
  };

  const res = await client.instantiate(
    creator,
    codeId,
    initMsg,
    "amulet-remote-pos",
    createFee(suite, 5_000_000),
    { funds: [coin(5_000_000, "untrn")] },
  );

  return res.contractAddress;
}
