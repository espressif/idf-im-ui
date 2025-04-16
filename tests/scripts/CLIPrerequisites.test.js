import { expect } from "chai";
import { describe, it, before, after, beforeEach, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import os from "os";
import path from "path";

/**
 * Setup the following environmental variables to execute this test:
 *
 * EIM_FILE_PATH to point to the eim application.
 *
 * use:
 * Windows: $env:<variable>="<value>"
 * Linux/mac: export <variable>="<value>"
 *
 */

export function runCLIPrerequisitesTest(pathToEim) {
    // let pathToEim;

    // if (process.env.EIM_FILE_PATH) {
    //     pathToEim = process.env.EIM_FILE_PATH;
    // } else {
    //     pathToEim = path.join(os.homedir(), "eim-cli/eim");
    // }

    describe("Check if prerequisites are installed ->", function () {
        this.timeout(600000);
        let testRunner;

        beforeEach(async function () {
            this.timeout(5000);
            testRunner = new InteractiveCLITestRunner();
            try {
                await testRunner.start();
                testRunner.sendInput(`${pathToEim}\r`);
            } catch (error) {
                logger.info(`Error starting process: ${error}`);
                logger.info(` Error: ${error}`);
            }
        });

        afterEach(async function () {
            this.timeout(20000);
            try {
                await testRunner.stop();
            } catch (error) {
                logger.info("Error to clean up terminal after test");
                logger.info(` Error: ${error}`);
            }
            testRunner = null;
        });

        /** Linux/MAC Specific Tests
         * Tests below will only be executed on Unix Based systems
         */

        // The following test can only be executed if the prerequisites have not been installed in the OS.
        (os.platform() !== "win32" ? describe : describe.skip)(
            "Pre-Requisites test on non windows platform",
            function () {
                afterEach(function () {
                    this.timeout(10000);
                    if (this.currentTest.state === "failed") {
                        logger.info(
                            `Terminal output on failure: >>\r ${testRunner.output}`
                        );
                    }
                });

                it("Should detect missing requirements", async function () {
                    logger.info(
                        `Starting test - confirm requirements are missing`
                    );
                    this.timeout(25000);
                    const missingRequisites = await testRunner.waitForOutput(
                        "Error: Please install the missing prerequisites",
                        20000
                    );
                    expect(
                        missingRequisites,
                        'EIM did not show error message indicating "Please install prerequisites"'
                    ).to.be.true;
                });
            }
        );

        /** Windows Specific Tests
         * Tests below will only be executed on win32 platform
         */

        (os.platform() === "win32" ? describe : describe.skip)(
            "Pre-requisites test on Windows",
            function () {
                afterEach(async function () {
                    this.timeout(10000);
                    if (this.currentTest.state === "failed") {
                        logger.info(
                            `Terminal output on failure: >>>>>>>>>>>>>>>\r ${testRunner.output}`
                        );
                    }
                });

                it("should offer to install prerequisites and exit upon negative answer", async function () {
                    logger.info(
                        `Starting test - confirm requirements are missing`
                    );
                    this.timeout(25000);
                    const promptRequisites = await testRunner.waitForOutput(
                        "Do you want to install prerequisites?"
                    );

                    expect(
                        promptRequisites,
                        "EIM did not offer to install the missing prerequisites"
                    ).to.be.true;

                    testRunner.sendInput("n");

                    const terminalExited = await testRunner.waitForOutput(
                        "Please install the missing prerequisites and try again"
                    );
                    expect(
                        terminalExited,
                        "EIM did not fails after denying to install pre-requisites"
                    ).to.be.true;
                });
            }
        );
    });
}
