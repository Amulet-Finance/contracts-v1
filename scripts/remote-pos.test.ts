import { describe, it, beforeAll, afterAll, expect } from "bun:test";
import { TestSuite } from "./suite";
import { artifact, readContractFileBytes } from "./utils";
import {
  StakingExtension,
  QueryClient as StargateQueryClient,
  setupStakingExtension,
  BankExtension,
  setupBankExtension,
  StdFee,
  calculateFee,
  SigningStargateClient,
  MsgTransferEncodeObject,
} from "@cosmjs/stargate";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { Tendermint37Client } from "@cosmjs/tendermint-rpc";
import { Validator } from "cosmjs-types/cosmos/staking/v1beta1/staking";
import { Coin, DirectSecp256k1HdWallet, coin } from "@cosmjs/proto-signing";
import {
  Config,
  DepositAssetResponse,
  ExecuteMsg,
  InstantiateMsg,
  Metadata,
  ReconcileState,
  SharesAssetResponse,
  StateResponse,
  ValidatorSet,
} from "../ts/AmuletRemotePos.types";
import { GENESIS_ALLOCATION } from "./suite/constants";

type Wallet = DirectSecp256k1HdWallet;
type QueryClient = StakingExtension & BankExtension;
type HostClient = SigningCosmWasmClient;
type RemoteClient = SigningStargateClient;

const TOTAL_VALIDATOR_COUNT = 10;
const UNBONDING_PERIOD_SECS = 600;
const IBC_TRANSFER_AMOUNT = Math.floor(GENESIS_ALLOCATION * 0.9); // 90% of genesis allocation
const VALIDATOR_LIQUID_STAKE_CAP = 0.5;
const VALIDATOR_BALANCE = GENESIS_ALLOCATION / 10;

async function createQueryClient(rpc: string): Promise<QueryClient> {
  const tmClient = await Tendermint37Client.connect(`http://${rpc}`);
  const qClient = StargateQueryClient.withExtensions(
    tmClient,
    setupStakingExtension,
    setupBankExtension,
  );
  return qClient;
}

async function queryValidators(client: QueryClient): Promise<Validator[]> {
  const bondedValidators =
    await client.staking.validators("BOND_STATUS_BONDED");
  const unbondedValidators = await client.staking.validators(
    "BOND_STATUS_UNBONDED",
  );
  const unbondingValidators = await client.staking.validators(
    "BOND_STATUS_UNBONDING",
  );

  return [
    ...bondedValidators.validators,
    ...unbondedValidators.validators,
    ...unbondingValidators.validators,
  ];
}

async function createWallet(mnemonic: string, prefix: string): Promise<Wallet> {
  const wallet = await DirectSecp256k1HdWallet.fromMnemonic(mnemonic, {
    prefix,
  });
  return wallet;
}

async function createRemoteWallet(
  suite: ITestSuite,
  wallet_id: string,
): Promise<Wallet> {
  const mnemonic = suite.getWalletMnemonics()[wallet_id];
  const prefix = suite.getRemotePrefix();
  return createWallet(mnemonic, prefix);
}

async function createHostWallet(
  suite: ITestSuite,
  wallet_id: string,
): Promise<Wallet> {
  const mnemonic = suite.getWalletMnemonics()[wallet_id];
  const prefix = suite.getHostPrefix();
  return createWallet(mnemonic, prefix);
}

