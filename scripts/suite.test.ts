import { describe, it, beforeAll, afterAll, expect } from "bun:test";
import { TestSuite } from "./suite";
import {
    GAIA_GAS_PRICE,
    NEUTRON_GAS_PRICE,
    WALLET_MNEMONIC_WORD_COUNT,
} from "./suite/constants";
import { isContainerPaused } from "./utils";

let suite: ITestSuite;

describe("TestSuite sanity check", () => {
    beforeAll(async () => {
        suite = await TestSuite.create({
            networkOverrides: {
                gaia: {
                    validators: 6,
                    validators_balance: [
                        "100000000",
                        "100000000",
                        "100000000",
                        "100000000",
                        "100000000",
                        "100000000",
                    ],
                },
            },
        });
        // suite = await TestSuite.create();
    });

    afterAll(async () => {
        await suite.cleanup();
    });

    it("should get the correct host prefix", () => {
        const prefix = suite.getHostPrefix();
        expect(prefix).toBe("neutron");
    });

    it("should get the correct host RPC", () => {
        const rpc = suite.getHostRpc();
        expect(rpc).toMatch(/^127\.0\.0\.1:\d{5}$/);
    });

    it("should get the host gas price", () => {
        const price = suite.getHostGasPrices("neutron");
        const priceStr = `${NEUTRON_GAS_PRICE.amount}${NEUTRON_GAS_PRICE.denom}`;
        expect(price).toBe(priceStr);
    });

    it("should get the correct remote prefix", () => {
        const prefix = suite.getRemotePrefix("gaia");
        expect(prefix).toBe("cosmos");
    });

    it("should get the correct remote RPC", () => {
        const rpc = suite.getRemoteRpc("gaia");
        expect(rpc).toMatch(/^127\.0\.0\.1:\d{5}$/);
    });

    it("should get the remote gas price", () => {
        const price = suite.getRemoteGasPrices("gaia");
        const priceStr = `${GAIA_GAS_PRICE.amount}${GAIA_GAS_PRICE.denom}`;
        expect(price).toBe(priceStr);
    });

    it("should get the master mnemonic", () => {
        const mnemonic = suite.getMasterMnemonic();
        expect(mnemonic.split(" ").length).toBe(WALLET_MNEMONIC_WORD_COUNT);
    });

    it("should pause the ICQ relayer", async () => {
        await suite.pauseIcqRelaying();
        const isPaused = await isContainerPaused("default-relayer_neutron1-1");
        expect(isPaused).toBe(true);
    });

    it("should resume the ICQ relayer", async () => {
        await suite.resumeIcqRelaying();
        const isPaused = await isContainerPaused("default-relayer_neutron1-1");
        expect(isPaused).toBe(false);
    });

    it("should slash validator for downtime", async () => {
        const validator = await suite.slashValidator();
        expect(validator).toMatch(/^cosmosvalcons/);
    });
});
