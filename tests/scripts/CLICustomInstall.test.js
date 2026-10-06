import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import { startTestProxy, stopTestProxy } from "../helpers/testProxy.js";
import {
  startTerminal,
  stopTerminal,
  logFailedTest,
  waitForTerminalOutput,
} from "../helpers/cliTestHelpers.js";
import { downloadOfflineArchive } from "../helper.js";
import fs from "fs";
import path from "path";
import os from "os";


// This function executes an unattended IDF installation based on the args provided. If no args are provided, the test will use the default arguments.
export function runCLICustomInstallTest({
  id = 0,
  pathToEIM,
  args = [],
  offlineIDFVersion = null,
  offlinePkgFilename = null,
  testProxyMode = false,
  proxyBlockList = [],
  pythonWheelsVersion = [],
}) {
  describe(`${id}- Run custom |`, function () {
    let testRunner = null;
    let proxy = null;
    let pathToOfflineArchive = null;
    const archiveDir = path.join(os.homedir(), "archive");

    // The setup function should start the proxy server if enabled, download the offline archive if provided and start the terminal
    // If the offline archive is provided the file will be extracted in a folder to validate its contents
    before(async function () {
      logger.debug(
        `Installing custom IDF version with parameters ${args.join(" ")}`,
      );
      this.timeout(900000);
      testRunner = new CLITestRunner();
      if (offlineIDFVersion) {
        pathToOfflineArchive = await downloadOfflineArchive({
          idfVersion: offlineIDFVersion,
          packageFilename: offlinePkgFilename,
        });
      }
      if (testProxyMode) {
        proxy = await startTestProxy(testProxyMode, proxyBlockList);
      }
      await startTerminal(testRunner);
      if (pathToOfflineArchive) {
        args.push(`--use-local-archive "${pathToOfflineArchive}"`);
        testRunner.sendInput(`mkdir ${archiveDir}`);
        testRunner.sendInput(
          `tar -xf ${pathToOfflineArchive} -C ${archiveDir} ; echo "Job" ; echo "Completed"`,
        );
        await testRunner.waitForOutput("JobCompleted", 60000);
      }
      if (offlineIDFVersion && !pathToOfflineArchive) {
        logger.info(">>>>>>> Offline archive not found, skipping this test");
        this.skip();
      }
    });

    // The afterEach function should log the terminal output on failure
    afterEach(function () {
      if (this.currentTest.state === "failed") {
        logFailedTest(this.currentTest, testRunner, { tail: 1000 });
      }
    });

    // The tear down function should stop the terminal and proxy server if enabled
    // The offline archive should be removed to save space in the runner
    after(async function () {
      logger.info("Custom installation routine completed");
      this.timeout(50000);
      await stopTerminal(testRunner);
      testRunner = null;
      if (testProxyMode) {
        await stopTestProxy(proxy);
      }
      // Remove offline archive to save space in the runner
      if (pathToOfflineArchive) {
        try {
          fs.rmSync(pathToOfflineArchive, { force: true });
          fs.rmSync(archiveDir, { recursive: true, force: true });
          logger.info(`Successfully deleted offline archive`);
        } catch (err) {
          logger.info(`Error deleting offline archive`);
        }
      }
    });

    // This test verifies the presence of the wheels for the python versions provided in the offline archive
    // The test will skip if no offline archive is provided
    for (let pythonVersion of pythonWheelsVersion) {
      it(`0- Should verify wheels for python${pythonVersion} on offline archive`, function () {
        this.timeout(20000);
        if (!offlineIDFVersion && !pathToOfflineArchive) {
          this.skip();
        }
        logger.info(`Verifying wheels for ${pythonVersion} on offline archive`);
        expect(
          fs.existsSync(pathToOfflineArchive),
          `Offline archive not found at ${pathToOfflineArchive}`,
        ).to.be.true;
        logger.info(`Checking for wheels folder for Python ${pythonVersion}`);
        logger.info(`Archive contents: ${fs.readdirSync(archiveDir)}`);
        expect(
          fs.existsSync(path.join(archiveDir, `wheels_py${pythonVersion}`)),
          `Wheels folder for Python ${pythonVersion} not found in offline archive`,
        ).to.be.true;
        expect(
          fs.readdirSync(path.join(archiveDir, `wheels_py${pythonVersion}`))
            .length > 0,
          `No wheels found for Python ${pythonVersion} in offline archive`,
        ).to.be.true;
      });
    }

    /** Run installation with full parameters, no need to ask questions
     *
     * It is expected to have all requirements installed
     *
     */

    it("1- Should install IDF using specified parameters", async function () {
      logger.info(`Starting test - IDF custom installation`);
      testRunner.callEIM(pathToEIM, ["install", ...args]);
      await new Promise((resolve) => setTimeout(resolve, 5000));
      if (args.includes("-n false")) {
        await waitForTerminalOutput(
          testRunner,
          "Do you want to save the installer configuration",
        );

        expect(
          testRunner.output,
          "Failed to ask to save installation configuration - failure to install using full arguments on run time",
        ).to.include("Do you want to save the installer configuration");

        logger.info("Installation completed");
        testRunner.output = "";
        testRunner.sendInput("n");
      }

      await waitForTerminalOutput(testRunner, "Now you can start using IDF tools", {
        pollMs: 500,
      });

      expect(
        testRunner.output,
        "Failed to complete installation, missing 'Successfully Installed IDF'",
      ).to.include("Successfully installed IDF");

      expect(
        testRunner.output,
        "Failed to complete installation, missing 'Now you can start using IDF tools'",
      ).to.include("Now you can start using IDF tools");

      if (testProxyMode === "block" && proxy.attempts.length > 0) {
        logger.error(
          ">>>>>>>>>>>>>>>>>>Internet Connection Attempt Detected - This should be a failure",
        );
      }
    });
  });
}
