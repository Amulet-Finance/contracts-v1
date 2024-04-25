import { test, expect } from "bun:test";
import { coin } from "@cosmjs/proto-signing";

import { Contract, HostChain, RemoteChain, Chain } from "./client";
import { e2eTestConfig } from "./config";
import { readContractFileBytes } from "./utils";

import { InstantiateMsg, Metadata, QueryMsg, ReconcileState, SharesAssetResponse, StateResponse } from "../ts/AmuletRemotePos.types";

const VALIDATOR_ADDR: string =
  "cosmosvaloper18hl5c9xn5dze2g50uaw0l2mr02ew57zk0auktn";
const ONE_HUNDRED_PERCENT_BPS: number = 10000;
const IBC_TRANSFER_TIMEOUT: number = 10 * 60 * 60; // 10 minutes
const ICTX_TIMEOUT: number = 60 * 60 * 60; // 1 hour
const FEE_PAYMENT_COOLDOWN: number = 10;
const FEE_BPS_BLOCK_INCREMENT: number = 10;
const MAX_FEE_BPS: number = 2500;
const ICQ_UPDATE_INTERVAL: number = 2;
const REMOTE_DENOM: string = "uatom";
const REMOTE_DENOM_DECIMALS: number = 6;
const UNBONDING_PERIOD: number = 600;
const MAX_UNBONDING_ENTRIES: number = 7;
const ESTIMATED_BLOCK_INTERVAL: number = 3;

const config = e2eTestConfig();

const wasmBytes: Uint8Array = await readContractFileBytes(`${__dirname}/../artifacts/amulet-remote-pos.wasm`);

const disconnectedHostChain = await HostChain.create(
  config.HOST_CHAIN_PREFIX,
  config.WALLET_MNEMONIC,
  config.HOST_CHAIN_GAS_PRICE,
);

const hostChain = await disconnectedHostChain.connect(config.HOST_CHAIN_RPC);

const disconnectedRemoteChain = await RemoteChain.create(
  config.REMOTE_CHAIN_PREFIX,
  config.WALLET_MNEMONIC,
  config.REMOTE_CHAIN_GAS_PRICE,
);

const remoteChain = await disconnectedRemoteChain.connect(config.REMOTE_CHAIN_RPC);

const metadataQuery: QueryMsg = { metadata: {} };
const vaultStateQuery: QueryMsg = { state: {} };
const reconcileStateQuery: QueryMsg = { reconcile_state: {} };
const sharesAssetQuery: QueryMsg = { shares_asset: {} };

const reconcile = async (chain: Chain, reconciler: string, contract: Contract, fee_recipient?: string | null) => {
  let state: ReconcileState = await contract.query(reconcileStateQuery);

  await contract.execute({ reconcile: { fee_recipient } }, reconciler, 500_000, coin(state.cost, "untrn"));

  console.log("reconciling!");

  while(true) {
    state = await contract.query(reconcileStateQuery);

    if (state.state === "idle") {
      console.log("done!");
      return;
    }

    if (state.state === "failed") {
      throw new Error("reconcile failed");
    }

    await chain.nextBlock();

    Bun.write(Bun.stdout, '.');
  }
};

const waitNSecs = async (chain: Chain, n_secs: number) => {
    const start = Date.now();

    while ((Date.now() - start) < (n_secs * 1000)) {
      await chain.nextBlock();

      Bun.write(Bun.stdout, '.');
    }

    console.log("done!");
}

