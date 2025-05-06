import { expect } from "chai";
import { describe, it, before, after, beforeEach, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";

export function runCLIArgumentsTest(pathToEim, eimVersion) {
  describe("Basic Arguments Tests ->", function () {
    let testRunner = null;

    beforeEach(function () {
      testRunner = new CLITestRunner();
    });

    afterEach(async function () {
      this.timeout(20000);
      if (this.currentTest.state === "failed") {
        logger.info(`Test failed: ${this.currentTest.title}`);
        logger.debug(`Terminal output on failure: >>\r ${testRunner.output}`);
      }
      try {
        await testRunner.stop();
      } catch (error) {
        logger.info("Error to clean up terminal after test");
        logger.info(` Error: ${error}`);
      }
      testRunner = null;
    });

    it("should show correct version number", async function () {
      logger.info(`Starting test - show correct version`);
      await testRunner.start();
      testRunner.sendInput(`${pathToEim} -V\r`);
      const meetVersion = await testRunner.waitForOutput(eimVersion);
      expect(meetVersion, "EIM showing incorrect version number").to.be.true;
    });

    it("should show help with --help argument", async function () {
      logger.info(`Starting test - show help`);
      await testRunner.start();
      testRunner.sendInput(`${pathToEim} --help\r`);
      const printHelp = await testRunner.waitForOutput("Options:");
      expect(printHelp, "EIM failed to print help options").to.be.true;
      expect(testRunner.output, "EIM failed to print usage help").to.include(
        "Usage:"
      );
    });

    it("should handle invalid arguments", async function () {
      logger.info(`Starting test - invalid argument`);
      await testRunner.start();
      testRunner.sendInput(`${pathToEim} --KK\r`);
      const wrongArgument = await testRunner.waitForOutput(
        "unexpected argument"
      );
      expect(wrongArgument, "Missing error when sending non-existing argument")
        .to.be.true;
    });
  });
}
