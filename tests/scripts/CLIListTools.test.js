import { expect } from "chai";
import { describe, it, after, beforeEach, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import path from "path";
import fs from "fs";

/**
 * This function verifies the `eim list-tools` command.
 *
 * Assumes a single IDF version has already been installed by a prior test
 * (typically a `custom` test) at `installFolder`. The function does not
 * install or remove the IDF itself.
 */
export function runListToolsTest({
  id = 0,
  pathToEIM,
  idfList,
  installFolder,
}) {
  describe(`${id}- EIM list-tools test |`, function () {
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
      logger.info("list-tools test completed");
    });

    const idfIdentifier = idfList[0];

    it("1- EIM list-tools with no args prompts for an IDF", async function () {
      logger.info(`Validating EIM list-tools interactive prompt`);
      testRunner = new CLITestRunner();
      await testRunner.start();
      testRunner.callEIM(pathToEIM, ["list-tools"]);
      const prompt = await testRunner.waitForOutput(
        "Which IDF installation do you want to list tools for?",
        10000
      );
      expect(prompt, "EIM list-tools not prompting for IDF").to.be.true;
      // Cancel the prompt without selecting
      testRunner.sendInput("\x03");
      await new Promise((resolve) => setTimeout(resolve, 1000));
    });

    it("2- EIM list-tools <name> prints the tool table", async function () {
      logger.info(`Validating EIM list-tools ${idfIdentifier}`);
      testRunner = new CLITestRunner();
      await testRunner.start();
      testRunner.callEIM(pathToEIM, ["list-tools", idfIdentifier]);
      const header = await testRunner.waitForOutput(
        "Tools for IDF:",
        30000
      );
      expect(header, "EIM list-tools not printing header").to.be.true;
      expect(testRunner.output, "expected recommended marker").to.include(
        "recommended"
      );
      expect(testRunner.output, "expected installed marker").to.include(
        "[installed:"
      );
    });

    it("3- EIM list-tools <name> --outdated prints the outdated section", async function () {
      logger.info(`Validating EIM list-tools --outdated`);
      testRunner = new CLITestRunner();
      await testRunner.start();
      testRunner.callEIM(pathToEIM, ["list-tools", idfIdentifier, "--outdated"]);
      const header = await testRunner.waitForOutput(
        "Tools for IDF:",
        30000
      );
      expect(header, "EIM list-tools --outdated not printing header").to.be.true;
      // Either the header is followed by a populated outdated list, or by
      // the empty-state line. Both are valid outcomes; just make sure one
      // of the two appears.
      const hasOutdated = testRunner.output.includes("Outdated tools:") &&
        testRunner.output.includes("is outdated by");
      const hasNoOutdated = testRunner.output.includes("No outdated tools.");
      expect(
        hasOutdated || hasNoOutdated,
        "EIM list-tools --outdated produced neither an outdated list nor a no-outdated message"
      ).to.be.true;
    });

    it("4- EIM list-tools <unknown> reports the error", async function () {
      logger.info(`Validating EIM list-tools error path`);
      testRunner = new CLITestRunner();
      await testRunner.start();
      testRunner.callEIM(pathToEIM, ["list-tools", "definitely-not-installed"]);
      const error = await testRunner.waitForOutput(
        "not found",
        10000
      );
      expect(error, "EIM list-tools did not report missing identifier").to.be.true;
    });
  });
}
