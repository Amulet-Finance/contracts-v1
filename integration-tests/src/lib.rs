use anyhow::{anyhow, Context, Result};
use cosmwasm_std::Coin;
use osmosis_test_tube::{
    osmosis_std, Account, Module, OsmosisTestApp as TestApp, Runner, SigningAccount, Wasm,
};
use serde::{de::DeserializeOwned, Serialize};

pub fn init_accounts<const N: usize>(app: &TestApp, coins: &[Coin]) -> Result<[SigningAccount; N]> {
    let Ok(accounts) = app.init_accounts(coins, N as u64)?.try_into() else {
        panic!("osmosos test app did not create {N} accounts");
    };

    Ok(accounts)
}

pub fn store_contract_artifact(
    app: &TestApp,
    admin: &SigningAccount,
    contract_name: &str,
) -> Result<(u64, String)> {
    let wasm_byte_code = std::fs::read(format!("../artifacts/{contract_name}.wasm"))
        .context(format!("loading {contract_name} wasm failed"))?;
    let sha256_bytes = hmac_sha256::Hash::hash(&wasm_byte_code);
    let sha256_hex = hex::encode_upper(sha256_bytes);
    let code_id = Wasm::new(app)
        .store_code(&wasm_byte_code, None, admin)?
        .data
        .code_id;
    Ok((code_id, sha256_hex))
}

pub fn instantiate_contract<Msg: Serialize>(
    app: &TestApp,
    admin: &SigningAccount,
    code_id: u64,
    msg: &Msg,
    funds: &[Coin],
) -> Result<String> {
    let address = Wasm::new(app)
        .instantiate(
            code_id,
            msg,
            Some(admin.address().as_str()),
            Some("test-contract"),
            funds,
            admin,
        )?
        .data
        .address;
    Ok(address)
}

pub fn execute_contract<Msg: Serialize>(
    app: &TestApp,
    admin: &SigningAccount,
    contract: &str,
    msg: &Msg,
    funds: &[Coin],
) -> Result<()> {
    Wasm::new(app).execute(contract, msg, funds, admin)?;
    Ok(())
}

pub fn query_contract<Msg: Serialize, Response: DeserializeOwned>(
    app: &TestApp,
    contract: &str,
    msg: &Msg,
) -> Result<Response> {
    let res = Wasm::new(app).query(contract, msg)?;
    Ok(res)
}

pub fn query_staking_delegations(app: &TestApp, delegator: &str) -> Result<Vec<(String, u128)>> {
    let res: osmosis_std::types::cosmos::staking::v1beta1::QueryDelegatorDelegationsResponse = app
        .query(
            "/cosmos.staking.v1beta1.Query/DelegatorDelegations",
            &osmosis_std::types::cosmos::staking::v1beta1::QueryDelegatorDelegationsRequest {
                delegator_addr: delegator.to_owned(),
                pagination: None,
            },
        )?;

    let mut delegations = Vec::with_capacity(res.delegation_responses.len());

    for d in res.delegation_responses {
        let validator = d.delegation.unwrap().validator_address;
        let amount = d.balance.unwrap().amount.parse()?;
        delegations.push((validator, amount));
    }

    Ok(delegations)
}

#[derive(Debug)]
pub struct UnbondingEntry {
    pub amount: u128,
    pub completion_timestamp: u64,
}

#[derive(Debug)]
pub struct StakingUnbondings {
    pub validator: String,
    pub entries: Vec<UnbondingEntry>,
}

pub fn query_staking_unbondings(app: &TestApp, delegator: &str) -> Result<Vec<StakingUnbondings>> {
    let res: osmosis_std::types::cosmos::staking::v1beta1::QueryDelegatorUnbondingDelegationsResponse =
        app.query(
            "/cosmos.staking.v1beta1.Query/DelegatorUnbondingDelegations",
            &osmosis_std::types::cosmos::staking::v1beta1::QueryDelegatorUnbondingDelegationsRequest {
                delegator_addr: delegator.to_owned(),
                pagination: None
            })?;

    let mut unbondings = Vec::with_capacity(res.unbonding_responses.len());

    for u in res.unbonding_responses {
        let entries = u.entries.iter().try_fold(
            Vec::with_capacity(u.entries.len()),
            |mut entries, e| -> Result<Vec<_>> {
                entries.push(UnbondingEntry {
                    amount: e.balance.parse()?,
                    completion_timestamp: e
                        .completion_time
                        .as_ref()
                        .ok_or_else(|| anyhow!("no completion time"))?
                        .seconds
                        .try_into()?,
                });
                Ok(entries)
            },
        )?;
        unbondings.push(StakingUnbondings {
            validator: u.validator_address,
            entries,
        });
    }

    Ok(unbondings)
}

pub fn query_bank_balance(app: &TestApp, account: &str, denom: &str) -> Result<u128> {
    let res: osmosis_std::types::cosmos::bank::v1beta1::QueryBalanceResponse = app.query(
        "/cosmos.bank.v1beta1.Query/Balance",
        &osmosis_std::types::cosmos::bank::v1beta1::QueryBalanceRequest {
            address: account.to_owned(),
            denom: denom.to_owned(),
        },
    )?;

    let balance: u128 = res
        .balance
        .ok_or_else(|| anyhow!("no {denom} balance found for {account}"))?
        .amount
        .parse()?;

    Ok(balance)
}
