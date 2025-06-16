import { expect } from "chai";
import { describe, it, after, beforeEach, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import path from "path";
import fs from "fs";
import os from "os";

function runInstallVerification({
  installFolder,
  idfList,
  validTarget = "esp32",
}) {
  describe("3- Installation verification test ->", function () {
    this.timeout(600000);
    let testRunner = null;
    let verificationStepFailed = false;

    const toolsFolder =
      os.platform() !== "win32"
        ? path.join(os.homedir(), `.espressif`)
        : `C:\\Espressif`;

    const eimJsonFilePath = path.join(toolsFolder, "tools", "eim_idf.json");

    beforeEach(async function () {
      this.timeout(10000);
      if (verificationStepFailed) {
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
            `Terminal output: >>\r ${testRunner.output.slice(-1000)}`
          );
          logger.debug(`Terminal output on failure: >>\r ${testRunner.output}`);
        }
        verificationStepFailed = true;
      }
      if (testRunner) {
        try {
          await testRunner.stop();
        } catch (error) {
          logger.info("Error to clean up terminal after test");
          logger.info(` Error: ${error}`);
        }
      }
    });

    after(function () {
      logger.info("Post install test completed, starting cleanup");
      try {
        fs.rmSync(installFolder, { recursive: true, force: true });
        fs.rmSync(toolsFolder, {
          recursive: true,
          force: true,
        });
        logger.info(`Successfully deleted ${installFolder} and tools folder`);
      } catch (err) {
        logger.info(`Error deleting ${installFolder}`);
      }
    });

    it("1 - EIM json file should have expected contents", async function () {
      /**
       * This test checks the eim_idf.json file in the tools folder.
       * It verifies that the file exists and contains the expected structure.
       * It also checks that each IDF entry has the required properties and valid paths.
       * The test will fail if any of the expected properties are missing or if the paths are invalid.
       *
       */
      logger.info(`Validating eim_idf.json file contents`);
      expect(
        fs.existsSync(eimJsonFilePath),
        "eim-idf.json file not found on the tools folder."
      ).to.be.true;
      const eimJsonContent = JSON.parse(
        fs.readFileSync(eimJsonFilePath, "utf-8")
      );
      expect(
        eimJsonContent,
        "Content of eim_ide.json is not an object"
      ).to.be.an("object");
      expect(
        eimJsonContent,
        "Eim_idf.json file does not contain idfInstalled key"
      ).to.have.property("idfInstalled");
      expect(
        eimJsonContent.idfInstalled,
        "Eim_idf.json file does not contain an array on idfInstalled key"
      ).to.be.a("array").that.is.not.empty;
      expect(
        eimJsonContent,
        "Eim_idf.json file does not contain gitPath key"
      ).to.have.property("gitPath");
      expect(
        eimJsonContent,
        "Eim_idf.json file does not contain idfSelectedId key"
      ).to.have.property("idfSelectedId");
      expect(
        eimJsonContent,
        "Eim_idf.json file does not contain eimPath key"
      ).to.have.property("eimPath");
      expect(
        fs.existsSync(eimJsonContent.eimPath),
        "eim-idf.json file does not provide valid path to eim."
      ).to.be.true;

      for (let idf of eimJsonContent.idfInstalled) {
        expect(
          idf,
          `No activationScript path on idf ${idf.name || "invalid IDF Entry"}`
        ).to.have.property("activationScript");

        expect(
          fs.existsSync(idf.activationScript),
          `Invalid activation script path on idf ${
            idf.name || "invalid IDF Entry"
          }`
        ).to.be.true;

        expect(
          idf,
          `No id on idf ${idf.name || "invalid IDF Entry"}`
        ).to.have.property("id");

        expect(
          idf,
          `No idfToolsPath on idf ${idf.name || "invalid IDF Entry"}`
        ).to.have.property("idfToolsPath");

        expect(
          fs.existsSync(idf.idfToolsPath),
          `Invalid tools path on idf ${idf.name || "invalid IDF Entry"}`
        ).to.be.true;

        expect(idf, `No name on idf entry`).to.have.property("name");

        expect(
          idf,
          `No IDF path on idf ${idf.name || "invalid IDF Entry"}`
        ).to.have.property("path");

        expect(
          fs.existsSync(idf.path),
          `Invalid IDF path on idf ${idf.name || "invalid IDF Entry"}`
        ).to.be.true;

        expect(
          idf,
          `No python path on idf ${idf.name || "invalid IDF Entry"}`
        ).to.have.property("python");

        expect(
          fs.existsSync(idf.python),
          `Invalid Python path on idf ${idf.name || "invalid IDF Entry"}`
        ).to.be.true;
      }
    });

    it("2 - IDF activation script should exist", async function () {
      /**
       * This test checks if there is an entry in the eim_idf.json file for each IDF version installed.
       * It also verifies that an activation script is present and if the scripts are named correctly.
       * The test also checks IDF is installed in the correct folder and if Python path matches the expected path.
       */
      logger.info(`Validating entries for installed IDFs in eim_idf.json`);
      const eimJsonContent = JSON.parse(
        fs.readFileSync(eimJsonFilePath, "utf-8")
      );

      for (let idf of idfList) {
        let eimJsonEntry = null;

        for (let entry of eimJsonContent.idfInstalled) {
          if (entry.name === idf) {
            eimJsonEntry = entry;
            break;
          }
        }

        expect(eimJsonEntry, `No entry for IDF ${idf} in eim_idf.json`).to.not
          .be.null;

        const pathToIDFScript =
          os.platform() !== "win32"
            ? path.join(toolsFolder, "tools", `activate_idf_${idf}.sh`)
            : path.join(
                toolsFolder,
                "tools",
                `Microsoft.${idf}.PowerShell_profile.ps1`
              );

        expect(
          eimJsonEntry.activationScript,
          `Activation script on eim_idf.json not matching expected path for IDF ${idf}`
        ).to.equal(pathToIDFScript);
        expect(
          eimJsonEntry.path,
          `IDF path on eim_idf.json not matching expected path for IDF ${idf}`
        ).to.equal(path.join(installFolder, idf, "esp-idf"));
        expect(
          eimJsonEntry.python,
          `Python path on eim_idf.json not matching expected path for IDF ${idf}`
        ).to.equal(
          os.platform() !== "win32"
            ? path.join(
                toolsFolder,
                "tools",
                "python",
                idf,
                "venv",
                "bin",
                "python3"
              )
            : path.join(
                toolsFolder,
                "tools",
                "python",
                idf,
                "venv",
                "Scripts",
                "Python.exe"
              )
        );
      }
    });

    it("3 - Should create a new project based on a template", async function () {
      /**
       * This test should attempt to create a copy of the Hello World Project into the ~/esp folder
       * The commands might differ for each operating system.
       * The assert is based on the existence of the project files in the expected folder.
       */
      let testRunner = null;
      testRunner = new CLITestRunner();
      logger.info(`Starting test - create new project`);
      for (let idf of idfList) {
        let pathToProjectFolder = path.join(installFolder, idf, "project");
        const pathToIDFScript =
          os.platform() !== "win32"
            ? path.join(toolsFolder, "tools", `activate_idf_${idf}.sh`)
            : path.join(
                toolsFolder,
                "tools",
                `Microsoft.${idf}.PowerShell_profile.ps1`
              );
        try {
          await testRunner.runIDFTerminal(pathToIDFScript);
        } catch (error) {
          logger.info("Error to start IDF terminal");
          logger.info(testRunner.output);
          this.test.error(new Error("Error starting IDF Terminal"));
          logger.info(` Error: ${error}`);
        }

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
      }
    });

    it("4 - Should set the target", async function () {
      /**
       * This test attempts to set a target MCU for the project created in the previous test.
       */
      this.timeout(750000);
      let testRunner = null;
      testRunner = new CLITestRunner();
      logger.info(`Starting test - set target`);

      for (let idf of idfList) {
        let pathToProjectFolder = path.join(
          installFolder,
          idf,
          "project",
          "hello_world"
        );
        const pathToIDFScript =
          os.platform() !== "win32"
            ? path.join(toolsFolder, "tools", `activate_idf_${idf}.sh`)
            : path.join(
                toolsFolder,
                "tools",
                `Microsoft.${idf}.PowerShell_profile.ps1`
              );
        try {
          await testRunner.runIDFTerminal(pathToIDFScript);
        } catch (error) {
          logger.info("Error to start IDF terminal");
          logger.info(testRunner.output);
          this.test.error(new Error("Error starting IDF Terminal"));
          logger.info(` Error: ${error}`);
        }

        testRunner.sendInput(`cd ${pathToProjectFolder}\r`);
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
            logger.debug("Target Set!!!");
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
      }
    });

    it("5 - Should build project for the selected target", async function () {
      /**
       * This test attempts to build artifacts for the project and targets selected above.
       * The test is successful if the success message is printed in the terminal.
       */
      this.timeout(600000);
      let testRunner = null;
      testRunner = new CLITestRunner();
      logger.info(`Starting test - build project`);
      for (let idf of idfList) {
        let pathToProjectFolder = path.join(
          installFolder,
          idf,
          "project",
          "hello_world"
        );
        const pathToIDFScript =
          os.platform() !== "win32"
            ? path.join(toolsFolder, "tools", `activate_idf_${idf}.sh`)
            : path.join(
                toolsFolder,
                "tools",
                `Microsoft.${idf}.PowerShell_profile.ps1`
              );
        try {
          await testRunner.runIDFTerminal(pathToIDFScript);
        } catch (error) {
          logger.info("Error to start IDF terminal");
          logger.info(testRunner.output);
          this.test.error(new Error("Error starting IDF Terminal"));
          logger.info(` Error: ${error}`);
        }

        testRunner.sendInput(`cd ${pathToProjectFolder}\r`);
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
      }
    });
  });
}

export { runInstallVerification };
