import { WALLET_KEYS } from "../suite/constants";
import networkConfigs from "../suite/networks";
import relayerConfigs from "../suite/relayers";
import {
    QueryParamsResponse,
    QuerySigningInfoResponse,
    QuerySigningInfosResponse,
} from "cosmjs-types/cosmos/slashing/v1beta1/query";

declare global {
    interface SlashingExtension {
        readonly slashing: {
            signingInfo: (consAddress: string) => Promise<QuerySigningInfoResponse>;
            signingInfos: (
                paginationKey?: Uint8Array
            ) => Promise<QuerySigningInfosResponse>;
            params: () => Promise<QueryParamsResponse>;
        };
    }

    type NetworkOptsType = Partial<
        Record<keyof typeof networkConfigs | "*", any>
    >;
    type NetworkKeys = keyof typeof networkConfigs;
    type HostNetworkKeys = Extract<NetworkKeys, "neutron">;
    type RemoteNetworkKeys = Exclude<NetworkKeys, "neutron">;

    type RelayerOptsType = Partial<
        Record<keyof typeof relayerConfigs, any | boolean>
    >;
    type RelayerKeys = keyof typeof relayerConfigs;

    type WalletKeys = (typeof WALLET_KEYS)[number];

    interface TestSuiteParams {
        ctx?: string;
        networks?: NetworkKeys[];
        relayerOverrides?: RelayerOptsType;
        networkOverrides?: NetworkOptsType;
    }

    interface ITestSuite {
        getHostPrefix(network: HostNetworkKeys = "neutron"): string;
        getHostRpc(network: HostNetworkKeys = "neutron"): string;
        getHostGasPrices(network: HostNetworkKeys = "neutron"): string;

        getRemotePrefix(network: RemoteNetworkKeys = "gaia"): string;
        getRemoteRpc(network: RemoteNetworkKeys = "gaia"): string;
        getRemoteGasPrices(network: RemoteNetworkKeys = "gaia"): string;

        getMasterMnemonic(): string;

        slashValidator(): Promise<string>;
        pauseIcqRelaying(): Promise<void>;
        resumeIcqRelaying(): Promise<void>;
        cleanup(): Promise<void>;
    }
}

export { };
