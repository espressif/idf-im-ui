import { expect } from "chai";
import { describe, it, beforeEach, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import { getPlatformKey } from "../helper.js";
import path from "path";
import fs from "fs";
import os from "os";

export function runInstallVerification({
  id,
  installFolder,
  idfList,
  targetList = ["esp32"],
  toolsFolder,
}) {
  describe(`${id}- Installation verification test |`, function () {
    this.timeout(600000);
    let testRunner = null;
    let verificationStepFailed = false;

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
            `Terminal output: >>\r ${testRunner.output.slice(-2000)}`
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
        } finally {
          testRunner = null;
        }
      }
    });

    it("1- EIM json file should have expected contents", async function () {
      /**
       * This test checks the eim_idf.json file in the tools folder.
       * It verifies that the file exists and contains the expected structure.
       * It also checks that each IDF entry has the required properties and valid paths.
       * The test will fail if any of the expected properties are missing or if the paths are invalid.
       *
       */
      logger.info(`Validating eim_idf.json file contents`);
      logger.debug(`EIM Json file path: ${eimJsonFilePath}`);
      expect(
        fs.existsSync(eimJsonFilePath),
        "eim-idf.json file not found on the tools folder."
      ).to.be.true;
      const eimJsonContent = JSON.parse(
        fs.readFileSync(eimJsonFilePath, "utf-8")
      );
      logger.debug("EIM Json file content:", eimJsonContent);
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

    it("2- IDF activation script should exist", async function () {
      /**
       * This test checks if there is an entry in the eim_idf.json file for each IDF version installed.
       * It also verifies that an activation script is present and if the scripts are named correctly.
       * The test also checks IDF is installed in the correct folder and if Python path matches the expected path.
       */
      logger.info(`Validating entries for installed IDFs in eim_idf.json`);
      const eimJsonContent = JSON.parse(
        fs.readFileSync(eimJsonFilePath, "utf-8")
      );
      logger.debug("EIM Json file content: ", eimJsonContent);

      for (let idf of idfList) {
        let eimJsonEntry = null;

        for (let entry of eimJsonContent.idfInstalled) {
          if (entry.name === idf) {
            eimJsonEntry = entry;
            break;
          }
        }
        logger.debug(`EIM Json entry for IDF ${idf}: `, eimJsonEntry);
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
          fs.existsSync(eimJsonEntry.python),
          `Invalid python path provided on eim.json ${eimJsonEntry.python}`
        ).to.be.true;
      }
    });

    it("3- Check python environment requirements", async function () {
      /**
       * This test checks if the Python environment is set up correctly.
       */
      logger.info(`Validating python requirements`);
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

        testRunner = new CLITestRunner();

        let pythonRequirementPath = fs.existsSync(
          path.join(
            eimJsonEntry.path,
            "tools",
            "requirements",
            "requirements.core.txt"
          )
        )
          ? path.join(
              eimJsonEntry.path,
              "tools",
              "requirements",
              "requirements.core.txt"
            )
          : path.join(eimJsonEntry.path, "requirements.txt");

        expect(
          fs.existsSync(pythonRequirementPath),
          `Python requirements file not found for IDF ${idf}`
        ).to.be.true;

        try {
          await testRunner.runIDFTerminal(eimJsonEntry.activationScript);
        } catch (error) {
          logger.info("Error to start IDF terminal");
          logger.info(testRunner.output);
          logger.info(` Error: ${error}`);
          throw new Error("Error starting IDF Terminal");
        }

        testRunner.sendInput(
          `${eimJsonEntry.python} ${path.join(
            eimJsonEntry.path,
            "tools",
            "check_python_dependencies.py"
          )} -r ${pythonRequirementPath}`
        );
        const satisfiedReqs = await testRunner.waitForOutput(
          "Python requirements are satisfied"
        );
        expect(satisfiedReqs, "Python Requirements not installed").to.be.true;

        try {
          await testRunner.stop();
        } catch (error) {
          logger.info("Error to stop terminal");
          logger.debug(` Error: ${error}`);
        } finally {
          testRunner = null;
        }
      }
    });

    it("4- Should have correct tools version installed on path", async function () {
      /**
       * This test checks if the tools folder contains the expected tools versions.
       * The tools are activated by the activation script.
       *
       */
      logger.info(`Validating tools versions installed on path`);
      for (let idf of idfList) {
        testRunner = new CLITestRunner();
        let pathToIDFScript =
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
          logger.info(` Error: ${error}`);
          throw new Error("Error starting IDF Terminal");
        }

        let toolsIndexFile = JSON.parse(
          fs.readFileSync(
            path.join(installFolder, idf, "esp-idf", "tools", "tools.json"),
            "utf-8"
          )
        );
        expect(
          toolsIndexFile,
          `tools.json file not found on the tools folder for IDF ${idf}`
        ).to.be.an("object").that.is.not.empty;
        expect(
          toolsIndexFile,
          `tools.json file does not contain expected tools for IDF ${idf}`
        ).to.have.property("tools");

        // Should check which are the tools that are supposed to be installed based on the OS architecture and the selected targets
        // This information comes from the keys platform_overrides and supported_targets

        const platformKey = getPlatformKey();
        const osRequiredTools = toolsIndexFile.tools.filter((tool) => {
          if (tool.platform_overrides) {
            for (let entry of tool.platform_overrides) {
              if (entry.install && entry.platforms.includes(platformKey)) {
                if (
                  entry.install === "always" ||
                  entry.install === "on_request"
                ) {
                  return true;
                }
              }
            }
          }
          if (tool.install === "always" || tool.install === "on_request") {
            return true;
          }
          return false;
        });

        logger.info(
          `Required tools for IDF ${idf} on platform ${platformKey}: ${osRequiredTools
            .map((tool) => tool.name)
            .join(", ")}`
        );

        const requiredTools = osRequiredTools
          .map((tool) => {
            if (targetList.some((t) => t.toLowerCase() === "all")) {
              return tool.name;
            }
            if (!tool.supported_targets) {
              return tool.name;
            }
            if (tool.supported_targets.some((t) => t.toLowerCase() === "all")) {
              return tool.name;
            }
            if (
              targetList.some((target) =>
                tool.supported_targets
                  .map((t) => t.toLowerCase())
                  .includes(target.toLowerCase())
              )
            ) {
              return tool.name;
            }
            return undefined;
          })
          .filter(Boolean);

        logger.info(
          `Required tools for IDF ${idf} on platform ${platformKey} and targets ${targetList.join(
            ", "
          )}: ${requiredTools.join(", ")}`
        );

        for (let tool of toolsIndexFile.tools) {
          testRunner.output = "";
          await new Promise((resolve) => setTimeout(resolve, 1000));
          if (requiredTools.includes(tool.name)) {
            expect(
              tool,
              `Tool entry in tools.json file does not contain expected properties for IDF ${idf}`
            ).to.have.property("name");
            expect(
              tool,
              `Tool entry in tools.json file does not contain expected properties for IDF ${idf}`
            ).to.have.property("version_cmd");

            if (tool.version_cmd.join(" ") !== "") {
              testRunner.sendInput(`${tool.version_cmd.join(" ")}`);
              let toolVersionOutput = await testRunner.waitForOutput(
                `${tool.versions[0].name}`,
                20000
              );
              logger.debug(
                `Tool ${tool.name} version output: ${testRunner.output} expected: ${tool.versions[0].name} result: ${toolVersionOutput}`
              );
              expect(
                toolVersionOutput,
                `Tool ${tool.name} version not matching expected version ${tool.versions[0].name}`
              ).to.be.true;
            }

            if (
              tool.name === "esp-rom-elfs" &&
              tool.version_cmd.join(" ") === ""
            ) {
              const espRomElfsVersion = tool.versions[0].name;
              const espRomElfsPath = path.join(
                toolsFolder,
                "tools",
                "esp-rom-elfs",
                espRomElfsVersion
              );

              expect(
                fs.existsSync(espRomElfsPath),
                `esp-rom-elfs path does not exist: ${espRomElfsPath}`
              ).to.be.true;

              const files = fs.readdirSync(espRomElfsPath);
              const hasRomElf = files.some((f) => f.endsWith("rom.elf"));
              expect(
                hasRomElf,
                `No *rom.elf files found in esp-rom-elfs path: ${espRomElfsPath}`
              ).to.be.true;
            }
          }
        }
        try {
          await testRunner.stop();
        } catch (error) {
          logger.info("Error to stop terminal");
          logger.debug(` Error: ${error}`);
        } finally {
          testRunner = null;
        }
      }
    });

    it("5- Should create a new project based on a template", async function () {
      /**
       * This test should attempt to create a copy of the Hello World Project into the ~/esp folder
       * The commands might differ for each operating system.
       * The assert is based on the existence of the project files in the expected folder.
       */
      logger.info(`Starting test - create new project`);
      for (let idf of idfList) {
        testRunner = new CLITestRunner();
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
          logger.info(` Error: ${error}`);
          throw new Error("Error starting IDF Terminal");
        }

        testRunner.sendInput(`mkdir ${pathToProjectFolder}`);
        await new Promise((resolve) => setTimeout(resolve, 500));

        testRunner.sendInput(`cd ${pathToProjectFolder}`);
        await new Promise((resolve) => setTimeout(resolve, 500));

        testRunner.sendInput(
          os.platform() !== "win32"
            ? `cp -r $IDF_PATH/examples/get-started/hello_world .`
            : `xcopy /E /I $env:IDF_PATH\\examples\\get-started\\hello_world hello_world`
        );
        if (os.platform() === "win32") {
          const confirmFilesCopied = await testRunner.waitForOutput("copied");
          expect(confirmFilesCopied).to.be.true;
        }

        testRunner.output = "";
        2;
        testRunner.sendInput("cd hello_world");
        await new Promise((resolve) => setTimeout(resolve, 500));
        testRunner.sendInput("ls");

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

        try {
          await testRunner.stop();
        } catch (error) {
          logger.info("Error to stop terminal");
          logger.debug(` Error: ${error}`);
        } finally {
          testRunner = null;
        }
      }
    });

    it("6- Should set the target", async function () {
      /**
       * This test attempts to set a target MCU for the project created in the previous test.
       */
      this.timeout(750000);
      logger.info(`Starting test - set target`);

      for (let idf of idfList) {
        testRunner = new CLITestRunner();
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
          logger.info(` Error: ${error}`);
          throw new Error("Error starting IDF Terminal");
        }

        const validTarget =
          targetList[0].toLowerCase() === "all" ? "esp32" : targetList[0];

        testRunner.sendInput(`cd ${pathToProjectFolder}`);
        testRunner.sendInput(`idf.py set-target ${validTarget}`);

        const startTime = Date.now();
        while (Date.now() - startTime < 1200000) {
          if (Date.now() - testRunner.lastDataTimestamp >= 600000) {
            logger.info(">>>>>>>Exited due to Idle terminal!!!!!");
            break;
          }
          if (
            await testRunner.waitForOutput(
              "Build files have been written to",
              1000
            )
          ) {
            logger.info("Target Set!!!");
            break;
          }
          await new Promise((resolve) => setTimeout(resolve, 1000));
        }
        if (Date.now() - startTime >= 1200000) {
          logger.info("Set Target timed out after 20 minutes");
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

        try {
          await testRunner.stop();
        } catch (error) {
          logger.info("Error to stop terminal");
          logger.debug(` Error: ${error}`);
        } finally {
          testRunner = null;
        }
      }
    });

    it("7- Should build project for the selected target", async function () {
      /**
       * This test attempts to build artifacts for the project and targets selected above.
       * The test is successful if the success message is printed in the terminal.
       */
      this.timeout(600000);
      logger.info(`Starting test - build project`);
      for (let idf of idfList) {
        testRunner = new CLITestRunner();
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
          logger.info(` Error: ${error}`);
          throw new Error("Error starting IDF Terminal");
        }

        testRunner.sendInput(`cd ${pathToProjectFolder}`);
        testRunner.sendInput("idf.py build");

        const startTime = Date.now();
        while (Date.now() - startTime < 480000) {
          if (Date.now() - testRunner.lastDataTimestamp >= 300000) {
            logger.info(">>>>>>>Exited due to Idle terminal!!!!!");
            break;
          }
          if (await testRunner.waitForOutput("Project build complete", 1000)) {
            logger.info("Build Complete!!!");
            break;
          }
          await new Promise((resolve) => setTimeout(resolve, 1000));
        }

        const buildComplete = await testRunner.waitForOutput(
          "Project build complete"
        );
        if (Date.now() - startTime >= 480000) {
          logger.info("Build timed out after 8 minutes");
        }

        expect(
          buildComplete,
          "Expecting 'Project build complete', failed to build the sample project"
        ).to.be.true;
        const validTarget =
          targetList[0].toLowerCase() === "all" ? "esp32" : targetList[0];
        expect(
          testRunner.output,
          "Expecting to successfully create target image, failed to build the sample project"
        ).to.include(`Successfully created ${validTarget} image`);
        logger.info("Build Passed");

        try {
          await testRunner.stop();
        } catch (error) {
          logger.info("Error to stop terminal");
          logger.debug(` Error: ${error}`);
        } finally {
          testRunner = null;
        }
      }
    });
  });
}
