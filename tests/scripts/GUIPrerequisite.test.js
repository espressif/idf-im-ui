import { expect } from "chai";
import { describe, it } from "mocha";
import logger from "../classes/logger.class.js";
import {
  registerGUIAppLifecycle,
  registerGUIFailureHooks,
} from "../helpers/guiTestHelpers.js";
import os from "os";
import { tGui } from "../helpers/i18n.js";

// This function verifies the EIM GUI properly lists the missing prerequisites
// On Windows, the prerequisites are installed as part of the test.
export function runGUIPrerequisitesTest({ id = 0, pathToEIM, prerequisites = [] }) {
  
  describe(`${id}- Prerequisites check |`, () => {
    let eimRunner = null;


    // Start the EIM application GUI before the tests and stop it after them
    registerGUIAppLifecycle({
      pathToEIM,
      getRunner: () => eimRunner,
      setRunner: (runner) => (eimRunner = runner),
    });

    // Save a screenshot of the EIM application GUI on failure
    registerGUIFailureHooks({ id, getRunner: () => eimRunner, skipAfterFailure: false });

    it("1- Should check prerequisites", async function () {
      this.timeout(25000);
      await new Promise((resolve) => setTimeout(resolve, 10000));
      await eimRunner.clickByDataId("new-installation-button");
      await new Promise((resolve) => setTimeout(resolve, 2000));
      await eimRunner.clickByDataId("custom-mode-button");
      await new Promise((resolve) => setTimeout(resolve, 5000));
      const prerequisitesList = await eimRunner.findByDataId(
        "prerequisites-items-list"
      );
      const requisitesList = await prerequisitesList.getText();
      logger.info(`Prerequisites found on the GUI: ${requisitesList}`);
      expect(requisitesList).to.not.be.empty;
      for (let requisite of prerequisites) {
        expect(requisitesList).to.include(requisite);
      }
    });

    it("2- Should show option to install pre-requisites on Windows", async function () {
      if (os.platform() !== "win32") {
        this.skip();
      }
      const installReqButton = await eimRunner.findByDataId(
        "install-prerequisites-button"
      );
      expect(installReqButton, "Expected Install Missing Prerequisites button to be present").to.not.be.false;

    });

    it("3- Should show option to check pre-requisites again", async function () {
      this.timeout(15000);
      if (os.platform() !== "win32") {
        this.skip();
      }
      const checkReqButton = await eimRunner.findByDataId(
        "check-prerequisites-button"
      );
      expect(checkReqButton, "Expected check Prerequisites button to be present").to.not.be.false;
      await eimRunner.clickByDataId("check-prerequisites-button");
      await new Promise((resolve) => setTimeout(resolve, 10000));
      const prerequisitesList = await eimRunner.findByDataId(
        "prerequisites-items-list"
      );
      const requisitesList = await prerequisitesList.getText();
      expect(requisitesList).to.not.be.empty;
      for (let requisite of prerequisites) {
        expect(requisitesList).to.include(requisite);
      }
    });

    it("4- Should successfully install prerequisites on Windows", async function () {
      this.timeout(80000);
      if (os.platform() !== "win32") {
        this.skip();
      }
      await eimRunner.clickByDataId("install-prerequisites-button");
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const result = await eimRunner.findByDataId("python-check-result", 60000);
      expect(result, "Expected python check screen").to.not.be.false;
      expect(await result.getText()).to.include(
        tGui("pythonSanitycheck.status.setupRequired.title")
      );
    });
  });
}