async function createRemoteClient(
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

async function createHostClient(
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

async function ibcTransfer(
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

async function queryVaultMetadata(
  client: HostClient,
  vault: string,
): Promise<Metadata> {
  return client.queryContractSmart(vault, { metadata: {} });
}

async function queryVaultDepositsState(
  client: HostClient,
  vault: string,
): Promise<StateResponse> {
  return client.queryContractSmart(vault, { state: {} });
}

async function queryVaultReconcileState(
  client: HostClient,
  vault: string,
): Promise<ReconcileState> {
  return client.queryContractSmart(vault, { reconcile_state: {} });
}

async function reconcileVault(
  client: HostClient,
  vault: string,
  reconciler: string,
  fee_recipient?: string,
  expectedFailurePhase?: string,
): Promise<[string, string]> {
  const initialState = await queryVaultReconcileState(client, vault);

  const msg: ExecuteMsg = {
    reconcile: fee_recipient ? { fee_recipient } : {},
  };

  await client.execute(reconciler, vault, msg, gasFee, "", [
    coin(initialState.cost, "untrn"),
  ]);

  const expiry = Date.now() + 30_000;

  while (Date.now() < expiry) {
    const state = await queryVaultReconcileState(client, vault);

    if (state.state == "failed") {
      // return the reconcile state if an expected failure occurs
      if (expectedFailurePhase && state.phase == expectedFailurePhase)
        return [state.phase, state.state];

      // otherwise retry in the time remaining, could be a spurious IBC thing
      await client.execute(reconciler, vault, msg, gasFee, "", [
        coin(state.cost, "untrn"),
      ]);
    }

    // when it gets back to the start, return the reconcile state
    if (state.phase == "start_reconcile" && state.state == "idle")
      return [state.phase, state.state];
  }

  throw new Error("timeout waiting for reconciliation");
}

async function forceNextVault(
  client: HostClient,
  vault: string,
  reconciler: string,
  expectedFailurePhase?: string,
): Promise<[string, string]> {
  const initialState = await queryVaultReconcileState(client, vault);

  await client.execute(reconciler, vault, { force_next: {} }, gasFee, "", [
    coin(initialState.cost, "untrn"),
  ]);

  const expiry = Date.now() + 30_000;

  while (Date.now() < expiry) {
    const state = await queryVaultReconcileState(client, vault);

    if (state.state == "failed") {
      // return the reconcile state if an expected failure occurs
      if (expectedFailurePhase && state.phase == expectedFailurePhase)
        return [state.phase, state.state];

      // otherwise try to reconcile, could be a spurious IBC thing
      await client.execute(reconciler, vault, { reconcile: {} }, gasFee, "", [
        coin(state.cost, "untrn"),
      ]);
    }

    // when it gets back to the start, return the reconcile state
    if (state.phase == "start_reconcile" && state.state == "idle")
      return [state.phase, state.state];
  }

  throw new Error("timeout waiting for reconciliation after force-next");
}

async function instantiateVault(
  client: HostClient,
  codeId: number,
  creator: string,
  initial_validator_set: string[],
  initial_validator_weights: number[],
): Promise<string> {
  const initMsg: InstantiateMsg = {
    connection_id: "connection-0",
    estimated_block_interval_seconds: 1,
    fee_bps_block_increment: 1,
    fee_payment_cooldown_blocks: 100,
    icq_update_interval: 5,
    initial_validator_set,
    initial_validator_weights,
    interchain_tx_timeout_seconds: 60 * 60 * 60,
    max_fee_bps: 200,
    max_unbonding_entries: 7,
    remote_denom: "stake",
    remote_denom_decimals: 6,
    transfer_in_channel: "channel-0",
    transfer_in_timeout_seconds: 60 * 60 * 60,
    transfer_out_channel: "channel-0",
    transfer_out_timeout_seconds: 60 * 60 * 60,
    unbonding_period: UNBONDING_PERIOD_SECS,
  };

  const res = await client.instantiate(
    creator,
    codeId,
    initMsg,
    "amulet-remote-pos",
    gasFee,
    { funds: [coin(5_000_000, "untrn")] },
  );

  const timeoutExpiry = Date.now() + 30_000;

  while (Date.now() < timeoutExpiry) {
    const metadata = await queryVaultMetadata(
      operatorClient,
      res.contractAddress,
    );

    if (
      metadata.delegations_icq != null &&
      metadata.main_ica_balance_icq != null &&
      metadata.rewards_ica_balance_icq != null
    ) {
      return res.contractAddress;
    }

    await Bun.sleep(1000);
  }

  throw new Error("timeout waiting for ICA/ICQ setup to complete");
}

function createFee(suite: ITestSuite, amount: number): StdFee {
  const price = suite.getHostGasPrices();
  return calculateFee(amount, price);
}

let suite: ITestSuite;
let validators: Validator[];
let remoteQueryClient: QueryClient;
let hostQueryClient: QueryClient;
let operatorAddress: string;
let aliceAddress: string;
let bobAddress: string;
let operatorClient: SigningCosmWasmClient;
let aliceClient: SigningCosmWasmClient;
let bobClient: SigningCosmWasmClient;
let codeId: number;
let vaultOneAddress: string;
let gasFee: StdFee;
let depositAssetDenom: string;

describe("Remote Proof-of-Stake Vault tests", () => {
  beforeAll(async () => {
    suite = await TestSuite.create({
      networkOverrides: {
        gaia: {
          validators: TOTAL_VALIDATOR_COUNT,
          validators_balance: new Array(TOTAL_VALIDATOR_COUNT).fill(
            String(GENESIS_ALLOCATION / 10),
          ),
          genesis_opts: {
            "app_state.staking.params.unbonding_time": `${UNBONDING_PERIOD_SECS}s`,
            "app_state.staking.params.validator_liquid_staking_cap": `${VALIDATOR_LIQUID_STAKE_CAP}`,
          },
        },
        neutron: {
          genesis_opts: {
            "app_state.interchaintxs.params.msg_submit_tx_max_messages": String(
              TOTAL_VALIDATOR_COUNT / 2,
            ),
            "app_state.feeburner.params.treasury_address":
              // aribitrarily picked testnet address
              "neutron12z4p3g6zjrnlz79znrjef4sxklsnnmpglgzhx2",
          },
        },
      },
      relayerOverrides: {
        hermes: {
          config: {
            "chains.1.trusting_period": `${UNBONDING_PERIOD_SECS / 2}s`,
            "chains.0.trusting_period": `${UNBONDING_PERIOD_SECS / 2}s`,
          },
        },
      },
    });

    const operatorWallet = await createHostWallet(suite, "demo1");
    const aliceWallet = await createHostWallet(suite, "demo2");
    const bobWallet = await createHostWallet(suite, "demo3");

    operatorAddress = (await operatorWallet.getAccounts())[0].address;
    aliceAddress = (await aliceWallet.getAccounts())[0].address;
    bobAddress = (await bobWallet.getAccounts())[0].address;

    operatorClient = await createHostClient(suite, operatorWallet);
    aliceClient = await createHostClient(suite, aliceWallet);
    bobClient = await createHostClient(suite, bobWallet);

    remoteQueryClient = await createQueryClient(suite.getRemoteRpc());
    hostQueryClient = await createQueryClient(suite.getHostRpc());

    validators = await queryValidators(remoteQueryClient);
    gasFee = createFee(suite, 5_000_000);
  });

  afterAll(async () => {
    await suite.cleanup();
  });

  it("should upload the amulet-remote-pos vault contract byte code", async () => {
    const wasmFilePath = artifact("amulet-remote-pos");
    const wasmBytes = await readContractFileBytes(wasmFilePath);
    const res = await operatorClient.upload(operatorAddress, wasmBytes, gasFee);

    expect(res.codeId).toBe(1);

    codeId = res.codeId;
  });

  it("should deploy the first amulet-remote-pos vault using all but the last validator", async () => {
    let initial_validator_set = validators
      .slice(0, -1)
      .map((v) => v.operatorAddress);

    let initial_validator_weights = new Array(TOTAL_VALIDATOR_COUNT - 1).fill(
      Math.floor(10_000 / TOTAL_VALIDATOR_COUNT - 1),
    );

    initial_validator_weights[0] +=
      10_000 - initial_validator_weights.reduce((acc, val) => acc + val, 0);

    vaultOneAddress = await instantiateVault(
      operatorClient,
      codeId,
      operatorAddress,
      initial_validator_set,
      initial_validator_weights,
    );
  });

  it("should transfer remote staking balances to host chain & vault deposit asset matches IBC denom", async () => {
    const accounts = [
      ["demo1", operatorAddress],
      ["demo2", aliceAddress],
      ["demo3", bobAddress],
    ];

    for (const [id, receiver] of accounts) {
      const remoteWallet = await createRemoteWallet(suite, id);
      const remoteAddress = (await remoteWallet.getAccounts())[0].address;
      const remoteClient = await createRemoteClient(suite, remoteWallet);

      await ibcTransfer(
        suite,
        remoteClient,
        coin(IBC_TRANSFER_AMOUNT, "stake"),
        remoteAddress,
        receiver,
      );
    }

    const depositAsset: DepositAssetResponse =
      await operatorClient.queryContractSmart(vaultOneAddress, {
        deposit_asset: {},
      });

    const timeoutExpiry = Date.now() + 10_000;

    while (Date.now() < timeoutExpiry) {
      const operatorBalance = await hostQueryClient.bank.balance(
        operatorAddress,
        depositAsset.denom,
      );

      const aliceBalance = await hostQueryClient.bank.balance(
        aliceAddress,
        depositAsset.denom,
      );

      const bobBalance = await hostQueryClient.bank.balance(
        bobAddress,
        depositAsset.denom,
      );

      if (
        +operatorBalance.amount == IBC_TRANSFER_AMOUNT &&
        +aliceBalance.amount == IBC_TRANSFER_AMOUNT &&
        +bobBalance.amount == IBC_TRANSFER_AMOUNT
      ) {
        depositAssetDenom = depositAsset.denom;
        return;
      }

      await Bun.sleep(1000);
    }

    throw new Error("timeout waiting for IBC transfer to complete");
  });

  it("alice makes the initial deposit for vault 1, shares received and pending deposits increase", async () => {
    const depositAmount = VALIDATOR_BALANCE / 10;

    await aliceClient.execute(
      aliceAddress,
      vaultOneAddress,
      { deposit: {} },
      gasFee,
      "",
      [coin(depositAmount, depositAssetDenom)],
    );

    const metadata = await queryVaultMetadata(operatorClient, vaultOneAddress);

    expect(+metadata.pending_deposit).toBe(depositAmount);

    const sharesAsset: SharesAssetResponse =
      await operatorClient.queryContractSmart(vaultOneAddress, {
        shares_asset: {},
      });

    const aliceSharesBalance = await hostQueryClient.bank.balance(
      aliceAddress,
      sharesAsset.denom,
    );

    expect(+aliceSharesBalance.amount).toBe(depositAmount * 10 ** 12);

    const vaultDepositState = await queryVaultDepositsState(
      operatorClient,
      vaultOneAddress,
    );

    expect(+vaultDepositState.total_deposits).toBe(depositAmount);
    expect(+vaultDepositState.total_issued_shares).toBe(
      depositAmount * 10 ** 12,
    );
  });

  it("bob donates some deposit assets to vault 1 and pending deposits increase", async () => {
    const donateAmount = VALIDATOR_BALANCE / 100;

    const preDonateMetadata = await queryVaultMetadata(
      operatorClient,
      vaultOneAddress,
    );

    const preDonateDepositState = await queryVaultDepositsState(
      operatorClient,
      vaultOneAddress,
    );

    await bobClient.execute(
      bobAddress,
      vaultOneAddress,
      { donate: {} },
      gasFee,
      "",
      [coin(donateAmount, depositAssetDenom)],
    );

    const postDonateMetadata = await queryVaultMetadata(
      operatorClient,
      vaultOneAddress,
    );

    const postDonateDepositState = await queryVaultDepositsState(
      operatorClient,
      vaultOneAddress,
    );

    expect(+postDonateMetadata.pending_deposit).toBe(
      +preDonateMetadata.pending_deposit + donateAmount,
    );

    expect(+postDonateDepositState.total_deposits).toBe(
      +preDonateDepositState.total_deposits + donateAmount,
    );

    expect(postDonateDepositState.total_issued_shares).toBe(
      preDonateDepositState.total_issued_shares,
    );
  });

  it("initial reconciliation of vault 1 transfers and stakes pending deposits", async () => {
    const preReconcileMetadata = await queryVaultMetadata(
      operatorClient,
      vaultOneAddress,
    );

    expect(+preReconcileMetadata.delegated).toBe(0);

    await reconcileVault(operatorClient, vaultOneAddress, operatorAddress);

    const postReconcileMetadata = await queryVaultMetadata(
      operatorClient,
      vaultOneAddress,
    );

    expect(+postReconcileMetadata.pending_deposit).toBe(0);
    expect(+postReconcileMetadata.delegated).toBe(
      +preReconcileMetadata.pending_deposit,
    );

    const delegations = await remoteQueryClient.staking.delegatorDelegations(
      postReconcileMetadata.main_ica_address || "",
    );

    expect(delegations.delegationResponses.length).toBe(9);

    const totalActualDelegated = delegations.delegationResponses.reduce(
      (acc, d) => acc + +d.balance.amount,
      0,
    );

    expect(totalActualDelegated).toBe(+postReconcileMetadata.delegated);
  });

  it("reconciliation failure due to liquid stake capacity can be resolved via force-next & redelegation", async () => {
    const preRedelegationValSet: ValidatorSet =
      await operatorClient.queryContractSmart(vaultOneAddress, {
        validator_set: {},
      });

    const lastSlotValidatorAddr =
      preRedelegationValSet.validators[
        preRedelegationValSet.validators.length - 1
      ];

    const lastSlotValidator = await remoteQueryClient.staking.validator(
      lastSlotValidatorAddr,
    );

    // https://github.com/cosmos/cosmos-sdk/blob/feature/v0.47.x-ics-lsm/x/staking/keeper/liquid_stake.go#L107
    // To get the the liquid staking limit: I = (L - CT)/(C - 1), where:
    //   I is the liquid stake increase
    //   L is the current liquid staked amount
    //   T is the current total staked amount
    //   C is the liquid staking cap

    const T = +lastSlotValidator.validator.tokens;
    const L = +lastSlotValidator.validator.tokens - VALIDATOR_BALANCE;
    const stakeIncrease = Math.floor(
      (L - VALIDATOR_LIQUID_STAKE_CAP * T) / (VALIDATOR_LIQUID_STAKE_CAP - 1),
    );

    // instantiate a vault that will only delegate to the last validator in vault one's set
    const greedyVaultAddr = await instantiateVault(
      operatorClient,
      codeId,
      operatorAddress,
      [lastSlotValidatorAddr],
      [10_000],
    );

    // delegate enough to exhaust the liquid stake capacity for the validator
    await operatorClient.execute(
      operatorAddress,
      greedyVaultAddr,
      { deposit: {} },
      gasFee,
      "",
      [coin(stakeIncrease, depositAssetDenom)],
    );

    await reconcileVault(operatorClient, greedyVaultAddr, operatorAddress);

    // now try to deposit more assets into vault one, it should fail in the second delegate batch
    const depositAmount = IBC_TRANSFER_AMOUNT / 10;

    await aliceClient.execute(
      aliceAddress,
      vaultOneAddress,
      { deposit: {} },
      gasFee,
      "",
      [coin(depositAmount, depositAssetDenom)],
    );

    const [_, state] = await reconcileVault(
      operatorClient,
      vaultOneAddress,
      operatorAddress,
      operatorAddress,
      "delegate",
    );

    expect(state).toBe("failed");

    const postFailureMetadata = await queryVaultMetadata(
      operatorClient,
      vaultOneAddress,
    );

    expect(postFailureMetadata.msg_success_count).toBe(
      TOTAL_VALIDATOR_COUNT / 2,
    );

    // force the vault out of the stuck delegate phase
    await forceNextVault(operatorClient, vaultOneAddress, operatorAddress);

    // redelegate away from the validator with no more capacity
    await operatorClient.execute(
      operatorAddress,
      vaultOneAddress,
      {
        redelegate_slot: {
          slot: 8,
          validator: validators[validators.length - 1].operatorAddress,
        },
      },
      gasFee,
      "",
      [coin(1_000_000, "untrn")],
    );

    const postRedelegationValSet: ValidatorSet =
      await operatorClient.queryContractSmart(vaultOneAddress, {
        validator_set: {},
      });

    expect(postRedelegationValSet.pending_redelegation_slot).toBe(8);
    expect(postRedelegationValSet.pending_redelegate_to).toBe(
      validators[validators.length - 1].operatorAddress,
    );

    const postRedelegationMetadata = await queryVaultMetadata(
      operatorClient,
      vaultOneAddress,
    );

    expect(postRedelegationMetadata.next_delegations_icq).toBeNumber();

    // wait for 10 blocks to allow for delegations icq to update
    const targetHeight = (await operatorClient.getBlock()).header.height + 10;

    while (true) {
      const block = await operatorClient.getBlock();

      if (block.header.height > targetHeight) break;
    }

    // the next reconcile sequence should be successful
    await reconcileVault(operatorClient, vaultOneAddress, operatorAddress);

    const postReconcileMetadata = await queryVaultMetadata(
      operatorClient,
      vaultOneAddress,
    );

    expect(+postReconcileMetadata.pending_deposit).toBe(0);
    expect(+postReconcileMetadata.delegated).toBe(
      +postRedelegationMetadata.delegated +
        +postRedelegationMetadata.inflight_deposit,
    );

    const postReconcileValSet: ValidatorSet =
      await operatorClient.queryContractSmart(vaultOneAddress, {
        validator_set: {},
      });

    expect(postReconcileValSet.pending_redelegation_slot).toBeNull();
    expect(postReconcileValSet.pending_redelegate_to).toBeNull();
    expect(postReconcileValSet.validators[8]).toBe(
      postRedelegationValSet.pending_redelegate_to || "",
    );

    const actualDelegations =
      await remoteQueryClient.staking.delegatorDelegations(
        postReconcileMetadata.main_ica_address || "",
      );

    expect(actualDelegations.delegationResponses.length).toBe(9);
    expect(
      actualDelegations.delegationResponses[8].delegation.validatorAddress,
    ).toBe(postRedelegationValSet.pending_redelegate_to || "");
  });

  it("reconciling after waiting for the fee cooldown to expire results in a fee taken from rewards", async () => {
    const preReconcileMetadata = await queryVaultMetadata(
      operatorClient,
      vaultOneAddress,
    );

    const config: Config = await operatorClient.queryContractSmart(
      vaultOneAddress,
      {
        config: {},
      },
    );

    if (!preReconcileMetadata.last_reconcile_height) {
      throw new Error("No initital reconciliation performed");
    }

    const targetHeight =
      preReconcileMetadata.last_reconcile_height +
      config.fee_payment_cooldown_blocks +
      10;

    // wait for cooldown to expire + 10 blocks
    while (true) {
      const block = await operatorClient.getBlock();

      if (block.header.height > targetHeight) break;
    }

    const recipientWallet = await DirectSecp256k1HdWallet.generate(12, {
      prefix: suite.getRemotePrefix(),
    });

    const recipientAddress = (await recipientWallet.getAccounts())[0].address;

    await reconcileVault(
      operatorClient,
      vaultOneAddress,
      operatorAddress,
      recipientAddress,
    );

    const postReconcileMetadata = await queryVaultMetadata(
      operatorClient,
      vaultOneAddress,
    );

    expect(+postReconcileMetadata.delegated).toBeGreaterThan(
      +preReconcileMetadata.delegated,
    );

    const feeRecipientBalance = await remoteQueryClient.bank.balance(
      recipientAddress,
      "stake",
    );

    expect(+feeRecipientBalance.amount).toBeGreaterThan(0);
  });
});
