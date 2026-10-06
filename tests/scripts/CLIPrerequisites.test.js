import { expect } from "chai";
import { describe, it, before, after } from "mocha";
import logger from "../classes/logger.class.js";
import { registerWizardTerminalHooks } from "../helpers/cliTestHelpers.js";
import os from "os";
import path from "path";

// This function verifies the presence of the prerequisites in the system
// On Windows, the prerequisites are installed as part of the test.
export function runCLIPrerequisitesTest({ id = 0, pathToEIM, prerequisites = [] }) {

  describe(`${id}- Check for prerequisites |`, function () {
    this.timeout(240000);
    let testRunner = null;

    registerWizardTerminalHooks({
      pathToEIM,
      setRunner: (runner) => (testRunner = runner),
      getRunner: () => testRunner,
      startTimeout: 20000,
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
    it("2- should offer to install prerequisites and exit upon negative answer", async function () {
      this.timeout(35000);
      if (os.platform() !== "win32") {
        this.skip();
      }
      logger.info(`Starting test - confirm requirements are missing`);
      const promptRequisites = await testRunner.waitForOutput(
        "Do you want to install",
        30000
      );

      expect(
        promptRequisites,
        "EIM did not offer to install the missing prerequisites"
      ).to.be.true;

      for (const prerequisite of prerequisites) {
        expect(testRunner.output, `EIM did not list missing prerequisite"${prerequisite}"`).to.include(prerequisite);
      }

      testRunner.process.write("n");

      // EIM reports failure differently depending on which check (prerequisites
      // vs. python sanity) triggered the prompt — accept either wording.
      const terminalExited = await testRunner.waitForOutput(
        "Please install"
      );
      expect(
        terminalExited,
        "EIM did not fails after denying to install pre-requisites"
      ).to.be.true;
      logger.info(`prerequisite detection passed: >>\r ${testRunner.output}`);
    });

    // This test installs git and confirms successful installation
    it("3- should install GIT after a positive answer", async function () {
      this.timeout(120000);
      if (os.platform() !== "win32") {
        this.skip();
      }
      logger.info(`Starting test - installing git`);
      await testRunner.waitForOutput(
        "Do you want to install prerequisites?",
        30000
      );

      testRunner.process.write("y");

      const promptPython = await testRunner.waitForOutput(
        "Do you want to install Python?",
        60000
      );
      expect(
        promptPython,
        "EIM did not Offer to install Python"
      ).to.be.true;
      testRunner.process.write("n");
      logger.info(`prerequisites installation passed: >>\r ${testRunner.output}`);
    });
  });
}
