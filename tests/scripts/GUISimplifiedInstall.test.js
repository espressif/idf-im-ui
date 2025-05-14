import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import logger from "../classes/logger.class.js";

export function runGUISimplifiedInstallTest(id, pathToEIM) {
  let eimRunner = "";

  describe("1- EIM Application Launch", () => {
    let simplifiedInstallFailed = false;

    before(async function () {
      this.timeout(30000);
      eimRunner = new GUITestRunner(pathToEIM);
      try {
        await eimRunner.start();
      } catch (err) {
        logger.info("Error starting EIM application");
      }
    });

    beforeEach(async function () {
      if (simplifiedInstallFailed) {
        logger.info("Test failed, skipping next tests");
        this.skip();
      }
    });

    afterEach(async function () {
      if (this.currentTest.state === "failed") {
        await eimRunner.takeScreenshot(`${id} ${this.currentTest.title}.png`);
        logger.info(`Screenshot saved as ${id} ${this.currentTest.title}.png`);
        simplifiedInstallFailed = true;
      }
    });

    after(async function () {
      this.timeout(5000);
      try {
        await eimRunner.stop();
      } catch (error) {
        logger.info("Error to close IEM application");
      }
    });

    it("1- Should show welcome page", async function () {
      this.timeout(10000);
      // Wait for the header to be present
      const header = await eimRunner.findByCSS("h1");
      const text = await header.getText();
      expect(text, "Expected welcome text").to.equal(
        "Welcome to ESP-IDF Installation Manager!"
      );
    });

    it("2- Should show installation options", async function () {
      this.timeout(10000);

      await eimRunner.clickButton("Get Started");
      const header = await eimRunner.findByDataId("main-title");
      const text = await header.getText();
      expect(text, "Expected installation setup screen").to.equal(
        "Installation Setup"
      );
      const simplified = await eimRunner.findByText("Simplified Installation");
      expect(simplified, "Expected option for simplified installation").to.not
        .be.false;
      expect(
        await simplified.isDisplayed(),
        "Expected option for simplified installation"
      ).to.be.true;
    });

    it("3- Should install IDF using simplified setup", async function () {
      this.timeout(2730000);
      await eimRunner.clickButton("Start Simplified Setup");
      await new Promise((resolve) => setTimeout(resolve, 5000));
      const installing = await eimRunner.findByText(
        "Installing ESP-IDF...",
        20000
      );

      expect(installing, "Expected installation progress screen").to.not.be
        .false;
      expect(
        await installing.isDisplayed(),
        "Expected installation progress screen"
      ).to.be.true;

      const startTime = Date.now();
      while (Date.now() - startTime < 2700000) {
        if (await eimRunner.findByText("Installation Failed", 1000)) {
          logger.debug("failed!!!!");
          break;
        }
        if (await eimRunner.findByText("Installation Complete!", 1000)) {
          logger.debug("Completed!!!");
          break;
        }
        await new Promise((resolve) => setTimeout(resolve, 1000));
      }
      if (Date.now() - startTime >= 2700000) {
        logger.info("Installation timed out after 45 minutes");
      }
      const completed = await eimRunner.findByText("Installation Complete!");
      expect(completed, "Expected installation to be completed").to.not.be
        .false;
      expect(
        await completed.isDisplayed(),
        "Expected 'Installation Complete' text displayed"
      ).to.be.true;
    });

    it("4- Should offer to save installation configuration", async function () {
      this.timeout(10000);
      const saveConfig = await eimRunner.findByText("Save Configuration");
      expect(saveConfig, "Expected screen for saving configuration").to.not.be
        .false;
      expect(
        await saveConfig.isDisplayed(),
        "Expected option to save configuration to be displayed"
      ).to.be.true;
      const exit = await eimRunner.findByText("Exit Installer");
      expect(exit, "Expected option to exit installer").to.not.be.false;
    });
  });
}
