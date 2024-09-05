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

interface DUAL_CHAIN_ENV {
  WALLET_MNEMONIC: string | undefined;
  HOST_CHAIN_RPC: string | undefined;
  HOST_CHAIN_GAS_PRICE: string | undefined;
  HOST_CHAIN_PREFIX: string | undefined;
  REMOTE_CHAIN_RPC: string | undefined;
  REMOTE_CHAIN_GAS_PRICE: string | undefined;
  REMOTE_CHAIN_PREFIX: string | undefined;
}

interface dualChainConfig {
  WALLET_MNEMONIC: string;
  HOST_CHAIN_RPC: string;
  HOST_CHAIN_GAS_PRICE: string;
  HOST_CHAIN_PREFIX: string;
  REMOTE_CHAIN_RPC: string;
  REMOTE_CHAIN_GAS_PRICE: string;
  REMOTE_CHAIN_PREFIX: string;
}

const getRawE2ETestConfig = (): DUAL_CHAIN_ENV => {
  return {
    WALLET_MNEMONIC: process.env.TEST_WALLET_MNEMONIC,
    HOST_CHAIN_RPC: process.env.TEST_HOST_CHAIN_RPC,
    HOST_CHAIN_GAS_PRICE: process.env.TEST_HOST_CHAIN_GAS_PRICE,
    HOST_CHAIN_PREFIX: process.env.TEST_HOST_CHAIN_PREFIX,
    REMOTE_CHAIN_RPC: process.env.TEST_REMOTE_CHAIN_RPC,
    REMOTE_CHAIN_GAS_PRICE: process.env.TEST_REMOTE_CHAIN_GAS_PRICE,
    REMOTE_CHAIN_PREFIX: process.env.TEST_REMOTE_CHAIN_PREFIX,
  };
};

const getRawDeployConfig = (mainnet: boolean): SINGLE_CHAIN_ENV => {
  return {
    WALLET_MNEMONIC: mainnet
      ? process.env.MAINNET_DEPLOY_WALLET_MNEMONIC
      : process.env.TESTNET_DEPLOY_WALLET_MNEMONIC,
    HOST_CHAIN_RPC: mainnet
      ? process.env.MAINNET_DEPLOY_HOST_CHAIN_RPC
      : process.env.TESTNET_DEPLOY_HOST_CHAIN_RPC,
    HOST_CHAIN_GAS_PRICE: mainnet
      ? process.env.MAINNET_DEPLOY_HOST_CHAIN_GAS_PRICE
      : process.env.TESTNET_DEPLOY_HOST_CHAIN_GAS_PRICE,
    HOST_CHAIN_PREFIX: mainnet
      ? process.env.MAINNET_DEPLOY_HOST_CHAIN_PREFIX
      : process.env.TESTNET_DEPLOY_HOST_CHAIN_PREFIX,
  };
};

const getSingleChainConfig = (config: SINGLE_CHAIN_ENV): singleChainConfig => {
  for (const [key, value] of Object.entries(config)) {
    if (value === undefined) {
      throw new Error(`Missing expected environment variable ${key}`);
    }
  }
  return config as singleChainConfig;
};

const getDualChainConfig = (config: DUAL_CHAIN_ENV): dualChainConfig => {
  for (const [key, value] of Object.entries(config)) {
    if (value === undefined) {
      throw new Error(`Missing expected environment variable ${key}`);
    }
  }
  return config as dualChainConfig;
};

export const e2eTestConfig = () => getDualChainConfig(getRawE2ETestConfig());

export const deployConfig = (mainnet: boolean) =>
  getSingleChainConfig(getRawDeployConfig(mainnet));
