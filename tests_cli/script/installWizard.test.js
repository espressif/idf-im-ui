import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import { InteractiveCLITestRunner } from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import os from "os";

export function runInstallWizardTests(pathToEim) {
    describe("1 - Run Install Wizard steps ->", function () {
        this.timeout(800000);
        let testRunner = null;
        let installationFailed = false;

        before(async function () {
            logger.debug(`Starting installation wizard with default options`);
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
                    `Installation using wizard failed -> output: \r >>${testRunner.output}<<`
                );
                installationFailed = true;
            }
        });

        after(async function () {
            logger.info("Installation wizard test cleanup");
            this.timeout(20000);
            try {
                await testRunner.stop();
            } catch (error) {
                logger.info("Error to clean up terminal after test");
                logger.info(`${error}`);
            }
        });

        /** Run install wizard
         *
         * It is expected to have all requirements installed
         * The step to install the prerequisites in windows is not tested
         *
         */

        it("Should install IDF using wizard and default values", async function () {
            logger.info(`Starting test - IDF installation wizard`);
            this.timeout(1500000);
            testRunner.sendInput(`${pathToEim}\r`);
            const selectTargetQuestion = await testRunner.waitForOutput(
                "Please select all of the target platforms",
                20000
            );
            expect(
                selectTargetQuestion,
                "Failed to ask for installation targets"
            ).to.be.true;
            expect(
                testRunner.output,
                "Failed to offer installation for 'all' targets"
            ).to.include("all");

            logger.info("Select Target Passed");
            testRunner.output = "";
            testRunner.sendInput("\r");

            const selectIDFVersion = await testRunner.waitForOutput(
                "Please select the desired ESP-IDF version"
            );
            expect(selectIDFVersion, "Failed to ask for IDF version").to.be
                .true;
            expect(
                testRunner.output,
                "Failed to offer installation for master branch"
            ).to.include("master");

            logger.info("Select IDF Version passed");
            testRunner.output = "";
            testRunner.sendInput("\r");

            const selectIDFMirror = await testRunner.waitForOutput(
                "Select the source from which to download esp-idf"
            );
            expect(selectIDFMirror, "Failed to ask for IDF download mirrors").to
                .be.true;
            expect(
                testRunner.output,
                "Failed to offer github as a download mirror option"
            ).to.include("https://github.com");

            logger.info("Select IDF mirror passed");
            testRunner.output = "";
            testRunner.sendInput("\r");

            const selectToolsMirror = await testRunner.waitForOutput(
                "Select a source from which to download tools"
            );
            expect(selectToolsMirror, "Failed to ask for tools download mirror")
                .to.be.true;
            expect(
                testRunner.output,
                "Failed to offer github as tools download mirror"
            ).to.include("https://github.com");

            logger.info("Select tools mirror passed");
            testRunner.output = "";
            testRunner.sendInput("\r");

            const selectInstallPath = await testRunner.waitForOutput(
                "Please select the ESP-IDF installation location"
            );
            expect(selectInstallPath, "Failed to ask for installation path").to
                .be.true;

            const defaultPath =
                os.platform() === "win32"
                    ? "(C:\\esp)"
                    : `(${os.homedir()}/.espressif)`;
            expect(
                testRunner.output,
                "Failed to provide default installation path"
            ).to.include(defaultPath);

            logger.info("Select install path passed");
            testRunner.output = "";
            testRunner.sendInput("\r");

            const installationCompleted = await testRunner.waitForOutput(
                "Do you want to save the installer configuration",
                1200000
            );
            expect(
                installationCompleted,
                "Failed to ask to save installation configuration - failure to install using wizard parameters"
            ).to.be.true;
            expect(
                testRunner.output,
                "Error message during installation"
            ).to.not.include("error");
            expect(
                testRunner.output,
                "Error to download the tools, missing 'Downloading Tools'"
            ).to.include("Downloading tools");

            logger.info("Installation completed");
            testRunner.output = "";
            testRunner.sendInput("\r");

            const installationSuccessful = await testRunner.waitForOutput(
                "Successfully installed IDF"
            );
            expect(
                installationSuccessful,
                "Failed to complete installation, missing 'Successfully Installed IDF'"
            ).to.be.true;

            logger.info("installation successful");
        });
    });
}
