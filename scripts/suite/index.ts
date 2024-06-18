import Cosmopark, { CosmoparkConfig } from "@neutron-org/cosmopark";
import {
    CosmoparkNetworkConfig,
    CosmoparkRelayer,
} from "@neutron-org/cosmopark/lib/types";
import { runCommand, sleep, waitFor } from "../utils";
import { DirectSecp256k1HdWallet } from "@cosmjs/proto-signing";
import { Client as NeutronClient } from "@neutron-org/client-ts";
import {
    QueryClient,
    setupSlashingExtension,
    setupStakingExtension,
    StakingExtension,
} from "@cosmjs/stargate";
import { connectComet } from "@cosmjs/tendermint-rpc";
import networkConfigs from "./networks";
import relayerConfigs from "./relayers";
import {
    GAS_PRICES,
    WALLET_KEYS,
    WALLET_MNEMONIC_WORD_COUNT,
} from "./constants";
import * as path from "node:path";
import { BunFile } from "bun";

async function walletReducer(
    acc: Promise<Record<WalletKeys, string>>,
    key: WalletKeys
): Promise<Record<WalletKeys, string>> {
    try {
        const accObj = await acc;
        const wallet = await DirectSecp256k1HdWallet.generate(
            WALLET_MNEMONIC_WORD_COUNT
        );
        accObj[key] = wallet.mnemonic;
        return accObj;
    } catch (err) {
        throw err;
    }
}

export async function generateWallets(): Promise<Record<WalletKeys, string>> {
    return WALLET_KEYS.reduce(
        walletReducer,
        Promise.resolve({} as Record<WalletKeys, string>)
    );
}

export function isCosmoparkNetworkConfigKey(
    key: any
): key is keyof CosmoparkNetworkConfig {
    return [
        "binary",
        "chain_id",
        "denom",
        "image",
        "prefix",
        "trace",
        "validators",
        "validators_balance",
        "loglevel",
        "type",
        "commands",
        "genesis_opts",
        "config_opts",
        "app_opts",
        "upload",
        "post_start",
    ].includes(key);
}

export function isCosmoparkRelayerKey(key: any): key is keyof CosmoparkRelayer {
    return [
        "type",
        "networks",
        "connections",
        "environment",
        "image",
        "log_level",
        "binary",
        "config",
        "mnemonic",
        "balance",
    ].includes(key);
}

export function getNetworkConfig(
    id: NetworkKeys,
    opts: NetworkOptsType = {}
): CosmoparkNetworkConfig {
    let config = { ...networkConfigs[id] };

    const extOpts = { ...opts["*"], ...opts[id] };

    for (const [key, value] of Object.entries(extOpts)) {
        if (isCosmoparkNetworkConfigKey(key)) {
            // Handle object merges
            if (
                typeof value === "object" &&
                value !== null &&
                !Array.isArray(value)
            ) {
                config = {
                    ...config,
                    [key]: { ...(config[key] as object), ...value },
                };
            } else {
                // Directly assign for arrays and other types
                config = { ...config, [key]: value };
            }
        } else {
            console.warn(`Key ${key} is not a valid config property.`);
        }
    }

    return config;
}

export function getRelayerConfig(
    id: RelayerKeys,
    opts: RelayerOptsType = {}
): CosmoparkRelayer {
    let config = { ...relayerConfigs[id] };

    for (const [key, value] of Object.entries(opts)) {
        if (isCosmoparkRelayerKey(key)) {
            // Handle object merges, excluding arrays
            if (
                typeof value === "object" &&
                value !== null &&
                !Array.isArray(value)
            ) {
                config = {
                    ...config,
                    [key]: { ...(config[key] as object), ...value },
                };
            } else {
                // Directly assign for arrays and other types
                config = { ...config, [key]: value };
            }
        } else {
            console.warn(`Key ${key} is not a valid config property.`);
        }
    }

    return config;
}

