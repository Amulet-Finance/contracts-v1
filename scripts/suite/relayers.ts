import { ORG, VERSION, NEUTRON_GAS_PRICE, GAIA_GAS_PRICE } from "./constants";
import { CosmoparkRelayer } from "@neutron-org/cosmopark/lib/types";

const hermesRelayer: CosmoparkRelayer = {
  type: "hermes",
  networks: ["gaia", "neutron"],
  connections: [["gaia", "neutron"]],
  mnemonic: "",
  binary: "hermes",
  image: `hermes-${ORG}-${VERSION}`,
  config: {
    "chains.0.max_gas": 1_000_000_000,
    "chains.0.default_gas": 1_000_000,
    "chains.0.gas_multiplier": 1.5,
    "chains.0.trusting_period": "7days",
    "chains.1.max_gas": 1_000_000_000,
    "chains.1.gas_multiplier": 1.5,
    "chains.1.default_gas": 1_000_000,
    "chains.1.gas_price": {
      price: Number(NEUTRON_GAS_PRICE.amount),
      denom: NEUTRON_GAS_PRICE.denom,
    },
    "chains.0.gas_price": {
      price: Number(GAIA_GAS_PRICE.amount),
      denom: GAIA_GAS_PRICE.denom,
    },
    "chains.0.clock_drift": "1m",
    "chains.1.clock_drift": "1m",
  },
  log_level: "info",
  balance: "10000000000000",
};

const neutronRelayer: CosmoparkRelayer = {
  type: "neutron",
  networks: ["gaia", "neutron"],
  mnemonic: "",
  binary: "neutron-query-relayer",
  image: `neutron-relayer-${ORG}-${VERSION}`,
  log_level: "info",
  balance: "1000000000000",
};

export default {
  hermes: hermesRelayer,
  neutron: neutronRelayer,
};