test("remote-pos-vault", async () => {
  const [codeId] = await hostChain.uploadWasm(wasmBytes);

  console.log(`uploaded remote pos vault contract: ${codeId}`);

  const creator = await hostChain.accountAddress();

  const initMsg: InstantiateMsg = {
    connection_id: "connection-0",
    estimated_block_interval_seconds: ESTIMATED_BLOCK_INTERVAL,
    fee_bps_block_increment: FEE_BPS_BLOCK_INCREMENT,
    fee_payment_cooldown_blocks: FEE_PAYMENT_COOLDOWN,
    icq_update_interval: ICQ_UPDATE_INTERVAL,
    initial_validator_set: [ VALIDATOR_ADDR ],
    initial_validator_weights: [ ONE_HUNDRED_PERCENT_BPS ],
    interchain_tx_timeout_seconds:  ICTX_TIMEOUT,
    max_fee_bps: MAX_FEE_BPS,
    max_unbonding_entries: MAX_UNBONDING_ENTRIES,
    remote_denom: REMOTE_DENOM,
    remote_denom_decimals: REMOTE_DENOM_DECIMALS,
    transfer_in_channel: "channel-0",
    transfer_in_timeout_seconds: IBC_TRANSFER_TIMEOUT,
    transfer_out_channel: "channel-0",
    transfer_out_timeout_seconds: IBC_TRANSFER_TIMEOUT,
    unbonding_period: UNBONDING_PERIOD,
  };

  const [vault] = await hostChain.initContract(codeId, initMsg, "remote-pos-vault", 5_000_000);

  console.log(`vault contract: ${vault.address}`);

  console.log("waiting for ica/icq registration to complete:");
  
  while(true) {
    let metadata: Metadata = await vault.query(metadataQuery);

    if (metadata.main_ica_address 
      && metadata.rewards_ica_address 
      && metadata.delegations_icq 
      && metadata.main_ica_balance_icq 
      && metadata.rewards_ica_balance_icq) {
      console.log("done!");
      console.log(`main ica address: ${metadata.main_ica_address}`);
      console.log(`rewards ica address: ${metadata.rewards_ica_address}`);
      console.log(`main ica delegations icq id: ${metadata.delegations_icq}`);
      console.log(`main ica balance icq id: ${metadata.main_ica_balance_icq}`);
      console.log(`rewards ica balance icq id: ${metadata.rewards_ica_balance_icq}`);
      break;
    }

    await hostChain.nextBlock();

    Bun.write(Bun.stdout, '.');
  }

  await remoteChain.ibcTransfer("channel-0", creator, 100_000_000_000, REMOTE_DENOM);

  var ibcDepositAsset = "";

  {
    const metadata: Metadata = await vault.query(metadataQuery);

    ibcDepositAsset = metadata.ibc_deposit_asset;

    console.log(`waiting for receipt of: ${ibcDepositAsset}`);

    while(true) {
      const balance = await hostChain.balanceOf(creator, ibcDepositAsset);

      if (balance) {
        console.log("done!");
        break;
      }

      await hostChain.nextBlock();

      Bun.write(Bun.stdout, '.');
    }
  }

  console.log("making initial deposit");

  await vault.execute({ deposit: {} }, creator, 500_000, coin(1_000 * 10**6, ibcDepositAsset));

  {
    const metadata: Metadata = await vault.query(metadataQuery);
    const vaultState: StateResponse = await vault.query(vaultStateQuery);

    expect(+metadata.pending_deposit).toBe(1_000 * 10**6);
    expect(+vaultState.total_deposits).toBe(1_000 * 10**6);
    expect(+vaultState.total_issued_shares).toBe(1_000 * 10**18);
  }

  {
    await reconcile(hostChain, creator, vault);

    const metadata: Metadata = await vault.query(metadataQuery);
    const vaultState: StateResponse = await vault.query(vaultStateQuery);

    expect(+metadata.pending_deposit).toBe(0);
    expect(+metadata.delegated).toBe(1_000 * 10**6);
    expect(+vaultState.total_deposits).toBe(1_000 * 10**6);
  }

  console.log(`wait ${FEE_PAYMENT_COOLDOWN} blocks for rewards to accrue`);
  
  {
    const height = await hostChain.nextBlock();

    while ((await hostChain.nextBlock() - height) < (FEE_PAYMENT_COOLDOWN)) {
      Bun.write(Bun.stdout, '.');
    }

    console.log("done!");
  }

  console.log("reconciling to trigger reward distribution");

  await reconcile(hostChain, creator, vault);

  console.log(`wait ${FEE_PAYMENT_COOLDOWN + 5} blocks for fee payment cooldown to expire and ICQs to update`);
  
  {
    const height = await hostChain.nextBlock();

    while ((await hostChain.nextBlock() - height) < (FEE_PAYMENT_COOLDOWN)) {
      Bun.write(Bun.stdout, '.');
    }

    console.log("done!");
  }

  const feeRecipient = await remoteChain.accountAddress();

  console.log("reconciling for a fee");
  
  {
    const feeRecipientStartingBalance = await remoteChain.balanceOf(feeRecipient, REMOTE_DENOM) || 0;
    const startingMetadata: Metadata = await vault.query(metadataQuery);

    await reconcile(hostChain, creator, vault, feeRecipient);

    const feeRecipientEndingBalance = await remoteChain.balanceOf(feeRecipient, REMOTE_DENOM) || 0;
    const endingMetadata: Metadata = await vault.query(metadataQuery);
    const vaultState: StateResponse = await vault.query(vaultStateQuery);

    expect(BigInt(feeRecipientEndingBalance) - BigInt(feeRecipientStartingBalance)).toBeGreaterThan(0n);
    expect(BigInt(endingMetadata.delegated) - BigInt(startingMetadata.delegated)).toBeGreaterThan(0n);
    expect(+vaultState.total_deposits).toBe(+endingMetadata.delegated);
  }

  console.log("redeeming 50% of shares")

  const sharesTokenResponse: SharesAssetResponse = await vault.query(sharesAssetQuery);

  const sharesAsset = sharesTokenResponse.denom;

  var expectedInitialUnbond = 0;

  {
    await vault.execute(
      { redeem: { recipient: creator }}, 
      creator, 
      500_000, 
      coin((500n * 10n**18n).toString(), sharesAsset)
    );

    const preRecMetadata: Metadata = await vault.query(metadataQuery);
    const preRecVaultState: StateResponse = await vault.query(vaultStateQuery);

    expect(+preRecMetadata.pending_unbond).toBe(Math.floor(+preRecMetadata.delegated / 2));
    expect(+preRecVaultState.total_deposits).toBe(+preRecMetadata.delegated - +preRecMetadata.pending_unbond);

    expectedInitialUnbond = +preRecMetadata.pending_unbond;

    console.log("reconciling to clear pending unbond");

    await reconcile(hostChain, creator, vault);

    const postRecMetadata: Metadata = await vault.query(metadataQuery);
    const postRecVaultState: StateResponse = await vault.query(vaultStateQuery);

    let rewardsEarned = +postRecMetadata.delegated - Math.floor(+preRecMetadata.delegated / 2);

    expect(rewardsEarned).toBeLessThan(+preRecMetadata.delegated / 2);
    expect(+postRecMetadata.delegated).toBeLessThan(+preRecMetadata.delegated);
    expect(+postRecMetadata.delegated).toBe(Math.floor(+preRecMetadata.delegated / 2) + rewardsEarned);
    expect(+postRecMetadata.pending_unbond).toBe(0);
    expect(+postRecVaultState.total_deposits).toBe(+postRecMetadata.delegated);
  }

  console.log(`waiting ${(UNBONDING_PERIOD + (3 * ICQ_UPDATE_INTERVAL)) / 60} minutes for unbond to complete`);

  await waitNSecs(hostChain, (UNBONDING_PERIOD + (3 * ICQ_UPDATE_INTERVAL)));

  console.log("reconciling to retreive unbonded assets");
  
  await reconcile(hostChain, creator, vault);

  {
    const metadata: Metadata = await vault.query(metadataQuery);
    const balance = await hostChain.balanceOf(vault.address, ibcDepositAsset) || 0;

    expect(+metadata.total_actual_unbonded).toBe(expectedInitialUnbond);
    expect(+metadata.total_expected_unbonded).toBe(expectedInitialUnbond);
    expect(+(metadata.unbonding_issued_count || 0)).toBe(1);
    expect(+(metadata.unbonding_ack_count || 0)).toBe(1);
    expect(balance).toBe(BigInt(expectedInitialUnbond));
  }

  console.log("redeeming 50% of shares again, in two batches");

  {
    var currentExpectedUnbond = 0;

    console.log("redeeming first batch");
    
    await vault.execute(
      { redeem: { recipient: creator }}, 
      creator, 
      500_000, 
      coin((125n * 10n**18n).toString(), sharesAsset)
    );

    {
      const metadata: Metadata = await vault.query(metadataQuery);
      currentExpectedUnbond += +metadata.pending_unbond;
    }

    await reconcile(hostChain, creator, vault);

    console.log("redeeming second batch");

    await vault.execute(
      { redeem: { recipient: creator }}, 
      creator, 
      500_000, 
      coin((125n * 10n**18n).toString(), sharesAsset)
    );

    const preUnbondMetadata: Metadata = await vault.query(metadataQuery);

    expect(+preUnbondMetadata.pending_unbond).toBe(0);

    console.log(`waiting ${+preUnbondMetadata.minimum_unbond_interval / 60} minutes for minimum unbond interval to expire`);

    await waitNSecs(hostChain, +preUnbondMetadata.minimum_unbond_interval);

    console.log("starting pending unbonding batch");

    await vault.execute({ start_unbond: {} }, creator, 500_000);

    {
      const metadata: Metadata = await vault.query(metadataQuery);
      expect(+metadata.pending_unbond).toBeGreaterThan(0);
      currentExpectedUnbond += +metadata.pending_unbond;
    }

    console.log("reconciling to clear pending unbond");

    await reconcile(hostChain, creator, vault);

    console.log(`waiting ${(UNBONDING_PERIOD + (3 * ICQ_UPDATE_INTERVAL)) / 60} minutes for unbond to complete`);

    await waitNSecs(hostChain, (UNBONDING_PERIOD + (3 * ICQ_UPDATE_INTERVAL)));

    console.log("reconciling to retreive unbonded assets");
  
    await reconcile(hostChain, creator, vault);

    {
      const metadata: Metadata = await vault.query(metadataQuery);
      const balance = await hostChain.balanceOf(vault.address, ibcDepositAsset) || 0;

      expect(+metadata.total_actual_unbonded).toBe(expectedInitialUnbond + currentExpectedUnbond);
      expect(+metadata.total_expected_unbonded).toBe(expectedInitialUnbond + currentExpectedUnbond);
      expect(+(metadata.unbonding_issued_count || 0)).toBe(3);
      expect(+(metadata.unbonding_ack_count || 0)).toBe(3);
      expect(balance).toBe(BigInt(expectedInitialUnbond + currentExpectedUnbond));
    }
  }
});

