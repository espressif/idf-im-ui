import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import logger from "../classes/logger.class.js";
import os from "os";

export function runGUIPrerequisitesTest({ id = 0, pathToEIM, prerequisites = [] }) {
  
  describe(`${id}- Prerequisites check |`, () => {
    let eimRunner = null;
    before(async function () {
      this.timeout(60000);
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

    it("1- Should check prerequisites", async function () {
      this.timeout(25000);
      await new Promise((resolve) => setTimeout(resolve, 10000));      
      await eimRunner.clickButton("Start Installation");
      await new Promise((resolve) => setTimeout(resolve, 2000));      
      await eimRunner.clickButton("Start Configuration Wizard");
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
      const installReqButton = await eimRunner.findByText("Install Missing Prerequisites");
      expect(installReqButton, "Expected Install Missing Prerequisites button to be present").to.not.be.false;

    });

    it("3- Should show option to check pre-requisites again", async function () {
      this.timeout(15000);
      if (os.platform() !== "win32") {
        this.skip();
      }
      const checkReqButton = await eimRunner.findByText("Check Prerequisites");
      expect(checkReqButton, "Expected check Prerequisites button to be present").to.not.be.false;
      await eimRunner.clickButton("Check Prerequisites");
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
      await eimRunner.clickButton("Install Missing Prerequisites");
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const result = await eimRunner.findByText("Python Setup Required", 60000);
      expect(result, "Expected python check screen").to.not.be.false;
    });
  });
}
