import { expect } from "chai";
import { describe, it, before, after, beforeEach, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import os from "os";
import path from "path";


export function runCLIPythonCheckTest({ id = 0, pathToEIM, prerequisites = [] }) {

  describe(`${id}- Check for python installation |`, function () {
    this.timeout(600000);
    let testRunner = null;


    before(async function () {
      this.timeout(5000);
      testRunner = new CLITestRunner();
    });


    beforeEach(async function () {
      this.timeout(5000);
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
      this.timeout(5000);
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
    // The following test can only be executed if python have not been installed in the OS.
    it("1- Should detect missing python", async function () {
      this.timeout(25000);
      if (os.platform() === "win32") {
        this.skip();
      }
      logger.info(`Starting test - confirm python is missing`);
      const missingPython = await testRunner.waitForOutput(
        "Please install python3 with pip, venv and ssl support and try again",
        20000
      );
      expect(
        missingPython,
        'EIM did not show error message indicating "Please install python"'
      ).to.be.true;
      expect(testRunner.output, `EIM did not show error message indicating "Python is missing"`
       ).to.include("Python is missing, or it does not meet the requirements");
      logger.info(`python detection passed: >>\r ${testRunner.output}`);
    });


    /** Windows Specific Tests
     * Tests below will only be executed on win32 platform
     */
    it("1- should offer to install python and exit upon negative answer", async function () {
      this.timeout(25000);
      if (os.platform() !== "win32") {
        this.skip();
      }
      logger.info(`Starting test - confirm python is missing`);
      const promptPython = await testRunner.waitForOutput(
        "Do you want to install python?"
      );

      expect(
        promptRequisites,
        "EIM did not offer to install python"
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
