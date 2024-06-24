import { readFileSync } from "fs";
import { resolve } from "path";

export function getPackageVersion(): string {
  const packageJsonPath = resolve(__dirname, "../../package.json");
  const packageJson = JSON.parse(readFileSync(packageJsonPath, "utf8"));
  return packageJson.version;
}

export const GAIA_GAS_PRICE = {
  amount: "0",
  denom: "stake",
};

export const NEUTRON_GAS_PRICE = {
  amount: "0.03",
  denom: "untrn",
};

export const VERSION = getPackageVersion();
export const ORG = "amulet";

export const GAS_PRICES = {
  neutron: NEUTRON_GAS_PRICE,
  gaia: GAIA_GAS_PRICE,
};

export const WALLET_KEYS = [
  "master",
  "hermes",
  "neutronqueryrelayer",
  "demowallet1",
  "demo1",
  "demo2",
  "demo3",
  "relayer_0",
  "relayer_1",
] as const;

export const WALLET_MNEMONIC_WORD_COUNT = 12;

export const GENESIS_ALLOCATION = 1_000_000_000_000;
