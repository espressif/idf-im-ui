import os from "os";
import path from "path";
import { expect } from "chai";
import { describe, it, before, after, beforeEach, afterEach } from "mocha";
import { EIMRunner } from "../classes/tauriRunner.class.js";
import logger from "../classes/logger.class.js";

let pathToEim;

if (process.env.EIM_GUI_PATH) {
    pathToEim = process.env.EIM_GUI_PATH;
} else {
    pathToEim = path.resolve(os.homedir(), "eim-gui", "eim.exe");
}

let eimRunner = "";

describe("EIM Application Launch", () => {
    before(async function () {
        this.timeout(10000);
        eimRunner = new EIMRunner(pathToEim);
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

    it("Should show installation options", async function () {
        this.timeout(10000);

        try {
            await eimRunner.clickButton("Get Started");
            const header = await eimRunner.findByCSS("h1");
            const text = await header.getText();
            expect(text).to.equal("Installation Setup");
            const simplified = await eimRunner.findByText(
                "Simplified Installation"
            );
            expect(await simplified.isDisplayed()).to.be.true;
        } catch (error) {
            logger.info("Failed to locate get started button", error);
            throw error;
        }
    });

    it("Should start simplified setup", async function () {
        this.timeout(1300000);

        try {
            await eimRunner.clickButton("Start Simplified Setup");
            const installing = await eimRunner.findByText(
                "Please wait while the installation progresses..."
            );
            expect(await installing.isDisplayed()).to.be.true;
            const startTime = Date.now();

            while (Date.now() - startTime < 1200000) {
                if (await eimRunner.findByText("Installation Failed", 1000)) {
                    logger.debug("failed!!!!");
                    break;
                }
                if (
                    await eimRunner.findByText("installation completed", 1000)
                ) {
                    logger.debug("Completed!!!");
                    break;
                }
                await new Promise((resolve) => setTimeout(resolve, 500));
            }
            const completed = await eimRunner.findByText(
                "installation completed"
            );
            expect(completed).to.not.be.false;
            expect(await completed.isDisplayed()).to.be.true;
        } catch (error) {
            logger.info("Failed to complete installation", error);
            throw error;
        }
    });
});
