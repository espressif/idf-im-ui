import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import { By, Key } from "selenium-webdriver";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import {
  IDFMIRRORS,
  TOOLSMIRRORS,
  IDFAvailableVersions,
  availableTargets,
} from "../config.js";
import logger from "../classes/logger.class.js";
import os from "os";

export function runGUICustomInstallTest(
  id,
  pathToEIM,
  installFolder,
  targetList,
  idfVersionList,
  toolsMirror,
  idfMirror
) {
  let eimRunner = "";

  describe("1- Run expert mode", () => {
    let customInstallFailed = false;

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
      if (customInstallFailed) {
        logger.info("Test failed, skipping next tests");
        this.skip();
      }
    });

    afterEach(async function () {
      if (this.currentTest.state === "failed") {
        await eimRunner.takeScreenshot(`${id} ${this.currentTest.title}.png`);
        logger.info(`Screenshot saved as ${id} ${this.currentTest.title}.png`);
        customInstallFailed = true;
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

    it("01- Should show welcome page", async function () {
      this.timeout(25000);
      // Wait for the header to be present
      await new Promise((resolve) => setTimeout(resolve, 10000));
      const header = await eimRunner.findByCSS("h1");
      const text = await header.getText();
      expect(text, "Expected welcome text").to.equal(
        "Welcome to ESP-IDF Installation Manager"
      );
    });

    it("02- Should show expert installation option", async function () {
      this.timeout(10000);
      await eimRunner.clickButton("Start Installation");
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const header = await eimRunner.findByCSS("h1");
      const text = await header.getText();
      expect(text, "Expected installation setup screen").to.equal(
        "Install ESP-IDF"
      );
      const custom = await eimRunner.findByText("Custom Installation");
      expect(custom, "Expected option for custom installation").to.not.be.false;
      expect(
        await custom.isDisplayed(),
        "Expected option for simplified installation"
      ).to.be.true;
    });

    it("03- Should check prerequisites", async function () {
      this.timeout(20000);
      await eimRunner.clickButton("Start Configuration Wizard");
      await new Promise((resolve) => setTimeout(resolve, 5000));
      const prerequisitesList = await eimRunner.findByDataId(
        "prerequisites-items-list"
      );
      const requisitesList = await prerequisitesList.getText();
      expect(requisitesList).to.not.be.empty;
      expect(requisitesList).to.not.include("❌");
      let expectedRequisites =
        os.platform() === "win32"
          ? ["git"]
          : [
              "git",
              "wget",
              "flex",
              "bison",
              "gperf",
              "ccache",
              "dfu-util",
              "libffi-dev",
              "libusb-1.0-0",
              "libssl-dev",
              "libgcrypt20",
              "libglib2.0-0",
              "libpixman-1-0",
              "libsdl2-2.0-0",
              "libslirp0",
            ];
      for (let requisite of expectedRequisites) {
        expect(requisitesList).to.include(requisite);
      }
    });

    it("04- Should check python installation", async function () {
      this.timeout(15000);
      await eimRunner.clickButton("Continue to Next Step");
      await new Promise((resolve) => setTimeout(resolve, 5000));
      const result = await eimRunner.findByDataId("python-check-result");
      expect(await result.getText()).to.include("Python Environment Ready");
    });

    it("05- Should show targets list", async function () {
      this.timeout(10000);
      await eimRunner.clickButton("Continue to Next Step");
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const targetsList = await eimRunner.findByDataId("targets-grid");
      const targetsText = await targetsList.getText();
      for (let target of availableTargets) {
        expect(targetsText).to.include(target);
      }
      let targetAll = await eimRunner.findByText("All");
      expect(
        await targetAll.findElement(By.css("Div")).getAttribute("class")
      ).to.include("checked");
      let targetESP32 = await eimRunner.findByDataId("target-item-esp32");
      expect(await targetESP32.getAttribute("class")).to.not.include(
        "selected"
      );
      await eimRunner.clickElement("esp32");
      expect(
        await targetAll.findElement(By.css("Div")).getAttribute("class")
      ).not.to.include("checked");
      expect(await targetESP32.getAttribute("class")).to.include("selected");
      await eimRunner.clickElement("esp32");
      expect(
        await targetAll.findElement(By.css("Div")).getAttribute("class")
      ).to.include("checked");
      await eimRunner.clickElement("All");

      for (let target of targetList) {
        await eimRunner.clickElement(target);
        if (target === "All") {
          expect(
            await targetAll.findElement(By.css("Div")).getAttribute("class")
          ).to.include("checked");
        } else {
          let selectedTarget = await eimRunner.findByDataId(
            `target-item-${target}`
          );
          expect(await selectedTarget.getAttribute("class")).to.include(
            "selected"
          );
        }
      }
    });

    it("06- Should show IDF version list", async function () {
      this.timeout(15000);
      await eimRunner.clickButton("Continue with Selected Targets");
      await new Promise((resolve) => setTimeout(resolve, 4000));
      const IDFList = await eimRunner.findByDataId("versions-grid");
      const IDFListText = await IDFList.getText();
      for (let version of IDFAvailableVersions) {
        expect(IDFListText).to.include(version);
      }
      let IDFMaster = await eimRunner.findByDataId("version-item-master");
      expect(await IDFMaster.getAttribute("class")).to.not.include("selected");
      await eimRunner.clickElement("master");
      expect(await IDFMaster.getAttribute("class")).to.include("selected");
      const selectedMaster = await eimRunner.findByDataId(
        "selected-tag-master"
      );
      let closeButton = await selectedMaster.findElement(By.css("button"));
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        closeButton
      );
      expect(await IDFMaster.getAttribute("class")).to.not.include("selected");
      for (let version of idfVersionList) {
        await eimRunner.clickElement(version);
      }

      const selectedVersions = await eimRunner.findByText("Selected versions");
      const selectedVersionsText = await selectedVersions.getText();
      const expected = ["Selected versions:", ...idfVersionList];
      for (let substring of expected) {
        expect(selectedVersionsText).to.include(substring);
      }
    });

    it("07- Should show IDF download mirrors", async function () {
      this.timeout(15000);
      await eimRunner.clickButton("Continue Installation");
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const IDFMirrorsList = await eimRunner.findByRelation(
        "parent",
        "div",
        "ESP-IDF Repository Mirror"
      );
      let IDFMirrorsListText = await IDFMirrorsList.getText();
      for (let mirror of Object.values(IDFMIRRORS)) {
        expect(IDFMirrorsListText).to.include(mirror);
      }

      let githubMirror = await eimRunner.findByDataId(
        "idf-mirror-option-https://github.com"
      );
      let jihulabMirror = await eimRunner.findByDataId(
        "idf-mirror-option-https://jihulab.com/esp-mirror"
      );
      expect(await githubMirror.getAttribute("class")).to.include("selected");
      expect(await jihulabMirror.getAttribute("class")).to.not.include(
        "selected"
      );
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        jihulabMirror
      );
      expect(await githubMirror.getAttribute("class")).to.not.include(
        "selected"
      );
      expect(await jihulabMirror.getAttribute("class")).to.include("selected");

      idfMirror === "github" &&
        (await eimRunner.driver.executeScript(
          "arguments[0].click();",
          githubMirror
        ));
      idfMirror === "jihulab" &&
        (await eimRunner.driver.executeScript(
          "arguments[0].click();",
          jihulabMirror
        ));
    });

    it("08- Should show tools download mirrors", async function () {
      this.timeout(10000);
      const toolsMirrorsList = await eimRunner.findByRelation(
        "parent",
        "div",
        "ESP-IDF Tools Mirror"
      );
      let toolsMirrorsListText = await toolsMirrorsList.getText();
      for (let mirror of Object.values(TOOLSMIRRORS)) {
        expect(toolsMirrorsListText).to.include(mirror);
      }
      let githubMirror = await eimRunner.findByDataId(
        "tools-mirror-option-https://github.com"
      );
      let espressifComMirror = await eimRunner.findByDataId(
        "tools-mirror-option-https://dl.espressif.com/github_assets"
      );
      let espressifCnMirror = await eimRunner.findByDataId(
        "tools-mirror-option-https://dl.espressif.cn/github_assets"
      );
      expect(await githubMirror.getAttribute("class")).to.include("selected");
      expect(await espressifComMirror.getAttribute("class")).to.not.include(
        "selected"
      );
      expect(await espressifCnMirror.getAttribute("class")).to.not.include(
        "selected"
      );
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        espressifComMirror
      );
      expect(await githubMirror.getAttribute("class")).to.not.include(
        "selected"
      );
      expect(await espressifComMirror.getAttribute("class")).to.include(
        "selected"
      );
      expect(await espressifCnMirror.getAttribute("class")).to.not.include(
        "selected"
      );
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        espressifCnMirror
      );
      expect(await githubMirror.getAttribute("class")).to.not.include(
        "selected"
      );
      expect(await espressifComMirror.getAttribute("class")).to.not.include(
        "selected"
      );
      expect(await espressifCnMirror.getAttribute("class")).to.include(
        "selected"
      );

      toolsMirror === "github" &&
        (await eimRunner.driver.executeScript(
          "arguments[0].click();",
          githubMirror
        ));
      toolsMirror === "dl_com" &&
        (await eimRunner.driver.executeScript(
          "arguments[0].click();",
          espressifComMirror
        ));
      toolsMirror === "dl_cn" &&
        (await eimRunner.driver.executeScript(
          "arguments[0].click();",
          espressifCnMirror
        ));
    });

    it("09- Should show installation path", async function () {
      this.timeout(10000);
      await eimRunner.clickButton("Continue with Selected Mirrors");
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const installPath = await eimRunner.findByDataId("path-info-title");
      expect(await installPath.getText()).to.equal(
        "ESP-IDF Installation Directory"
      );
      expect(await installPath.isDisplayed()).to.be.true;
      const pathInput = await eimRunner.findByDataId("installation-path-input");
      const input = await pathInput.findElement(By.css("input"));
      const defaultInput =
        os.platform() === "win32" ? "C:\\esp" : "/.espressif";
      expect(await input.getAttribute("value")).to.include(defaultInput);
      await input.sendKeys(Key.CONTROL + "a");
      await input.sendKeys(Key.CONTROL + "a");
      await input.sendKeys(Key.BACK_SPACE);
      await input.sendKeys(installFolder);
      expect(await input.getAttribute("value")).to.equal(installFolder);
    });

    it("10- Should show installation summary", async function () {
      this.timeout(10000);
      await eimRunner.clickButton("Continue");
      const versionSummary = await eimRunner.findByDataId("versions-info");
      expect(await versionSummary.getText()).to.include(
        "Installing ESP-IDF Versions"
      );
      const selectedVersions = await eimRunner.findByDataId("version-chips");
      const selectedVersionsText = await selectedVersions.getText();
      for (let idfVersion of idfVersionList) {
        expect(selectedVersionsText).to.include(idfVersion);
      }
    });

    it("11- Should install IDF using expert setup", async function () {
      this.timeout(2730000);

      try {
        await eimRunner.clickButton("Start Installation");
        const installing = await eimRunner.findByText("Installation Progress");
        expect(await installing.isDisplayed()).to.be.true;
        const startTime = Date.now();

        while (Date.now() - startTime < 2700000) {
          if (await eimRunner.findByText("Installation Failed", 1000)) {
            logger.debug("failed!!!!");
            break;
          }
          if (await eimRunner.findByText("Complete Installation", 1000)) {
            logger.debug("Completed!!!");
            break;
          }
          await new Promise((resolve) => setTimeout(resolve, 1000));
        }
        if (Date.now() - startTime >= 2700000) {
          logger.info("Installation timed out after 45 minutes");
        }
        const completed = await eimRunner.findByText("Complete Installation");
        expect(completed).to.not.be.false;
        expect(await completed.isDisplayed()).to.be.true;
      } catch (error) {
        logger.info("Failed to complete installation", error);
        throw error;
      }
    });

    it("12- Should offer to save installation configuration", async function () {
      this.timeout(15000);

      try {
        await eimRunner.clickButton("Complete Installation");
        const completed = await eimRunner.findByText("Installation Complete!");
        expect(await completed.isDisplayed()).to.be.true;
        const saveConfig = await eimRunner.findByText("Save Configuration");
        expect(saveConfig).to.not.be.false;
        expect(await saveConfig.isDisplayed()).to.be.true;
        const exit = await eimRunner.findByText("Exit Installer");
        expect(exit).to.not.be.false;
      } catch (error) {
        logger.info("Failed to complete installation", error);
        throw error;
      }
    });
  });
}
