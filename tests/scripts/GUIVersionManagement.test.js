import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import logger from "../classes/logger.class.js";
import { By } from "selenium-webdriver";
import fs from "fs";  
import path from "path";
import { Key, until } from "selenium-webdriver";

export function runGUIVersionManagementTest({ id = 0, pathToEIM, idfList, installFolder, toolsFolder }) {
  
  describe(`${id}- EIM GUI Version Management |`, () => {
    let eimRunner = null;
    let totalInstallations = 0;
    let testStepFailed = false;
    before(async function () {
      this.timeout(60000);
      eimRunner = new GUITestRunner(pathToEIM);
      try {
        logger.info("Starting EIM application");
        await eimRunner.start();
      } catch (err) {
        logger.info("Error starting EIM application");
      }
    });

    beforeEach(async function () {
      this.timeout(10000);
      if (testStepFailed) {
        logger.info("Test failed, skipping next tests");
        this.skip();
      }
    });

    afterEach(async function () {
      if (this.currentTest.state === "failed") {
        await eimRunner.takeScreenshot(`${id} ${this.currentTest.title}.png`);
        logger.info(`Screenshot saved as ${id} ${this.currentTest.title}.png`);
        testStepFailed = true;
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
      // Wait for the header to be presented
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
        const openTerminalButton = await card.findElement(By.css(`[data-id="openIDFTerminal"]`)).catch(() => false);
        expect(openTerminalButton, "Expected to find open terminal button").to.not.be.false;
        const renameButton = await card.findElement(By.css(`[data-id="renameVersion"]`)).catch(() => false);
        expect(renameButton, "Expected to find rename button").to.not.be.false;
        const fixInstallButton = await card.findElement(By.css(`[data-id="fixVersion"]`)).catch(() => false);
        expect(fixInstallButton, "Expected to find fix installation button").to.not.be.false;
        const openInExplorerButton = await card.findElement(By.css(`[data-id="openInExplorer"]`)).catch(() => false);
        expect(openInExplorerButton, "Expected to find button to open IDF folder on explorer").to.not.be.false;
        const removeButton = await card.findElement(By.css(`[data-id="removeVersion"]`)).catch(() => false);
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

    it("4- Should allow renaming existing installation", async function () {
      this.timeout(10000);
      const cards = await eimRunner.findMultipleByClass("n-card"); 
      const renameButton = await cards[0].findElement(By.css(`[data-id="renameVersion"]`)).catch(() => false);
      expect(renameButton, "Expected to find rename button").to.not.be.false;
      await eimRunner.driver.executeScript("arguments[0].click();", renameButton);
      await new Promise((resolve) => setTimeout(resolve, 1000));
      const input = await eimRunner.driver.wait(until.elementLocated(By.css(`input`)));
      await input.sendKeys(Key.CONTROL + "a");
      await input.sendKeys(Key.CONTROL + "a");
      await input.sendKeys(Key.BACK_SPACE);
      await input.sendKeys("NewName");
      await eimRunner.clickElement("Rename")

      let renameVersionsList = [];
      for (let card of cards) {
        const versionElement = await card.findElement(
          By.className("version-info")
        );
        const versionText = await versionElement.getText();
        renameVersionsList.push(versionText);
      }
      logger.debug(`Installed versions after rename: ${renameVersionsList}`);
      expect(renameVersionsList.includes("NewName"),
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
      expect(installedIDFName.includes("NewName"), "Expected json file to contain renamed IDF installation").to.be.true;
    });


    it("5- Should allow deleting existing installation", async function () {
      this.timeout(60000);
      const cards = await eimRunner.findMultipleByClass("n-card"); 
      const IDFToDelete = await cards[0].findElement(By.className("version-info"));
      const IDFToDeleteText = await IDFToDelete.getText();
      logger.debug(`IDF version to delete: ${IDFToDeleteText}`);
      const removeButton = await cards[0].findElement(By.css(`[data-id="removeVersion"]`));
      await eimRunner.driver.executeScript("arguments[0].click();", removeButton);
      await new Promise((resolve) => setTimeout(resolve, 1000));
      const confirmation = await eimRunner.findByText("Are you sure");
      const confirmationText = await confirmation.getText();
      expect(confirmationText.includes(IDFToDeleteText), `Expected confirmation dialog to mention IDF version ${IDFToDeleteText}` ).to.be.true; 
      await eimRunner.clickElement("Remove");
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
      expect(deleteVersionsList.includes(IDFToDeleteText),
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
      expect(installedIDFName.includes(IDFToDeleteText), "Expected json file to not contain removed IDF installation").to.not.be.true;
    });

    it("6- Should allow purging all installation", async function () {
      this.timeout(60000);
      const cards = await eimRunner.findMultipleByClass("n-card");
      expect(cards.length, "Expected at least one installation to purge").to.be.gte(1); 

      let purgeVersionsList = [];
      for (let card of cards) {
        const versionElement = await card.findElement(
          By.className("version-info")
        ).catch(() => false);
        const versionText = await versionElement.getText();
        purgeVersionsList.push(versionText);
      }
      logger.debug(`Installed versions before purge all: ${purgeVersionsList}`);
      const quickActions = await eimRunner.findByClass("quick-actions");
      expect(quickActions, "Expected to find quick actions section").to.not.be.false;
      const purgeButton = await quickActions.findElement(By.xpath(`//*[contains(text(), 'Purge All')]`)).catch(() => false);
      await eimRunner.driver.executeScript("arguments[0].click();", purgeButton);
      await new Promise((resolve) => setTimeout(resolve, 500));

      const confirmation = await eimRunner.findByText("This will remove ALL ESP-IDF installations!");
      expect(confirmation, "Expected to find confirmation dialog for purge all").to.not.be.false;

      const confirmationIDFList = await eimRunner.findByText("The following installations will be deleted");
      const confirmationIDFListText = await confirmationIDFList.getText();
      for (let idfVersion of purgeVersionsList) {
        expect(
          confirmationIDFListText.includes(idfVersion),
          `Expected confirmation dialog to list IDF version ${idfVersion} `
        ).to.be.true;
      }

      await eimRunner.clickElement("I understand this action cannot be undone");
      await new Promise((resolve) => setTimeout(resolve, 500));
      const buttons = await eimRunner.driver.wait(until.elementsLocated(By.xpath(`//*[contains(text(), 'Purge All')]/ancestor-or-self::button`)));
      await eimRunner.driver.executeScript("arguments[0].click();", buttons[1]);
      await new Promise((resolve) => setTimeout(resolve, 500));


      const noInstalls = await eimRunner.findByText("No ESP-IDF versions installed", 45000);
      expect(noInstalls, "Expected to find no installations message").to.not.be.false;

      const updatedCards = await eimRunner.findMultipleByClass("n-card");
      expect(updatedCards, "Expected all installations to be deleted").to.be.false;
    });
  });
}
