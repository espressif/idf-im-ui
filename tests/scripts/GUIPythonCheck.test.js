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
      this.timeout(100000);
      eimRunner = new GUITestRunner(pathToEIM);
      try {
        await eimRunner.start();
      } catch (err) {
        logger.info("Error starting EIM application");
        throw err;
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
      this.timeout(25000);
      await new Promise((resolve) => setTimeout(resolve, 10000));
      await eimRunner.clickButton(tGui("welcome.cards.new.button"));
      await new Promise((resolve) => setTimeout(resolve, 2000));
      await eimRunner.clickButton(tGui("basicInstaller.cards.custom.button"));
      await new Promise((resolve) => setTimeout(resolve, 5000));
      const result = await eimRunner.findByDataId("python-check-result");
      expect(await result.getText()).to.include(
        tGui("pythonSanitycheck.status.setupRequired.title")
      );
    });

    it("2- Should show option to install python on Windows", async function () {
      if (os.platform() !== "win32") {
        this.skip();
      }
      const installpythonButton = await eimRunner.findByText(
        tGui("pythonSanitycheck.actions.installPython")
      );
      expect(installpythonButton, "Expected Install Python button to be present").to.not.be.false;
    });

    it("3- Should successfully install python on Windows", async function () {
      this.timeout(80000);
      if (os.platform() !== "win32") {
        this.skip();
      }
      await eimRunner.clickButton(tGui("pythonSanitycheck.actions.installPython"));
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const result = await eimRunner.findByText(tGui("targetSelect.title"), 450000);
      expect(result, "Expected Select Target Chips text to be present").to.not.be.false;
    });
  });
}
