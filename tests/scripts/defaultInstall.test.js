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
    pathToEim =
        os.platform() !== "win32"
            ? path.resolve(os.homedir(), "eim-gui", "eim")
            : path.resolve(os.homedir(), "eim-gui", "eim.exe");
}

let eimRunner = "";

describe("EIM Application Launch", () => {
    before(async function () {
        this.timeout(30000);
        eimRunner = new EIMRunner(pathToEim);
        try {
            await eimRunner.launchEIM();
        } catch (err) {
            logger.info("Error starting EIM application");
        }
    });

    afterEach(async function () {
        if (this.currentTest.state === "failed") {
            await eimRunner.takeScreenshot(`${this.currentTest.title}.png`);
            logger.info(`Screenshot saved as ${this.currentTest.title}.png`);
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
            expect(text, "Expected welcome text").to.equal(
                "Welcome to ESP-IDF Installation Manager!"
            );
        } catch (error) {
            logger.info("Failed to get Welcome header", error);
            throw error;
        }
    });

    it("Should show installation options", async function () {
        this.timeout(10000);

        try {
            await eimRunner.clickButton("Get Started");
            const header = await eimRunner.findByDataId("main-title");
            const text = await header.getText();
            expect(text, "Expected installation setup screen").to.equal(
                "Installation Setup"
            );
            const simplified = await eimRunner.findByText(
                "Simplified Installation"
            );
            expect(
                await simplified.isDisplayed(),
                "Expected option for simplified installation"
            ).to.be.true;
        } catch (error) {
            logger.info("Failed to locate get started button", error);
            throw error;
        }
    });

    it("Should install IDF using simplified setup", async function () {
        this.timeout(1300000);

        try {
            await eimRunner.clickButton("Start Simplified Setup");
            const installing = await eimRunner.findByText(
                "Installing ESP-IDF..."
            );
            expect(
                await installing.isDisplayed(),
                "Expected installation progress screen"
            ).to.be.true;
            const startTime = Date.now();

            while (Date.now() - startTime < 1200000) {
                if (await eimRunner.findByText("Installation Failed", 1000)) {
                    logger.debug("failed!!!!");
                    break;
                }
                if (
                    await eimRunner.findByText("Installation Complete!", 1000)
                ) {
                    logger.debug("Completed!!!");
                    break;
                }
                await new Promise((resolve) => setTimeout(resolve, 500));
            }
            const completed = await eimRunner.findByText(
                "Installation Complete!"
            );
            expect(completed, "Expected installation to be completed").to.not.be
                .false;
            expect(
                await completed.isDisplayed(),
                "Expected 'Installation Complete' text displayed"
            ).to.be.true;
        } catch (error) {
            logger.info("Failed to complete installation", error);
            throw error;
        }
    });

    it("Should offer to save installation configuration", async function () {
        this.timeout(1300000);

        try {
            const saveConfig = await eimRunner.findByText("Save Configuration");
            expect(saveConfig, "Expected screen for saving configuration").to
                .not.be.false;
            expect(
                await saveConfig.isDisplayed(),
                "Expected option to save configuration to be displayed"
            ).to.be.true;
            const exit = await eimRunner.findByText("Exit Installer");
            expect(exit, "Expected option to exit installer").to.not.be.false;
        } catch (error) {
            logger.info("Failed to complete installation", error);
            throw error;
        }
    });
});
