import { describe, it, beforeAll, afterAll, expect } from "bun:test";
import { TestSuite } from "./suite";
import { artifact, readContractFileBytes } from "./utils";
import { StdFee } from "@cosmjs/stargate";
import { SigningCosmWasmClient } from "@cosmjs/cosmwasm-stargate";
import { coin } from "@cosmjs/proto-signing";
import {
  StateResponse as VaultStateResponse,
  ClaimableResponse as VaultClaimableResponse,
} from "../ts/AmuletGenericLst.types";
import { ExecuteMsg as MintExecuteMsg } from "../ts/AmuletMint.types";
import {
  ExecuteMsg as HubExecuteMsg,
  InstantiateMsg as HubInstantiateMsg,
  PositionResponse,
  VaultMetadata,
} from "../ts/AmuletHub.types";
import { GENESIS_ALLOCATION } from "./suite/constants";
import {
  QueryClient,
  createFee,
  createHostClient,
  createHostWallet,
  createQueryClient,
  initGenericLstVault,
  toBeWithinN,
} from "./test-helpers";

function sharesValue(vaultState: VaultStateResponse, shares: any): bigint {
  return (
    (BigInt(shares) * BigInt(vaultState.total_deposits)) /
    BigInt(vaultState.total_issued_shares)
  );
}

let suite: ITestSuite;
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
let mockOracleCodeId: number;
let vaultAddress: string;
let hubAddress: string;
let mintAddress: string;
let mockOracleAddress: string;
let gasFee: StdFee;

