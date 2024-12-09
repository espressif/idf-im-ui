import os from "os";
import path from "path";
import { expect } from "chai";
import { describe, it, before, after, beforeEach, afterEach } from "mocha";
import { EIMRunner } from "../classes/tauriRunner.class.js";
import logger from "../classes/logger.class.js";

const application = path.resolve(os.homedir(), "eim-gui", "eim.exe");
let eimRunner = "";

describe("EIM Application Launch", () => {
    before(async function () {
        this.timeout(10000);
        eimRunner = new EIMRunner(application);
        try {
            await eimRunner.launchEIM();
        } catch (err) {
            logger.info("Error starting EIM application");
        }
    });

    after(async function () {
        this.timeout(5000);
        try {
            await eimRunner.closeEIM();
        } catch (error) {
            logger.info("Error to close IEM application");
        }
    });

    it("Should show welcome page", async function () {
        this.timeout(10000);

        try {
            // Wait for the header to be present
            const header = await eimRunner.findByCSS("h1");
            const text = await header.getText();
            expect(text).to.equal("Welcome to ESP-IDF Installation Manager!");
        } catch (error) {
            logger.info("Failed to get Welcome header", error);
            throw error;
        }
    });
});