export function awaitNeutronChannels(rest: string, rpc: string): Promise<void> {
    return waitFor(async () => {
        try {
            const client = new NeutronClient({
                apiURL: `http://${rest}`,
                rpcURL: `http://${rpc}`,
                prefix: "neutron",
            });
            const res = await client.IbcCoreChannelV1.query.queryChannels(undefined, {
                timeout: 1000,
            });
            if (
                res.data.channels &&
                res.data.channels.length > 0 &&
                res.data.channels[0].counterparty &&
                res.data.channels[0].counterparty.channel_id !== ""
            ) {
                return true;
            }
            await sleep(10000);
            return false;
        } catch (e) {
            await sleep(10000);
            return false;
        }
    }, 100_000);
}

export function getRelayerWallet(
    wallets: Record<WalletKeys, string>,
    relayer: RelayerKeys
) {
    if (relayer === "neutron") {
        return wallets.neutronqueryrelayer;
    } else if (relayer === "hermes") {
        return wallets.hermes;
    }

    throw new Error("Invalid relayer type. Could not get wallet.");
}

export async function initCosmopark(
    ctx: string = "default",
    networks: NetworkKeys[] = ["gaia", "neutron"],
    relayerOverrides: RelayerOptsType = {},
    networkOverrides: NetworkOptsType = {}
): Promise<Cosmopark> {
    try {
        // Create test environment wallets
        const wallets = await generateWallets();

        // Create the cosmopark config
        const baseConfig: CosmoparkConfig = {
            context: ctx,
            networks: {},
            master_mnemonic: wallets.master,
            loglevel: "info",
            wallets: {
                demowallet1: {
                    mnemonic: wallets.demowallet1,
                    balance: "1000000000",
                },
                demo1: { mnemonic: wallets.demo1, balance: "1000000000" },
                demo2: { mnemonic: wallets.demo2, balance: "1000000000" },
                demo3: { mnemonic: wallets.demo3, balance: "1000000000" },
            },
            relayers: Object.values(relayerConfigs),
        };

        // Configure networks
        for (const network of networks) {
            baseConfig.networks[network] = getNetworkConfig(
                network,
                networkOverrides
            );
        }

        // Configure relayers
        baseConfig.relayers = Object.keys(relayerConfigs).map((relayer) => {
            const relayerKey = relayer as RelayerKeys;

            return {
                ...getRelayerConfig(relayerKey, relayerOverrides),
                networks,
                mnemonic: getRelayerWallet(wallets, relayerKey),
            };
        });

        // 6. Create the cosmopark instance
        const cosmoparkInstance = await Cosmopark.create(baseConfig);

        // 7. Wait for the first block
        await cosmoparkInstance.awaitFirstBlock();

        // 8. Wait for neutron channels to be ready
        if (networks.includes("neutron")) {
            await awaitNeutronChannels(
                `127.0.0.1:${cosmoparkInstance.ports["neutron"].rest}`,
                `127.0.0.1:${cosmoparkInstance.ports["neutron"].rpc}`
            ).catch((err: unknown) => {
                if (err instanceof Error) {
                    console.log(`Failed to await neutron channels: ${err.message}`);
                } else {
                    console.log(`Unknown error awaiting neutron channels:`, err);
                }
                throw err;
            });
        }

        return cosmoparkInstance;
    } catch (err) {
        throw err;
    }
}

export class TestSuite implements ITestSuite {
    private cosmopark!: Cosmopark;
    private gaiaQueryClient!: QueryClient & SlashingExtension & StakingExtension;

    private constructor() { }

    public static async create({
        ctx = "default",
        networks = ["gaia", "neutron"],
        relayerOverrides = {},
        networkOverrides = {},
    }: TestSuiteParams = {}): Promise<ITestSuite> {
        try {
            const ts = new TestSuite();
            await ts.init(ctx, networks, relayerOverrides, networkOverrides);
            return ts;
        } catch (err) {
            console.error("TestSuite.create:", err);
            return Promise.reject(err);
        }
    }

    private async init(
        ctx?: string,
        networks?: NetworkKeys[],
        relayerOverrides?: RelayerOptsType,
        networkOverrides?: NetworkOptsType
    ): Promise<void> {
        try {
            this.cosmopark = await initCosmopark(
                ctx,
                networks,
                relayerOverrides,
                networkOverrides
            );

            const rpc = `http://127.0.0.1:${this.cosmopark.ports["gaia"].rpc}`;
            const client = await connectComet(rpc);
            this.gaiaQueryClient = QueryClient.withExtensions(
                client,
                setupSlashingExtension,
                setupStakingExtension
            );
        } catch (err) {
            return Promise.reject(err);
        }
    }

