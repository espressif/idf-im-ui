import { expect } from "chai";
import { describe, it, after, beforeEach, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import os from "os";
import path from "path";
import fs from "fs";

function runPostInstallTest(
  pathToIDFScript,
  installFolder,
  toolsfolder,
  validTarget = "esp32",
  invalidTarget = ""
) {
  describe("2- Post installation test ->", function () {
    this.timeout(600000);
    let testRunner = null;
    let pathToProjectFolder = path.join(installFolder, "project");
    let postInstallStepFailed = false;

    beforeEach(async function () {
      if (postInstallStepFailed) {
        logger.info("Test failed, skipping next tests");
        this.skip();
      }
      this.timeout(10000);
      logger.debug(
        `Starting IDF terminal using activation script ${pathToIDFScript}, sample project copied at ${pathToProjectFolder}`
      );
      testRunner = new CLITestRunner();
      try {
        await testRunner.runIDFTerminal(pathToIDFScript);
      } catch (error) {
        logger.info("Error to start IDF terminal");
        logger.info(testRunner.output);
        this.test.error(new Error("Error starting IDF Terminal"));
        logger.info(` Error: ${error}`);
      }
    });

    afterEach(async function () {
      this.timeout(20000);
      if (this.currentTest.state === "failed") {
        logger.info(`Test failed: ${this.currentTest.title}`);
        logger.info(`Terminal output: >>\r ${testRunner.output.slice(-1000)}`);
        logger.debug(`Terminal output on failure: >>\r ${testRunner.output}`);
        postInstallStepFailed = true;
      }
      try {
        await testRunner.stop();
      } catch (error) {
        logger.info("Error to clean up terminal after test");
        logger.info(` Error: ${error}`);
      }
    });

    after(function () {
      logger.info("Post install test completed, starting cleanup");
      try {
        fs.rmSync(installFolder, { recursive: true, force: true });
        fs.rmSync(toolsfolder, {
          recursive: true,
          force: true,
        });
        logger.info(`Successfully deleted ${installFolder}`);
      } catch (err) {
        logger.info(`Error deleting ${installFolder}`);
      }
    });

    it("1 - Should create a new project based on a template", async function () {
      /**
       * This test should attempt to create a copy of the Hello World Project into the ~/esp folder
       * The commands might differ for each operating system.
       * The assert is based on the existence of the project files in the expected folder.
       */
      logger.info(`Starting test - create new project`);
      testRunner.sendInput(`mkdir ${pathToProjectFolder}\r`);
      testRunner.sendInput(`cd ${pathToProjectFolder}\r`);

      testRunner.sendInput(
        os.platform() !== "win32"
          ? `cp -r $IDF_PATH/examples/get-started/hello_world .\r`
          : `xcopy /E /I $env:IDF_PATH\\examples\\get-started\\hello_world hello_world\r`
      );
      if (os.platform() === "win32") {
        const confirmFilesCopied = await testRunner.waitForOutput("copied");
        expect(confirmFilesCopied).to.be.true;
      }
      testRunner.output = "";
      2;
      testRunner.sendInput("cd hello_world\r");
      testRunner.sendInput("ls\r");

      const confirmFolderContent = await testRunner.waitForOutput(
        "sdkconfig.ci"
      );

      expect(
        confirmFolderContent,
        "sdkconfig.ci file not shown after a ls command, file copy failed"
      ).to.be.true;
      expect(
        testRunner.output,
        "pytest_hello_world.py file not shown after a ls command, file copy failed"
      ).to.include("pytest_hello_world.py");
      expect(
        testRunner.output,
        "main folder not shown after a ls command, file copy failed"
      ).to.include("main");

      logger.info("sample project creation Passed");
    });

    it("2 - Should set the target", async function () {
      /**
       * This test attempts to set a target MCU for the project created in the previous test.
       */
      logger.info(`Starting test - set target`);
      this.timeout(750000);
      testRunner.sendInput(`cd ${pathToProjectFolder}\r`);
      testRunner.sendInput("cd hello_world\r");
      testRunner.sendInput(`idf.py set-target ${validTarget}\r`);

      const startTime = Date.now();
      while (Date.now() - startTime < 900000) {
        if (await testRunner.waitForOutput("failed", 1000)) {
          logger.debug("failed to se target!!!!");
          break;
        }
        if (
          await testRunner.waitForOutput(
            "Build files have been written to",
            1000
          )
        ) {
          logger.debug("Targets Set!!!");
          break;
        }
        await new Promise((resolve) => setTimeout(resolve, 500));
      }
      if (Date.now() - startTime >= 900000) {
        logger.info("Set Target timed out after 15 minutes");
      }

      const targetSet = await testRunner.waitForOutput(
        "Build files have been written to"
      );

      expect(
        targetSet,
        "expecting 'Build files have been written to', failed to complete the set-target task"
      ).to.be.true;
      expect(
        testRunner.output,
        "expecting 'configuring done', failed to complete the set-target task"
      ).to.include("Configuring done");
      expect(
        testRunner.output,
        "expecting 'Generating Done', failed to complete the set-target task"
      ).to.include("Generating done");

      logger.info("Set Target Passed");
    });

    it("3 - Should build project for the selected target", async function () {
      /**
       * This test attempts to build artifacts for the project and targets selected above.
       * The test is successful if the success message is printed in the terminal.
       */
      logger.info(`Starting test - build project`);
      this.timeout(600000);
      testRunner.sendInput(`cd ${pathToProjectFolder}\r`);
      testRunner.sendInput("cd hello_world\r");
      testRunner.sendInput("idf.py build\r");

      const startTime = Date.now();
      while (Date.now() - startTime < 480000) {
        if (await testRunner.waitForOutput("failed", 1000)) {
          logger.debug("Build failed!!!!");
          break;
        }
        if (await testRunner.waitForOutput("Project build complete", 1000)) {
          logger.debug("Build Complete!!!");
          break;
        }
        await new Promise((resolve) => setTimeout(resolve, 500));
      }

      const buildComplete = await testRunner.waitForOutput(
        "Project build complete"
      );
      if (Date.now() - startTime >= 480000) {
        logger.info("Build timed out after 8 minutes");
      }

      expect(
        buildComplete,
        "Expecting 'Project build complete', filed to build the sample project"
      ).to.be.true;
      expect(
        testRunner.output,
        "Expecting to successfully create target image, filed to build the sample project"
      ).to.include(`Successfully created ${validTarget} image`);
      logger.info("Build Passed");
    });
  });
}

function runPostInstallCleanUp(installFolder, toolsfolder) {
  describe("2- Clean UP after install ->", function () {
    after(function () {
      this.timeout(30000);
      logger.info("Starting cleanup");
      try {
        fs.rmSync(installFolder, { recursive: true, force: true });
        fs.rmSync(toolsfolder, {
          recursive: true,
          force: true,
        });
        logger.info(`Successfully deleted ${installFolder}`);
      } catch (err) {
        logger.info(`Error deleting ${installFolder}`);
      }
    });

    it("Should run after installation is complete", function () {
      expect(true).to.be.true;
    });
  });
}

export { runPostInstallTest, runPostInstallCleanUp };
