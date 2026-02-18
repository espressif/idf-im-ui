import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import logger from "../classes/logger.class.js";
import os from "os";

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
      }
    });

    afterEach(async function () {
      if (this.currentTest.state === "failed") {
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
      await eimRunner.clickButton("Start Installation");
      await new Promise((resolve) => setTimeout(resolve, 2000));      
      await eimRunner.clickButton("Start Configuration Wizard");
      await new Promise((resolve) => setTimeout(resolve, 5000));      
      const result = await eimRunner.findByDataId("python-check-result");
      expect(await result.getText()).to.include("Python Setup Required");
    });

    it("2- Should show option to install python on Windows", async function () {
      if (os.platform() !== "win32") {
        this.skip();
      }
      const installpythonButton = await eimRunner.findByText("Install Python");
      expect(installpythonButton, "Expected Install Python button to be present").to.not.be.false;
    });

    it("3- Should successfully install python on Windows", async function () {
      this.timeout(45000);
      if (os.platform() !== "win32") {
        this.skip();
      }
      await eimRunner.clickButton("Install Python");
      await new Promise((resolve) => setTimeout(resolve, 30000));
      const result = await eimRunner.findByText("Select Target Chips");
      expect(result, "Expected Select Target Chips text to be present").to.not.be.false;
    });
  });
}