    getHostPrefix(network: HostNetworkKeys = "neutron"): string {
        return this.cosmopark.networks[network].config.prefix;
    }

    getHostRpc(network: HostNetworkKeys = "neutron"): string {
        return `127.0.0.1:${this.cosmopark.ports[network].rpc}`;
    }

    getHostGasPrices(network: HostNetworkKeys = "neutron"): string {
        const prices = GAS_PRICES[network];
        if (!prices)
            throw new Error(`Was unable to find gas prices for ${network}`);
        return `${prices.amount}${prices.denom}`;
    }

    getRemotePrefix(network: RemoteNetworkKeys = "gaia"): string {
        return this.cosmopark.networks[network].config.prefix;
    }

    getRemoteRpc(network: RemoteNetworkKeys = "gaia"): string {
        return `127.0.0.1:${this.cosmopark.ports[network].rpc}`;
    }

    getRemoteGasPrices(network: RemoteNetworkKeys = "gaia"): string {
        const prices = GAS_PRICES[network];
        if (!prices)
            throw new Error(`Was unable to find gas prices for ${network}`);

        return `${prices.amount}${prices.denom}`;
    }

    getMasterMnemonic(): string {
        return this.cosmopark.config.master_mnemonic;
    }

    async slashValidator(): Promise<string> {
        try {
            let slashedAddress = "";
            const ctx = this.cosmopark.config.context ?? "default";
            const validatorCount = this.cosmopark.networks["gaia"].config.validators;
            if (!validatorCount) throw new Error("No validator count was found.");

            // Always pauses the last validator defined by cosmopark
            const validatorContainer = `${ctx}-gaia_val${validatorCount}-1`;
            await runCommand(`docker pause ${validatorContainer}`);

            await waitFor(async () => {
                let found = false;

                const signingInfos = await this.gaiaQueryClient.slashing.signingInfos();

                for (const info of signingInfos.info) {
                    if (!found) {
                        found = info.jailedUntil.seconds > 0;
                        if (found) {
                            slashedAddress = info.address;
                        }
                    }
                }

                return found;
            }, 60000);

            await runCommand(`docker unpause ${validatorContainer}`);

            return slashedAddress;
        } catch (err: unknown) {
            console.error("TestSuite.slashValidator:", err);
            return Promise.reject(err);
        }
    }

    async pauseIcqRelaying(): Promise<void> {
        try {
            const relayers = this.cosmopark.config.relayers;
            if (relayers) {
                const idx = relayers.findIndex((relayer) => {
                    return relayer.type === "neutron";
                });

                return this.cosmopark.pauseRelayer("neutron", idx);
            } else {
                return Promise.reject("No relayers found in Cosmopark config to pause");
            }
        } catch (err) {
            console.error("TestSuite.pauseIcqRelaying:", err);
            return Promise.reject(err);
        }
    }

    async resumeIcqRelaying(): Promise<void> {
        try {
            const relayers = this.cosmopark.config.relayers;
            if (relayers) {
                const idx = relayers.findIndex((relayer) => {
                    return relayer.type === "neutron";
                });

                return this.cosmopark.resumeRelayer("neutron", idx);
            } else {
                return Promise.reject(
                    "No relayers found in Cosmopark config to resume"
                );
            }
        } catch (err) {
            console.error("TestSuite.resumeIcqRelaying:", err);
            return Promise.reject(err);
        }
    }

    async cleanup(): Promise<void> {
        try {
            const ctx = this.cosmopark.config.context ?? "default";
            const composeFilePath = path.resolve(
                __dirname,
                `../../docker-compose-${ctx}.yml`
            );

            const file: BunFile = Bun.file(composeFilePath);

            if (!(await file.exists())) {
                throw new Error(
                    `Docker compose file ${composeFilePath} does not exist`
                );
            }

            await runCommand(
                `docker-compose -f ${composeFilePath} down --volumes --remove-orphans`,
                true
            );

            return Promise.resolve();
        } catch (err) {
            console.error("TestSuite.cleanup:", err);
            return Promise.reject(err);
        }
    }
}
