import { describe, it, beforeAll, afterAll, expect } from "bun:test";
import { TestSuite } from "./suite";
import { artifact, readContractFileBytes } from "./utils";
import { StdFee } from "@cosmjs/stargate";
import { coin } from "@cosmjs/proto-signing";
import { ExecuteMsg as MintExecuteMsg } from "../ts/AmuletMint.types";
import {
  ExecuteMsg as HubExecuteMsg,
  InstantiateMsg as HubInstantiateMsg,
  PositionResponse,
} from "../ts/AmuletHub.types";
import {
  InstantiateMsg as ProxyInstantiateMsg,
  MetadataResponse,
  DepositAmountResponse,
  ExecuteMsg as ProxyExecuteMsg,
} from "../ts/DepositCapProxy.types";
import {
  HostClient,
  createFee,
  createHostClient,
  createHostWallet,
  initGenericLstVault,
} from "./test-helpers";

const TOTAL_DEPOSIT_CAP = 1_000_000_000;
const INDIVIDUAL_DEPOSIT_CAP = 600_000_000;
const TOTAL_MINT_CAP = 1_000_000_000;

let suite: ITestSuite;
let operatorAddress: string;
let aliceAddress: string;
let bobAddress: string;
let operatorClient: HostClient;
let aliceClient: HostClient;
let bobClient: HostClient;
let vaultCodeId: number;
let mockOracleCodeId: number;
let hubCodeId: number;
let mintCodeId: number;
let proxyCodeId: number;
let vaultAddress: string;
let mockOracleAddress: string;
let hubAddress: string;
let mintAddress: string;
let proxyAddress: string;
let gasFee: StdFee;
let depositAssetDenom: string = "untrn";

