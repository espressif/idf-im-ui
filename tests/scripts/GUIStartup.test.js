import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import logger from "../classes/logger.class.js";

export function runGUIStartupTest(id, pathToEIM, eimVersion) {
  let eimRunner = "";

  describe("1- EIM startup", () => {
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
      } catch (error) {
        logger.info("Error to close EIM application");
      }
    });

    it("1- Should show splash screen", async function () {
      this.timeout(10000);
      const header = await eimRunner.findByCSS("h1");
      const text = await header.getText();
      expect(text, "Should Show Splash Screen").to.equal(
        "ESP-IDF Installation Manager"
      );
    });

    it("2- Should show welcome page", async function () {
      this.timeout(25000);
      // Wait for the header to be present
      await new Promise((resolve) => setTimeout(resolve, 10000));
      const header = await eimRunner.findByCSS("h1");
      const text = await header.getText();
      expect(text, "Expected welcome text").to.equal(
        "Welcome to ESP-IDF Installation Manager"
      );
    });

    it("3- Should show correct version number", async function () {
      const footer = await eimRunner.findByClass("app-footer");
      const text = await footer.getText();
      expect(text, "Expected correct version shown on page").to.include(
        `ESP-IDF Installation Manager v${eimVersion}`
      );
    });
  });
}
