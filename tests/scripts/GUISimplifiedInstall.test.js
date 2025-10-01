import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import logger from "../classes/logger.class.js";

export function runGUISimplifiedInstallTest({ id = 0, pathToEIM }) {
  let eimRunner = "";

  describe("1- Run simplified mode", () => {
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
        logger.info("Error to close EIM application");
      }
    });

    it("1- Should show welcome page", async function () {
      this.timeout(25000);
      // Wait for the header to be present
      await new Promise((resolve) => setTimeout(resolve, 10000));
      const header = await eimRunner.findByCSS("h1");
      const text = await header.getText();
      expect(text, "Expected welcome text").to.equal(
        "Welcome to ESP-IDF Installation Manager"
      );
    });

    it("2- Should show installation options", async function () {
      this.timeout(10000);

      await eimRunner.clickButton("Start Installation");
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const header = await eimRunner.findByCSS("h1");
      const text = await header.getText();
      expect(text, "Expected installation setup screen").to.equal(
        "Install ESP-IDF"
      );
      const simplified = await eimRunner.findByText("Easy Installation");
      expect(simplified, "Expected option for simplified installation").to.not
        .be.false;
      expect(
        await simplified.isDisplayed(),
        "Expected option for simplified installation"
      ).to.be.true;
    });

    it("3- Should show installation summary", async function () {
      this.timeout(25000);

      await eimRunner.clickButton("Start Easy Installation");
      await new Promise((resolve) => setTimeout(resolve, 15000));
      const header = await eimRunner.findByCSS("h2");
      const text = await header.getText();
      expect(text, "Expected installation summary screen").to.equal(
        "Ready to Install"
      );
      const startButton = await eimRunner.findByText("Start Installation");
      expect(startButton, "Expected button to start installation").to.not.be
        .false;
      expect(
        await startButton.isDisplayed(),
        "Expected start button to be displayed"
      ).to.be.true;
    });

    it("4- Should install IDF using simplified setup", async function () {
      this.timeout(2730000);
      await eimRunner.clickButton("Start Installation");
      await new Promise((resolve) => setTimeout(resolve, 5000));
      const installing = await eimRunner.findByText(
        "Downloading ESP-IDF",
        20000
      );

      expect(installing, "Expected installation to start Downloading ESP-IDF")
        .to.not.be.false;
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

    it("5- Should show installation complete summary", async function () {
      this.timeout(10000);
      const documentationButton = await eimRunner.findByText(
        "View Documentation"
      );
      expect(documentationButton, "Expected button to show documentation").to
        .not.be.false;
      expect(
        await documentationButton.isDisplayed(),
        "Expected Expected button to show documentation to be displayed"
      ).to.be.true;
      const dashboardButton = await eimRunner.findByText("Go to Dashboard");
      expect(dashboardButton, "Expected button to return to dashboard").to.not
        .be.false;
      expect(
        await dashboardButton.isDisplayed(),
        "Expected Expected button to return to dashboard to be displayed"
      ).to.be.true;
    });
  });
}
