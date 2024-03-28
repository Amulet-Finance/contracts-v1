import { test, expect } from "bun:test";
import { coin } from "@cosmjs/proto-signing";

import { HostChain } from "./client";
import { e2eTestConfig } from "./config";
import { readContractFileBytes } from "./utils";

import { 
  InstantiateMsg as MintInitMsg, 
  QueryMsg as MintQueryMsg, 
  Metadata as SynthMetadata,
  AllAssetsResponse, 
  WhitelistedResponse 
} from "../ts/AmuletMint.types";
import { 
  InstantiateMsg as HubInitMsg, 
  QueryMsg as HubQueryMsg, 
  PositionResponse, 
  VaultMetadata 
} from "../ts/AmuletHub.types";
import { ClaimableResponse, InstantiateMsg as LstVaultInitMsg, StateResponse } from "../ts/AmuletGenericLst.types";
import { InstantiateMsg as LstOracleInitMsg } from "../ts/MockLstOracle.types";

function artifact(name: string): string {
  return `${__dirname}/../artifacts/${name}.wasm`
}

function toBeWithinN(n: number, actual: any, expected: number) {
  expect(Number(actual)).toBeLessThanOrEqual(expected + n);
  expect(Number(actual)).toBeGreaterThanOrEqual(expected - n);
}

const config = e2eTestConfig();

const mintWasmBytes: Uint8Array = await readContractFileBytes(artifact("amulet-mint"));

const hubWasmBytes: Uint8Array = await readContractFileBytes(artifact("amulet-hub"));

const lstVaultWasmBytes: Uint8Array = await readContractFileBytes(artifact("amulet-generic-lst"));

const lstOracleWasmBytes: Uint8Array = await readContractFileBytes(artifact("mock-lst-oracle"));

const disconnectedHostChain = await HostChain.create(
  config.HOST_CHAIN_PREFIX,
  config.WALLET_MNEMONIC,
  config.HOST_CHAIN_GAS_PRICE,
);

const hostChain = await disconnectedHostChain.connect(config.HOST_CHAIN_RPC);

