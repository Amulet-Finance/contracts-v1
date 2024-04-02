interface SINGLE_CHAIN_ENV {
  WALLET_MNEMONIC: string | undefined;
  HOST_CHAIN_RPC: string | undefined;
  HOST_CHAIN_GAS_PRICE: string | undefined;
  HOST_CHAIN_PREFIX: string | undefined;
}

interface singleChainConfig {
  WALLET_MNEMONIC: string;
  HOST_CHAIN_RPC: string;
  HOST_CHAIN_GAS_PRICE: string;
  HOST_CHAIN_PREFIX: string;
}

const getRawE2ETestConfig = (): SINGLE_CHAIN_ENV => {
  return {
    WALLET_MNEMONIC: process.env.TEST_WALLET_MNEMONIC,
    HOST_CHAIN_RPC: process.env.TEST_HOST_CHAIN_RPC,
    HOST_CHAIN_GAS_PRICE: process.env.TEST_HOST_CHAIN_GAS_PRICE,
    HOST_CHAIN_PREFIX: process.env.TEST_HOST_CHAIN_PREFIX,
  };
};

const getRawDeployConfig = (): SINGLE_CHAIN_ENV => {
  return {
    WALLET_MNEMONIC: process.env.DEPLOY_WALLET_MNEMONIC,
    HOST_CHAIN_RPC: process.env.DEPLOY_HOST_CHAIN_RPC,
    HOST_CHAIN_GAS_PRICE: process.env.DEPLOY_HOST_CHAIN_GAS_PRICE,
    HOST_CHAIN_PREFIX: process.env.DEPLOY_HOST_CHAIN_PREFIX,
  };
};

const getSingleChainConfig = (
  config: SINGLE_CHAIN_ENV,
): singleChainConfig => {
  for (const [key, value] of Object.entries(config)) {
    if (value === undefined) {
      throw new Error(`Missing expected environment variable ${key}`);
    }
  }
  return config as singleChainConfig;
};

export const e2eTestConfig = () =>
  getSingleChainConfig(getRawE2ETestConfig());

export const deployConfig = () =>
  getSingleChainConfig(getRawDeployConfig());

