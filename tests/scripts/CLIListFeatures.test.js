import { expect } from "chai";
import { describe, it, after, beforeEach, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";

/**
 * This function verifies the `eim list-features` command.
 *
 * Assumes a single IDF version has already been installed by a prior test
 * (typically a `custom` test) at `installFolder`. The function does not
 * install or remove the IDF itself.
 */
export function runListFeaturesTest({
  id = 0,
  pathToEIM,
  idfList,
  installFolder,
}) {
  describe(`${id}- EIM list-features test |`, function () {
    this.timeout(120000);
    let testRunner = null;
    let testStepFailed = false;

    beforeEach(async function () {
      this.timeout(10000);
      if (testStepFailed) {
        logger.info("Test failed, skipping next tests");
        this.skip();
      }
    });

    afterEach(async function () {
      this.timeout(20000);
      if (this.currentTest.state === "failed") {
        logger.info(`Test failed: ${this.currentTest.title}`);
        if (testRunner) {
          logger.info(
            `Terminal output: >>\r ${testRunner.output.slice(-2000)}`
          );
          logger.debug(`Terminal output on failure: >>\r ${testRunner.output}`);
        }
        testStepFailed = true;
      }
      if (testRunner) {
        try {
          await testRunner.stop();
        } catch (error) {
          logger.info("Error cleaning up terminal after test");
          logger.info(` Error: ${error}`);
        } finally {
          testRunner = null;
        }
      }
    });

    after(function () {
      logger.info("list-features test completed");
    });

    const idfIdentifier = idfList[0];

    it("1- EIM list-features with no args prompts for an IDF", async function () {
      logger.info(`Validating EIM list-features interactive prompt`);
      testRunner = new CLITestRunner();
      await testRunner.start();
      testRunner.callEIM(pathToEIM, ["list-features"]);
      const prompt = await testRunner.waitForOutput(
        "Which IDF installation do you want to list features for?",
        10000
      );
      expect(prompt, "EIM list-features not prompting for IDF").to.be.true;
      // Cancel the prompt without selecting
      testRunner.sendInput("\x03");
      await new Promise((resolve) => setTimeout(resolve, 1000));
    });

    it("2- EIM list-features <name> prints the feature table", async function () {
      logger.info(`Validating EIM list-features ${idfIdentifier}`);
      testRunner = new CLITestRunner();
      await testRunner.start();
      testRunner.callEIM(pathToEIM, ["list-features", idfIdentifier]);
      const header = await testRunner.waitForOutput(
        "Features for IDF:",
        30000
      );
      expect(header, "EIM list-features not printing header").to.be.true;
      // "core" is a required feature and should always be reported installed
      expect(testRunner.output, "expected core feature listed").to.include(
        "core:"
      );
      expect(testRunner.output, "expected installed marker").to.include(
        "[installed]"
      );
      expect(testRunner.output, "expected optional marker").to.include(
        "(optional)"
      );
    });

    it("3- EIM list-features <unknown> reports the error", async function () {
      logger.info(`Validating EIM list-features error path`);
      testRunner = new CLITestRunner();
      await testRunner.start();
      testRunner.callEIM(pathToEIM, ["list-features", "definitely-not-installed"]);
      const error = await testRunner.waitForOutput(
        "not found",
        10000
      );
      expect(error, "EIM list-features did not report missing identifier").to.be.true;
    });
  });
}
