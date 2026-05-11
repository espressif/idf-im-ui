import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import logger from "../classes/logger.class.js";
import { getOSName, getArchitecture } from "../helper.js";
import { tGui, xpathText } from "../helpers/i18n.js";

// This function verifies the EIM GUI properly starts and displays the welcome page

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

    it("1- Should show welcome page", async function () {
      this.timeout(45000);
      // Wait for the header to be present
      await new Promise((resolve) => setTimeout(resolve, 15000));
      const header = await eimRunner.findByDataId("welcome-header", 25000);
      expect(header, "Expected welcome header").to.not.be.false;
      const text = await header.getText();
      expect(text, "Expected welcome text").to.equal(
        `${tGui("welcome.welcome")} ESP-IDF ${tGui("welcome.title")}`
      );
    });

    it("2- Should show correct version number", async function () {
      const footer = await eimRunner.findByClass("app-footer");
      const text = await footer.getText();
      expect(text, "Expected correct version shown on page").to.include(
        tGui("footer.app.version", { version: eimVersion })
      );
    });

    it("3- Should show option to hide welcome page", async function () {
      const dontShowText = xpathText("welcome.preferences.dontShow");
      const hideWelcome = await eimRunner.findByText(dontShowText, 20000);
      expect(hideWelcome, "Expected option to hide welcome page").to.not.be
        .false;
      const isDisplayed = await hideWelcome.isDisplayed();
      expect(isDisplayed, "Expected option to hide welcome page").to.be.true;
      const checkBox = await eimRunner.findByRelation(
        "parent",
        "div",
        dontShowText
      );
      const checked = await checkBox.getAttribute("class");
      expect(checked, "Expected checkbox to be unchecked").to.not.include(
        "checked"
      );
    });

    it("4- Should show usage statistics opt-in (off with do-not-track)", async function () {
      const allowStatistics = await eimRunner.findByText(
        tGui("welcome.preferences.allowTracking"),
        20000
      );
      expect(
        allowStatistics,
        "Expected option to allow sending usage statistics"
      ).to.not.be.false;
      const isDisplayed = await allowStatistics.isDisplayed();
      expect(isDisplayed, "Expected option to allow sending usage statistics")
        .to.be.true;
      const checkBox = await eimRunner.findByDataId(
        "allow-usage-tracking-checkbox"
      );
      expect(checkBox, "Expected usage statistics checkbox").to.not.be.false;
      const checked = await checkBox.getAttribute("class");
      expect(checked, "Expected usage statistics opt-in off under do-not-track").to
        .not.include("checked");
    });

    it("5- Should show navigation options in the app footer", async function () {
      const documentationLink = await eimRunner.findByRelation(
        "parent",
        "a",
        tGui("footer.buttons.documentation")
      );
      expect(documentationLink, "Expected Documentation link in footer").to.not
        .be.false;

      const logsButton = await eimRunner.findByRelation(
        "parent",
        "button",
        tGui("footer.buttons.logs")
      );
      expect(logsButton, "Expected Logs button in footer").to.not.be.false;

      const reportIssueButton = await eimRunner.findByRelation(
        "parent",
        "button",
        tGui("footer.buttons.reportIssue")
      );
      expect(reportIssueButton, "Expected Report Issue button in footer").to.not
        .be.false;

      const aboutButton = await eimRunner.findByRelation(
        "parent",
        "button",
        tGui("footer.buttons.about")
      );
      expect(aboutButton, "Expected About button in footer").to.not.be.false;
    });

    it("6 - Should allow reporting an issue from the footer link and attach OS information", async function () {
      this.timeout(10000);
      await eimRunner.clickButton(tGui("footer.buttons.reportIssue"));
      await new Promise((resolve) => setTimeout(resolve, 3000));
      const OSData = await eimRunner.findByRelation(
        "parent",
        "div",
        tGui("footer.modal.report.labels.os")
      );
      const osText = await OSData.getText();
      expect(osText, "Expected OS information to be present").to.include(
        getOSName()
      );

      const architectureData = await eimRunner.findByRelation(
        "parent",
        "div",
        tGui("footer.modal.report.labels.arch")
      );
      const architectureText = await architectureData.getText();
      expect(
        architectureText,
        "Expected Architecture information to be present"
      ).to.include(getArchitecture());

      const appVersionData = await eimRunner.findByRelation(
        "parent",
        "div",
        tGui("footer.modal.report.labels.appVersion")
      );
      const appVersionText = await appVersionData.getText();
      expect(
        appVersionText,
        "Expected App Version information to be present"
      ).to.include(eimVersion);
      await eimRunner.clickButton(tGui("footer.modal.report.buttons.cancel"));
      await new Promise((resolve) => setTimeout(resolve, 1000));
      const generateButton = await eimRunner.findByText(
        tGui("footer.modal.report.buttons.generate")
      );
      expect(
        generateButton,
        "Expected Generate Report button to not be present after cancel"
      ).to.be.false;
    });

    it("7- Should show application information when clicking on about", async function () {
      this.timeout(5000);
      await eimRunner.clickButton(tGui("footer.buttons.about"));
      await new Promise((resolve) => setTimeout(resolve, 3000));
      const aboutText = await eimRunner.findByText(
        tGui("footer.modal.about.description.line1")
      );
      expect(aboutText, "Expected about text to be present").to.not.be.false;
    });
  });
}
