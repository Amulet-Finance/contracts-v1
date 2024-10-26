import { describe, it, beforeAll, afterAll, expect } from "bun:test";
import { TestSuite } from "./suite";
import { artifact, readContractFileBytes } from "./utils";
import { StdFee } from "@cosmjs/stargate";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { coin, coins } from "@cosmjs/proto-signing";
import { ExecuteMsg as MintExecuteMsg } from "../ts/AmuletMint.types";
import {
  ExecuteMsg as HubExecuteMsg,
  InstantiateMsg as HubInstantiateMsg,
  VaultMetadata,
} from "../ts/AmuletHub.types";
import { GENESIS_ALLOCATION } from "./suite/constants";
import {
  QueryClient,
  createFee,
  createHostClient,
  createHostWallet,
  createQueryClient,
} from "./test-helpers";
import { StateResponse } from "../ts/AmuletGenericLst.types";

let suite: ITestSuite;
let hostQueryClient: QueryClient;
let operatorAddress: string;
let aliceAddress: string;
let operatorClient: SigningCosmWasmClient;
let aliceClient: SigningCosmWasmClient;
let cw20CodeId: number;
let metavaultCodeId: number;
let hubCodeId: number;
let mintCodeId: number;
let cw20Address: string;
let metavaultAddress: string;
let hubAddress: string;
let mintAddress: string;
let gasFee: StdFee;

