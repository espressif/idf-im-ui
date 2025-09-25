import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import TestProxy from "../classes/TestProxy.class.js";
import { downloadOfflineArchive } from "../helper.js";
import fs from "fs";

export function runCLICustomInstallTest({
  id,
  pathToEim,
  args = [],
  offlineIDFVersion = null,
  offlinePkgName = null,
  testProxyMode = false,
}) {
  describe(`${id}- Run custom |`, function () {
    let testRunner = null;
    let proxy = null;
    let pathToOfflineArchive = null;

    before(async function () {
      logger.debug(
        `Installing custom IDF version with parameters ${args.join(" ")}`
      );
      this.timeout(900000);
      testRunner = new CLITestRunner();
      if (offlineIDFVersion) {
        pathToOfflineArchive = await downloadOfflineArchive({
          idfVersion: offlineIDFVersion,
          packageName: offlinePkgName,
        });

        args.push(`--use-local-archive "${pathToOfflineArchive}"`);
      }
      if (testProxyMode) {
        try {
          proxy = new TestProxy({ mode: testProxyMode });
          await proxy.start();
        } catch (error) {
          logger.info("Error to start proxy server");
          logger.debug(`Error: ${error}`);
        }
      }
      try {
        await testRunner.start({
          isolatedEnvironment: testProxyMode === false ? false : true,
        });
      } catch (error) {
        logger.info("Error to start terminal");
        logger.debug(`Error: ${error}`);
      }
    });

    afterEach(function () {
      if (this.currentTest.state === "failed") {
        logger.info(`Test failed: ${this.currentTest.title}`);
        logger.info(`Terminal output: >>\r ${testRunner.output.slice(-1000)}`);
        logger.debug(`Terminal output on failure: >>\r ${testRunner.output}`);
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
      } finally {
        testRunner = null;
      }
      try {
        await proxy.stop();
      } catch (error) {
        logger.info("Error stopping proxy server");
        logger.debug(`Error: ${error}`);
      }
      //remove offline archive to save space in the runner
      fs.rmSync(pathToOfflineArchive, { force: true });
    });

    /** Run installation with full parameters, no need to ask questions
     *
     * It is expected to have all requirements installed
     *
     */

    it("1- Should install IDF using specified parameters", async function () {
      logger.info(`Starting test - IDF custom installation`);
      testRunner.sendInput(`${pathToEim} install ${args.join(" ")}`);
      await new Promise((resolve) => setTimeout(resolve, 5000));
      if (args.includes("-n false")) {
        const startTime = Date.now();
        while (Date.now() - startTime < 3600000) {
          if (Date.now() - testRunner.lastDataTimestamp >= 600000) {
            logger.info(">>>>>>>Exited due to Idle terminal!!!!!");
            break;
          }
          if (await testRunner.waitForOutput("panicked", 1000)) {
            logger.info(">>>>>>>Rust App failure!!!!");
            break;
          }
          if (
            await testRunner.waitForOutput(
              "Do you want to save the installer configuration",
              1000
            )
          ) {
            logger.info(">>>>>>>Completed!!!");
            break;
          }
          await new Promise((resolve) => setTimeout(resolve, 1000));
        }
        if (Date.now() - startTime >= 3600000) {
          logger.info("Installation timed out after 1 hour");
        }

        expect(
          testRunner.output,
          "Failed to ask to save installation configuration - failure to install using full arguments on run time"
        ).to.include("Do you want to save the installer configuration");

        logger.info("Installation completed");
        testRunner.output = "";
        testRunner.sendInput("n");
      }

      const startTime = Date.now();
      while (Date.now() - startTime < 3600000) {
        if (Date.now() - testRunner.lastDataTimestamp >= 600000) {
          logger.info(">>>>>>>Exited due to Idle terminal!!!!!");
          break;
        }
        if (await testRunner.waitForOutput("panicked", 1000)) {
          logger.info(">>>>>>>Rust App failure!!!!");
          break;
        }
        if (
          await testRunner.waitForOutput(
            "Now you can start using IDF tools",
            1000
          )
        ) {
          logger.info(">>>>>>>Completed!!!");
          break;
        }
        await new Promise((resolve) => setTimeout(resolve, 500));
      }
      if (Date.now() - startTime >= 3600000) {
        logger.info("Installation timed out after 1 hour");
      }

      expect(
        testRunner.output,
        "Failed to complete installation, missing 'Successfully Installed IDF'"
      ).to.include("Successfully installed IDF");

      expect(
        testRunner.output,
        "Failed to complete installation, missing 'Now you can start using IDF tools'"
      ).to.include("Now you can start using IDF tools");

      if (testProxyMode === "block" && proxy.attempts.length > 0) {
        logger.error(
          ">>>>>>>>>>>>>>>>>>Internet Connection Attempt Detected - This should be a failure"
        );
      }
    });
  });
}
