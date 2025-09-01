import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import TestProxy from "../classes/testProxy.class.js";

export function runCLICustomInstallTest({
  pathToEim,
  args = [],
  testProxyMode = false,
}) {
  describe("1- Run custom ->", function () {
    let testRunner = null;
    let proxy = null;

    before(async function () {
      logger.debug(
        `Installing custom IDF version with parameters ${args.join(" ")}`
      );
      this.timeout(5000);
      testRunner = new CLITestRunner();
      if (testProxyMode) {
        try {
          proxy = new TestProxy({ mode: testProxyMode });
          await proxy.start();
          testRunner.createIsolatedEnvironment();
        } catch {
          logger.info("Error to start proxy server");
        }
      }
      try {
        await testRunner.start();
      } catch {
        logger.info("Error to start terminal");
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
      }
      try {
        await proxy.stop();
      } catch {
        logger.info("Error stopping proxy server");
      }
    });

    /** Run installation with full parameters, no need to ask questions
     *
     * It is expected to have all requirements installed
     *
     */

    it("Should install IDF using specified parameters", async function () {
      logger.info(`Starting test - IDF custom installation`);
      testRunner.sendInput(`${pathToEim} install ${args.join(" ")}\r`);
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

      if (testProxyMode === "block") {
        expect(
          proxy.attempts,
          "Network access attempt detected during installation"
        ).to.be.empty;
      }
    });
  });
}
