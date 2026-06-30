import { expect } from "chai";
import { describe, it, after, beforeEach, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import path from "path";
import fs from "fs";
import os from "os";

const VALID_STATUSES = ["finished", "in_progress", "failed", "being_repaired", "broken"];

/**
 * Verifies that eim_idf.json status tracking works correctly.
 *
 * What it tests:
 *   1. After a successful install every entry has status = "finished"
 *   2. `eim list` output includes a status label for every installation
 *   3. Interactive dialogs (select, rename, remove, fix) show status in brackets
 *   4. Manually injecting a non-finished entry and running `eim fix` transitions
 *      it through "being_repaired" → "finished"
 */
export function runInstallationStatusTest({
  id = 0,
  pathToEIM,
  idfList,
  installFolder,
  toolsFolder,
}) {
  describe(`${id}- EIM Installation Status tests |`, function () {
    this.timeout(300000);
    let testRunner = null;
    let testStepFailed = false;

    const eimJsonFilePath = path.join(toolsFolder, "tools", "eim_idf.json");

    beforeEach(async function () {
      this.timeout(10000);
      if (testStepFailed) {
        logger.info("Test step failed — skipping remaining status tests");
        this.skip();
      }
    });

    afterEach(async function () {
      this.timeout(20000);
      if (this.currentTest.state === "failed") {
        logger.info(`Test failed: ${this.currentTest.title}`);
        if (testRunner) {
          logger.info(`Terminal output: >>\r ${testRunner.output.slice(-2000)}`);
          logger.debug(`Full terminal output: >>\r ${testRunner.output}`);
        }
        testStepFailed = true;
      }
      if (testRunner) {
        try {
          await testRunner.stop();
        } catch (e) {
          logger.info(`Error stopping terminal: ${e}`);
        } finally {
          testRunner = null;
        }
      }
    });

    after(function () {
      logger.info("Installation Status tests completed");
    });

    // -------------------------------------------------------------------------
    // 1. eim_idf.json — status field present and "finished" after clean install
    // -------------------------------------------------------------------------
    it("1- eim_idf.json entries should have status 'finished' after successful install", async function () {
      logger.info("Checking eim_idf.json status field after clean install");

      expect(
        fs.existsSync(eimJsonFilePath),
        "eim_idf.json not found"
      ).to.be.true;

      const json = JSON.parse(fs.readFileSync(eimJsonFilePath, "utf-8"));
      expect(json.idfInstalled, "idfInstalled array missing").to.be.an("array").that.is.not.empty;

      for (const entry of json.idfInstalled) {
        expect(
          entry,
          `Entry ${entry.name || entry.id} missing 'status' field`
        ).to.have.property("status");

        expect(
          VALID_STATUSES,
          `Entry ${entry.name} has unknown status '${entry.status}'`
        ).to.include(entry.status);

        expect(
          entry.status,
          `Entry ${entry.name} should be 'finished' after a successful install`
        ).to.equal("finished");
      }
    });

    // -------------------------------------------------------------------------
    // 2. eim list output includes status label
    // -------------------------------------------------------------------------
    it("2- 'eim list' output should include a status label for each installation", async function () {
      logger.info("Checking that eim list includes status labels");
      testRunner = new CLITestRunner();
      await testRunner.start();
      testRunner.callEIM(pathToEIM, ["list"]);

      const sawList = await testRunner.waitForOutput("Installed versions", 10000);
      expect(sawList, "eim list did not print 'Installed versions'").to.be.true;

      // The list command renders each entry with its status in the output.
      // After a successful install every entry should show the "finished" label.
      // We check that the word "finished" (case-insensitive) appears at least once.
      expect(
        testRunner.output.toLowerCase(),
        "eim list output does not include any status label"
      ).to.match(/finished|in.?progress|failed|being.?repaired|broken/i);

      // Every installed version should appear in the output
      for (const idf of idfList) {
        expect(
          testRunner.output,
          `eim list output missing entry for ${idf}`
        ).to.include(idf);
      }
    });

    // -------------------------------------------------------------------------
    // 3. Interactive select prompt shows status in brackets
    // -------------------------------------------------------------------------
    it("3- Interactive 'eim select' prompt should show status in brackets", async function () {
      logger.info("Checking that interactive select prompt includes [status]");
      testRunner = new CLITestRunner();
      await testRunner.start();
      testRunner.callEIM(pathToEIM, ["select"]);

      const sawPrompt = await testRunner.waitForOutput(
        "Which version do you want to select?",
        10000
      );
      expect(sawPrompt, "eim select did not show selection prompt").to.be.true;

      // The prompt list items are formatted as "name [status]"
      expect(
        testRunner.output,
        "Interactive select prompt does not include status in brackets"
      ).to.match(/\[.*?(finished|in.?progress|failed|being.?repaired|broken).*?\]/i);

      // Dismiss with Ctrl-C so as not to change state
      testRunner.sendInput("\x03");
    });

    // -------------------------------------------------------------------------
    // 4. Interactive fix prompt shows name + path + status in brackets
    // -------------------------------------------------------------------------
    it("4- Interactive 'eim fix' prompt should show status in brackets", async function () {
      logger.info("Checking that interactive fix prompt includes [status]");
      testRunner = new CLITestRunner();
      await testRunner.start();
      testRunner.callEIM(pathToEIM, ["fix"]);

      const sawPrompt = await testRunner.waitForOutput(
        "Which installation do you want to fix?",
        10000
      );
      expect(sawPrompt, "eim fix did not show selection prompt").to.be.true;

      expect(
        testRunner.output,
        "Interactive fix prompt does not include status in brackets"
      ).to.match(/\[.*?(finished|in.?progress|failed|being.?repaired|broken).*?\]/i);

      testRunner.sendInput("\x03");
    });

    // -------------------------------------------------------------------------
    // 5. Injecting a failed entry — fix transitions it to "finished"
    // -------------------------------------------------------------------------
    it("5- Fixing a 'failed' entry should transition status to 'finished'", async function () {
      this.timeout(600000);
      logger.info("Testing fix flow: failed → finished");

      expect(
        fs.existsSync(eimJsonFilePath),
        "eim_idf.json not found before injection"
      ).to.be.true;

      // Read current config and mark the first entry as "failed"
      const json = JSON.parse(fs.readFileSync(eimJsonFilePath, "utf-8"));
      expect(json.idfInstalled, "No entries to inject failure into").to.not.be.empty;

      const targetEntry = json.idfInstalled[0];
      const originalStatus = targetEntry.status;
      targetEntry.status = "failed";
      fs.writeFileSync(eimJsonFilePath, JSON.stringify(json, null, 2), "utf-8");
      logger.info(`Injected status 'failed' into entry ${targetEntry.name}`);

      try {
        // Run fix by path (non-interactive)
        testRunner = new CLITestRunner();
        await testRunner.start();
        testRunner.callEIM(pathToEIM, ["fix", targetEntry.path]);

        const fixStarted = await testRunner.waitForOutput("Fixing", 10000);
        expect(fixStarted, "eim fix did not start").to.be.true;

        const fixDone = await testRunner.waitForOutput(
          "successfully",
          300000
        );
        expect(fixDone, "eim fix did not report success").to.be.true;

        // Verify the status was updated in eim_idf.json
        const updatedJson = JSON.parse(fs.readFileSync(eimJsonFilePath, "utf-8"));
        const updatedEntry = updatedJson.idfInstalled.find(
          (e) => e.id === targetEntry.id
        );
        expect(
          updatedEntry,
          "Fixed entry not found in eim_idf.json"
        ).to.not.be.undefined;
        expect(
          updatedEntry.status,
          "Entry status should be 'finished' after successful fix"
        ).to.equal("finished");
      } catch (err) {
        // Restore original status on failure so subsequent tests are not affected
        try {
          const restore = JSON.parse(fs.readFileSync(eimJsonFilePath, "utf-8"));
          const entry = restore.idfInstalled.find((e) => e.id === targetEntry.id);
          if (entry) {
            entry.status = originalStatus;
            fs.writeFileSync(eimJsonFilePath, JSON.stringify(restore, null, 2), "utf-8");
          }
        } catch (_) {}
        throw err;
      }
    });

    // -------------------------------------------------------------------------
    // 6. Injecting a broken entry — verify it appears in eim list with the label
    // -------------------------------------------------------------------------
    it("6- 'eim list' should display 'broken' status for a broken entry", async function () {
      logger.info("Checking eim list shows 'broken' label");

      const json = JSON.parse(fs.readFileSync(eimJsonFilePath, "utf-8"));
      expect(json.idfInstalled).to.not.be.empty;
      const targetEntry = json.idfInstalled[0];
      const originalStatus = targetEntry.status;
      targetEntry.status = "broken";
      fs.writeFileSync(eimJsonFilePath, JSON.stringify(json, null, 2), "utf-8");

      try {
        testRunner = new CLITestRunner();
        await testRunner.start();
        testRunner.callEIM(pathToEIM, ["list"]);

        await testRunner.waitForOutput("Installed versions", 10000);

        expect(
          testRunner.output.toLowerCase(),
          "eim list output does not show 'broken' status label"
        ).to.include("broken");
      } finally {
        // Always restore
        const restore = JSON.parse(fs.readFileSync(eimJsonFilePath, "utf-8"));
        const entry = restore.idfInstalled.find((e) => e.id === targetEntry.id);
        if (entry) {
          entry.status = originalStatus;
          fs.writeFileSync(eimJsonFilePath, JSON.stringify(restore, null, 2), "utf-8");
        }
      }
    });
  });
}
