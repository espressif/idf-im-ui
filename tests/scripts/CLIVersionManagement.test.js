import { expect } from "chai";
import { describe, it, after, beforeEach, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import path from "path";
import fs from "fs";

export function runVersionManagementTest({
  id,
  pathToEim,
  idfList,
  installFolder,
}) {
  describe(`${id}- EIM Version Management test |`, function () {
    this.timeout(120000);
    let testRunner = null;
    let testStepFailed = false;

    beforeEach(async function () {
      this.timeout(10000);
      if (testStepFailed) {
        logger.info("Test failed, skipping next tests");
        this.skip();
      }
    });

    afterEach(async function () {
      this.timeout(20000);
      if (this.currentTest.state === "failed") {
        logger.info(`Test failed: ${this.currentTest.title}`);
        if (testRunner) {
          logger.info(
            `Terminal output: >>\r ${testRunner.output.slice(-2000)}`
          );
          logger.debug(`Terminal output on failure: >>\r ${testRunner.output}`);
        }
        testStepFailed = true;
      }
      if (testRunner) {
        try {
          await testRunner.stop();
        } catch (error) {
          logger.info("Error to clean up terminal after test");
          logger.info(` Error: ${error}`);
        } finally {
          testRunner = null;
        }
      }
    });

    after(function () {
      logger.info("Version Management test Completed");
    });

    it("1- EIM list should show available IDF versions", async function () {
      /**
       * This test executes eim list command and checks if the expected IDF versions
       * are present in the output, with the correct path
       *
       */
      logger.info(`Validating EIM List`);
      testRunner = new CLITestRunner();
      await testRunner.start();
      testRunner.sendInput(`${pathToEim} list`);
      const installedList = await testRunner.waitForOutput(
        "Installed versions",
        5000
      );
      expect(installedList, "EIM not showing list of installed IDF").to.be.true;
      for (let idf of idfList) {
        expect(
          testRunner.output,
          `EIM list failed show IDF version ${idf}`
        ).to.include(idf);
      }
    });

    it("2- Select active IDF installation", async function () {
      /**
       * This test executes eim select command and checks if the IDF version is set as
       * active correctly
       */
      logger.info(`Validating EIM Select`);
      testRunner = new CLITestRunner();
      await testRunner.start();
      testRunner.sendInput(`${pathToEim} select`);
      const selectQuery = await testRunner.waitForOutput(
        "Which version do you want to select?",
        5000
      );
      expect(selectQuery, "EIM select not prompting for IDF version").to.be
        .true;

      testRunner.sendInput("");
      const selectedIDF = await testRunner.waitForOutput(
        "Selected version",
        5000
      );
      expect(selectedIDF, "EIM select failed to select idf from prompt").to.be
        .true;
      testRunner.output = "";
      await new Promise((resolve) => setTimeout(resolve, 2000));
      testRunner.sendInput(`${pathToEim} list`);
      const idfListOutput = await testRunner.waitForOutput(
        "Installed versions",
        5000
      );
      let idfToSelect;
      if (testRunner.output.includes(`${idfList[0]} (selected)`)) {
        idfToSelect = idfList[1];
      } else {
        idfToSelect = idfList[0];
      }
      logger.info(`IDF version to run select: ${idfToSelect}`);
      testRunner.sendInput(`${pathToEim} select ${idfToSelect}`);
      const selectOutput = await testRunner.waitForOutput(
        `Selected version: ${idfToSelect}`,
        5000
      );
      expect(selectOutput, "EIM failed to select IDF version").to.be.true;
      testRunner.output = "";
      await new Promise((resolve) => setTimeout(resolve, 2000));
      testRunner.sendInput(`${pathToEim} list`);
      const selectedList = await testRunner.waitForOutput(
        "Installed versions",
        5000
      );
      expect(selectedList, "EIM not showing list of installed IDF").to.be.true;
      expect(
        testRunner.output,
        `EIM list not showing selected IDF version ${idfToSelect}`
      ).to.include(`${idfToSelect} (selected)`);

      testRunner.output = "";
      await new Promise((resolve) => setTimeout(resolve, 2000));
      testRunner.sendInput(`${pathToEim} select random`);
      const errorOutput = await testRunner.waitForOutput(
        `Version random not installed`,
        5000
      );
      expect(
        errorOutput,
        "EIM select failed to notify user of incorrect version name"
      ).to.be.true;
    });

    it("3- EIM rename should rename existing installation", async function () {
      /**
       * This test executes eim rename command and checks if the IDF version is renamed correctly
       *
       */
      logger.info(`Validating EIM Rename`);
      testRunner = new CLITestRunner();
      await testRunner.start();
      testRunner.sendInput(`${pathToEim} rename`);
      const renameQuery = await testRunner.waitForOutput(
        "Which version do you want to rename?",
        5000
      );
      expect(renameQuery, "EIM rename not prompting for IDF version").to.be
        .true;
      testRunner.sendInput("");
      const newNameQuery = await testRunner.waitForOutput(
        "Enter new name",
        5000
      );
      expect(newNameQuery, "EIM rename not prompting for new name input").to.be
        .true;
      testRunner.sendInput("newName");
      const renamedOutput = await testRunner.waitForOutput(
        "Version renamed",
        5000
      );
      expect(renamedOutput, "EIM rename failed to rename IDF installation").to
        .be.true;
      testRunner.output = "";
      await new Promise((resolve) => setTimeout(resolve, 2000));
      testRunner.sendInput(`${pathToEim} list`);
      const installedList = await testRunner.waitForOutput(
        "Installed versions",
        5000
      );
      expect(installedList, "EIM not showing list of installed IDF").to.be.true;
      expect(
        testRunner.output,
        `EIM not showing newly renamed IDF version`
      ).to.include("newName");

      testRunner.sendInput(`${pathToEim} rename newName renamedIDF`);
      const renameOutput = await testRunner.waitForOutput(
        `Version renamed`,
        5000
      );
      expect(renameOutput, "EIM failed to rename IDF version").to.be.true;
      testRunner.output = "";
      await new Promise((resolve) => setTimeout(resolve, 2000));
      testRunner.sendInput(`${pathToEim} list`);
      const updatedList = await testRunner.waitForOutput("renamedIDF", 5000);
      expect(updatedList, "EIM not showing renamed IDF installation").to.be
        .true;
    });

    it("4- EIM remove should delete existing installation", async function () {
      /**
       * This test executes eim remove command and checks if the IDF version is removed correctly
       *
       */
      logger.info(`Validating EIM Remove`);
      testRunner = new CLITestRunner();
      await testRunner.start();

      testRunner.sendInput(`${pathToEim} list`);
      const idfListOutput = await testRunner.waitForOutput(
        "Installed versions",
        5000
      );
      const versionToRemove = testRunner.output.includes(`${idfList[0]} `)
        ? idfList[0]
        : idfList[1];

      testRunner.sendInput(`${pathToEim} remove ${versionToRemove}`);
      const removeOutput = await testRunner.waitForOutput(
        `Removed version: ${versionToRemove}`,
        30000
      );
      expect(removeOutput, "EIM failed to remove IDF version").to.be.true;

      testRunner.output = "";
      await new Promise((resolve) => setTimeout(resolve, 2000));
      testRunner.sendInput(`${pathToEim} list`);
      const installedList = await testRunner.waitForOutput(
        "Installed versions",
        5000
      );
      expect(installedList, "EIM not showing list of installed IDF").to.be.true;
      logger.debug(`Output after listing IDF installed: ${testRunner.output}`);

      const installedVersionsIndex =
        testRunner.output.indexOf("Installed versions");
      const relevantOutput = testRunner.output.substring(
        installedVersionsIndex + "Installed versions".length
      );

      expect(
        relevantOutput,
        "EIM list still showing removed IDF installation"
      ).to.not.include(versionToRemove);
      expect(
        fs.existsSync(path.join(installFolder, versionToRemove)),
        "IDF folder exists after version have been removed"
      ).to.be.false;

      testRunner.output = "";

      await new Promise((resolve) => setTimeout(resolve, 2000));
      testRunner.sendInput(`${pathToEim} remove`);

      const removeQuery = await testRunner.waitForOutput(
        "Which version do you want to remove?",
        5000
      );
      expect(removeQuery, "EIM remove not prompting for IDF version").to.be
        .true;
      testRunner.sendInput("\x03");
      testRunner = null;

      testRunner = new CLITestRunner();
      await testRunner.start();

      testRunner.sendInput(`${pathToEim} remove random`);
      const errorOutput = await testRunner.waitForOutput(
        `Version random not installed`,
        5000
      );
      expect(
        errorOutput,
        "EIM remove failed to notify user of incorrect version name"
      ).to.be.true;
    });

    it("5- EIM purge should remove all IDF installations", async function () {
      /**
       * This test executes eim purge command and checks if all IDF versions and tools are removed correctly
       *
       */
      logger.info(`Validating EIM Purge`);
      testRunner = new CLITestRunner();
      await testRunner.start();
      testRunner.sendInput(`${pathToEim} purge`);
      const purgeOutput = await testRunner.waitForOutput(
        `All versions removed successfully`,
        30000
      );
      expect(purgeOutput, "EIM failed to purge IDF versions").to.be.true;
      for (let idf of idfList) {
        expect(
          fs.existsSync(path.join(installFolder, idf)),
          `IDF installation folder still exists for ${idf} after purge`
        ).to.be.false;
      }
    });
  });
}
