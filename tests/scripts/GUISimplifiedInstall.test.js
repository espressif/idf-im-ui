import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import { By, Key, until } from "selenium-webdriver";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import TestProxy from "../classes/TestProxy.class.js";
import logger from "../classes/logger.class.js";
import { tGui, matchable } from "../helpers/i18n.js";
import os from "os";

// Poll for any of the given substrings to appear in the DOM. Returns the
// first matching element, or false if none appear within `timeout`.
// Useful for UI states that alternate between several titles (e.g. the
// install progress screen flips between "Downloading package" and
// "Installing ESP-IDF" depending on cache state).
async function findByAnyText(eimRunner, texts, timeout = 30000) {
  const startTime = Date.now();
  while (Date.now() - startTime < timeout) {
    for (const text of texts) {
      const el = await eimRunner.findByText(text, 100);
      if (el) return el;
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  return false;
}

// This function executes the simplified installation functionality of the EIM GUI.
// When `drive` is provided (Windows only), the test exercises the per-install
// drive override — the user ticks the "install on a different drive" checkbox,
// picks `drive` (e.g. "D:\") in the naive-ui select, and the verification step
// is later asked to look for the install at the drive-swapped path.
export function runGUISimplifiedInstallTest({
  id = 0,
  pathToEIM,
  testProxyMode = false,
  proxyBlockList = [],
  drive = null,
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

    it("4- Should select drive for installation", async function () {
      // Drive override is Windows-only and only runs when the suite asks for it.
      if (!drive || os.platform() !== "win32") {
        this.skip();
      }
      this.timeout(20000);

      // Tick the "install on a different drive" acknowledge checkbox. naive-ui
      // renders n-checkbox as a <label> wrapping the box + text; clicking the
      // label text toggles the bound ref.
      const ackText = tGui("simpleSetup.drive.acknowledge");
      const ackLabel = await eimRunner.findByText(ackText);
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        ackLabel,
      );

      // The drive picker (v-if=allowDriveChange) appears once the box is ticked.
      await new Promise((resolve) => setTimeout(resolve, 1500));
      const drivePicker = await eimRunner.findByClass("drive-picker");
      expect(drivePicker, "Expected drive picker to appear").to.not.be.false;

      // Open the naive-ui n-select. naive-ui's n-select ignores a plain
      // `click()` (it listens for `mousedown` on the inner n-base-selection
      // and toggles the popup there), and synthetic mousedown events fired on
      // the outer n-select sometimes don't trigger the inner handler because
      // the listener is on a different element. The most reliable path is a
      // real WebElement `click()` which the browser dispatches as a full
      // mousedown+mouseup+click sequence through the focused element. If that
      // still doesn't open the popup, fall back to focusing + sending Enter
      // (the keyboard activation path).
      const selectTrigger = await drivePicker.findElement(
        By.className("n-select"),
      );
      await selectTrigger.click();
      await new Promise((resolve) => setTimeout(resolve, 1500));

      // Verify the popup actually opened. naive-ui renders the popup in a
      // teleport at the document root, so we search the whole document
      // (not just the drive-picker subtree) for the v-binder-follower
      // wrapper. If the popup is not present, fall back to a keyboard
      // activation (focus + Enter) which naive-ui also supports.
      let popupOpen = true;
      try {
        await eimRunner.driver.wait(
          until.elementLocated(By.css(".v-binder-follower-content")),
          2000,
          "n-select popup did not open after WebElement click",
        );
      } catch (_) {
        popupOpen = false;
      }
      if (!popupOpen) {
        await eimRunner.driver.executeScript(
          "arguments[0].focus();",
          selectTrigger,
        );
        await selectTrigger.sendKeys(Key.ENTER);
        await new Promise((resolve) => setTimeout(resolve, 1500));
      }

      // The drive label is the same string as the value ("D:"). naive-ui option
      // labels end up as plain text in the popup; clickElement finds the first
      // visible match.
      const optionClicked = await eimRunner.clickElement(drive);
      expect(optionClicked, `Expected to select drive ${drive}`).to.be.true;

      // Sanity-check: the drive warning should now mention the picked drive.
      // Use matchable() to grab the portion of the locale string before the
      // {drive} placeholder, since Selenium's getText() returns the rendered
      // text including the interpolated value.
      await new Promise((resolve) => setTimeout(resolve, 1000));
      const warningPrefix = matchable("simpleSetup.drive.warning");
      const warningNode = await eimRunner.findByText(warningPrefix);
      expect(warningNode, "Expected drive warning to be visible").to.not.be
        .false;
      const warningText = await warningNode.getText();
      expect(
        warningText,
        `Expected drive warning to mention ${drive}`,
      ).to.include(drive);
    });

    it("5- Should install IDF using simplified setup", async function () {
      this.timeout(2730000);
      await eimRunner.clickButton(tGui("simpleSetup.ready.startButton"));
      // The h2 alternates between the download phase ("Downloading package",
      // long-lived for offline installs while the archive downloads) and the
      // install phase ("Installing ESP-IDF", reached faster for cached
      // online installs). Accept either — both mean the install has started.
      const installingTitles = [
        tGui("simpleSetup.installation.downloadingTitle"),
        tGui("simpleSetup.installation.title"),
      ];
      const installing = await findByAnyText(
        eimRunner,
        installingTitles,
        60000
      );

      expect(installing, "Expected installation to start").to.not.be.false;
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

    it("6- Should show installation complete summary", async function () {
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