describe("CW20 Silo Metavault", () => {
  beforeAll(async () => {
    suite = await TestSuite.create({
      networkOverrides: {
        neutron: {
          genesis_opts: {
            "app_state.interchaintxs.params.msg_submit_tx_max_messages": "16",
            "app_state.feeburner.params.treasury_address":
              // aribitrarily picked testnet address
              "neutron12z4p3g6zjrnlz79znrjef4sxklsnnmpglgzhx2",
          },
        },
      },
    });

    const operatorWallet = await createHostWallet(suite, "demo1");
    const aliceWallet = await createHostWallet(suite, "demo2");

    operatorAddress = (await operatorWallet.getAccounts())[0].address;
    aliceAddress = (await aliceWallet.getAccounts())[0].address;

    operatorClient = await createHostClient(suite, operatorWallet);
    aliceClient = await createHostClient(suite, aliceWallet);

    hostQueryClient = await createQueryClient(suite.getHostRpc());

    gasFee = createFee(suite, 5_000_000);
  });

  afterAll(async () => {
    await suite.cleanup();
  });

  it("should fetch the cw20-base byte code and upload it to the chain", async () => {
    const fetchArtifactResponse = await fetch(
      "https://github.com/CosmWasm/cw-plus/releases/download/v1.1.2/cw20_base.wasm",
    );
    const wasmBytes = new Uint8Array(await fetchArtifactResponse.arrayBuffer());
    const res = await operatorClient.upload(operatorAddress, wasmBytes, gasFee);
    cw20CodeId = res.codeId;
  });

  it("should upload the cw20-silo-metavault contract byte code", async () => {
    const wasmFilePath = artifact("cw20-silo-metavault");
    const wasmBytes = await readContractFileBytes(wasmFilePath);
    const res = await operatorClient.upload(operatorAddress, wasmBytes, gasFee);
    metavaultCodeId = res.codeId;
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

  it("should deploy the cw20", async () => {
    const res = await operatorClient.instantiate(
      operatorAddress,
      cw20CodeId,
      {
        name: "wATOM",
        symbol: "wATOM",
        decimals: 6,
        initial_balances: [
          { address: aliceAddress, amount: String(GENESIS_ALLOCATION) },
        ],
      },
      "cw20",
      gasFee,
    );

    cw20Address = res.contractAddress;
  });

  it("should deploy the cw20-silo-metavault with alice as the owner", async () => {
    const res = await operatorClient.instantiate(
      operatorAddress,
      metavaultCodeId,
      {
        owner: aliceAddress,
        cw20: cw20Address,
        hub: hubAddress,
        underlying_decimals: 6,
      },
      "cw20-silo-metavault",
      gasFee,
    );

    metavaultAddress = res.contractAddress;
  });

  it("should create the amATOM synthetic", async () => {
    const msg: MintExecuteMsg = {
      create_synthetic: {
        decimals: 6,
        ticker: "amATOM",
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
          vault: metavaultAddress,
          synthetic: `factory/${mintAddress}/amatom`,
        },
      };

      await operatorClient.execute(operatorAddress, hubAddress, msg, gasFee);
    }
    {
      const msg: HubExecuteMsg = {
        set_deposits_enabled: {
          vault: metavaultAddress,
          enabled: true,
        },
      };

      await operatorClient.execute(operatorAddress, hubAddress, msg, gasFee);
    }
    {
      const msg: HubExecuteMsg = {
        set_advance_enabled: {
          vault: metavaultAddress,
          enabled: true,
        },
      };

      await operatorClient.execute(operatorAddress, hubAddress, msg, gasFee);
    }
  });

  it("should configure the deposit, mint, advance & redeem proxy for the metavault", async () => {
    const msg: HubExecuteMsg = {
      set_proxy_config: {
        vault: metavaultAddress,
        deposit: metavaultAddress,
        mint: metavaultAddress,
        advance: metavaultAddress,
        redeem: metavaultAddress,
      },
    };

    await operatorClient.execute(operatorAddress, hubAddress, msg, gasFee);
  });

  it("alice mints synthetics via the metavault via cw20 send", async () => {
    const maxDepositAmount = GENESIS_ALLOCATION / 10;

    const deposits_as_percentages = [100, 56, 11, 29, 78];

    var total_deposits = 0n;
    var total_shares_issued = 0n;

    for (const perc of deposits_as_percentages) {
      const depositAmount = BigInt(Math.floor((maxDepositAmount * perc) / 100));

      let mint_msg_b64 = btoa(JSON.stringify({ mint: {} }));

      await aliceClient.execute(
        aliceAddress,
        cw20Address,
        {
          send: {
            contract: metavaultAddress,
            amount: String(depositAmount),
            msg: mint_msg_b64,
          },
        },
        gasFee,
      );

      const aliceSynthBalance = await hostQueryClient.bank.balance(
        aliceAddress,
        `factory/${mintAddress}/amatom`,
      );

      const vaultMeta: VaultMetadata = await operatorClient.queryContractSmart(
        hubAddress,
        { vault_metadata: { vault: metavaultAddress } },
      );

      const vaultState: StateResponse = await operatorClient.queryContractSmart(
        metavaultAddress,
        {
          state: {},
        },
      );

      const expectedSharesIssued = depositAmount * 10n ** 12n;
      expect(BigInt(aliceSynthBalance.amount)).toBe(
        total_deposits + depositAmount,
      );
      expect(BigInt(vaultMeta.reserve_balance)).toBe(
        total_deposits + depositAmount,
      );
      expect(BigInt(vaultState.total_deposits)).toBe(
        total_deposits + depositAmount,
      );
      expect(BigInt(vaultState.total_issued_shares)).toBe(
        total_shares_issued + expectedSharesIssued,
      );
      expect(+vaultMeta.treasury_shares).toBe(0);

      total_deposits += depositAmount;
      total_shares_issued += expectedSharesIssued;
    }
  });

  it("alice redeems synthetics via the metavault", async () => {
    const redeemAmount = GENESIS_ALLOCATION / 10;

    const preVaultMeta: VaultMetadata = await operatorClient.queryContractSmart(
      hubAddress,
      { vault_metadata: { vault: metavaultAddress } },
    );

    await aliceClient.execute(
      aliceAddress,
      metavaultAddress,
      { redeem: {} },
      gasFee,
      "",
      [coin(redeemAmount, `factory/${mintAddress}/amatom`)],
    );

    const postVaultMeta: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: metavaultAddress },
      });

    expect(+postVaultMeta.reserve_balance).toBe(
      +preVaultMeta.reserve_balance - redeemAmount,
    );
  });

  it("alice claims redeemed cw20s from the metavault", async () => {
    const claimable = await operatorClient.queryContractSmart(
      metavaultAddress,
      {
        claimable: { address: aliceAddress },
      },
    );

    const preClaimCw20Balance = await operatorClient.queryContractSmart(
      cw20Address,
      {
        balance: { address: aliceAddress },
      },
    );

    await aliceClient.execute(
      aliceAddress,
      metavaultAddress,
      { claim: {} },
      gasFee,
    );

    const postClaimCw20Balance = await operatorClient.queryContractSmart(
      cw20Address,
      {
        balance: { address: aliceAddress },
      },
    );

    expect(+postClaimCw20Balance.balance - +preClaimCw20Balance.balance).toBe(
      +claimable.amount,
    );
  });

  it("anyone but alice unauthorized to mint synthetics via the metavault", async () => {
    const depositAmount = GENESIS_ALLOCATION / 10;

    await aliceClient.execute(
      aliceAddress,
      cw20Address,
      {
        transfer: {
          recipient: operatorAddress,
          amount: String(depositAmount),
        },
      },
      gasFee,
    );

    let mint_msg_b64 = btoa(JSON.stringify({ mint: {} }));

    expect(async () => {
      await operatorClient.execute(
        operatorAddress,
        cw20Address,
        {
          send: {
            contract: metavaultAddress,
            amount: String(depositAmount),
            msg: mint_msg_b64,
          },
        },
        gasFee,
      );
    }).toThrow("unauthorized");
  });

  it("anyone but alice unauthorized to redeem synthetics via the metavault", async () => {
    const depositAmount = GENESIS_ALLOCATION / 10;

    let mint_msg_b64 = btoa(JSON.stringify({ mint: {} }));

    await aliceClient.execute(
      aliceAddress,
      cw20Address,
      {
        send: {
          contract: metavaultAddress,
          amount: String(depositAmount),
          msg: mint_msg_b64,
        },
      },
      gasFee,
    );

    aliceClient.sendTokens(
      aliceAddress,
      operatorAddress,
      coins(depositAmount, `factory/${mintAddress}/amatom`),
      gasFee,
    );

    expect(async () => {
      await operatorClient.execute(
        operatorAddress,
        metavaultAddress,
        { redeem: {} },
        gasFee,
        "",
        [coin(depositAmount, `factory/${mintAddress}/amatom`)],
      );
    }).toThrow("unauthorized");
  });
});
