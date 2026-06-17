import { expect } from "chai";
import { describe, it, before, after, afterEach, beforeEach } from "mocha";
import GUITestRunner from "../classes/GUITestRunner.class.js";
import logger from "../classes/logger.class.js";
import { tGui, matchable } from "../helpers/i18n.js";
import { By } from "selenium-webdriver";
import fs from "fs";
import path from "path";

/**
 * Tests for the incomplete-installation modal that appears on app startup
 * whenever eim_idf.json contains entries with status != "finished".
 *
 * The suite:
 *   1. Injects a "failed" status entry into eim_idf.json before launching the GUI.
 *   2. Verifies the modal appears with the correct content.
 *   3. Tests the Delete action — entry is removed and the version management
 *      dashboard refreshes.
 *   4. Injects a "broken" entry, tests the Fix action — navigates to the
 *      installation progress page.
 *   5. Tests Dismiss — modal closes without changing the entry.
 *
 * The suite restores eim_idf.json to its original state in the after() hook
 * so downstream tests are not affected.
 */
export function runGUIIncompleteInstallationsTest({
  id = 0,
  pathToEIM,
  idfList,
  toolsFolder,
}) {
  describe(`${id}- EIM GUI Incomplete Installations modal |`, function () {
    let eimRunner = null;
    let testStepFailed = false;
    let originalJsonContent = null;

    const eimJsonFilePath = path.join(toolsFolder, "tools", "eim_idf.json");

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------
    function readJson() {
      return JSON.parse(fs.readFileSync(eimJsonFilePath, "utf-8"));
    }

    function writeJson(obj) {
      fs.writeFileSync(eimJsonFilePath, JSON.stringify(obj, null, 2), "utf-8");
    }

    function injectStatus(entryIndex, status) {
      const json = readJson();
      json.idfInstalled[entryIndex].status = status;
      writeJson(json);
      return json.idfInstalled[entryIndex];
    }

    function restoreJson() {
      if (originalJsonContent) {
        writeJson(originalJsonContent);
        logger.info("Restored eim_idf.json to original state");
      }
    }

    // ------------------------------------------------------------------
    // Setup / Teardown
    // ------------------------------------------------------------------
    before(async function () {
      this.timeout(30000);

      expect(
        fs.existsSync(eimJsonFilePath),
        "eim_idf.json must exist before running incomplete-installation tests"
      ).to.be.true;

      originalJsonContent = readJson();
      expect(
        originalJsonContent.idfInstalled,
        "Need at least one installed IDF entry"
      ).to.be.an("array").with.lengthOf.at.least(1);
    });

    beforeEach(async function () {
      this.timeout(10000);
      if (testStepFailed) {
        logger.info("Previous step failed — skipping");
        this.skip();
      }
    });

    afterEach(async function () {
      if (this.currentTest.state === "failed" && eimRunner?.driver) {
        await eimRunner.takeScreenshot(`${id} ${this.currentTest.title}.png`);
        logger.info(`Screenshot saved: ${id} ${this.currentTest.title}.png`);
        testStepFailed = true;
      }
      // Stop the GUI between tests so each test starts fresh
      if (eimRunner) {
        try {
          await eimRunner.stop();
        } catch (_) {}
        eimRunner = null;
      }
    });

    after(async function () {
      this.timeout(10000);
      restoreJson();
    });

    // ------------------------------------------------------------------
    // Tests
    // ------------------------------------------------------------------

    it("1- Modal should appear when a 'failed' entry exists in eim_idf.json", async function () {
      this.timeout(60000);

      const injected = injectStatus(0, "failed");
      logger.info(`Injected 'failed' status into entry: ${injected.name}`);

      eimRunner = new GUITestRunner(pathToEIM);
      await eimRunner.start();
      await new Promise((r) => setTimeout(r, 8000)); // wait for startup check

      const modal = await eimRunner.findByDataId(
        "incomplete-installations-modal",
        20000
      );
      expect(modal, "Incomplete installations modal did not appear").to.not.be.false;

      // Modal should mention the injected installation name
      const modalText = await modal.getText();
      expect(
        modalText,
        `Modal should mention the failed installation '${injected.name}'`
      ).to.include(injected.name);

      // There should be a Fix and Delete button for the entry
      const fixButton = await eimRunner
        .findByDataId(`fix-incomplete-${injected.id}`, 5000)
        .catch(() => false);
      expect(fixButton, "Fix button not found in modal").to.not.be.false;

      const deleteButton = await eimRunner
        .findByDataId(`delete-incomplete-${injected.id}`, 5000)
        .catch(() => false);
      expect(deleteButton, "Delete button not found in modal").to.not.be.false;
    });

    it("2- Modal should show correct status tag for different statuses", async function () {
      this.timeout(60000);

      for (const status of ["failed", "broken", "in_progress", "being_repaired"]) {
        injectStatus(0, status);

        eimRunner = new GUITestRunner(pathToEIM);
        await eimRunner.start();
        await new Promise((r) => setTimeout(r, 8000));

        const modal = await eimRunner.findByDataId(
          "incomplete-installations-modal",
          20000
        );
        expect(modal, `Modal should appear for status '${status}'`).to.not.be.false;

        // Status tag text should correspond to the injected status
        const expectedLabel = tGui(`app.incompleteInstallations.status${status
          .split("_")
          .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
          .join("")}`);
        const modalText = await modal.getText();
        expect(
          modalText,
          `Modal should show status label for '${status}'`
        ).to.include(expectedLabel);

        await eimRunner.stop();
        eimRunner = null;
      }
    });

    it("3- Dismiss button should close the modal without modifying eim_idf.json", async function () {
      this.timeout(60000);

      const injected = injectStatus(0, "failed");
      logger.info(`Injected 'failed' status into: ${injected.name}`);

      eimRunner = new GUITestRunner(pathToEIM);
      await eimRunner.start();
      await new Promise((r) => setTimeout(r, 8000));

      const modal = await eimRunner.findByDataId(
        "incomplete-installations-modal",
        20000
      );
      expect(modal, "Modal should appear").to.not.be.false;

      const dismissButton = await eimRunner.findByDataId("dismiss-incomplete-modal", 5000);
      await eimRunner.driver.executeScript("arguments[0].click();", dismissButton);
      await new Promise((r) => setTimeout(r, 2000));

      // Modal should be gone
      const modalAfterDismiss = await eimRunner
        .findByDataId("incomplete-installations-modal", 2000)
        .catch(() => false);
      // A dismissed modal may still be in the DOM but hidden; check visibility via text absence
      if (modalAfterDismiss) {
        const visible = await modalAfterDismiss
          .isDisplayed()
          .catch(() => false);
        expect(visible, "Modal should not be visible after dismiss").to.be.false;
      }

      // eim_idf.json should still have the "failed" status (dismiss does not fix)
      const jsonAfter = readJson();
      const entryAfter = jsonAfter.idfInstalled.find(
        (e) => e.id === injected.id
      );
      expect(
        entryAfter?.status,
        "Dismiss should not change the entry status in eim_idf.json"
      ).to.equal("failed");
    });

    it("4- Delete button should remove entry from eim_idf.json and refresh the dashboard", async function () {
      this.timeout(60000);

      // Ensure at least 2 entries so delete doesn't empty the list entirely
      const json = readJson();
      if (json.idfInstalled.length < 2) {
        logger.info("Only one IDF entry — skipping delete test to avoid emptying config");
        this.skip();
        return;
      }

      const injected = injectStatus(0, "failed");
      logger.info(`Injected 'failed' status into: ${injected.name}`);

      eimRunner = new GUITestRunner(pathToEIM);
      await eimRunner.start();
      await new Promise((r) => setTimeout(r, 8000));

      const modal = await eimRunner.findByDataId(
        "incomplete-installations-modal",
        20000
      );
      expect(modal, "Modal should appear").to.not.be.false;

      const deleteButton = await eimRunner.findByDataId(
        `delete-incomplete-${injected.id}`,
        5000
      );
      await eimRunner.driver.executeScript("arguments[0].click();", deleteButton);
      await new Promise((r) => setTimeout(r, 5000));

      // eim_idf.json should no longer contain the deleted entry
      const jsonAfter = readJson();
      const entryAfter = jsonAfter.idfInstalled.find(
        (e) => e.id === injected.id
      );
      expect(
        entryAfter,
        "Deleted entry should no longer appear in eim_idf.json"
      ).to.be.undefined;

      // The version management dashboard should not show the deleted version
      // Navigate there if not already there
      const dashboardCards = await eimRunner
        .findMultipleByClass("n-card")
        .catch(() => []);
      const cardTexts = await Promise.all(
        (dashboardCards || []).map((c) => c.getText().catch(() => ""))
      );
      expect(
        cardTexts.some((t) => t.includes(injected.name)),
        `Dashboard should not show deleted installation '${injected.name}'`
      ).to.be.false;
    });

    it("5- Fix button should close the modal and navigate to installation progress page", async function () {
      this.timeout(60000);

      const injected = injectStatus(0, "broken");
      logger.info(`Injected 'broken' status into: ${injected.name}`);

      eimRunner = new GUITestRunner(pathToEIM);
      await eimRunner.start();
      await new Promise((r) => setTimeout(r, 8000));

      const modal = await eimRunner.findByDataId(
        "incomplete-installations-modal",
        20000
      );
      expect(modal, "Modal should appear").to.not.be.false;

      const fixButton = await eimRunner.findByDataId(
        `fix-incomplete-${injected.id}`,
        5000
      );
      await eimRunner.driver.executeScript("arguments[0].click();", fixButton);
      await new Promise((r) => setTimeout(r, 3000));

      // Modal should close after clicking Fix
      const modalAfterFix = await eimRunner
        .findByDataId("incomplete-installations-modal", 2000)
        .catch(() => false);
      if (modalAfterFix) {
        const visible = await modalAfterFix.isDisplayed().catch(() => false);
        expect(
          visible,
          "Modal should close after clicking Fix"
        ).to.be.false;
      }

      // Should have navigated to the installation progress page
      const progressPage = await eimRunner
        .findByDataId("installation-progress", 15000)
        .catch(() => false);
      expect(
        progressPage,
        "Should navigate to installation progress page after Fix"
      ).to.not.be.false;
    });

    it("6- Modal should not appear when all entries have status 'finished'", async function () {
      this.timeout(60000);

      // Ensure all entries are "finished"
      const json = readJson();
      for (const entry of json.idfInstalled) {
        entry.status = "finished";
      }
      writeJson(json);

      eimRunner = new GUITestRunner(pathToEIM);
      await eimRunner.start();
      await new Promise((r) => setTimeout(r, 15000)); // give the startup check time to run

      const modal = await eimRunner
        .findByDataId("incomplete-installations-modal", 3000)
        .catch(() => false);

      if (modal) {
        const visible = await modal.isDisplayed().catch(() => false);
        expect(
          visible,
          "Modal should not appear when all installations are finished"
        ).to.be.false;
      }
    });
  });
}
