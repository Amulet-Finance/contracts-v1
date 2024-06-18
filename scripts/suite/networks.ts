import { ORG, VERSION, NEUTRON_GAS_PRICE, GAIA_GAS_PRICE } from "./constants";
import { CosmoparkNetworkConfig } from "@neutron-org/cosmopark/lib/types";

const gaiaConfig: CosmoparkNetworkConfig = {
    image: `gaia-${ORG}-${VERSION}`,
    denom: "stake",
    binary: "gaiad",
    chain_id: "testgaia",
    prefix: "cosmos",
    validators: 10,
    type: "default",
    validators_balance: [
        "1000000000",
        "1100000000",
        "1200000000",
        "1300000000",
        "1400000000",
        "1500000000",
        "1600000000",
        "1700000000",
        "1800000000",
        "1900000000",
    ],
    loglevel: "info",
    trace: true,
    commands: {
        addGenesisAccount: "genesis add-genesis-account",
        gentx: "genesis gentx",
        collectGenTx: "genesis collect-gentxs",
    },
    genesis_opts: {
        "app_state.slashing.params.downtime_jail_duration": "100s",
        "app_state.slashing.params.signed_blocks_window": "10",
        "app_state.slashing.params.min_signed_per_window": "0.9",
        "app_state.slashing.params.slash_fraction_downtime": "0.1",
        "app_state.staking.params.validator_bond_factor": "10",
        "app_state.staking.params.unbonding_time": "1814400s",
        "app_state.mint.minter.inflation": "0.9",
        "app_state.mint.params.inflation_max": "0.95",
        "app_state.mint.params.inflation_min": "0.5",
        "app_state.interchainaccounts.host_genesis_state.params.allow_messages": [
            "*",
        ],
    },
    config_opts: {
        "rpc.laddr": "tcp://0.0.0.0:26657",
        "consensus.timeout_commit": "1s",
        "consensus.timeout_propose": "1s",
    },
    app_opts: {
        "api.enable": true,
        "api.address": "tcp://0.0.0.0:1317",
        "api.swagger": true,
        "grpc.enable": true,
        "grpc.address": "0.0.0.0:9090",
        "minimum-gas-prices": `${GAIA_GAS_PRICE.amount}${GAIA_GAS_PRICE.denom}`,
        "rosetta.enable": true,
    },
    public: false,
    upload: ["./scripts/suite/artifacts/init-gaia.sh"],
    post_init: ["chmod +x /opt/init-gaia.sh"],
    post_start: [`/opt/init-gaia.sh > /opt/init-gaia.log 2>&1`],
};

const neutronConfig: CosmoparkNetworkConfig = {
    image: `neutron-${ORG}-${VERSION}`,
    denom: "untrn",
    binary: "neutrond",
    chain_id: "ntrntest",
    prefix: "neutron",
    type: "ics",
    loglevel: "info",
    trace: true,
    public: false,
    commands: {},
    genesis_opts: {
        "app_state.globalfee.params.minimum_gas_prices": [NEUTRON_GAS_PRICE],
        "app_state.feemarket.state.base_gas_price": NEUTRON_GAS_PRICE.amount,
        "app_state.feemarket.params.fee_denom": NEUTRON_GAS_PRICE.denom,
        "app_state.feemarket.params.enabled": false,
        "app_state.feemarket.params.min_base_gas_price": NEUTRON_GAS_PRICE.amount,
        "app_state.crisis.constant_fee.denom": NEUTRON_GAS_PRICE.denom,
        "app_state.slashing.params.signed_blocks_window": "10",
        "app_state.slashing.params.min_signed_per_window": "0.9",
        "app_state.slashing.params.slash_fraction_downtime": "0.1",
        "app_state.slashing.params.slash_fraction_double_sign": "0.01",
        "app_state.interchainaccounts.host_genesis_state.params.allow_messages": [
            "*",
        ],
        "consensus_params.block.max_gas": "1000000000",
    },
    config_opts: {
        "rpc.laddr": "tcp://0.0.0.0:26657",
    },
    app_opts: {
        "api.enable": true,
        "api.address": "tcp://0.0.0.0:1317",
        "api.swagger": true,
        "grpc.enable": true,
        "grpc.address": "0.0.0.0:9090",
        "minimum-gas-prices": `${NEUTRON_GAS_PRICE.amount}${NEUTRON_GAS_PRICE.denom}`,
        "rosetta.enable": true,
    },
};

export default {
    gaia: gaiaConfig,
    neutron: neutronConfig,
};
