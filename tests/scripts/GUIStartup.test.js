import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import logger from "../classes/logger.class.js";
import { getOSName, getArchitecture } from "../helper.js";

export function runGUIStartupTest({ id = 0, pathToEIM, eimVersion }) {
  
  describe(`${id}- EIM startup |`, () => {
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

    it("1- Should show welcome page", async function () {
      this.timeout(20000);
      // Wait for the header to be present
      await new Promise((resolve) => setTimeout(resolve, 15000));
      const header = await eimRunner.findByCSS("h1");
      const text = await header.getText();
      expect(text, "Expected welcome text").to.equal(
        "Welcome to ESP-IDF Installation Manager"
      );
    });

    it("2- Should show correct version number", async function () {
      const footer = await eimRunner.findByClass("app-footer");
      const text = await footer.getText();
      expect(text, "Expected correct version shown on page").to.include(
        `ESP-IDF Installation Manager v${eimVersion}`
      );
    });

    it("3- Should show option to hide welcome page", async function () {
      const hideWelcome = await eimRunner.findByText(
        "show this welcome screen again"
      );
      const isDisplayed = await hideWelcome.isDisplayed();
      expect(isDisplayed, "Expected option to hide welcome page").to.be.true;
      const checkBox = await eimRunner.findByRelation(
        "parent",
        "div",
        "show this welcome screen again"
      );
      const checked = await checkBox.getAttribute("class");
      expect(checked, "Expected checkbox to be unchecked").to.not.include(
        "checked"
      );
    });

    it("4- Should show option to block sending usage statistics", async function () {
      const allowStatistics = await eimRunner.findByText(
        "Allow sending usage statistics"
      );
      const isDisplayed = await allowStatistics.isDisplayed();
      expect(isDisplayed, "Expected option to allow sending usage statistics")
        .to.be.true;
      const checkBox = await eimRunner.findByRelation(
        "parent",
        "div",
        "Allow sending usage statistics"
      );
      const checked = await checkBox.getAttribute("class");
      expect(checked, "Expected checkbox to be unchecked").to.include(
        "checked"
      );
    });

    it("5- Should show navigation options in the app footer", async function () {
      const documentationLink = await eimRunner.findByRelation(
        "parent",
        "a",
        "Documentation"
      );
      expect(documentationLink, "Expected Documentation link in footer").to.not
        .be.false;

      const logsButton = await eimRunner.findByRelation(
        "parent",
        "button",
        "Logs"
      );
      expect(logsButton, "Expected Logs button in footer").to.not.be.false;

      const reportIssueButton = await eimRunner.findByRelation(
        "parent",
        "button",
        "Report Issue"
      );
      expect(reportIssueButton, "Expected Report Issue button in footer").to.not
        .be.false;

      const aboutButton = await eimRunner.findByRelation(
        "parent",
        "button",
        "About"
      );
      expect(aboutButton, "Expected About button in footer").to.not.be.false;
    });

    it("6 - Should allow reporting an issue from the footer link and attach OS information", async function () {
      this.timeout(10000);
      await eimRunner.clickButton("Report Issue");
      await new Promise((resolve) => setTimeout(resolve, 3000));
      const OSData = await eimRunner.findByRelation("parent", "div", "OS");
      const osText = await OSData.getText();
      expect(osText, "Expected OS information to be present").to.include(
        getOSName()
      );

      const architectureData = await eimRunner.findByRelation(
        "parent",
        "div",
        "Architecture"
      );
      const architectureText = await architectureData.getText();
      expect(
        architectureText,
        "Expected Architecture information to be present"
      ).to.include(getArchitecture());

      const appVersionData = await eimRunner.findByRelation(
        "parent",
        "div",
        "App Version"
      );
      const appVersionText = await appVersionData.getText();
      expect(
        appVersionText,
        "Expected App Version information to be present"
      ).to.include(eimVersion);
      await eimRunner.clickButton("Cancel");
      await new Promise((resolve) => setTimeout(resolve, 1000));
      const generateButton = await eimRunner.findByText("Generate Report");
      expect(
        generateButton,
        "Expected Generate Report button to not be present after cancel"
      ).to.be.false;
    });

    it("7- Should show application information when clicking on about", async function () {
      this.timeout(5000);
      await eimRunner.clickButton("About");
      await new Promise((resolve) => setTimeout(resolve, 3000));
      const aboutText = await eimRunner.findByText(
        "A cross-platform tool for installing and managing ESP-IDF development environment"
      );
      expect(aboutText, "Expected about text to be present").to.not.be.false;
    });
  });
}