test("mint->hub->vault", async () => {
  const [mintCodeId] = await hostChain.uploadWasm(mintWasmBytes);

  console.log(`uploaded mint contract: ${mintCodeId}`);

  const [hubCodeId] = await hostChain.uploadWasm(hubWasmBytes);

  console.log(`uploaded hub contract: ${hubCodeId}`);

  const [lstVaultCodeId] = await hostChain.uploadWasm(lstVaultWasmBytes);

  console.log(`uploaded LST vault contract: ${lstVaultCodeId}`);

  const [lstOracleCodeId] = await hostChain.uploadWasm(lstOracleWasmBytes);

  console.log(`uploaded LST oracle contract: ${lstOracleCodeId}`);

  const creator = await hostChain.accountAddress();
  
  const mintInitMsg: MintInitMsg = {};

  const [mint] = await hostChain.initContract(mintCodeId, mintInitMsg, "mint");

  console.log(`mint contract: ${mint.address}`);

  const hubInitMsg: HubInitMsg = { synthetic_mint: mint.address };

  const [hub] = await hostChain.initContract(hubCodeId, hubInitMsg, "hub");

  console.log(`hub contract: ${hub.address}`);

  const lstOracleInitMsg: LstOracleInitMsg = {};

  const [lstOracle] = await hostChain.initContract(lstOracleCodeId, lstOracleInitMsg, "lst-oracle");

  console.log(`LST oracle contract: ${lstOracle.address}`);

  const lstVaultInitMsg: LstVaultInitMsg = { 
    lst_decimals: 6, 
    lst_denom: "untrn", 
    lst_redemption_rate_oracle: lstOracle.address, 
    underlying_decimals: 6 
  };

  const [lstVault] = await hostChain.initContract(lstVaultCodeId, lstVaultInitMsg, "lst-vault");

  console.log(`LST vault contract: ${lstVault.address} (We are pretending that untrn is an LST)`);

  await mint.execute({ create_synthetic: { ticker: "amNTRN", decimals: 6 } }, creator, 200000);

  console.log(`created amNTRN synthetic`);
  
  await mint.execute({ set_whitelisted: { minter: hub.address, whitelisted: true }}, creator, 200000);

  console.log(`whitelisted hub for minting`);

  const allSynths: AllAssetsResponse = await mint.query({ all_assets: {}});
  
  const [amNTRN] = allSynths.assets;

  console.log(`amNTRN: ${amNTRN.denom}`);

  expect(amNTRN.decimals).toBe(6);
  expect(amNTRN.ticker).toBe("amntrn");

  const hubWhitelisted: WhitelistedResponse = await mint.query({ whitelisted: { minter: hub.address }});

  expect(hubWhitelisted.whitelisted).toBe(true);

  await hub.execute({ register_vault: { vault: lstVault.address, synthetic: amNTRN.denom }}, creator, 500000);

  console.log(`registered vault with hub`);

  await hub.execute({ set_deposits_enabled: { vault: lstVault.address, enabled: true }}, creator, 200000);

  console.log(`enabled deposits`);

  await hub.execute({ set_advance_enabled: { vault: lstVault.address, enabled: true }}, creator, 200000);

  console.log(`enabled enabled advance`);

  await hub.execute({ deposit: { vault: lstVault.address }}, creator, 1000000, coin(1 * 10**6, "untrn"));

  console.log(`deposited ${1 * 10**6} untrn`);

  const lstVaultPositionQuery: HubQueryMsg = { position: { vault: lstVault.address, account: creator }};

  const lstVaultMetadataQuery: HubQueryMsg = { vault_metadata: { vault: lstVault.address }};

  {
    let position: PositionResponse = await hub.query(lstVaultPositionQuery);

    let vaultMeta: VaultMetadata = await hub.query(lstVaultMetadataQuery);

    expect(BigInt(position.collateral)).toBe(BigInt(10**6));
    expect(BigInt(position.debt)).toBe(BigInt(0));
    expect(BigInt(position.credit)).toBe(BigInt(0));
    expect(BigInt(vaultMeta.collateral_balance)).toBe(BigInt(10**6));
    expect(BigInt(vaultMeta.collateral_shares)).toBe(BigInt(10**18));
    expect(BigInt(vaultMeta.reserve_balance)).toBe(BigInt(0));
    expect(BigInt(vaultMeta.reserve_shares)).toBe(BigInt(0));
    expect(BigInt(vaultMeta.treasury_shares)).toBe(BigInt(0));
    expect(BigInt(vaultMeta.amo_shares)).toBe(BigInt(0));
    expect(vaultMeta.sum_payment_ratio).toBeNull();

    expect(vaultMeta.synthetic).toBe(amNTRN.denom);
  }

  await lstOracle.execute({ rate: "1.1" }, creator, 200000);

  console.log(`set lst redemption rate to 1.1 - 10% earned in yield`);

  console.log(`checking that querying the position now shows credit increase (without requiring an evalulation)`);

  {
    let position: PositionResponse = await hub.query(lstVaultPositionQuery);

    // 10% yield on collateral less 10% treasury fee
    toBeWithinN(1, position.credit, 10**6 * 0.1 * 0.9);
  }

  await hub.execute({ evaluate: { vault: lstVault.address } }, creator, 500000);

  console.log(`evaluated LST vault hub positions`);

  {
    let position: PositionResponse = await hub.query(lstVaultPositionQuery);

    let vaultMeta: VaultMetadata = await hub.query(lstVaultMetadataQuery);

    expect(BigInt(position.collateral)).toBe(BigInt(10**6));
    expect(BigInt(position.debt)).toBe(BigInt(0));
    toBeWithinN(1, position.credit, 10**6 * 0.1 * 0.9);
    expect(BigInt(vaultMeta.collateral_balance)).toBe(BigInt(10**6));
    toBeWithinN(1, vaultMeta.reserve_balance, 10**6 * 0.1 * 0.9);
    expect(BigInt(vaultMeta.treasury_shares)).toBeGreaterThan(0);
    expect(BigInt(vaultMeta.reserve_shares)).toBeGreaterThan(0);
    expect(
      BigInt(vaultMeta.collateral_shares) 
        + BigInt(vaultMeta.reserve_shares) 
        + BigInt(vaultMeta.treasury_shares)
    ).toBe(BigInt(10**18));
    expect(BigInt(vaultMeta.amo_shares)).toBe(BigInt(0));
    expect(BigInt(vaultMeta.sum_payment_ratio?.ratio || 0)).toBeGreaterThan(0);
  }

  await hub.execute({ set_advance_fee_recipient: { vault: lstVault.address, recipient: lstOracle.address }}, creator, 200000);

  console.log(`set advance fee recipient to LST oracle contract (in order to see fee minting)`);

  await hub.execute({ advance: { vault: lstVault.address, amount: String(5 * 10**5) } }, creator, 700000);

  console.log(`advanced 500,000 uamNTRN`);

  let debtPostInitialAdvance = 0;
  let initialAdvanceFeePaid = 0;
  
  {
    const position: PositionResponse = await hub.query(lstVaultPositionQuery);

    const previousCredit = 10**6 * 0.1 * 0.9;

    const advanceDebt = (5 * 10**5) - previousCredit;

    const fee = advanceDebt * 0.0025;

    {
      const balance = await hostChain.balanceOf(creator, amNTRN.denom);
      expect(balance).toBe(BigInt(5 * 10**5));
    }

    {
      const balance = await hostChain.balanceOf(lstOracle.address, amNTRN.denom); 
      toBeWithinN(1, balance, fee);
      initialAdvanceFeePaid = Number(balance);
    }

    expect(BigInt(position.collateral)).toBe(BigInt(10**6));
    toBeWithinN(1, position.debt, advanceDebt + fee);
    expect(BigInt(position.credit)).toBe(0n);

    debtPostInitialAdvance = +position.debt;
  }

  const secondAdvanceAmount = (5 * 10**5) - debtPostInitialAdvance;

  await hub.execute({ advance: { vault: lstVault.address, amount: String(secondAdvanceAmount) } }, creator, 700000);

  console.log(`advanced ${secondAdvanceAmount} in order to get to Max LTV`);
  
  {
    const position: PositionResponse = await hub.query(lstVaultPositionQuery);

    const fee = secondAdvanceAmount * 0.0025;

    {
      const balance = await hostChain.balanceOf(creator, amNTRN.denom);
      toBeWithinN(1, balance, 5 * 10**5 + (secondAdvanceAmount - fee));
    }

    {
      const balance = await hostChain.balanceOf(lstOracle.address, amNTRN.denom); 
      toBeWithinN(1, balance, fee + initialAdvanceFeePaid);
    }

    expect(BigInt(position.debt)).toBe(BigInt(5 * 10**5));
  }
  
  await lstOracle.execute({ rate: "1.21" }, creator, 200000);

  console.log(`set lst redemption rate to 1.21 - another 10% earned in yield`);

  console.log(`checking that querying the position now shows debt repayment (without requiring an evalulation)`);

  let halfDebtPostYield = 0n;

  {
    let position: PositionResponse = await hub.query(lstVaultPositionQuery);

    // Max Debt - (10% yield on collateral - 10% treasury fee)
    let expectedDebt = (5 * 10**5) - (10**6 * 0.1 * 0.9);

    toBeWithinN(2, position.debt, expectedDebt);

    halfDebtPostYield = BigInt(position.debt) / 2n;

  }

  const amNTRNMetadataQuery: MintQueryMsg = { synthetic: { denom: amNTRN.denom } };

  {
    const amNTRNMetaPre: SynthMetadata = await mint.query(amNTRNMetadataQuery);

    const amNTRNSupplyPre = BigInt(amNTRNMetaPre.total_supply);

    await hub.execute(
      { repay_synthetic: { vault: lstVault.address } }, 
      creator, 
      750000, 
      coin(String(halfDebtPostYield), amNTRN.denom)
    );

    console.log(`repaid 50% of debt with ${halfDebtPostYield} amNTRN`);

    const amNTRNMetaPost: SynthMetadata = await mint.query(amNTRNMetadataQuery);

    const amNTRNSupplyPost = BigInt(amNTRNMetaPost.total_supply);

    let position: PositionResponse = await hub.query(lstVaultPositionQuery);

    let vaultMeta: VaultMetadata = await hub.query(lstVaultMetadataQuery);

    expect(BigInt(position.debt)).toBe(BigInt(halfDebtPostYield));

    toBeWithinN(1, vaultMeta.reserve_balance, 10**6 * 0.2 * 0.9);

    expect(amNTRNSupplyPost).toBeLessThan(amNTRNSupplyPre);
    expect(amNTRNSupplyPre - amNTRNSupplyPost).toBe(halfDebtPostYield);
  }

  let quarterDebtPostYield = halfDebtPostYield / 2n;

  let requiredNtrnRepayment = Math.floor(Number(quarterDebtPostYield) / 1.21);

  await hub.execute(
    { repay_underlying: { vault: lstVault.address } }, 
    creator, 
    1000000, 
    coin(String(requiredNtrnRepayment), "untrn")
  );

  console.log(`repaid 50% of debt with ${requiredNtrnRepayment} untrn`);

  {
    let position: PositionResponse = await hub.query(lstVaultPositionQuery);

    let vaultMeta: VaultMetadata = await hub.query(lstVaultMetadataQuery);

    toBeWithinN(2, position.debt, Number(halfDebtPostYield - quarterDebtPostYield));

    toBeWithinN(2, vaultMeta.reserve_balance, (10**6 * 0.2 * 0.9) + Number(quarterDebtPostYield));
  }

  await hub.execute(
    { withdraw: { vault: lstVault.address, amount:  String(5 * 10**5)} }, 
    creator, 
    1000000, 
  );

  console.log(`withdrawn 50% of collateral`);

  let claimableNtrn = 0;
  let remainingDebt = 0;
  let remainingCollateral = 0;
  
  {
    let position: PositionResponse = await hub.query(lstVaultPositionQuery);

    let vaultMeta: VaultMetadata = await hub.query(lstVaultMetadataQuery);

    expect(+position.collateral).toBe(5 * 10**5)
    expect(+vaultMeta.collateral_balance).toBe(5 * 10**5)

    let claimable: ClaimableResponse = await lstVault.query({ claimable: { address: creator }});

    toBeWithinN(1, +claimable.amount, Math.floor((5 * 10**5) / 1.21));

    console.log(`${claimable.amount} is available to claim`);

    claimableNtrn = +claimable.amount;
    remainingDebt = +position.debt;
    remainingCollateral = +position.collateral;
  }

  {
    let ntrnBalancePreClaim = await hostChain.balanceOf(creator, "untrn");

    await lstVault.execute({ claim: {} }, creator, 500000);

    console.log(`claimed untrn from vault`);

    let ntrnBalancePostClaim = await hostChain.balanceOf(creator, "untrn");

    // within 2000 due to gas spend
    toBeWithinN(2000, Number(ntrnBalancePostClaim) - Number(ntrnBalancePreClaim), claimableNtrn);
  }

  await hub.execute({ self_liquidate: { vault: lstVault.address }}, creator, 1000000);

  console.log(`self liquidated position`);
  
  {
    let position: PositionResponse = await hub.query(lstVaultPositionQuery);

    let vaultMeta: VaultMetadata = await hub.query(lstVaultMetadataQuery);

    expect(+position.collateral).toBe(0)
    expect(+position.debt).toBe(0)
    expect(+position.credit).toBe(0)
    expect(+vaultMeta.collateral_balance).toBe(0)

    let claimable: ClaimableResponse = await lstVault.query({ claimable: { address: creator }});

    toBeWithinN(1, +claimable.amount, Math.floor((remainingCollateral - remainingDebt) / 1.21));

    console.log(`${claimable.amount} is available to claim`);
  }
  
  {
    let ntrnBalancePreClaim = await hostChain.balanceOf(creator, "untrn");

    await lstVault.execute({ claim: {} }, creator, 500000);

    console.log(`claimed untrn from vault`);

    let ntrnBalancePostClaim = await hostChain.balanceOf(creator, "untrn");

    // within 2000 due to gas spend
    toBeWithinN(
      2000, 
      Number(ntrnBalancePostClaim) - Number(ntrnBalancePreClaim), 
      Math.floor((remainingCollateral - remainingDebt) / 1.21)
    );
  }

  let redeemableBalance = 0;
  let amNtrnSupplyPreRedemption = 0;

  {
    const vaultMeta: VaultMetadata = await hub.query(lstVaultMetadataQuery);
    const amNTRNMeta: SynthMetadata = await mint.query(amNTRNMetadataQuery);

    amNtrnSupplyPreRedemption = +amNTRNMeta.total_supply;
    redeemableBalance = +vaultMeta.reserve_balance;
  }

  await hub.execute(
    { redeem: { vault: lstVault.address } }, 
    creator, 
    1000000, 
    coin(String(redeemableBalance / 2), amNTRN.denom)
  );

  console.log(`redeemed ${redeemableBalance / 2} amNTRN`);

  {
    let ntrnBalancePreClaim = await hostChain.balanceOf(creator, "untrn");

    await lstVault.execute({ claim: {} }, creator, 500000);

    console.log(`claimed untrn from vault`);

    let ntrnBalancePostClaim = await hostChain.balanceOf(creator, "untrn");

    // within 2000 due to gas spend
    toBeWithinN(
      2000, 
      Number(ntrnBalancePostClaim) - Number(ntrnBalancePreClaim), 
      Math.floor((redeemableBalance / 2) / 1.21)
    );

    const amNTRNMeta: SynthMetadata = await mint.query(amNTRNMetadataQuery);

    const amNtrnSupplyPostRedemption = +amNTRNMeta.total_supply;

    expect(amNtrnSupplyPreRedemption - amNtrnSupplyPostRedemption).toBe(redeemableBalance / 2);
  }

  {
    const amNtrnMetaPreMint: SynthMetadata = await mint.query(amNTRNMetadataQuery);

    const vaultMetaPreMint: VaultMetadata = await hub.query(lstVaultMetadataQuery);

    const amNtrnSupplyPreMint = +amNtrnMetaPreMint.total_supply;

    const amNtrnBalancePreMint = (await hostChain.balanceOf(creator, amNTRN.denom)) || 0n;
    
    let mintDepositAmount = 1000000;
    let mintDepositValue = Math.floor(mintDepositAmount * 1.21);
  
    await hub.execute({ mint: { vault: lstVault.address }}, creator, 1000000, coin(mintDepositAmount, "untrn"));

    console.log(`minted ${mintDepositValue} amNTRN by depositing ${mintDepositAmount} untrn`);

    const amNtrnMetaPostMint: SynthMetadata = await mint.query(amNTRNMetadataQuery);

    const vaultMetaPostMint: VaultMetadata = await hub.query(lstVaultMetadataQuery);

    const amNtrnSupplyPostMint = +amNtrnMetaPostMint.total_supply;
    
    const amNtrnBalancePostMint = (await hostChain.balanceOf(creator, amNTRN.denom)) || 0n;

    toBeWithinN(1, amNtrnSupplyPostMint - amNtrnSupplyPreMint, mintDepositValue);
    toBeWithinN(1, amNtrnBalancePostMint - amNtrnBalancePreMint, mintDepositValue);
    toBeWithinN(1, +vaultMetaPostMint.reserve_balance - +vaultMetaPreMint.reserve_balance, mintDepositValue);
    expect(+vaultMetaPostMint.reserve_shares).toBeGreaterThan(+vaultMetaPreMint.reserve_shares);
  }

  const redepositAmount = Math.floor(1000000 / 1.21);

  await hub.execute(
    { deposit: { vault: lstVault.address }}, 
    creator, 
    1000000, 
    coin(redepositAmount, "untrn")
  )

  console.log(`re-deposited: ${redepositAmount}`)

  await lstOracle.execute({ rate: "1.815" }, creator, 200000);

  console.log(`set lst redemption rate to 1.815 - another 50% earned in yield`);

  {
    await hub.execute({ evaluate: { vault: lstVault.address }}, creator, 500000);

    const positionPreConv: PositionResponse = await hub.query(lstVaultPositionQuery);
    const vaultMetaPreConv: VaultMetadata = await hub.query(lstVaultMetadataQuery);

    const creditToConvert = +positionPreConv.credit;
    expect(creditToConvert).toBeGreaterThan(0);
    expect(creditToConvert).toBeLessThanOrEqual(+vaultMetaPreConv.reserve_balance);

    await hub.execute(
      { convert_credit: { vault: lstVault.address, amount: String(creditToConvert) }}, 
      creator, 
      500000
    );

    const positionPostConv: PositionResponse = await hub.query(lstVaultPositionQuery);
    const vaultMetaPostConv: VaultMetadata = await hub.query(lstVaultMetadataQuery);

    console.log(`converted all available credit (${creditToConvert}) to collateral: ${positionPostConv.collateral}`);

    expect(+positionPostConv.credit).toBe(0);
    expect(+positionPostConv.collateral).toBe(+positionPreConv.collateral + creditToConvert);
    expect(+vaultMetaPostConv.collateral_balance).toBe(+vaultMetaPreConv.collateral_balance + creditToConvert);
    expect(+vaultMetaPostConv.collateral_shares).toBeGreaterThan(+vaultMetaPreConv.collateral_shares);
    expect(+vaultMetaPostConv.reserve_balance).toBe(+vaultMetaPreConv.reserve_balance - creditToConvert);
    expect(+vaultMetaPostConv.reserve_shares).toBeLessThan(+vaultMetaPreConv.reserve_shares);
  }
  
});

