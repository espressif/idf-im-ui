import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import logger from "../classes/logger.class.js";
import os from "os";
import { tGui } from "../helpers/i18n.js";

// This function verifies the EIM GUI properly lists the missing python installation
// On Windows, the python is installed as part of the test.

export function runGUIPythonCheckTest({ id = 0, pathToEIM}) {
  
  describe(`${id}- Python check |`, () => {
    let eimRunner = null;

    
    before(async function () {
      this.timeout(600000);
      eimRunner = new GUITestRunner(pathToEIM);
      try {
        await eimRunner.start();
      } catch (err) {
        logger.info("Error starting EIM application");
        throw err;
      }
      // Navigate from the welcome page into the wizard so each `it()` starts
      // on the Python Sanity Check (step 2). Without this, when prerequisites
      // are missing the wizard is stuck on step 1 and every subsequent test
      // hangs waiting for elements that only exist on later steps.
      await new Promise((resolve) => setTimeout(resolve, 10000));
      await eimRunner.clickButton(tGui("welcome.cards.new.button"));
      await new Promise((resolve) => setTimeout(resolve, 2000));
      await eimRunner.clickButton(tGui("basicInstaller.cards.custom.button"));
      await new Promise((resolve) => setTimeout(resolve, 5000));

      // On Windows, if a prerequisite (e.g. git) is missing the wizard shows
      // an "Install Missing Prerequisites" button that installs them via scoop.
      // Click it, wait for the install to complete and the "Continue" button
      // to appear (rendered after the prerequisite re-check passes).
      if (os.platform() === "win32") {
        const installPrereqsButton = await eimRunner.findByText(
          tGui("prerequisitiesCheck.actions.installMissing"),
          10000
        );
        if (installPrereqsButton) {
          await eimRunner.clickButton(
            tGui("prerequisitiesCheck.actions.installMissing")
          );
          await eimRunner.findByText(
            tGui("prerequisitiesCheck.actions.continue"),
            300000
          );
        }
      }

      // Advance past the Prerequisites Check. Handles both the
      // "all prerequisites already passed" and "just installed" cases.
      const continueButton = await eimRunner.findByText(
        tGui("prerequisitiesCheck.actions.continue"),
        30000
      );
      if (continueButton) {
        await eimRunner.clickButton(
          tGui("prerequisitiesCheck.actions.continue")
        );
        await new Promise((resolve) => setTimeout(resolve, 5000));
      }
    });

    afterEach(async function () {
      if (this.currentTest.state === "failed" && eimRunner?.driver) {
        await eimRunner.takeScreenshot(`${id} ${this.currentTest.title}.png`);
        logger.info(`Screenshot saved as ${id} ${this.currentTest.title}.png`);
      }
    });

    after(async function () {
      this.timeout(5000);

      try {
        await eimRunner.stop();
        eimRunner = null;
      } catch (error) {
        logger.info("Error to close EIM application");
      }
    });

    it("1- Should check python requirement", async function () {
      this.timeout(60000);
      const result = await eimRunner.findByDataId("python-check-result", 30000);
      expect(result, "python-check-result should be present on PythonSanitycheck").to.not.be.false;
      expect(await result.getText()).to.include(
        tGui("pythonSanitycheck.status.setupRequired.title")
      );
    });

    it("2- Should show option to install python on Windows", async function () {
      if (os.platform() !== "win32") {
        this.skip();
      }
      const installpythonButton = await eimRunner.findByText(
        tGui("pythonSanitycheck.actions.installPython"),
        30000
      );
      expect(installpythonButton, "Expected Install Python button to be present").to.not.be.false;
    });

    it("3- Should successfully install python on Windows", async function () {
      this.timeout(600000);
      if (os.platform() !== "win32") {
        this.skip();
      }
      await eimRunner.clickButton(tGui("pythonSanitycheck.actions.installPython"));
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const result = await eimRunner.findByText(tGui("targetSelect.title"), 580000);
      expect(result, "Expected Select Target Chips text to be present").to.not.be.false;
    });
  });
}
