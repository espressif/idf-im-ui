import { expect } from "chai";
import { describe, it, before, after, beforeEach, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import os from "os";
import path from "path";


export function runCLIPrerequisitesTest({ id = 0, pathToEIM, prerequisites = [] }) {

  describe(`${id}- Check for prerequisites |`, function () {
    this.timeout(120000);
    let testRunner = null;


    before(async function () {
      this.timeout(10000);
      testRunner = new CLITestRunner();
    });


    beforeEach(async function () {
      this.timeout(20000);
      testRunner = new CLITestRunner();
      try {
        await testRunner.start();
        testRunner.sendInput(`${pathToEIM} wizard`);
      } catch (error) {
        logger.info(`Error starting process: ${error}`);
        logger.debug(` Error: ${error}`);
      }
    });

    afterEach(async function () {
      this.timeout(20000);
      if (this.currentTest.state === "failed") {
        logger.info(`Test failed: ${this.currentTest.title}`);
        logger.info(`Terminal output: >>\r ${testRunner.output.slice(-1000)}`);
        logger.debug(`Terminal output on failure: >>\r ${testRunner.output}`);
      }
    });
  
    after(async function () {
      this.timeout(20000);
      if (testRunner) {
        try {
          await testRunner.stop();
        } catch (error) {
          logger.info("Error to clean up terminal after test");
          logger.info(` Error: ${error}`);
        } finally {
          testRunner = null;
        }
      };
    });

    // Linux/MAC Specific Tests
    // The following test can only be executed if the prerequisites have not been installed in the OS.
    it("1- Should detect missing requirements", async function () {
      this.timeout(55000);
      if (os.platform() === "win32") {
        this.skip();
      }
      logger.info(`Starting test - confirm requirements are missing`);
      const missingRequisites = await testRunner.waitForOutput(
        "Please install the missing prerequisites and try again",
        50000
      );
      expect(
        missingRequisites,
        'EIM did not show error message indicating "Please install prerequisites"'
      ).to.be.true;
      for (const prerequisite of prerequisites) {
        expect(testRunner.output, `EIM did not list missing prerequisite"${prerequisite}"`).to.include(prerequisite);
      }
      logger.info(`prerequisite detection passed: >>\r ${testRunner.output}`);
    });


    /** Windows Specific Tests
     * Tests below will only be executed on win32 platform
     */
    it("1- should offer to install prerequisites and exit upon negative answer", async function () {
      this.timeout(35000);
      if (os.platform() !== "win32") {
        this.skip();
      }
      logger.info(`Starting test - confirm requirements are missing`);
      const promptRequisites = await testRunner.waitForOutput(
        "Do you want to install prerequisites?",
        3000
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
  });
}
