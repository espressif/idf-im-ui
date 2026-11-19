import { expect } from "chai";
import { describe, it, before, after } from "mocha";
import logger from "../classes/logger.class.js";
import { registerWizardTerminalHooks } from "../helpers/cliTestHelpers.js";
import os from "os";
import path from "path";


// This function verifies the presence of python in the system
// On Windows, the python is installed as part of the test.
export function runCLIPythonCheckTest({ id = 0, pathToEIM, prerequisites = [] }) {

  describe(`${id}- Check for python installation |`, function () {
    this.timeout(600000);
    let testRunner = null;


    registerWizardTerminalHooks({
      pathToEIM,
      setRunner: (runner) => (testRunner = runner),
      getRunner: () => testRunner,
      startTimeout: 5000,
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
        "Python sanity check failed",
        20000
      );
      expect(
        missingPython,
        'EIM did not show python check results with failures'
      ).to.be.true;
      expect(testRunner.output, `EIM did not indicate failed step"`
       ).to.include("[FAIL]");
      logger.info(`python detection passed: >>\r ${testRunner.output}`);
    });


    /** Windows Specific Tests
     * Tests below will only be executed on win32 platform
     */
    it("2- should offer to install python and exit upon negative answer", async function () {
      this.timeout(25000);
      if (os.platform() !== "win32") {
        this.skip();
      }
      logger.info(`Starting test - confirm python is missing`);
      const promptPython = await testRunner.waitForOutput(
        "Do you want to install"
      );

      expect(
        promptPython,
        "EIM did not offer to install python"
      ).to.be.true;

      testRunner.process.write("n");

      const terminalExited = await testRunner.waitForOutput(
        "Please install Python3"
      );
      expect(
        terminalExited,
        "EIM did not fails after denying to install pre-requisites"
      ).to.be.true;
      logger.info(`python detection passed: >>\r ${testRunner.output}`);
    });

    // This test installs python and confirms successful installation
    it("3- should install python after a positive answer", async function () {
      this.timeout(150000);
      if (os.platform() !== "win32") {
        this.skip();
      }
      logger.info(`Starting test - installing python`);
      await testRunner.waitForOutput(
        "Do you want to install Python?"
      );
      testRunner.process.write("y");

      const promptInstallation = await testRunner.waitForOutput(
        "Please select all of the target platforms",
        100000
      );
      expect(
        promptInstallation,
        "EIM completed installation of python"
      ).to.be.true;

      expect(testRunner.output, `EIM did not install python"`).to.include("Python installed successfully");
      logger.info(`python installation passed: >>\r ${testRunner.output}`);
    });
  });
}
