import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import logger from "../classes/logger.class.js";
import { tGui, matchable } from "../helpers/i18n.js";
import { By } from "selenium-webdriver";
import fs from "fs";
import path from "path";
import { Key, until } from "selenium-webdriver";


// This function verifies the version management functionality of the EIM GUI
export function runGUIVersionManagementTest({
  id = 0,
  pathToEIM,
  idfList,
  toolsFolder,
}) {
  describe(`${id}- EIM GUI Version Management |`, () => {
    let eimRunner = null;
    let totalInstallations = 0;
    let testStepFailed = false;

    // The setup function should start the EIM application GUI
    before(async function () {
      this.timeout(60000);
      eimRunner = new GUITestRunner(pathToEIM);
      try {
        logger.info("Starting EIM application");
        await eimRunner.start();
      } catch (err) {
        logger.info("Error starting EIM application");
        throw err;
      }
    });

    // The beforeEach function should skip the next tests if the previous test failed
    beforeEach(async function () {
      this.timeout(10000);
      if (testStepFailed) {
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
      if (this.currentTest.state === "failed") testStepFailed = true;
    });

    // The tear down function should stop the EIM application GUI
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
      // Wait for the header to be presented
      await new Promise((resolve) => setTimeout(resolve, 10000));
      const header = await eimRunner.findByDataId("welcome-header", 25000);
      expect(header, "Expected welcome header").to.not.be.false;
      const text = await header.getText();
      expect(text, "Expected welcome text").to.equal(
        `${tGui("welcome.welcome")} ESP-IDF ${tGui("welcome.title")}`
      );
    });

    it("2- Should show option to manage installations", async function () {
      this.timeout(10000);
      const dashboardCard = await eimRunner.findByText(
        tGui("welcome.cards.manage.title")
      );
      expect(
        dashboardCard,
        "Expected dashboard card to be shown on welcome page"
      ).to.not.be.false;
      const dashboardContent = await eimRunner.findByText(
        matchable("welcome.cards.manage.description")
      );
      const text = await dashboardContent.getText();
      const numberMatch = text.match(/\d+/);
      totalInstallations = numberMatch ? parseInt(numberMatch[0], 10) : 0;
      expect(
        totalInstallations,
        "Expected at least one installation"
      ).to.be.gte(1);
      const click = await eimRunner.clickButton(
        tGui("welcome.cards.manage.button")
      );
      await new Promise((resolve) => setTimeout(resolve, 1000));
      expect(click, "Expected to click on Open Dashboard button").to.be.true;
    });

    it("3- Should show dashboard with installations", async function () {
      this.timeout(10000);
      const cards = await eimRunner.findMultipleByClass("n-card");
      expect(cards.length, "Expected matching number of cards").to.be.equal(
        totalInstallations
      );
      let installedVersionsList = [];
      for (let card of cards) {
        const versionElement = await card.findElement(
          By.className("version-info")
        );
        const versionText = await versionElement.getText();
        installedVersionsList.push(versionText);
        const openTerminalButton = await card
          .findElement(By.css(`[data-id^="open-idf-terminal-button"]`))
          .catch(() => false);
        expect(openTerminalButton, "Expected to find open terminal button").to
          .not.be.false;
        const renameButton = await card
          .findElement(By.css(`[data-id^="rename-version-button"]`))
          .catch(() => false);
        expect(renameButton, "Expected to find rename button").to.not.be.false;
        const fixInstallButton = await card
          .findElement(By.css(`[data-id^="fix-version-button"]`))
          .catch(() => false);
        expect(fixInstallButton, "Expected to find fix installation button").to
          .not.be.false;
        const openInExplorerButton = await card
          .findElement(By.css(`[data-id^="open-in-explorer-button"]`))
          .catch(() => false);
        expect(
          openInExplorerButton,
          "Expected to find button to open IDF folder on explorer"
        ).to.not.be.false;
        const removeButton = await card
          .findElement(By.css(`[data-id^="remove-version-button"]`))
          .catch(() => false);
        expect(removeButton, "Expected to find delete button").to.not.be.false;
      }
      logger.debug(`Installed versions: ${installedVersionsList}`);
      for (let idfVersion of idfList) {
        expect(
          installedVersionsList.includes(idfVersion),
          `Expected dashboard card to be shown for version ${idfVersion} `
        ).to.be.true;
      }
    });

    it("4- Should show tool list for an installation, with add-more-tools option", async function () {
      this.timeout(20000);
      const cards = await eimRunner.findMultipleByClass("n-card");
      const listToolsButton = await cards[0]
        .findElement(By.css(`[data-id^="list-tools-button"]`))
        .catch(() => false);
      expect(listToolsButton, "Expected to find list tools button").to.not.be
        .false;
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        listToolsButton
      );

      const content = await eimRunner.findByDataId(
        "list-tools-content",
        20000
      );
      expect(content, "Expected list-tools modal content to load").to.not.be
        .false;

      // Tool/version rows are filtered by platform-download availability
      // (`has_platform_download`), which varies by CI runner OS/arch (e.g.
      // a "recommended" version may have no download for this platform
      // while an older "supported" one does). So we don't assert on
      // specific status text or on the "optional" marker being present -
      // only that real row data actually rendered for this installation.
      const toolRows = await eimRunner.driver.findElements(
        By.css(`[data-id^="list-tools-tool-"]`)
      );
      expect(
        toolRows.length,
        "Expected at least one tool/version row to be rendered"
      ).to.be.greaterThan(0);

      const installedMarker = await eimRunner
        .findByClass("installed-yes", 10000)
        .catch(() => false);
      expect(
        installedMarker,
        "Expected at least one tool version to be marked installed"
      ).to.not.be.false;

      const addToolsButton = await eimRunner.findByDataId(
        "show-add-tools-button",
        10000
      );
      expect(addToolsButton, "Expected to find 'Add more tools' button").to
        .not.be.false;
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        addToolsButton
      );

      const addToolsPanel = await eimRunner.findByDataId(
        "add-tools-panel",
        10000
      );
      expect(addToolsPanel, "Expected 'add more tools' panel to open").to.not
        .be.false;

      // Whether there are on_request tools left to add depends on the
      // default (non-interactive) tool selection for this target/version -
      // either outcome (a checkbox list, or the "all installed" empty
      // state) is valid; the panel just needs to render one of them.
      const [checkboxGroup, emptyState] = await Promise.all([
        eimRunner
          .findByDataId("add-tools-checkbox-group", 3000)
          .catch(() => false),
        eimRunner.findByDataId("add-tools-none", 3000).catch(() => false),
      ]);
      expect(
        Boolean(checkboxGroup) || Boolean(emptyState),
        "Expected either an 'add tools' checkbox list or the empty-state message"
      ).to.be.true;

      // Close the panel without triggering an actual reinstall
      const cancelButton = await eimRunner.findByDataId(
        "add-tools-cancel-button",
        5000
      );
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        cancelButton
      );
      await new Promise((resolve) => setTimeout(resolve, 500));
      const panelAfterCancel = await eimRunner.driver
        .findElement(By.css(`[data-id="add-tools-panel"]`))
        .catch(() => false);
      expect(
        panelAfterCancel,
        "Expected 'add more tools' panel to close after cancel"
      ).to.be.false;

      // Close the list-tools modal itself before the next test interacts
      // with the dashboard cards again.
      const closeButton = await eimRunner.findByClass(
        "n-card-header__close",
        5000
      );
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        closeButton
      );
      await new Promise((resolve) => setTimeout(resolve, 500));
    });

    it("5- Should show feature list for an installation, with add-more-features option", async function () {
      this.timeout(20000);
      const cards = await eimRunner.findMultipleByClass("n-card");
      const listFeaturesButton = await cards[0]
        .findElement(By.css(`[data-id^="list-features-button"]`))
        .catch(() => false);
      expect(listFeaturesButton, "Expected to find list features button").to
        .not.be.false;
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        listFeaturesButton
      );

      const content = await eimRunner.findByDataId(
        "list-features-content",
        20000
      );
      expect(content, "Expected list-features modal content to load").to.not
        .be.false;

      const coreRow = await eimRunner.findByDataId(
        "list-features-feature-core",
        10000
      );
      expect(coreRow, "Expected the required 'core' feature to be listed").to
        .not.be.false;
      const coreRowText = await coreRow.getText();
      expect(
        coreRowText.includes(
          tGui("versionManagement.modals.listFeatures.requiredStatus")
        ),
        "Expected 'core' feature to be marked as required"
      ).to.be.true;
      expect(
        coreRowText.includes(
          tGui("versionManagement.modals.listFeatures.installedMarker")
        ),
        "Expected 'core' feature to be marked as installed"
      ).to.be.true;

      const modalText = await content.getText();
      expect(
        modalText.includes(
          tGui("versionManagement.modals.listFeatures.optional")
        ),
        "Expected at least one optional feature to be listed"
      ).to.be.true;

      const addFeaturesButton = await eimRunner.findByDataId(
        "show-add-features-button",
        10000
      );
      expect(addFeaturesButton, "Expected to find 'Add more features' button")
        .to.not.be.false;
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        addFeaturesButton
      );

      const addFeaturesPanel = await eimRunner.findByDataId(
        "add-features-panel",
        10000
      );
      expect(addFeaturesPanel, "Expected 'add more features' panel to open")
        .to.not.be.false;

      // A freshly, non-interactively installed IDF only has the required
      // "core" feature installed, so every optional feature should be
      // offered as a candidate to add.
      const checkboxGroup = await eimRunner
        .findByDataId("add-features-checkbox-group", 5000)
        .catch(() => false);
      expect(
        checkboxGroup,
        "Expected optional features available to add on a freshly installed (required-only) IDF"
      ).to.not.be.false;

      // Close the panel without triggering an actual reinstall
      const cancelButton = await eimRunner.findByDataId(
        "add-features-cancel-button",
        5000
      );
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        cancelButton
      );
      await new Promise((resolve) => setTimeout(resolve, 500));
      const panelAfterCancel = await eimRunner.driver
        .findElement(By.css(`[data-id="add-features-panel"]`))
        .catch(() => false);
      expect(
        panelAfterCancel,
        "Expected 'add more features' panel to close after cancel"
      ).to.be.false;

      // Close the list-features modal itself before the next test interacts
      // with the dashboard cards again.
      const closeButton = await eimRunner.findByClass(
        "n-card-header__close",
        5000
      );
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        closeButton
      );
      await new Promise((resolve) => setTimeout(resolve, 500));
    });

    it("6- Should allow renaming existing installation", async function () {
      this.timeout(10000);
      const cards = await eimRunner.findMultipleByClass("n-card");
      const renameButton = await cards[0]
        .findElement(By.css(`[data-id^="rename-version-button"]`))
        .catch(() => false);
      expect(renameButton, "Expected to find rename button").to.not.be.false;
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        renameButton
      );
      await new Promise((resolve) => setTimeout(resolve, 1000));
      const input = await eimRunner.driver.wait(
        until.elementLocated(By.css(`input`))
      );
      await input.sendKeys(Key.CONTROL + "a");
      await input.sendKeys(Key.CONTROL + "a");
      await input.sendKeys(Key.BACK_SPACE);
      await input.sendKeys("NewName");
      await eimRunner.clickElement(
        tGui("versionManagement.modals.rename.confirmButton")
      );
      await new Promise((resolve) => setTimeout(resolve, 1000));
      let renameVersionsList = [];
      for (let card of cards) {
        const versionElement = await card.findElement(
          By.className("version-info")
        );
        const versionText = await versionElement.getText();
        renameVersionsList.push(versionText);
      }
      logger.debug(`Installed versions after rename: ${renameVersionsList}`);
      expect(
        renameVersionsList.includes("NewName"),
        `Expected dashboard card to shown renamed IDF instalaltion `
      ).to.be.true;
      const eimJsonFilePath = path.join(toolsFolder, "tools", "eim_idf.json");
      const eimJsonContent = JSON.parse(
        fs.readFileSync(eimJsonFilePath, "utf-8")
      );
      let installedIDFName = [];
      for (let idf of eimJsonContent.idfInstalled) {
        installedIDFName.push(idf.name);
      }
      expect(
        installedIDFName.includes("NewName"),
        "Expected json file to contain renamed IDF installation"
      ).to.be.true;
    });

    it("7- Should allow deleting existing installation", async function () {
      this.timeout(60000);
      const cards = await eimRunner.findMultipleByClass("n-card");
      const IDFToDelete = await cards[0].findElement(
        By.className("version-info")
      );
      const IDFToDeleteText = await IDFToDelete.getText();
      logger.debug(`IDF version to delete: ${IDFToDeleteText}`);
      const removeButton = await cards[0].findElement(
        By.css(`[data-id^="remove-version-button"]`)
      );
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        removeButton
      );
      await new Promise((resolve) => setTimeout(resolve, 1000));
      const confirmation = await eimRunner.findByText(
        matchable("versionManagement.modals.remove.message")
      );
      const confirmationText = await confirmation.getText();
      expect(
        confirmationText.includes(IDFToDeleteText),
        `Expected confirmation dialog to mention IDF version ${IDFToDeleteText}`
      ).to.be.true;
      await eimRunner.clickElement(
        tGui("versionManagement.modals.remove.confirmButton")
      );
      await new Promise((resolve) => setTimeout(resolve, 20000));

      const updatedCards = await eimRunner.findMultipleByClass("n-card");
      let deleteVersionsList = [];
      for (let card of updatedCards) {
        const versionElement = await card.findElement(
          By.className("version-info")
        );
        const versionText = await versionElement.getText();
        deleteVersionsList.push(versionText);
      }
      logger.debug(`Installed versions after remove: ${deleteVersionsList}`);
      expect(
        deleteVersionsList.includes(IDFToDeleteText),
        `Expected dashboard card not to show removed IDF `
      ).to.not.be.true;
      const eimJsonFilePath = path.join(toolsFolder, "tools", "eim_idf.json");
      const eimJsonContent = JSON.parse(
        fs.readFileSync(eimJsonFilePath, "utf-8")
      );
      let installedIDFName = [];
      for (let idf of eimJsonContent.idfInstalled) {
        installedIDFName.push(idf.name);
      }
      expect(
        installedIDFName.includes(IDFToDeleteText),
        "Expected json file to not contain removed IDF installation"
      ).to.not.be.true;
    });

    it("8- Should allow purging all installation", async function () {
      this.timeout(60000);
      const cards = await eimRunner.findMultipleByClass("n-card");
      expect(
        cards.length,
        "Expected at least one installation to purge"
      ).to.be.gte(1);

      let purgeVersionsList = [];
      for (let card of cards) {
        const versionElement = await card
          .findElement(By.className("version-info"))
          .catch(() => false);
        const versionText = await versionElement.getText();
        purgeVersionsList.push(versionText);
      }
      logger.debug(`Installed versions before purge all: ${purgeVersionsList}`);
      const quickActions = await eimRunner.findByClass("quick-actions");
      expect(quickActions, "Expected to find quick actions section").to.not.be
        .false;
      const purgeAllText = tGui("versionManagement.quickActions.purgeAll");
      const purgeButton = await quickActions
        .findElement(By.xpath(`//*[contains(text(), '${purgeAllText}')]`))
        .catch(() => false);
      await eimRunner.driver.executeScript(
        "arguments[0].click();",
        purgeButton
      );
      await new Promise((resolve) => setTimeout(resolve, 500));

      const confirmation = await eimRunner.findByText(
        tGui("versionManagement.modals.purge.warning")
      );
      expect(confirmation, "Expected to find confirmation dialog for purge all")
        .to.not.be.false;

      const confirmationIDFList = await eimRunner.findByText(
        matchable("versionManagement.modals.purge.listMessage")
      );
      const confirmationIDFListText = await confirmationIDFList.getText();
      for (let idfVersion of purgeVersionsList) {
        expect(
          confirmationIDFListText.includes(idfVersion),
          `Expected confirmation dialog to list IDF version ${idfVersion} `
        ).to.be.true;
      }

      await eimRunner.clickElement(
        tGui("versionManagement.modals.purge.confirmation")
      );
      await new Promise((resolve) => setTimeout(resolve, 500));
      const buttons = await eimRunner.driver.wait(
        until.elementsLocated(
          By.xpath(
            `//*[contains(text(), '${purgeAllText}')]/ancestor-or-self::button`
          )
        )
      );
      await eimRunner.driver.executeScript("arguments[0].click();", buttons[1]);
      await new Promise((resolve) => setTimeout(resolve, 500));

      const noInstalls = await eimRunner.findByText(
        tGui("versionManagement.sections.noVersions"),
        45000
      );
      expect(noInstalls, "Expected to find no installations message").to.not.be
        .false;

      const updatedCards = await eimRunner.findMultipleByClass("n-card");
      expect(updatedCards, "Expected all installations to be deleted").to.be
        .false;
    });
  });
}