describe("Deposit Cap Proxy", () => {
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
    const bobWallet = await createHostWallet(suite, "demo3");

    operatorAddress = (await operatorWallet.getAccounts())[0].address;
    aliceAddress = (await aliceWallet.getAccounts())[0].address;
    bobAddress = (await bobWallet.getAccounts())[0].address;

    operatorClient = await createHostClient(suite, operatorWallet);
    aliceClient = await createHostClient(suite, aliceWallet);
    bobClient = await createHostClient(suite, bobWallet);

    gasFee = createFee(suite, 5_000_000);
  });

  afterAll(async () => {
    await suite.cleanup();
  });

  it("should upload the mock-lst-oracle contract byte code", async () => {
    const wasmFilePath = artifact("mock-lst-oracle");
    const wasmBytes = await readContractFileBytes(wasmFilePath);
    const res = await operatorClient.upload(operatorAddress, wasmBytes, gasFee);
    mockOracleCodeId = res.codeId;
  });

  it("should upload the amulet-generic-lst vault contract byte code", async () => {
    const wasmFilePath = artifact("amulet-generic-lst");
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

  it("should upload the desosit-cap-proxy contract byte code", async () => {
    const wasmFilePath = artifact("deposit-cap-proxy");
    const wasmBytes = await readContractFileBytes(wasmFilePath);
    const res = await operatorClient.upload(operatorAddress, wasmBytes, gasFee);
    proxyCodeId = res.codeId;
  });

  it("should deploy the mock-lst-oracle", async () => {
    const res = await operatorClient.instantiate(
      operatorAddress,
      mockOracleCodeId,
      {},
      "mock-lst-oracle",
      gasFee,
    );

    mockOracleAddress = res.contractAddress;
  });

  it("should deploy the amulet-generic-lst vault, pretending untrn is an LST", async () => {
    vaultAddress = await initGenericLstVault(
      suite,
      operatorClient,
      vaultCodeId,
      operatorAddress,
      mockOracleAddress,
      "untrn",
      6,
      6,
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

  it("should deploy the deposit-cap-proxy", async () => {
    const msg: ProxyInstantiateMsg = {
      hub_address: hubAddress,
    };

    const res = await operatorClient.instantiate(
      operatorAddress,
      proxyCodeId,
      msg,
      "deposit-cap-proxy",
      gasFee,
    );

    proxyAddress = res.contractAddress;
  });

  it("should configure deposit-cap-proxy caps for the vault", async () => {
    const msg: ProxyExecuteMsg = {
      set_config: {
        vault: vaultAddress,
        total_deposit_cap: String(TOTAL_DEPOSIT_CAP),
        individual_deposit_cap: String(INDIVIDUAL_DEPOSIT_CAP),
        total_mint_cap: String(TOTAL_MINT_CAP),
      },
    };

    await operatorClient.execute(operatorAddress, proxyAddress, msg, gasFee);
  });

  it("should create the amNTRN synthetic", async () => {
    const msg: MintExecuteMsg = {
      create_synthetic: {
        decimals: 6,
        ticker: "amNTRN",
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
          synthetic: `factory/${mintAddress}/amntrn`,
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

  it("should configure the deposit and mint proxy for the vault", async () => {
    const msg: HubExecuteMsg = {
      set_proxy_config: {
        vault: vaultAddress,
        deposit: proxyAddress,
        mint: proxyAddress,
      },
    };

    await operatorClient.execute(operatorAddress, hubAddress, msg, gasFee);
  });

  it("alice makes the initial deposit via the proxy", async () => {
    const depositAmount = INDIVIDUAL_DEPOSIT_CAP;

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

    expect(+position.collateral).toBe(INDIVIDUAL_DEPOSIT_CAP);

    const proxyMetadata: MetadataResponse =
      await operatorClient.queryContractSmart(proxyAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    expect(+proxyMetadata.total_deposit).toBe(INDIVIDUAL_DEPOSIT_CAP);

    const depositAmountRes: DepositAmountResponse =
      await operatorClient.queryContractSmart(proxyAddress, {
        deposit_amount: { vault: vaultAddress, account: aliceAddress },
      });

    expect(+depositAmountRes.amount).toBe(INDIVIDUAL_DEPOSIT_CAP);
  });

  it("bob cannot deposit more than the total cap", async () => {
    const depositAmount = INDIVIDUAL_DEPOSIT_CAP;

    expect(async () => {
      await bobClient.execute(
        bobAddress,
        proxyAddress,
        { deposit: { vault: vaultAddress } },
        gasFee,
        "",
        [coin(depositAmount, depositAssetDenom)],
      );
    }).toThrow("total deposit cap exceeded");
  });

  it("bob can mint up to the total cap via the proxy", async () => {
    const depositAmount = TOTAL_MINT_CAP;

    await bobClient.execute(
      bobAddress,
      proxyAddress,
      { mint: { vault: vaultAddress } },
      gasFee,
      "",
      [coin(depositAmount, depositAssetDenom)],
    );

    const syntheticBalance = await operatorClient.getBalance(
      bobAddress,
      `factory/${mintAddress}/amntrn`,
    );

    expect(+syntheticBalance.amount).toBeGreaterThan(0);

    const proxyMetadata: MetadataResponse =
      await operatorClient.queryContractSmart(proxyAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    expect(+proxyMetadata.total_mint).toBe(TOTAL_MINT_CAP);
  });

  it("alice can no longer mint any assets via the proxy", async () => {
    const depositAmount = INDIVIDUAL_DEPOSIT_CAP;

    expect(async () => {
      await aliceClient.execute(
        aliceAddress,
        proxyAddress,
        { mint: { vault: vaultAddress } },
        gasFee,
        "",
        [coin(depositAmount, depositAssetDenom)],
      );
    }).toThrow("total mint cap exceeded");
  });

  it("cannot mint directly with the hub", async () => {
    const depositAmount = INDIVIDUAL_DEPOSIT_CAP;

    expect(async () => {
      await aliceClient.execute(
        aliceAddress,
        hubAddress,
        { mint: { vault: vaultAddress } },
        gasFee,
        "",
        [coin(depositAmount, depositAssetDenom)],
      );
    }).toThrow("unauthorized");
  });

  it("cannot deposit directly with the hub", async () => {
    const depositAmount = INDIVIDUAL_DEPOSIT_CAP;

    expect(async () => {
      await aliceClient.execute(
        aliceAddress,
        hubAddress,
        { deposit: { vault: vaultAddress } },
        gasFee,
        "",
        [coin(depositAmount, depositAssetDenom)],
      );
    }).toThrow("unauthorized");
  });

  it("non-admin cannot alter config", async () => {
    const msg: ProxyExecuteMsg = {
      set_config: {
        vault: vaultAddress,
        total_deposit_cap: String(TOTAL_DEPOSIT_CAP * 2),
        individual_deposit_cap: String(INDIVIDUAL_DEPOSIT_CAP * 2),
        total_mint_cap: String(TOTAL_MINT_CAP * 2),
      },
    };

    expect(async () => {
      await aliceClient.execute(aliceAddress, proxyAddress, msg, gasFee);
    }).toThrow("unauthorized");
  });
});
