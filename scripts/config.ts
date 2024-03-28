interface E2E_TEST_ENV {
  WALLET_MNEMONIC: string | undefined;
  HOST_CHAIN_RPC: string | undefined;
  HOST_CHAIN_GAS_PRICE: string | undefined;
  HOST_CHAIN_PREFIX: string | undefined;
}

interface e2eTestConfig {
  WALLET_MNEMONIC: string;
  HOST_CHAIN_RPC: string;
  HOST_CHAIN_GAS_PRICE: string;
  HOST_CHAIN_PREFIX: string;
}

const getRawE2ETestConfig = (): E2E_TEST_ENV => {
  return {
    WALLET_MNEMONIC: process.env.TEST_WALLET_MNEMONIC,
    HOST_CHAIN_RPC: process.env.TEST_HOST_CHAIN_RPC,
    HOST_CHAIN_GAS_PRICE: process.env.TEST_HOST_CHAIN_GAS_PRICE,
    HOST_CHAIN_PREFIX: process.env.TEST_HOST_CHAIN_PREFIX,
  };
};

const getE2ETestConfig = (
  config: E2E_TEST_ENV,
): e2eTestConfig => {
  for (const [key, value] of Object.entries(config)) {
    if (value === undefined) {
      throw new Error(`Missing expected environment variable ${key}`);
    }
  }
  return config as e2eTestConfig;
};

export const e2eTestConfig = () =>
  getE2ETestConfig(getRawE2ETestConfig());

