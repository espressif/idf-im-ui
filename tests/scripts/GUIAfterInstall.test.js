import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import logger from "../classes/logger.class.js";
import { By } from "selenium-webdriver";

export function runGUIAfterInstallTest({ id = 0, pathToEIM, idfList }) {
  
  describe(`${id}- EIM GUI After Install |`, () => {
    let eimRunner = null;
    let totalInstallations = 0;
    let afterInstallFailed = false;
    
    before(async function () {
      this.timeout(60000);
      eimRunner = new GUITestRunner(pathToEIM);
      try {
        await eimRunner.start();
      } catch (err) {
        logger.info("Error starting EIM application");
      }
    });

    beforeEach(async function () {
      if (afterInstallFailed) {
        logger.info("Test failed, skipping next tests");
        this.skip();
      }
    });

    afterEach(async function () {
      if (this.currentTest.state === "failed") {
        await eimRunner.takeScreenshot(`${id} ${this.currentTest.title}.png`);
        logger.info(`Screenshot saved as ${id} ${this.currentTest.title}.png`);
        afterInstallFailed = true;
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

    it("1- Should show welcome page", async function () {
      this.timeout(25000);
      // Wait for the header to be present231e
      await new Promise((resolve) => setTimeout(resolve, 10000));
      const header = await eimRunner.findByCSS("h1");
      const text = await header.getText();
      expect(text, "Expected welcome text").to.equal(
        "Welcome to ESP-IDF Installation Manager"
      );
    });

    it("2- Should show option to manage installations", async function () {
      this.timeout(10000);
      const dashboardCard = await eimRunner.findByText("Manage Installations");
      expect(
        dashboardCard,
        "Expected dashboard card to be shown on welcome page"
      ).to.not.be.false;
      const dashboardContent = await eimRunner.findByText("View and manage");
      const text = await dashboardContent.getText();
      const numberMatch = text.match(/\d+/);
      totalInstallations = numberMatch ? parseInt(numberMatch[0], 10) : 0;
      expect(
        totalInstallations,
        "Expected at least one installation"
      ).to.be.gte(1);
      const click = await eimRunner.clickButton("Open Dashboard");
      expect(click, "Expected to click on Open Dashboard button").to.be.true;
    });

    it("3- Should show dashboard with installations", async function () {
      this.timeout(10000);
      await new Promise((resolve) => setTimeout(resolve, 5000));
      const cards = await eimRunner.findMultipleByClass("n-card");
      expect(cards.length, "Expected matching number of cards").to.be.equal(
        totalInstallations
      );
      let versionsList = [];
      for (let card of cards) {
        const versionElement = await card.findElement(
          By.className("version-info")
        );
        const versionText = await versionElement.getText();
        versionsList.push(versionText);
      }
      logger.debug(`Installed versions: ${versionsList}`);
      for (let idfVersion of idfList) {
        expect(
          versionsList.includes(idfVersion),
          `Expected dashboard card to be shown for version ${idfVersion} `
        ).to.be.true;
      }
    });
  });
}
