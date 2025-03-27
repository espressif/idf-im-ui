import { expect } from "chai";
import { describe, it, before, after, beforeEach, afterEach } from "mocha";
import { InteractiveCLITestRunner } from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";

export function runInstallCustom(pathToEim, args = []) {
    describe("1 - Run custom installation using given parameters ->", function () {
        let testRunner = null;

        before(async function () {
            logger.debug(
                `Installing custom IDF version with parameters ${args.join(
                    " "
                )}`
            );
            this.timeout(5000);
            testRunner = new InteractiveCLITestRunner();
            try {
                await testRunner.start();
            } catch {
                logger.info("Error to start terminal");
            }
        });

        afterEach(function () {
            if (this.currentTest.state === "failed") {
                logger.info(
                    `Terminal output on failure: >>\r ${testRunner.output}`
                );
            }
        });

        after(async function () {
            logger.info("Custom installation routine completed");
            this.timeout(20000);
            try {
                await testRunner.stop();
            } catch (error) {
                logger.info("Error to clean up terminal after test");
                logger.info(` Error: ${error}`);
            }
        });

        /** Run installation with full parameters, no need to ask questions
         *
         * It is expected to have all requirements installed
         *
         */

        it("Should install IDF using specified parameters", async function () {
            logger.info(`Starting test - IDF custom installation`);
            testRunner.sendInput(`${pathToEim} ${args.join(" ")}\r`);
            if (!"-n true" in args) {
                const installationCompleted = await testRunner.waitForOutput(
                    "Do you want to save the installer configuration",
                    1200000
                );
                expect(
                    installationCompleted,
                    "Failed to ask to save installation configuration - failure to install using full arguments on run time"
                ).to.be.true;
                expect(
                    testRunner.output,
                    "Error message during installation"
                ).to.not.include("error");
                expect(
                    testRunner.output,
                    "Failed to download submodules, missing 'Finished fetching submodules'"
                ).to.include("Finished fetching submodules");

                logger.info("Installation completed");
                testRunner.output = "";
                testRunner.sendInput("n");
            }
            const installationSuccessful = await testRunner.waitForOutput(
                "Successfully installed IDF",
                1200000
            );

            expect(
                installationSuccessful,
                "Failed to complete installation, missing 'Successfully Installed IDF'"
            ).to.be.true;

            expect(
                testRunner.output,
                "Failed to complete installation, missing 'Now you can start using IDF tools'"
            ).to.include("Now you can start using IDF tools");

            if ("-r true" in args || "--recursive-submodules" in args) {
                expect(
                    testRunner.output,
                    "Failed to download submodules, missing 'Finished fetching submodules'"
                ).to.include("Finished fetching submodules");
            }
        });
    });
}
