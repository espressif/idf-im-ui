import { expect } from "chai";
import { describe, it, before, after, beforeEach, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";

export function runCLIArgumentsTest({ id = 0, pathToEIM, eimVersion }) {
  describe(`${id}- Basic Arguments |`, function () {
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

    // Test to validate the EIM version number is correct
    it("1- should show correct version number", async function () {
      logger.info(`Starting test - show correct version`);
      await testRunner.start();
      testRunner.callEIM(pathToEIM, ["-V"]);
      const meetVersion = await testRunner.waitForOutput(eimVersion, 15000);
      expect(meetVersion, `EIM showing incorrect version number, expected: ${eimVersion}, actual: ${testRunner.output}`).to.be.true;
    });

    // Test to validate the EIM help options are correct
    // The test only checks for basic elements to validate that the help was printed. This can be improved by checking for specific options and their descriptions.
    it("2- should show help with --help argument", async function () {
      logger.info(`Starting test - show help`);
      await testRunner.start();
      testRunner.callEIM(pathToEIM, ["--help"]);
      const printHelp = await testRunner.waitForOutput("Options:");
      expect(printHelp, "EIM failed to print help options").to.be.true;
      expect(testRunner.output, "EIM failed to print usage help").to.include(
        "Usage:"
      );
    });

    // Test to validate the EIM handles invalid arguments correctly
    it("3- should handle invalid arguments", async function () {
      logger.info(`Starting test - invalid argument`);
      await testRunner.start();
      testRunner.callEIM(pathToEIM, ["--KK"]);
      const wrongArgument = await testRunner.waitForOutput(
        "unexpected argument"
      );
      expect(wrongArgument, "Missing error when sending non-existing argument")
        .to.be.true;
    });
  });
}
