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
  DepositAssetResponse,
  InstantiateMsg as VaultInstantiateMsg,
} from "../ts/AmuletRemotePos.types";
import { ExecuteMsg as MintExecuteMsg } from "../ts/AmuletMint.types";
import {
  ExecuteMsg as HubExecuteMsg,
  InstantiateMsg as HubInstantiateMsg,
  PositionResponse,
  VaultMetadata,
} from "../ts/AmuletHub.types";
import {
  InstantiateMsg as ProxyInstantiateMsg,
  ProxyExecuteMsg,
  WhitelistedResponse,
} from "../ts/WhitelistProxy.types";
import { GENESIS_ALLOCATION } from "./suite/constants";

type Wallet = DirectSecp256k1HdWallet;
type QueryClient = StakingExtension & BankExtension;
type HostClient = SigningCosmWasmClient;
type RemoteClient = SigningStargateClient;

const TOTAL_VALIDATOR_COUNT = 1;
const UNBONDING_PERIOD_SECS = 70;
const IBC_TRANSFER_AMOUNT = Math.floor(GENESIS_ALLOCATION * 0.9); // 90% of genesis allocation
const VALIDATOR_LIQUID_STAKE_CAP = 0.5;

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

async function instantiateVault(
  client: HostClient,
  codeId: number,
  creator: string,
  initial_validator_set: string[],
  initial_validator_weights: number[],
): Promise<string> {
  const initMsg: VaultInstantiateMsg = {
    connection_id: "connection-0",
    estimated_block_interval_seconds: 1,
    fee_bps_block_increment: 1,
    fee_payment_cooldown_blocks: 50,
    icq_update_interval: 2,
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

  return res.contractAddress;
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
let vaultCodeId: number;
let hubCodeId: number;
let mintCodeId: number;
let proxyCodeId: number;
let vaultAddress: string;
let hubAddress: string;
let mintAddress: string;
let proxyAddress: string;
let gasFee: StdFee;
let depositAssetDenom: string;

describe("Whitelist Proxy", () => {
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
            "app_state.slashing.params.slash_fraction_downtime": "0.5", // 50% slash for downtime (make it hard to miss)
          },
        },
        neutron: {
          genesis_opts: {
            "app_state.interchaintxs.params.msg_submit_tx_max_messages": "16",
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
    vaultCodeId = res.codeId;
  });

  it("should upload the amulet-hub contract byte code", async () => {
    const wasmFilePath = artifact("amulet-hub");
    const wasmBytes = await readContractFileBytes(wasmFilePath);
    const res = await operatorClient.upload(operatorAddress, wasmBytes, gasFee);
    hubCodeId = res.codeId;
  });

  it("should upload the amulet-mint contract byte code", async () => {
    const wasmFilePath = artifact("amulet-mint");
    const wasmBytes = await readContractFileBytes(wasmFilePath);
    const res = await operatorClient.upload(operatorAddress, wasmBytes, gasFee);
    mintCodeId = res.codeId;
  });

  it("should upload the whitelist-proxy contract byte code", async () => {
    const wasmFilePath = artifact("whitelist-proxy");
    const wasmBytes = await readContractFileBytes(wasmFilePath);
    const res = await operatorClient.upload(operatorAddress, wasmBytes, gasFee);
    proxyCodeId = res.codeId;
  });

  it("should deploy the amulet-remote-pos vault", async () => {
    let initial_validator_set = validators.map((v) => v.operatorAddress);

    let initial_validator_weights = [10_000];

    initial_validator_weights[0] +=
      10_000 - initial_validator_weights.reduce((acc, val) => acc + val, 0);

    vaultAddress = await instantiateVault(
      operatorClient,
      vaultCodeId,
      operatorAddress,
      initial_validator_set,
      initial_validator_weights,
    );
  });

  it("should deploy the amulet-mint", async () => {
    const res = await operatorClient.instantiate(
      operatorAddress,
      mintCodeId,
      {},
      "amulet-mint",
      gasFee,
    );

    mintAddress = res.contractAddress;
  });

  it("should deploy the amulet-hub", async () => {
    const msg: HubInstantiateMsg = { synthetic_mint: mintAddress };
    const res = await operatorClient.instantiate(
      operatorAddress,
      hubCodeId,
      msg,
      "amulet-hub",
      gasFee,
    );

    hubAddress = res.contractAddress;
  });

  it("should deploy the whitelist-proxy", async () => {
    const msg: ProxyInstantiateMsg = {
      hub_address: hubAddress,
    };
    const res = await operatorClient.instantiate(
      operatorAddress,
      proxyCodeId,
      msg,
      "whitelist-proxy",
      gasFee,
    );

    proxyAddress = res.contractAddress;
  });

  it("should create the amSTAKE synthetic", async () => {
    const msg: MintExecuteMsg = {
      create_synthetic: {
        decimals: 6,
        ticker: "amSTAKE",
      },
    };

    await operatorClient.execute(operatorAddress, mintAddress, msg, gasFee);
  });

  it("should whitelist the hub as a minter", async () => {
    const msg: MintExecuteMsg = {
      set_whitelisted: {
        minter: hubAddress,
        whitelisted: true,
      },
    };

    await operatorClient.execute(operatorAddress, mintAddress, msg, gasFee);
  });

  it("should register the vault with the hub and enable deposits/advance", async () => {
    {
      const msg: HubExecuteMsg = {
        register_vault: {
          vault: vaultAddress,
          synthetic: `factory/${mintAddress}/amstake`,
        },
      };

      await operatorClient.execute(operatorAddress, hubAddress, msg, gasFee);
    }
    {
      const msg: HubExecuteMsg = {
        set_deposits_enabled: {
          vault: vaultAddress,
          enabled: true,
        },
      };

      await operatorClient.execute(operatorAddress, hubAddress, msg, gasFee);
    }
    {
      const msg: HubExecuteMsg = {
        set_advance_enabled: {
          vault: vaultAddress,
          enabled: true,
        },
      };

      await operatorClient.execute(operatorAddress, hubAddress, msg, gasFee);
    }
  });

  it("should configure the deposit, mint, advance & redeem proxy for the vault", async () => {
    const msg: HubExecuteMsg = {
      set_proxy_config: {
        vault: vaultAddress,
        deposit: proxyAddress,
        mint: proxyAddress,
        advance: proxyAddress,
        redeem: proxyAddress,
      },
    };

    await operatorClient.execute(operatorAddress, hubAddress, msg, gasFee);
  });

  it("should whitelist alice", async () => {
    const msg: ProxyExecuteMsg = {
      set_whitelisted: {
        address: aliceAddress,
        whitelisted: true,
      },
    };

    await operatorClient.execute(operatorAddress, proxyAddress, msg, gasFee);
  });

  it("only the proxy admin can whitelist addresses", async () => {
    const msg: ProxyExecuteMsg = {
      set_whitelisted: {
        address: bobAddress,
        whitelisted: true,
      },
    };

    expect(async () => {
      await bobClient.execute(bobAddress, proxyAddress, msg, gasFee);
    }).toThrow("unauthorized");
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
      await operatorClient.queryContractSmart(vaultAddress, {
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

  it("alice makes a deposit via the proxy", async () => {
    const depositAmount = IBC_TRANSFER_AMOUNT / 10;

    await aliceClient.execute(
      aliceAddress,
      proxyAddress,
      { deposit: { vault: vaultAddress } },
      gasFee,
      "",
      [coin(depositAmount, depositAssetDenom)],
    );

    const position: PositionResponse = await operatorClient.queryContractSmart(
      hubAddress,
      { position: { account: aliceAddress, vault: vaultAddress } },
    );

    expect(+position.collateral).toBe(depositAmount);
  });

  it("alice mints synthetics via the proxy", async () => {
    const depositAmount = IBC_TRANSFER_AMOUNT / 10;

    await aliceClient.execute(
      aliceAddress,
      proxyAddress,
      { mint: { vault: vaultAddress } },
      gasFee,
      "",
      [coin(depositAmount, depositAssetDenom)],
    );

    const aliceSynthBalance = await hostQueryClient.bank.balance(
      aliceAddress,
      `factory/${mintAddress}/amstake`,
    );

    expect(+aliceSynthBalance.amount).toBe(depositAmount);
  });

  it("alice makes an advance via the proxy", async () => {
    const advanceAmount = IBC_TRANSFER_AMOUNT / 20;

    await aliceClient.execute(
      aliceAddress,
      proxyAddress,
      { advance: { vault: vaultAddress, amount: String(advanceAmount) } },
      gasFee,
    );

    const position: PositionResponse = await operatorClient.queryContractSmart(
      hubAddress,
      { position: { account: aliceAddress, vault: vaultAddress } },
    );

    expect(+position.debt).toBe(advanceAmount);
  });

  it("alice redeems synthetics via the proxy", async () => {
    const redeemAmount = IBC_TRANSFER_AMOUNT / 10;

    await aliceClient.execute(
      aliceAddress,
      proxyAddress,
      { redeem: { vault: vaultAddress } },
      gasFee,
      "",
      [coin(redeemAmount, `factory/${mintAddress}/amstake`)],
    );

    const vaultMeta: VaultMetadata = await operatorClient.queryContractSmart(
      hubAddress,
      { vault_metadata: { vault: vaultAddress } },
    );

    expect(+vaultMeta.reserve_balance).toBe(0);
  });

  it("bob cannot make a deposit via the proxy or hub", async () => {
    const depositAmount = IBC_TRANSFER_AMOUNT / 10;

    expect(async () => {
      await bobClient.execute(
        bobAddress,
        proxyAddress,
        { deposit: { vault: vaultAddress } },
        gasFee,
        "",
        [coin(depositAmount, depositAssetDenom)],
      );
    }).toThrow(`${bobAddress} is not whitelisted`);

    expect(async () => {
      await bobClient.execute(
        bobAddress,
        hubAddress,
        { deposit: { vault: vaultAddress } },
        gasFee,
        "",
        [coin(depositAmount, depositAssetDenom)],
      );
    }).toThrow(`unauthorized`);
  });

  it("bob cannot mint synthetics via the proxy or hub", async () => {
    const depositAmount = IBC_TRANSFER_AMOUNT / 10;

    expect(async () => {
      await bobClient.execute(
        bobAddress,
        proxyAddress,
        { mint: { vault: vaultAddress } },
        gasFee,
        "",
        [coin(depositAmount, depositAssetDenom)],
      );
    }).toThrow(`${bobAddress} is not whitelisted`);

    expect(async () => {
      await bobClient.execute(
        bobAddress,
        hubAddress,
        { mint: { vault: vaultAddress } },
        gasFee,
        "",
        [coin(depositAmount, depositAssetDenom)],
      );
    }).toThrow(`unauthorized`);
  });

  it("bob cannot make an advance via the proxy or hub", async () => {
    const advanceAmount = IBC_TRANSFER_AMOUNT / 20;

    expect(async () => {
      await bobClient.execute(
        bobAddress,
        proxyAddress,
        { advance: { vault: vaultAddress, amount: String(advanceAmount) } },
        gasFee,
      );
    }).toThrow(`${bobAddress} is not whitelisted`);

    expect(async () => {
      await bobClient.execute(
        bobAddress,
        hubAddress,
        { advance: { vault: vaultAddress, amount: String(advanceAmount) } },
        gasFee,
      );
    }).toThrow(`unauthorized`);
  });

  it("bob cannot redeem synthetics via the proxy or hub", async () => {
    const redeemAmount = IBC_TRANSFER_AMOUNT / 30;

    await aliceClient.sendTokens(
      aliceAddress,
      bobAddress,
      [coin(redeemAmount, `factory/${mintAddress}/amstake`)],
      gasFee,
    );

    expect(async () => {
      await bobClient.execute(
        bobAddress,
        proxyAddress,
        { redeem: { vault: vaultAddress } },
        gasFee,
        "",
        [coin(redeemAmount, `factory/${mintAddress}/amstake`)],
      );
    }).toThrow(`${bobAddress} is not whitelisted`);

    expect(async () => {
      await bobClient.execute(
        bobAddress,
        hubAddress,
        { redeem: { vault: vaultAddress } },
        gasFee,
        "",
        [coin(redeemAmount, `factory/${mintAddress}/amstake`)],
      );
    }).toThrow(`unauthorized`);
  });

  it("should remove alice from the whitelist", async () => {
    const msg: ProxyExecuteMsg = {
      set_whitelisted: {
        address: aliceAddress,
        whitelisted: false,
      },
    };

    await operatorClient.execute(operatorAddress, proxyAddress, msg, gasFee);
  });

  it("alice now cannot make a deposit via the proxy", async () => {
    const depositAmount = IBC_TRANSFER_AMOUNT / 10;

    expect(async () => {
      await aliceClient.execute(
        aliceAddress,
        proxyAddress,
        { deposit: { vault: vaultAddress } },
        gasFee,
        "",
        [coin(depositAmount, depositAssetDenom)],
      );
    }).toThrow(`${aliceAddress} is not whitelisted`);
  });

  it("alice now cannot mint synthetics via the proxy", async () => {
    const depositAmount = IBC_TRANSFER_AMOUNT / 10;

    expect(async () => {
      await aliceClient.execute(
        aliceAddress,
        proxyAddress,
        { mint: { vault: vaultAddress } },
        gasFee,
        "",
        [coin(depositAmount, depositAssetDenom)],
      );
    }).toThrow(`${aliceAddress} is not whitelisted`);
  });

  it("alice now cannot make an advance via the proxy", async () => {
    const advanceAmount = IBC_TRANSFER_AMOUNT / 20;

    expect(async () => {
      await aliceClient.execute(
        aliceAddress,
        proxyAddress,
        { advance: { vault: vaultAddress, amount: String(advanceAmount) } },
        gasFee,
      );
    }).toThrow(`${aliceAddress} is not whitelisted`);
  });

  it("alice now cannot redeem synthetics via the proxy", async () => {
    const aliceSynthBalance = await hostQueryClient.bank.balance(
      aliceAddress,
      `factory/${mintAddress}/amstake`,
    );

    expect(async () => {
      await aliceClient.execute(
        aliceAddress,
        proxyAddress,
        { redeem: { vault: vaultAddress } },
        gasFee,
        "",
        [aliceSynthBalance],
      );
    }).toThrow(`${aliceAddress} is not whitelisted`);
  });

  it("should remove the deposit, mint, advance & redeem proxy for the vault", async () => {
    const msgs: HubExecuteMsg[] = [
      {
        remove_deposit_proxy: {
          vault: vaultAddress,
        },
      },
      {
        remove_mint_proxy: {
          vault: vaultAddress,
        },
      },
      {
        remove_advance_proxy: {
          vault: vaultAddress,
        },
      },
      {
        remove_redeem_proxy: {
          vault: vaultAddress,
        },
      },
    ];

    for (let msg of msgs) {
      await operatorClient.execute(operatorAddress, hubAddress, msg, gasFee);
    }
  });

  it("bob can now make a deposit directly with the hub", async () => {
    const depositAmount = IBC_TRANSFER_AMOUNT / 10;

    await bobClient.execute(
      bobAddress,
      hubAddress,
      { deposit: { vault: vaultAddress } },
      gasFee,
      "",
      [coin(depositAmount, depositAssetDenom)],
    );

    const position: PositionResponse = await operatorClient.queryContractSmart(
      hubAddress,
      { position: { account: bobAddress, vault: vaultAddress } },
    );

    expect(+position.collateral).toBe(depositAmount);
  });

  it("bob can now mint synthetics directly with the hub", async () => {
    const depositAmount = IBC_TRANSFER_AMOUNT / 10;

    await bobClient.execute(
      bobAddress,
      hubAddress,
      { mint: { vault: vaultAddress } },
      gasFee,
      "",
      [coin(depositAmount, depositAssetDenom)],
    );

    const bobSynthBalance = await hostQueryClient.bank.balance(
      bobAddress,
      `factory/${mintAddress}/amstake`,
    );

    // bob already has some synthetics from before
    expect(+bobSynthBalance.amount).toBeGreaterThanOrEqual(depositAmount);
  });

  it("bob can now make an advance directly with the hub", async () => {
    const advanceAmount = IBC_TRANSFER_AMOUNT / 20;

    await bobClient.execute(
      bobAddress,
      hubAddress,
      { advance: { vault: vaultAddress, amount: String(advanceAmount) } },
      gasFee,
    );

    const position: PositionResponse = await operatorClient.queryContractSmart(
      hubAddress,
      { position: { account: bobAddress, vault: vaultAddress } },
    );

    expect(+position.debt).toBe(advanceAmount);
  });

  it("bob can now redeem synthetics directly with the hub", async () => {
    const redeemAmount = IBC_TRANSFER_AMOUNT / 10;

    await bobClient.execute(
      bobAddress,
      hubAddress,
      { redeem: { vault: vaultAddress } },
      gasFee,
      "",
      [coin(redeemAmount, `factory/${mintAddress}/amstake`)],
    );

    const vaultMeta: VaultMetadata = await operatorClient.queryContractSmart(
      hubAddress,
      { vault_metadata: { vault: vaultAddress } },
    );

    expect(+vaultMeta.reserve_balance).toBe(0);
  });
});
