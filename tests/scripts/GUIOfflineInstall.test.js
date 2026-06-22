import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import TestProxy from "../classes/TestProxy.class.js";
import { downloadOfflineArchive } from "../helper.js";
import { tGui } from "../helpers/i18n.js";
import logger from "../classes/logger.class.js";
import { By } from "selenium-webdriver";
import path from "path";
import os from "os";
import fs from "fs";

// This function executes the offline installation functionality of the EIM GUI
// Offline version to be installed are provided by arguments
export function runGUIOfflineInstallTest({
  id = 0,
  pathToEIM,
  offlineIDFVersion = null,
  testProxyMode = false,
  proxyBlockList = [],
}) {
  describe(`${id}- Run offline installation |`, () => {
    let eimRunner = null;
    let offlineInstallFailed = false;
    let pathToOfflineArchive = null;
    let proxy = null;

    // The setup function should start the proxy server if enabled, download the offline archive and start the EIM application GUI
    before(async function () {
      this.timeout(900000);
      eimRunner = new GUITestRunner(pathToEIM);
      pathToOfflineArchive = await downloadOfflineArchive({
        idfVersion: offlineIDFVersion,
      });
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
      if (!pathToOfflineArchive) {
        logger.info(">>>>>>> Offline archive not found, skipping this test");
        this.skip();
      }
    });

    // The beforeEach function should skip the next tests if the previous test failed
    beforeEach(async function () {
      if (offlineInstallFailed) {
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
      if (this.currentTest.state === "failed") offlineInstallFailed = true;
    });

    // The tear down function should stop the EIM application GUI and proxy server if enabled
    // The offline archive should be removed to save space in the runner
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
      if (pathToOfflineArchive) {
        try {
          fs.rmSync(pathToOfflineArchive, { force: true });
          logger.info(`Successfully deleted offline archive`);
        } catch (err) {
          logger.info(`Error deleting offline archive ${err}`);
        }
      }
    });

    it("1- Should show welcome page", async function () {
      this.timeout(45000);
      // Wait for the header to be present
      await new Promise((resolve) => setTimeout(resolve, 5000));
      const header = await eimRunner.findByDataId("welcome-header", 25000);
      expect(header, "Expected welcome header").to.not.be.false;
      const text = await header.getText();
      expect(text, "Expected welcome text").to.equal(
        `${tGui("welcome.welcome")} ESP-IDF ${tGui("welcome.title")}`,
      );
    });

    it("2- Should show offline installation option", async function () {
      this.timeout(10000);

      await eimRunner.clickButton(tGui("welcome.cards.new.button"));
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const header = await eimRunner.findByCSS("h1");
      const text = await header.getText();
      expect(text, "Expected installation setup screen").to.equal(
        tGui("basicInstaller.title"),
      );
      const simplified = await eimRunner.findByText(
        tGui("basicInstaller.cards.offline.title"),
      );
      expect(simplified, "Expected option for offline installation").to.not.be
        .false;
      expect(
        await simplified.isDisplayed(),
        "Expected option for offline installation",
      ).to.be.true;
    });

    it("3- Should show installation summary", async function () {
      this.timeout(25000);

      await eimRunner.driver.executeScript(
        "document.querySelector('#eim_offline_installation_input').value = arguments[0]",
        `${pathToOfflineArchive}`,
      );
      await eimRunner.clickButton(tGui("basicInstaller.cards.offline.button"));

      await new Promise((resolve) => setTimeout(resolve, 2000));
      const header = await eimRunner.findByCSS("h1");
      const text = await header.getText();
      expect(text, "Expected offline installation summary screen").to.equal(
        tGui("offlineInstaller.title"),
      );

      const selectedFile = await eimRunner.findByRelation(
        "parent",
        "div",
        tGui("offlineInstaller.config.archive.title"),
      );
      const selectedFileText = await selectedFile.getText();
      expect(selectedFileText, "Expected file path to be shown").to.include(
        `offlineArchive_${offlineIDFVersion}.zst`,
      );

      const pathInput = await eimRunner.findByCSS("input");
      const defaultInput =
        os.platform() === "win32"
          ? "C:\\esp"
          : path.join(os.homedir(), ".espressif");
      expect(await pathInput.getAttribute("value")).to.include(defaultInput);

      const useDefault = await eimRunner.findByText(
        tGui("offlineInstaller.config.path.useDefault"),
      );
      const isDisplayed = await useDefault.isDisplayed();
      expect(isDisplayed, "Expected option to use default installation path").to
        .be.true;
      const checkBox = await eimRunner.findByRelation(
        "parent",
        "div",
        tGui("offlineInstaller.config.path.useDefault"),
      );
      const checked = await checkBox.getAttribute("class");
      expect(checked, "Expected checkbox to be unchecked").to.include(
        "checked",
      );

      const startButton = await eimRunner.findByText(
        tGui("offlineInstaller.config.startButton"),
      );
      expect(startButton, "Expected button to start installation").to.not.be
        .false;
      expect(
        await startButton.isDisplayed(),
        "Expected start button to be displayed",
      ).to.be.true;
    });

    it("4- Should show advanced path options", async function () {
      this.timeout(20000);

      // The advanced-options section is the same surface as the
      // expert wizard's path step (EIM-863). It must be present in the
      // offline flow too so the user can opt into cleanup / custom
      // tool folders without leaving the page.
      const advancedSection = await eimRunner.findByDataId(
        "advanced-options-section",
      );
      expect(advancedSection, "Expected advanced options section").to.not.be
        .false;
      expect(
        await advancedSection.isDisplayed(),
        "Expected advanced options section to be visible",
      ).to.be.true;

      // naive-ui renders n-checkbox as a <div class="n-checkbox"> with
      // the data-id forwarded. When checked, the class becomes
      // "n-checkbox n-checkbox--checked". The simpler `checked`
      // substring matches every checkbox (the n-checkbox class always
      // contains it), so we assert on the --checked modifier instead.
      const cleanupCheckbox = await eimRunner.findByDataId("cleanup-checkbox");
      expect(cleanupCheckbox, "Expected cleanup checkbox").to.not.be.false;
      expect(
        await cleanupCheckbox.isDisplayed(),
        "Expected cleanup checkbox to be visible",
      ).to.be.true;
      expect(
        await cleanupCheckbox.getAttribute("class"),
        "Expected cleanup checkbox to start unchecked",
      ).to.not.include("n-checkbox--checked");

      const customFoldersCheckbox = await eimRunner.findByDataId(
        "custom-folders-checkbox",
      );
      expect(customFoldersCheckbox, "Expected custom folders checkbox").to.not
        .be.false;
      expect(
        await customFoldersCheckbox.getAttribute("class"),
        "Expected custom folders checkbox to start unchecked",
      ).to.not.include("n-checkbox--checked");

      // Folder-name inputs are rendered as n-input; the inner <input>
      // is what gets the `disabled` attribute. They should be disabled
      // until the user opts in.
      let downloadInput = await eimRunner.findByDataId(
        "tool-download-folder-input",
      );
      let downloadInputEl = await downloadInput.findElement(By.css("input"));
      expect(
        await downloadInputEl.getAttribute("disabled"),
        "Expected download folder input to start disabled",
      ).to.not.be.null;
      let installInput = await eimRunner.findByDataId(
        "tool-install-folder-input",
      );
      let installInputEl = await installInput.findElement(By.css("input"));
      expect(
        await installInputEl.getAttribute("disabled"),
        "Expected install folder input to start disabled",
      ).to.not.be.null;

      // Toggle the cleanup checkbox on; the warning paragraph is
      // rendered conditionally and must show up.
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        cleanupCheckbox,
      );
      await new Promise((resolve) => setTimeout(resolve, 500));
      expect(
        await cleanupCheckbox.getAttribute("class"),
        "Expected cleanup checkbox to be checked after click",
      ).to.include("n-checkbox--checked");
      const cleanupWarning = await eimRunner.findByDataId("cleanup-warning");
      expect(
        await cleanupWarning.isDisplayed(),
        "Expected cleanup warning to appear when cleanup is enabled",
      ).to.be.true;

      // Opt in to custom tool folders and verify the inputs enable,
      // are pre-populated with the defaults, and that the warning
      // paragraph shows up.
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        customFoldersCheckbox,
      );
      await new Promise((resolve) => setTimeout(resolve, 500));
      expect(
        await customFoldersCheckbox.getAttribute("class"),
        "Expected custom folders checkbox to be checked after click",
      ).to.include("n-checkbox--checked");

      const customFoldersWarning = await eimRunner.findByDataId(
        "custom-folders-warning",
      );
      expect(
        await customFoldersWarning.isDisplayed(),
        "Expected custom folders warning to appear when enabled",
      ).to.be.true;

      downloadInput = await eimRunner.findByDataId("tool-download-folder-input");
      downloadInputEl = await downloadInput.findElement(By.css("input"));
      expect(
        await downloadInputEl.getAttribute("disabled"),
        "Expected download folder input to be enabled after opting in",
      ).to.be.null;
      expect(
        await downloadInputEl.getAttribute("value"),
        "Expected download folder input to default to 'dist'",
      ).to.equal(
        tGui("installationPathSelect.toolFolders.downloadPlaceholder"),
      );

      installInput = await eimRunner.findByDataId("tool-install-folder-input");
      installInputEl = await installInput.findElement(By.css("input"));
      expect(
        await installInputEl.getAttribute("disabled"),
        "Expected install folder input to be enabled after opting in",
      ).to.be.null;
      expect(
        await installInputEl.getAttribute("value"),
        "Expected install folder input to default to 'tools'",
      ).to.equal(
        tGui("installationPathSelect.toolFolders.installPlaceholder"),
      );

      // Typing a custom value should stick; the composable does not
      // overwrite the input value while the option is on.
      await downloadInputEl.clear();
      await downloadInputEl.sendKeys("custom-dist");
      expect(await downloadInputEl.getAttribute("value")).to.equal(
        "custom-dist",
      );

      // Toggling the option back off must reset the folder names to
      // the defaults — the composable is responsible for that, and
      // leaving stale values would silently persist them on install.
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        customFoldersCheckbox,
      );
      await new Promise((resolve) => setTimeout(resolve, 500));
      expect(
        await customFoldersCheckbox.getAttribute("class"),
        "Expected custom folders checkbox to be unchecked after second click",
      ).to.not.include("n-checkbox--checked");

      downloadInput = await eimRunner.findByDataId("tool-download-folder-input");
      downloadInputEl = await downloadInput.findElement(By.css("input"));
      expect(
        await downloadInputEl.getAttribute("value"),
        "Expected download folder input to reset to 'dist' after opting out",
      ).to.equal(
        tGui("installationPathSelect.toolFolders.downloadPlaceholder"),
      );
      installInput = await eimRunner.findByDataId("tool-install-folder-input");
      installInputEl = await installInput.findElement(By.css("input"));
      expect(
        await installInputEl.getAttribute("value"),
        "Expected install folder input to reset to 'tools' after opting out",
      ).to.equal(
        tGui("installationPathSelect.toolFolders.installPlaceholder"),
      );

      // Turn the cleanup flag back off so the actual install that
      // follows runs with the default behaviour.
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        cleanupCheckbox,
      );
      await new Promise((resolve) => setTimeout(resolve, 500));
    });

    it("5- Should install IDF using offline file", async function () {
      this.timeout(2730000);
      await eimRunner.clickButton(tGui("offlineInstaller.config.startButton"));
      await new Promise((resolve) => setTimeout(resolve, 5000));
      const installing = await eimRunner.findByText(
        tGui("offlineInstaller.installation.title"),
        20000,
      );

      expect(
        installing,
        "Expected installation to start Installing ESP-IDF from Archive",
      ).to.not.be.false;
      expect(
        await installing.isDisplayed(),
        "Expected installation progress screen",
      ).to.be.true;

      const startTime = Date.now();
      // NOTE(EIM-661): the offline installer also surfaces a generic
      // "Installation Failed" banner; no dedicated i18n key exists for
      // the offline flow today, so `simpleSetup.error.title` is reused
      // as the source of truth for the literal. Flag for follow-up if
      // a dedicated key is added.
      while (Date.now() - startTime < 2700000) {
        if (await eimRunner.findByText(tGui("simpleSetup.error.title"), 1000)) {
          logger.debug("failed!!!!");
          break;
        }
        if (
          await eimRunner.findByText(tGui("installationProgress.alert.error"), 1000)
        ) {
          logger.debug("failed!!!!");
          break;
        }
        if (
          await eimRunner.findByText(
            tGui("offlineInstaller.installation.success.title"),
            1000,
          )
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
        tGui("offlineInstaller.installation.success.title"),
      );
      expect(completed, "Expected installation to be completed").to.not.be
        .false;
      expect(
        await completed.isDisplayed(),
        "Expected 'Installation Complete' text displayed",
      ).to.be.true;
    });

    it("6- Should return to dashboard once completed.", async function () {
      this.timeout(10000);
      await eimRunner.clickButton(
        tGui("offlineInstaller.installation.success.complete"),
      );
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const cards = await eimRunner.findMultipleByClass("n-card");
      let versionsList = [];
      for (let card of cards) {
        const versionElement = await card.findElement(
          By.className("version-info"),
        );
        const versionText = await versionElement.getText();
        versionsList.push(versionText);
      }
      logger.debug(`Installed versions: ${versionsList}`);
      expect(
        versionsList.includes(offlineIDFVersion),
        `Expected dashboard card to be shown for version ${offlineIDFVersion} `,
      ).to.be.true;
    });
  });
}
