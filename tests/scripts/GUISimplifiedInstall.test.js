import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import TestProxy from "../classes/TestProxy.class.js";
import logger from "../classes/logger.class.js";
import { tGui } from "../helpers/i18n.js";

// This function executes the simplified installation functionality of the EIM GUI
// No parameter is changed as part of this test
export function runGUISimplifiedInstallTest({
  id = 0,
  pathToEIM,
  testProxyMode = false,
  proxyBlockList = [],
}) {
  describe("1- Run simplified mode", () => {
    let eimRunner = null;
    let simplifiedInstallFailed = false;
    let proxy = null;

    // The setup function should start the proxy server if enabled and start the EIM application GUI
    before(async function () {
      this.timeout(60000);
      eimRunner = new GUITestRunner(pathToEIM);
      if (testProxyMode) {
        try {
          proxy = new TestProxy({
            mode: testProxyMode,
            blockedDomains: proxyBlockList,
          });
          await proxy.start();
        } catch (error) {
          logger.info("Error to start proxy server");
          logger.debug(`Error: ${error}`);
        }
      }
      try {
        await eimRunner.start();
      } catch (err) {
        logger.info("Error starting EIM application");
        throw err;
      }
    });

    // The beforeEach function should skip the next tests if the previous test failed
    beforeEach(async function () {
      if (simplifiedInstallFailed) {
        logger.info("Test failed, skipping next tests");
        this.skip();
      }
    });

    // The afterEach function should log the EIM application GUI screenshot on failure
    afterEach(async function () {
      if (this.currentTest.state === "failed" && eimRunner?.driver) {
        await eimRunner.takeScreenshot(`${id} ${this.currentTest.title}.png`);
        logger.info(`Screenshot saved as ${id} ${this.currentTest.title}.png`);
      }
      if (this.currentTest.state === "failed") simplifiedInstallFailed = true;
    });

    // The tear down function should stop the EIM application GUI and proxy server if enabled
    after(async function () {
      this.timeout(5000);
      try {
        await eimRunner.stop();
      } catch (error) {
        logger.info("Error to close EIM application");
      }
      if (testProxyMode) {
        try {
          await proxy.stop();
        } catch (error) {
          logger.info("Error stopping proxy server");
          logger.info(`${error}`);
        }
      }
    });

    it("1- Should show welcome page", async function () {
      this.timeout(45000);
      // Wait for the header to be present
      await new Promise((resolve) => setTimeout(resolve, 10000));
      const header = await eimRunner.findByDataId("welcome-header", 25000);
      expect(header, "Expected welcome header").to.not.be.false;
      const text = await header.getText();
      expect(text, "Expected welcome text").to.equal(
        `${tGui("welcome.welcome")} ESP-IDF ${tGui("welcome.title")}`
      );
    });

    it("2- Should show installation options", async function () {
      this.timeout(10000);

      await eimRunner.clickButton(tGui("welcome.cards.new.button"));
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const header = await eimRunner.findByCSS("h1");
      const text = await header.getText();
      expect(text, "Expected installation setup screen").to.equal(
        tGui("basicInstaller.title")
      );
      const simplified = await eimRunner.findByText(
        tGui("basicInstaller.cards.easy.title")
      );
      expect(simplified, "Expected option for simplified installation").to.not
        .be.false;
      expect(
        await simplified.isDisplayed(),
        "Expected option for simplified installation"
      ).to.be.true;
    });

    it("3- Should show installation summary", async function () {
      this.timeout(35000);

      await eimRunner.clickButton(tGui("basicInstaller.cards.easy.button"));
      await new Promise((resolve) => setTimeout(resolve, 25000));
      const header = await eimRunner.findByCSS("h2");
      const text = await header.getText();
      expect(text, "Expected installation summary screen").to.equal(
        tGui("simpleSetup.ready.title")
      );
      const startButton = await eimRunner.findByText(
        tGui("simpleSetup.ready.startButton")
      );
      expect(startButton, "Expected button to start installation").to.not.be
        .false;
      expect(
        await startButton.isDisplayed(),
        "Expected start button to be displayed"
      ).to.be.true;
    });

    it("4- Should install IDF using simplified setup", async function () {
      this.timeout(2730000);
      await eimRunner.clickButton(tGui("simpleSetup.ready.startButton"));
      await new Promise((resolve) => setTimeout(resolve, 5000));
      const installing = await eimRunner.findByText(
        tGui("simpleSetup.installation.steps.download.description"),
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
        if (await eimRunner.findByText(tGui("simpleSetup.error.title"), 1000)) {
          logger.debug("failed!!!!");
          break;
        }
        if (
          await eimRunner.findByText(tGui("simpleSetup.complete.title"), 1000)
        ) {
          logger.debug("Completed!!!");
          break;
        }
        await new Promise((resolve) => setTimeout(resolve, 1000));
      }
      if (Date.now() - startTime >= 2700000) {
        logger.info("Installation timed out after 45 minutes");
      }
      const completed = await eimRunner.findByText(
        tGui("simpleSetup.complete.title")
      );
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
        tGui("simpleSetup.complete.buttons.documentation")
      );
      expect(documentationButton, "Expected button to show documentation").to
        .not.be.false;
      expect(
        await documentationButton.isDisplayed(),
        "Expected Expected button to show documentation to be displayed"
      ).to.be.true;
      const dashboardButton = await eimRunner.findByText(
        tGui("simpleSetup.complete.buttons.dashboard")
      );
      expect(dashboardButton, "Expected button to return to dashboard").to.not
        .be.false;
      expect(
        await dashboardButton.isDisplayed(),
        "Expected Expected button to return to dashboard to be displayed"
      ).to.be.true;
    });
  });
}
