import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import { EIMRunner } from "../classes/tauriRunner.class.js";
import logger from "../classes/logger.class.js";

export function runStartupTest(pathToEIM, eimVersion) {
    let eimRunner = "";

    describe("EIM Application Launch", () => {
        before(async function () {
            this.timeout(60000);
            eimRunner = new EIMRunner(pathToEIM);
            try {
                await eimRunner.launchEIM();
            } catch (err) {
                logger.info("Error starting EIM application");
            }
        });

        afterEach(async function () {
            if (this.currentTest.state === "failed") {
                await eimRunner.takeScreenshot(`${this.currentTest.title}.png`);
                logger.info(
                    `Screenshot saved as ${this.currentTest.title}.png`
                );
            }
        });

        after(async function () {
            this.timeout(5000);
            try {
                await eimRunner.closeEIM();
            } catch (error) {
                logger.info("Error to close EIM application");
            }
        });

        it("Should show welcome page", async function () {
            this.timeout(10000);
            // Wait for the header to be present
            const header = await eimRunner.findByCSS("h1");
            const text = await header.getText();
            expect(text, "Expected welcome text").to.equal(
                "Welcome to ESP-IDF Installation Manager!"
            );
        });

        it("Should show correct version number", async function () {
            const footer = await eimRunner.findByClass("footer");
            const text = await footer.getText();
            expect(text, "Expected correct version shown on page").to.include(
                `ESP-IDF Installation Manager ${eimVersion}`
            );
        });
    });
}
