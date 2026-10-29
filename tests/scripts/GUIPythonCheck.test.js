import { expect } from "chai";
import { describe, it } from "mocha";
import {
  registerGUIAppLifecycle,
  registerGUIFailureHooks,
} from "../helpers/guiTestHelpers.js";
import os from "os";
import { tGui } from "../helpers/i18n.js";

// This function verifies the EIM GUI properly lists the missing python installation
// On Windows, the python is installed as part of the test.

export function runGUIPythonCheckTest({ id = 0, pathToEIM}) {
  
  describe(`${id}- Python check |`, () => {
    let eimRunner = null;

    
    registerGUIAppLifecycle({
      pathToEIM,
      getRunner: () => eimRunner,
      setRunner: (runner) => (eimRunner = runner),
      startTimeout: 600000,
      afterStart: async () => {
        // Navigate from the welcome page into the wizard so each `it()` starts
        // on the Python Sanity Check (step 2). Without this, when prerequisites
        // are missing the wizard is stuck on step 1 and every subsequent test
        // hangs waiting for elements that only exist on later steps.
        await new Promise((resolve) => setTimeout(resolve, 10000));
        await eimRunner.clickByDataId("new-installation-button");
        await new Promise((resolve) => setTimeout(resolve, 2000));
        await eimRunner.clickByDataId("custom-mode-button");
        await new Promise((resolve) => setTimeout(resolve, 5000));

        // On Windows, if a prerequisite (e.g. git) is missing the wizard shows
        // an "Install Missing Prerequisites" button that installs them via scoop.
        // Click it, wait for the install to complete and the "Continue" button
        // to appear (rendered after the prerequisite re-check passes).
        if (os.platform() === "win32") {
          const installPrereqsButton = await eimRunner.findByDataId(
            "install-prerequisites-button",
            10000
          );
          if (installPrereqsButton) {
            await eimRunner.clickByDataId("install-prerequisites-button");
            await eimRunner.findByDataId(
              "prerequisites-continue-button",
              300000
            );
          }
        }

        // Advance past the Prerequisites Check. Handles both the
        // "all prerequisites already passed" and "just installed" cases.
        const continueButton = await eimRunner.findByDataId(
          "prerequisites-continue-button",
          30000
        );
        if (continueButton) {
          await eimRunner.clickByDataId("prerequisites-continue-button");
          await new Promise((resolve) => setTimeout(resolve, 5000));
        }
      },
    });

    registerGUIFailureHooks({ id, getRunner: () => eimRunner, skipAfterFailure: false });

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
      const installpythonButton = await eimRunner.findByDataId(
        "install-python-button",
        30000
      );
      expect(installpythonButton, "Expected Install Python button to be present").to.not.be.false;
    });

    it("3- Should successfully install python on Windows", async function () {
      this.timeout(600000);
      if (os.platform() !== "win32") {
        this.skip();
      }
      await eimRunner.clickByDataId("install-python-button");
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const result = await eimRunner.findByDataId("target-select-title", 580000);
      expect(result, "Expected Select Target Chips text to be present").to.not.be.false;
    });
  });
}