describe("Mint, Hub & Vault Integration", () => {
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

  it("configure the fixed advance fee to 100 bps (1%)", async () => {
    await operatorClient.execute(
      operatorAddress,
      hubAddress,
      {
        set_fixed_advance_fee: { vault: vaultAddress, bps: 100 },
      },
      gasFee,
    );

    const vaultMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    expect(vaultMetadata.fixed_advance_fee_bps).toBe(100);
  });

  it("configure the advance fee recipient to be the operator address", async () => {
    await operatorClient.execute(
      operatorAddress,
      hubAddress,
      {
        set_advance_fee_recipient: {
          vault: vaultAddress,
          recipient: operatorAddress,
        },
      } as HubExecuteMsg,
      gasFee,
    );

    const vaultMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    expect(vaultMetadata.advance_fee_recipient).toBe(operatorAddress);
  });

  it("alice makes a deposit while the redemption rate is 1.0", async () => {
    const depositAmount = GENESIS_ALLOCATION / 10;

    await aliceClient.execute(
      aliceAddress,
      hubAddress,
      { deposit: { vault: vaultAddress } },
      gasFee,
      "",
      [coin(depositAmount, "untrn")],
    );

    const position: PositionResponse = await operatorClient.queryContractSmart(
      hubAddress,
      { position: { account: aliceAddress, vault: vaultAddress } },
    );

    expect(+position.collateral).toBe(depositAmount);
  });

  it("alice takes an advance while the redemption rate is still 1.0", async () => {
    const advanceAmount = GENESIS_ALLOCATION / 10 / 2;

    const advanceFeeAmount = advanceAmount / 100;

    await aliceClient.execute(
      aliceAddress,
      hubAddress,
      {
        advance: { vault: vaultAddress, amount: String(advanceAmount) },
      },
      gasFee,
    );

    const position: PositionResponse = await operatorClient.queryContractSmart(
      hubAddress,
      { position: { account: aliceAddress, vault: vaultAddress } },
    );

    const aliceSynthBalance = await hostQueryClient.bank.balance(
      aliceAddress,
      `factory/${mintAddress}/amntrn`,
    );

    const operatorSynthBalance = await hostQueryClient.bank.balance(
      operatorAddress,
      `factory/${mintAddress}/amntrn`,
    );

    expect(+position.debt).toBe(advanceAmount);
    toBeWithinN(1, +aliceSynthBalance.amount, advanceAmount - advanceFeeAmount);
    toBeWithinN(1, +operatorSynthBalance.amount, advanceFeeAmount);
  });

  it("set the redemption rate to 1.1 (10% increase in value)", async () => {
    await operatorClient.execute(
      operatorAddress,
      mockOracleAddress,
      {
        set_redemption_rate: {
          rate: "1.1",
        },
      },
      gasFee,
    );
  });

  it("alice evaluates her position after the redemption rate increase", async () => {
    await aliceClient.execute(
      aliceAddress,
      hubAddress,
      { evaluate: { vault: vaultAddress } },
      gasFee,
    );

    const position: PositionResponse = await operatorClient.queryContractSmart(
      hubAddress,
      { position: { account: aliceAddress, vault: vaultAddress } },
    );

    const vaultMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const vaultState: VaultStateResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        state: {},
      });

    const collateral = +position.collateral;
    const totalYield = collateral * 0.1;
    const debtPayment = totalYield * 0.9;

    const expectedDebt = collateral / 2 - debtPayment;

    const aggregateHubSharesBalances =
      BigInt(vaultMetadata.collateral_shares) +
      BigInt(vaultMetadata.reserve_shares) +
      BigInt(vaultMetadata.treasury_shares) +
      BigInt(vaultMetadata.amo_shares);

    expect(aggregateHubSharesBalances).toBe(
      BigInt(vaultState.total_issued_shares),
    );

    toBeWithinN(1, +position.debt, expectedDebt);
    toBeWithinN(1, +vaultMetadata.reserve_balance, debtPayment);

    const expectedTreasuryPaymentValue =
      BigInt(totalYield) - BigInt(debtPayment);

    const treasurySharesValue =
      (BigInt(vaultMetadata.treasury_shares) *
        BigInt(vaultState.total_deposits)) /
      BigInt(vaultState.total_issued_shares);

    toBeWithinN(1, treasurySharesValue, expectedTreasuryPaymentValue);
  });

  it("bob makes a deposit while the redemption rate is 1.1", async () => {
    const depositAmount = GENESIS_ALLOCATION / 10;

    await bobClient.execute(
      bobAddress,
      hubAddress,
      { deposit: { vault: vaultAddress } },
      gasFee,
      "",
      [coin(depositAmount, "untrn")],
    );

    const position: PositionResponse = await operatorClient.queryContractSmart(
      hubAddress,
      { position: { account: bobAddress, vault: vaultAddress } },
    );

    expect(BigInt(position.collateral)).toBe(
      BigInt(depositAmount * 110) / 100n,
    );
  });

  it("set the redemption rate to 1.21 (another 10% increase in value)", async () => {
    await operatorClient.execute(
      operatorAddress,
      mockOracleAddress,
      {
        set_redemption_rate: {
          rate: "1.21",
        },
      },
      gasFee,
    );
  });

  it("bob evaluates his position after the redemption rate increase", async () => {
    await bobClient.execute(
      bobAddress,
      hubAddress,
      { evaluate: { vault: vaultAddress } },
      gasFee,
    );

    const position: PositionResponse = await operatorClient.queryContractSmart(
      hubAddress,
      { position: { account: bobAddress, vault: vaultAddress } },
    );

    const collateral = +position.collateral;
    const totalYield = collateral * 0.1;
    const debtPayment = totalYield * 0.9;

    toBeWithinN(1, +position.credit, debtPayment);
  });

  it("alice can query her position and it will show further debt payments", async () => {
    const position: PositionResponse = await operatorClient.queryContractSmart(
      hubAddress,
      { position: { account: aliceAddress, vault: vaultAddress } },
    );

    const collateral = +position.collateral;
    const totalYield = collateral * 0.2;
    const debtPayment = totalYield * 0.9;

    const expectedDebt = collateral / 2 - debtPayment;

    // allow for 2 separate rounding errors on 2 debt payments
    toBeWithinN(2, +position.debt, expectedDebt);
  });

  it("bob converts his credit to collateral", async () => {
    const preConvertPosition: PositionResponse =
      await operatorClient.queryContractSmart(hubAddress, {
        position: { account: bobAddress, vault: vaultAddress },
      });

    const preConvertMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    await bobClient.execute(
      bobAddress,
      hubAddress,
      {
        convert_credit: {
          vault: vaultAddress,
          amount: preConvertPosition.credit,
        },
      },
      gasFee,
    );

    const postConvertPosition: PositionResponse =
      await operatorClient.queryContractSmart(hubAddress, {
        position: { account: bobAddress, vault: vaultAddress },
      });

    const postConvertMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const vaultState: VaultStateResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        state: {},
      });

    const expectedCollateral =
      +preConvertPosition.collateral + +preConvertPosition.credit;

    const reserveBalanceDecrease =
      +preConvertMetadata.reserve_balance -
      +postConvertMetadata.reserve_balance;

    const collateralBalanceIncrease =
      +postConvertMetadata.collateral_balance -
      +preConvertMetadata.collateral_balance;

    const reserveSharesDecrease =
      +preConvertMetadata.reserve_shares - +postConvertMetadata.reserve_shares;

    const collateralSharesIncrease =
      +postConvertMetadata.collateral_shares -
      +preConvertMetadata.collateral_shares;

    toBeWithinN(1, +postConvertPosition.collateral, expectedCollateral);
    toBeWithinN(1, reserveBalanceDecrease, +preConvertPosition.credit);
    toBeWithinN(1, collateralBalanceIncrease, +preConvertPosition.credit);
    toBeWithinN(
      1,
      sharesValue(vaultState, reserveSharesDecrease),
      +preConvertPosition.credit,
    );
    toBeWithinN(
      1,
      sharesValue(vaultState, collateralSharesIncrease),
      +preConvertPosition.credit,
    );

    expect(+postConvertPosition.credit).toBe(0);
  });

  it("bob one-to-one mints a number of amNTRN", async () => {
    const depositAmount = GENESIS_ALLOCATION / 10;

    const preMintMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const bobPreMintSynthBalance = await hostQueryClient.bank.balance(
      bobAddress,
      `factory/${mintAddress}/amntrn`,
    );

    await bobClient.execute(
      bobAddress,
      hubAddress,
      { mint: { vault: vaultAddress } },
      gasFee,
      "",
      [coin(depositAmount, "untrn")],
    );

    const bobPostMintSynthBalance = await hostQueryClient.bank.balance(
      bobAddress,
      `factory/${mintAddress}/amntrn`,
    );

    const postMintMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const vaultState: VaultStateResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        state: {},
      });

    const expectedSynthBalanceIncrease = depositAmount * 1.21;

    const synthBalanceIncrease =
      +bobPostMintSynthBalance.amount - +bobPreMintSynthBalance.amount;

    const reserveBalanceIncrease =
      +postMintMetadata.reserve_balance - +preMintMetadata.reserve_balance;

    const reserveSharesIncrease =
      +postMintMetadata.reserve_shares - +preMintMetadata.reserve_shares;

    toBeWithinN(1, synthBalanceIncrease, expectedSynthBalanceIncrease);
    toBeWithinN(1, reserveBalanceIncrease, expectedSynthBalanceIncrease);
    toBeWithinN(
      1,
      sharesValue(vaultState, reserveSharesIncrease),
      expectedSynthBalanceIncrease,
    );
  });

  it("bob withdraws half of his collateral", async () => {
    const preWithdrawPosition: PositionResponse =
      await operatorClient.queryContractSmart(hubAddress, {
        position: { account: bobAddress, vault: vaultAddress },
      });

    const preWithdrawMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const preWithdrawVaultState: VaultStateResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        state: {},
      });

    const withdrawAmount = BigInt(preWithdrawPosition.collateral) / 2n;

    await bobClient.execute(
      bobAddress,
      hubAddress,
      {
        withdraw: {
          vault: vaultAddress,
          amount: String(withdrawAmount),
        },
      },
      gasFee,
    );

    const postWithdrawPosition: PositionResponse =
      await operatorClient.queryContractSmart(hubAddress, {
        position: { account: bobAddress, vault: vaultAddress },
      });

    const postWithdrawMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const postWithdrawVaultState: VaultStateResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        state: {},
      });

    const bobClaimable: VaultClaimableResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        claimable: { address: bobAddress },
      });

    const expectedClaimable = Math.floor(Number(withdrawAmount) / 1.21);

    const bobCollateralDecrease =
      +preWithdrawPosition.collateral - +postWithdrawPosition.collateral;

    const collateralBalanceDecrease =
      +preWithdrawMetadata.collateral_balance -
      +postWithdrawMetadata.collateral_balance;

    const collateralSharesDecrease =
      +preWithdrawMetadata.collateral_shares -
      +postWithdrawMetadata.collateral_shares;

    const vaultDepositDecrease =
      +preWithdrawVaultState.total_deposits -
      +postWithdrawVaultState.total_deposits;

    const vaultSharesDecrease =
      +preWithdrawVaultState.total_issued_shares -
      +postWithdrawVaultState.total_issued_shares;

    toBeWithinN(1, +bobClaimable.amount, expectedClaimable);
    toBeWithinN(1, bobCollateralDecrease, withdrawAmount);
    toBeWithinN(1, collateralBalanceDecrease, withdrawAmount);
    toBeWithinN(1, vaultDepositDecrease, withdrawAmount);
    toBeWithinN(
      1,
      sharesValue(preWithdrawVaultState, collateralSharesDecrease),
      withdrawAmount,
    );
    toBeWithinN(
      1,
      sharesValue(preWithdrawVaultState, vaultSharesDecrease),
      withdrawAmount,
    );
  });

  it("bob claims his withdrawal", async () => {
    const bobPreClaimLstBalance = await hostQueryClient.bank.balance(
      bobAddress,
      "untrn",
    );

    const bobPreClaimClaimable: VaultClaimableResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        claimable: { address: bobAddress },
      });

    await bobClient.execute(
      bobAddress,
      vaultAddress,
      {
        claim: {},
      },
      gasFee,
    );

    const bobPostClaimLstBalance = await hostQueryClient.bank.balance(
      bobAddress,
      "untrn",
    );

    const bobPostClaimClaimable: VaultClaimableResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        claimable: { address: bobAddress },
      });

    const bobLstBalanceIncrease =
      +bobPostClaimLstBalance.amount - +bobPreClaimLstBalance.amount;

    const expectedBalanceIncrease =
      +bobPreClaimClaimable.amount - +gasFee.amount[0].amount;

    expect(bobLstBalanceIncrease).toBe(expectedBalanceIncrease);
    expect(+bobPostClaimClaimable.amount).toBe(0);
  });

  it("bob redeems half of his synthetics", async () => {
    const bobSynthBalance = await hostQueryClient.bank.balance(
      bobAddress,
      `factory/${mintAddress}/amntrn`,
    );

    const bobPreRedeemClaimable: VaultClaimableResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        claimable: { address: bobAddress },
      });

    const preRedeemMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const preRedeemVaultState: VaultStateResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        state: {},
      });

    const preRedeemSynthSupply = await hostQueryClient.bank.supplyOf(
      `factory/${mintAddress}/amntrn`,
    );

    const redeemAmount = +bobSynthBalance.amount / 2;

    await bobClient.execute(
      bobAddress,
      hubAddress,
      {
        redeem: { vault: vaultAddress },
      },
      gasFee,
      "",
      [coin(redeemAmount, `factory/${mintAddress}/amntrn`)],
    );

    const bobPostRedeemClaimable: VaultClaimableResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        claimable: { address: bobAddress },
      });

    const postRedeemMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const postRedeemVaultState: VaultStateResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        state: {},
      });

    const postRedeemSynthSupply = await hostQueryClient.bank.supplyOf(
      `factory/${mintAddress}/amntrn`,
    );

    const expectedClaimable = Math.floor(Number(redeemAmount) / 1.21);

    const bobClaimableIncrease =
      +bobPostRedeemClaimable.amount - +bobPreRedeemClaimable.amount;

    const reserveBalanceDecrease =
      +preRedeemMetadata.reserve_balance - +postRedeemMetadata.reserve_balance;

    const reserveSharesDecrease =
      +preRedeemMetadata.reserve_shares - +postRedeemMetadata.reserve_shares;

    const vaultDepositsDecrease =
      +preRedeemVaultState.total_deposits -
      +postRedeemVaultState.total_deposits;

    const vaultSharesDecrease =
      +preRedeemVaultState.total_issued_shares -
      +postRedeemVaultState.total_issued_shares;

    const synthSupplyDecrease =
      +preRedeemSynthSupply.amount - +postRedeemSynthSupply.amount;

    toBeWithinN(1, bobClaimableIncrease, expectedClaimable);
    toBeWithinN(1, reserveBalanceDecrease, redeemAmount);
    toBeWithinN(
      1,
      sharesValue(preRedeemVaultState, reserveSharesDecrease),
      redeemAmount,
    );
    toBeWithinN(2, vaultDepositsDecrease, redeemAmount);
    toBeWithinN(
      2,
      sharesValue(preRedeemVaultState, vaultSharesDecrease),
      redeemAmount,
    );
    expect(synthSupplyDecrease).toBe(redeemAmount);
  });

  it("alice repays a quarter of her debt with the underlying asset", async () => {
    const preRepayPosition: PositionResponse =
      await operatorClient.queryContractSmart(hubAddress, {
        position: { account: aliceAddress, vault: vaultAddress },
      });

    const preRepayMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const preRepayVaultState: VaultStateResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        state: {},
      });

    const quarterDebt = Math.floor(+preRepayPosition.debt / 4);

    const repayAmount = Math.ceil(quarterDebt / 1.21);

    await aliceClient.execute(
      aliceAddress,
      hubAddress,
      {
        repay_underlying: {
          vault: vaultAddress,
        },
      },
      gasFee,
      "",
      [coin(repayAmount, "untrn")],
    );

    const postRepayPosition: PositionResponse =
      await operatorClient.queryContractSmart(hubAddress, {
        position: { account: aliceAddress, vault: vaultAddress },
      });

    const postRepayMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const postRepayVaultState: VaultStateResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        state: {},
      });

    const aliceDebtDecrease = +preRepayPosition.debt - +postRepayPosition.debt;

    const reserveBalanceIncrease =
      +postRepayMetadata.reserve_balance - +preRepayMetadata.reserve_balance;

    const reserveSharesIncrease =
      +postRepayMetadata.reserve_shares - +preRepayMetadata.reserve_shares;

    const vaultDepositIncrease =
      +postRepayVaultState.total_deposits - +preRepayVaultState.total_deposits;

    const vaultSharesIncrease =
      +postRepayVaultState.total_issued_shares -
      +preRepayVaultState.total_issued_shares;

    toBeWithinN(1, aliceDebtDecrease, quarterDebt);
    toBeWithinN(1, reserveBalanceIncrease, quarterDebt);
    toBeWithinN(1, vaultDepositIncrease, quarterDebt);
    toBeWithinN(
      1,
      sharesValue(preRepayVaultState, reserveSharesIncrease),
      quarterDebt,
    );
    toBeWithinN(
      1,
      sharesValue(preRepayVaultState, vaultSharesIncrease),
      quarterDebt,
    );
  });

  it("alice repays half of her remaining debt with the synthetic asset", async () => {
    const preRepayPosition: PositionResponse =
      await operatorClient.queryContractSmart(hubAddress, {
        position: { account: aliceAddress, vault: vaultAddress },
      });

    const preRepaySynthSupply = await hostQueryClient.bank.supplyOf(
      `factory/${mintAddress}/amntrn`,
    );

    const halfDebt = Math.floor(+preRepayPosition.debt / 2);

    await aliceClient.execute(
      aliceAddress,
      hubAddress,
      {
        repay_synthetic: {
          vault: vaultAddress,
        },
      },
      gasFee,
      "",
      [coin(halfDebt, `factory/${mintAddress}/amntrn`)],
    );

    const postRepayPosition: PositionResponse =
      await operatorClient.queryContractSmart(hubAddress, {
        position: { account: aliceAddress, vault: vaultAddress },
      });

    const postRepaySynthSupply = await hostQueryClient.bank.supplyOf(
      `factory/${mintAddress}/amntrn`,
    );

    const aliceDebtDecrease = +preRepayPosition.debt - +postRepayPosition.debt;

    const synthSupplyDecrease =
      +preRepaySynthSupply.amount - +postRepaySynthSupply.amount;

    expect(aliceDebtDecrease).toBe(halfDebt);
    expect(synthSupplyDecrease).toBe(halfDebt);
  });

  it("alice self-liquidates her position", async () => {
    const preLiquidatePosition: PositionResponse =
      await operatorClient.queryContractSmart(hubAddress, {
        position: { account: aliceAddress, vault: vaultAddress },
      });

    const alicePreLiquidateClaimable: VaultClaimableResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        claimable: { address: aliceAddress },
      });

    const preLiquidateMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const preLiquidateVaultState: VaultStateResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        state: {},
      });

    await aliceClient.execute(
      aliceAddress,
      hubAddress,
      {
        self_liquidate: {
          vault: vaultAddress,
        },
      },
      gasFee,
    );

    const withdrawnCollateral =
      +preLiquidatePosition.collateral - +preLiquidatePosition.debt;

    const expectedClaimable = Math.floor(withdrawnCollateral / 1.21);

    const postLiquidatePosition: PositionResponse =
      await operatorClient.queryContractSmart(hubAddress, {
        position: { account: aliceAddress, vault: vaultAddress },
      });

    const alicePostLiquidateClaimable: VaultClaimableResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        claimable: { address: aliceAddress },
      });

    const postLiquidateMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const postLiquidateVaultState: VaultStateResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        state: {},
      });

    const aliceClaimableIncrease =
      +alicePostLiquidateClaimable.amount - +alicePreLiquidateClaimable.amount;

    const collateralBalanceDecrease =
      +preLiquidateMetadata.collateral_balance -
      +postLiquidateMetadata.collateral_balance;

    const collateralSharesDecrease =
      +preLiquidateMetadata.collateral_shares -
      +postLiquidateMetadata.collateral_shares;

    const reserveBalanceIncrease =
      +postLiquidateMetadata.reserve_balance -
      +preLiquidateMetadata.reserve_balance;

    const reserveSharesIncrease =
      +postLiquidateMetadata.reserve_shares -
      +preLiquidateMetadata.reserve_shares;

    const vaultDepositsDecrease =
      +preLiquidateVaultState.total_deposits -
      +postLiquidateVaultState.total_deposits;

    const vaultSharesDecrease =
      +preLiquidateVaultState.total_issued_shares -
      +postLiquidateVaultState.total_issued_shares;

    toBeWithinN(1, aliceClaimableIncrease, expectedClaimable);
    toBeWithinN(1, collateralBalanceDecrease, +preLiquidatePosition.collateral);
    toBeWithinN(
      1,
      sharesValue(preLiquidateVaultState, collateralSharesDecrease),
      +preLiquidatePosition.collateral,
    );
    toBeWithinN(1, reserveBalanceIncrease, +preLiquidatePosition.debt);
    toBeWithinN(
      1,
      sharesValue(preLiquidateVaultState, reserveSharesIncrease),
      +preLiquidatePosition.debt,
    );
    toBeWithinN(2, vaultDepositsDecrease, withdrawnCollateral);
    toBeWithinN(
      2,
      sharesValue(preLiquidateVaultState, vaultSharesDecrease),
      withdrawnCollateral,
    );
    expect(postLiquidatePosition.collateral).toBe("0");
    expect(postLiquidatePosition.credit).toBe("0");
    expect(postLiquidatePosition.debt).toBe("0");
  });

  it("set operator/admin as treasury address", async () => {
    await operatorClient.execute(
      operatorAddress,
      hubAddress,
      {
        set_treasury: {
          address: operatorAddress,
        },
      } as HubExecuteMsg,
      gasFee,
    );
  });

  it("treasury claims it's earnings", async () => {
    const preClaimMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const preClaimSharesBalance = await hostQueryClient.bank.balance(
      operatorAddress,
      `factory/${vaultAddress}/share`,
    );

    await operatorClient.execute(
      operatorAddress,
      hubAddress,
      {
        claim_treasury: {
          vault: vaultAddress,
        },
      },
      gasFee,
    );

    const postClaimMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const postClaimSharesBalance = await hostQueryClient.bank.balance(
      operatorAddress,
      `factory/${vaultAddress}/share`,
    );

    const treasurySharesBalanceIncrease =
      BigInt(postClaimSharesBalance.amount) -
      BigInt(preClaimSharesBalance.amount);

    expect(postClaimMetadata.treasury_shares).toBe("0");

    // possible that treasury earnings increases during claim
    expect(treasurySharesBalanceIncrease).toBeGreaterThanOrEqual(
      BigInt(preClaimMetadata.treasury_shares),
    );
  });

  it("set operator/admin as AMO address", async () => {
    await operatorClient.execute(
      operatorAddress,
      hubAddress,
      {
        set_amo: {
          vault: vaultAddress,
          amo: operatorAddress,
        },
      } as HubExecuteMsg,
      gasFee,
    );
  });

  it("set AMO allocation to 1000 BPS (10%)", async () => {
    await operatorClient.execute(
      operatorAddress,
      hubAddress,
      {
        set_amo_allocation: {
          vault: vaultAddress,
          bps: 1000,
        },
      } as HubExecuteMsg,
      gasFee,
    );
  });

  it("set the redemption rate to 1.331 (another 10% increase in value)", async () => {
    await operatorClient.execute(
      operatorAddress,
      mockOracleAddress,
      {
        set_redemption_rate: {
          rate: "1.331",
        },
      },
      gasFee,
    );
  });

  it("evaluate vault after the redemption rate increase and AMO setup", async () => {
    const preEvaluateMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    await operatorClient.execute(
      operatorAddress,
      hubAddress,
      { evaluate: { vault: vaultAddress } },
      gasFee,
    );

    const postEvaluateMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const vaultState: VaultStateResponse =
      await operatorClient.queryContractSmart(vaultAddress, {
        state: {},
      });

    const collateral = +preEvaluateMetadata.collateral_balance;
    const totalYield = collateral * 0.1;
    const debtPayment = totalYield * 0.9;
    const amoAllocationValue = Math.floor(debtPayment * 0.1);

    const amoSharesIncrease =
      BigInt(postEvaluateMetadata.amo_shares) -
      BigInt(preEvaluateMetadata.amo_shares);

    toBeWithinN(
      1,
      sharesValue(vaultState, amoSharesIncrease),
      amoAllocationValue,
    );
  });

  it("AMO claims it's earnings", async () => {
    const preClaimMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const preClaimSharesBalance = await hostQueryClient.bank.balance(
      operatorAddress,
      `factory/${vaultAddress}/share`,
    );

    await operatorClient.execute(
      operatorAddress,
      hubAddress,
      {
        claim_amo: {
          vault: vaultAddress,
        },
      },
      gasFee,
    );

    const postClaimMetadata: VaultMetadata =
      await operatorClient.queryContractSmart(hubAddress, {
        vault_metadata: { vault: vaultAddress },
      });

    const postClaimSharesBalance = await hostQueryClient.bank.balance(
      operatorAddress,
      `factory/${vaultAddress}/share`,
    );

    const treasurySharesBalanceIncrease =
      BigInt(postClaimSharesBalance.amount) -
      BigInt(preClaimSharesBalance.amount);

    expect(postClaimMetadata.amo_shares).toBe("0");

    // possible that AMO shares allocation increases during claim
    expect(treasurySharesBalanceIncrease).toBeGreaterThanOrEqual(
      BigInt(preClaimMetadata.amo_shares),
    );
  });
});
