import { describe, it, beforeAll, afterAll, expect } from "bun:test";
import { TestSuite } from "./suite";
import { artifact, readContractFileBytes } from "./utils";
import { StdFee } from "@cosmjs/stargate";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { coin } from "@cosmjs/proto-signing";
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
} from "../ts/WhitelistProxy.types";
import { GENESIS_ALLOCATION } from "./suite/constants";
import {
  QueryClient,
  createFee,
  createHostClient,
  createHostWallet,
  createQueryClient,
  initGenericLstVault,
} from "./test-helpers";

let suite: ITestSuite;
let hostQueryClient: QueryClient;
let operatorAddress: string;
let aliceAddress: string;
let bobAddress: string;
let operatorClient: SigningCosmWasmClient;
let aliceClient: SigningCosmWasmClient;
let bobClient: SigningCosmWasmClient;
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

describe("Whitelist Proxy", () => {
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

    hostQueryClient = await createQueryClient(suite.getHostRpc());

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

  it("should upload the whitelist-proxy contract byte code", async () => {
    const wasmFilePath = artifact("whitelist-proxy");
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

  it("alice makes a deposit via the proxy", async () => {
    const depositAmount = GENESIS_ALLOCATION / 10;

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
    const depositAmount = GENESIS_ALLOCATION / 10;

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
      `factory/${mintAddress}/amntrn`,
    );

    expect(+aliceSynthBalance.amount).toBe(depositAmount);
  });

  it("alice makes an advance via the proxy", async () => {
    const advanceAmount = GENESIS_ALLOCATION / 20;

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
    const redeemAmount = GENESIS_ALLOCATION / 10;

    await aliceClient.execute(
      aliceAddress,
      proxyAddress,
      { redeem: { vault: vaultAddress } },
      gasFee,
      "",
      [coin(redeemAmount, `factory/${mintAddress}/amntrn`)],
    );

    const vaultMeta: VaultMetadata = await operatorClient.queryContractSmart(
      hubAddress,
      { vault_metadata: { vault: vaultAddress } },
    );

    expect(+vaultMeta.reserve_balance).toBe(0);
  });

  it("bob cannot make a deposit via the proxy or hub", async () => {
    const depositAmount = GENESIS_ALLOCATION / 10;

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
    const depositAmount = GENESIS_ALLOCATION / 10;

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
    const advanceAmount = GENESIS_ALLOCATION / 20;

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
    const redeemAmount = Math.floor(GENESIS_ALLOCATION / 30);

    await aliceClient.sendTokens(
      aliceAddress,
      bobAddress,
      [coin(redeemAmount, `factory/${mintAddress}/amntrn`)],
      gasFee,
    );

    expect(async () => {
      await bobClient.execute(
        bobAddress,
        proxyAddress,
        { redeem: { vault: vaultAddress } },
        gasFee,
        "",
        [coin(redeemAmount, `factory/${mintAddress}/amntrn`)],
      );
    }).toThrow(`${bobAddress} is not whitelisted`);

    expect(async () => {
      await bobClient.execute(
        bobAddress,
        hubAddress,
        { redeem: { vault: vaultAddress } },
        gasFee,
        "",
        [coin(redeemAmount, `factory/${mintAddress}/amntrn`)],
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
    const depositAmount = GENESIS_ALLOCATION / 10;

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
    const depositAmount = GENESIS_ALLOCATION / 10;

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
    const advanceAmount = GENESIS_ALLOCATION / 20;

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
      `factory/${mintAddress}/amntrn`,
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
    const depositAmount = GENESIS_ALLOCATION / 10;

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
    const depositAmount = GENESIS_ALLOCATION / 10;

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
      `factory/${mintAddress}/amntrn`,
    );

    // bob already has some synthetics from before
    expect(+bobSynthBalance.amount).toBeGreaterThanOrEqual(depositAmount);
  });

  it("bob can now make an advance directly with the hub", async () => {
    const advanceAmount = GENESIS_ALLOCATION / 20;

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
    const redeemAmount = GENESIS_ALLOCATION / 10;

    await bobClient.execute(
      bobAddress,
      hubAddress,
      { redeem: { vault: vaultAddress } },
      gasFee,
      "",
      [coin(redeemAmount, `factory/${mintAddress}/amntrn`)],
    );

    const vaultMeta: VaultMetadata = await operatorClient.queryContractSmart(
      hubAddress,
      { vault_metadata: { vault: vaultAddress } },
    );

    expect(+vaultMeta.reserve_balance).toBe(0);
  });
});
