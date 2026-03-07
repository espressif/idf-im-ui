import { expect } from "chai";
import { describe, it, before, after } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import os from "os";
import path from "path";
import fs from "fs";
import { execSync } from "child_process";

export function runCLIExistingGitRepoClonedInstalledTest({ id = 0, pathToEIM, gitRepoUrl, gitRepoBranch, toolsFolder }) {
  describe(`${id}- Existing Git Repo Cloned Installed |`, function () {
    let testRunner = null;
    const gitCloneFolder = path.join(os.homedir(), "git_clone");
    const idfRepository = gitRepoUrl || "https://github.com/espressif/esp-idf.git";
    const idfRepositoryBranch = gitRepoBranch || "v5.5.3";


    before(async function () {
      logger.info(`Starting test - existing git`);
      this.timeout(60000);
      logger.debug(`Removing any existing IDF repository from ${gitCloneFolder}`);
      fs.rmSync(gitCloneFolder, { recursive: true, force: true });
      fs.mkdirSync(gitCloneFolder, { recursive: true });
      logger.info(`Cloning IDF repository from ${idfRepository} branch ${idfRepositoryBranch} to ${gitCloneFolder}`);
      execSync(`git clone --branch ${idfRepositoryBranch} "${idfRepository}" "${gitCloneFolder}"`, {
        stdio: "inherit",
        timeout: 120000,
      });
      logger.info(`IDF repository cloned successfully`);
      logger.info(`Starting test runner`);
      testRunner = new CLITestRunner();
      try {
        await testRunner.start();

      } catch (error) {
        logger.info(`Error starting process: ${error}`);
        logger.debug(` Error: ${error}`);
      }
    });

    after(async function () {
      this.timeout(20000);
      if (testRunner && this.currentTest.state === "failed") {
        logger.info(`Test failed: ${this.currentTest.title}`);
        logger.debug(`Terminal output on failure: >>\r ${testRunner.output}`);
      }
      try {
        if (testRunner) {
          logger.info(`Stopping test runner`);
          await testRunner.stop();
          logger.info(`Test runner stopped`);
        }
      } catch (error) {
        logger.info("Error to clean up terminal after test");
        logger.info(` Error: ${error}`);
      }

      testRunner = null;
    });

    it("1- Should install IDF using existing git repository", async function () {
      this.timeout(3660000);
      logger.info(`Starting test - install IDF using existing git repository at ${gitCloneFolder} with EIM at ${pathToEIM}`);
      const installPathArg = os.platform() === "win32" ? `"${gitCloneFolder}"` : gitCloneFolder;
      testRunner.sendInput(`${pathToEIM} install -p ${installPathArg}`);
      const successfullyInstalled = await testRunner.waitForOutput("Successfully installed IDF", 300000);
      expect(testRunner.output).to.include("Successfully installed IDF");
      expect(testRunner.output).to.include("Now you can start using IDF tools");
    });

    it("2- Installation metadata and idf.py --version", async function () {
      this.timeout(120000);
      const eimJsonPath = path.join(toolsFolder, "tools", "eim_idf.json");

      expect(fs.existsSync(eimJsonPath), "eim_idf.json not found").to.be.true;
      const eimJson = JSON.parse(fs.readFileSync(eimJsonPath, "utf-8"));
      expect(eimJson, "eim_idf.json must have idfInstalled").to.have.property("idfInstalled");
      expect(eimJson.idfInstalled).to.be.an("array").that.is.not.empty;

      const gitCloneResolved = path.resolve(gitCloneFolder);
      const entry = eimJson.idfInstalled.find(
        (e) => path.resolve(e.path) === gitCloneResolved
      );
      expect(entry, `No eim_idf.json entry for path ${gitCloneFolder}`).to.not.be.undefined;
      expect(entry, "Entry must have activationScript").to.have.property("activationScript");
      expect(entry, "Entry must have path").to.have.property("path");
      expect(entry, "Entry must have python").to.have.property("python");
      expect(fs.existsSync(entry.activationScript), "Activation script must exist").to.be.true;
      expect(fs.existsSync(entry.path), "IDF path must exist").to.be.true;
      expect(fs.existsSync(entry.python), "Python path must exist").to.be.true;

      if (os.platform() === "win32") {
        testRunner.sendInput(`. "${entry.activationScript}"`);
        await new Promise((resolve) => setTimeout(resolve, 10000));
      } else {
        testRunner.sendInput(`source ${entry.activationScript}`);
        const ready = await testRunner.waitForOutput("(venv)", 15000);
        if (!ready) {
          logger.info("Output: " + testRunner.output);
          throw new Error("IDF environment did not load (no (venv) in output)");
        }
      }
      testRunner.output = "";
      testRunner.sendInput("idf.py --version");
      const versionSeen = await testRunner.waitForOutput(idfRepositoryBranch, 15000);
      expect(testRunner.output, `idf.py --version should include ${idfRepositoryBranch}; output: ${testRunner.output}`).to.include(idfRepositoryBranch);
    });

  });
}
