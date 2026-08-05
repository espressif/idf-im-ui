import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import TestProxy from "../classes/TestProxy.class.js";
import path from "path";
import fs from "fs";
import os from "os";

/**
 * Verifies the regression fix for installing a named version on the same
 * `--idf-path` as an existing IDF installation.
 *
 * Background:
 *   `eim install --idf-path <path>` against a path that already maps to a
 *   tracked IDF in `eim_idf.json` used to abort with
 *   "The IDF at path '<path>' is already installed ... use the 'fix' command",
 *   even when the user also supplied `--version-name <newName>` to register
 *   a separate, named entry pointing at the same directory.
 *
 *   The CLI now skips the "already installed" short-circuit whenever
 *   `--version-name` is provided, allowing a second installation to register
 *   a new entry in `eim_idf.json` against the same path.
 *
 * Scenario under test:
 *   1. Run `eim install -p <installFolder> -i <idf> ...` once with no
 *      `--version-name`. This produces the default-named entry in
 *      `eim_idf.json` and the IDF checkout at `<installFolder>/<idf>/esp-idf`.
 *   2. Run `eim install -p <installFolder> -i <idf> ... --version-name <named>`
 *      against the SAME `<installFolder>` and same IDF version. The wizard
 *      will re-use the existing checkout (path-already-exists branch) and
 *      register a new entry under `<named>`. The CLI must NOT abort with
 *      the "already installed" message.
 *   3. After both installs, `eim_idf.json` must contain two distinct
 *      entries: the original default-named one and the new named one, both
 *      pointing at the same IDF path.
 */
export function runCLINamedVersionInstallTest({
  id = 0,
  pathToEIM,
  args = [],
  idfVersion,
  installFolder,
  namedVersion = "named-idf",
  testProxyMode = false,
  proxyBlockList = [],
}) {
  describe(`${id}- EIM named version install on existing idf-path |`, function () {
    this.timeout(7200000);
    let testRunner = null;
    let proxy = null;

    const espIdfJsonFolder =
      os.platform() === "win32"
        ? path.join("C:\\Espressif", "tools")
        : path.join(os.homedir(), ".espressif", "tools");
    const eimJsonFilePath = path.join(espIdfJsonFolder, "eim_idf.json");
    const idfInstallRoot = path.join(installFolder, idfVersion, "esp-idf");

    before(async function () {
      this.timeout(120000);
      logger.info(
        `Starting named-version install test for IDF ${idfVersion} at ${installFolder}`,
      );
      // Make sure no leftover eim_idf.json from a previous run can fool the
      // "already installed" check or the post-install verification.
      if (fs.existsSync(eimJsonFilePath)) {
        fs.rmSync(eimJsonFilePath, { force: true });
      }
      testRunner = new CLITestRunner();
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
        await testRunner.start();
      } catch (error) {
        logger.info("Error to start terminal");
        logger.debug(`Error: ${error}`);
      }
    });

    afterEach(function () {
      if (this.currentTest.state === "failed") {
        logger.info(`Test failed: ${this.currentTest.title}`);
        if (testRunner) {
          logger.info(
            `Terminal output: >>\r ${testRunner.output.slice(-2000)}`,
          );
          logger.debug(`Terminal output on failure: >>\r ${testRunner.output}`);
        }
      }
    });

    after(async function () {
      this.timeout(60000);
      try {
        if (testRunner) await testRunner.stop();
      } catch (error) {
        logger.info("Error to clean up terminal after test");
        logger.info(` Error: ${error}`);
      } finally {
        testRunner = null;
      }
      if (proxy) {
        try {
          await proxy.stop();
        } catch (error) {
          logger.info("Error stopping proxy server");
          logger.debug(`Error: ${error}`);
        }
      }
    });

    async function waitForInstallCompletion(timeoutMs) {
      const startTime = Date.now();
      while (Date.now() - startTime < timeoutMs) {
        if (Date.now() - testRunner.lastDataTimestamp >= 600000) {
          logger.info(">>>>>>>Exited due to Idle terminal!!!!!");
          return "idle";
        }
        if (await testRunner.waitForOutput("panicked", 1000)) {
          logger.info(">>>>>>>Rust App failure!!!!");
          throw new Error("Rust app failure");
        }
        if (
          await testRunner.waitForOutput(
            "Now you can start using IDF tools",
            1000,
          )
        ) {
          logger.info(">>>>>>>Completed!!!");
          return "completed";
        }
        await new Promise((resolve) => setTimeout(resolve, 1000));
      }
      throw new Error("Installation timed out");
    }

    it(`1- Should install IDF ${idfVersion} (default name)`, async function () {
      this.timeout(3600000);
      logger.info(`Installing IDF ${idfVersion} with default naming`);
      testRunner.callEIM(pathToEIM, ["install", ...args]);
      const result = await waitForInstallCompletion(3600000);
      expect(result, "Initial install did not complete").to.be.oneOf([
        "completed",
        "idle",
      ]);
      expect(
        testRunner.output,
        "Failed to complete initial installation",
      ).to.include("Successfully installed IDF");
    });

    it(`2- Re-installing at same idf-path with --version-name should NOT abort`, async function () {
      this.timeout(3600000);
      logger.info(
        `Re-installing IDF ${idfVersion} at same path with --version-name ${namedVersion}`,
      );
      const beforeOutputLength = testRunner.output.length;
      testRunner.callEIM(pathToEIM, [
        "install",
        "--version-name",
        namedVersion,
        ...args,
      ]);
      const result = await waitForInstallCompletion(3600000);
      const newOutput = testRunner.output.slice(beforeOutputLength);

      // The CLI used to abort before doing any work when the path already
      // mapped to an installed IDF. The bug surfaced as the
      // "already installed" + "use the 'fix' command" pair of messages.
      // After the fix, `--version-name` opts out of that short-circuit,
      // so neither message must appear in the new run output.
      expect(
        newOutput,
        `CLI aborted the named-version install with the "already installed" short-circuit. ` +
          `Output: ${newOutput}`,
      ).to.not.match(/already installed/i);
      expect(
        newOutput,
        `CLI suggested the "fix" command, which is wrong when --version-name is used. ` +
          `Output: ${newOutput}`,
      ).to.not.match(/use the 'fix' command/);

      expect(result, "Named-version install did not complete").to.be.oneOf([
        "completed",
        "idle",
      ]);
    });

    it("3- eim_idf.json should contain both entries sharing the same path", async function () {
      this.timeout(60000);
      logger.info(`Validating eim_idf.json entries`);
      expect(
        fs.existsSync(eimJsonFilePath),
        `eim_idf.json not found at ${eimJsonFilePath}`,
      ).to.be.true;

      const eimJsonContent = JSON.parse(
        fs.readFileSync(eimJsonFilePath, "utf-8"),
      );
      expect(
        eimJsonContent,
        "eim_idf.json does not contain idfInstalled key",
      ).to.have.property("idfInstalled");
      expect(
        eimJsonContent.idfInstalled,
        "eim_idf.json idfInstalled is not an array",
      ).to.be.an("array");
      expect(
        eimJsonContent.idfInstalled.length,
        `Expected exactly 2 entries (default-named + ${namedVersion}), found ${eimJsonContent.idfInstalled.length}`,
      ).to.equal(2);

      const names = eimJsonContent.idfInstalled.map((entry) => entry.name);
      expect(
        names,
        `Expected default entry named '${idfVersion}' to be present in eim_idf.json`,
      ).to.include(idfVersion);
      expect(
        names,
        `Expected new entry named '${namedVersion}' to be present in eim_idf.json`,
      ).to.include(namedVersion);

      for (const entry of eimJsonContent.idfInstalled) {
        expect(
          entry.path,
          `IDF path for entry '${entry.name}' should equal '${idfInstallRoot}'`,
        ).to.equal(idfInstallRoot);
        expect(
          fs.existsSync(entry.path),
          `IDF path for entry '${entry.name}' does not exist on disk: ${entry.path}`,
        ).to.be.true;
        expect(
          fs.existsSync(entry.activationScript),
          `Activation script for entry '${entry.name}' does not exist on disk: ${entry.activationScript}`,
        ).to.be.true;
      }
    });

    it(`4- Re-installing without --version-name should still abort with the "already installed" message`, async function () {
      this.timeout(600000);
      logger.info(
        `Sanity check: re-installing without --version-name should still be rejected`,
      );
      const beforeOutputLength = testRunner.output.length;
      testRunner.callEIM(pathToEIM, ["install", ...args]);

      // The CLI should refuse with the "already installed" message and
      // exit promptly. Give it a generous window but no full install
      // timeout: this branch must NOT trigger an actual install.
      const deadline = Date.now() + 600000;
      let sawMessage = false;
      while (Date.now() < deadline) {
        const tail = testRunner.output.slice(beforeOutputLength);
        if (/already installed/i.test(tail)) {
          sawMessage = true;
          break;
        }
        if (await testRunner.waitForOutput("panicked", 1000)) {
          break;
        }
        if (
          await testRunner.waitForOutput(
            "Now you can start using IDF tools",
            1000,
          )
        ) {
          break;
        }
        await new Promise((resolve) => setTimeout(resolve, 1000));
      }
      const tail = testRunner.output.slice(beforeOutputLength);
      expect(
        sawMessage,
        `CLI should have aborted the second install without --version-name ` +
          `with the "already installed" message. Output: ${tail}`,
      ).to.be.true;
      expect(
        tail,
        `Output should also reference the "fix" command. Output: ${tail}`,
      ).to.match(/use the 'fix' command/);
    });

    it("5- Both entries should still be present after the rejected install", async function () {
      this.timeout(30000);
      const eimJsonContent = JSON.parse(
        fs.readFileSync(eimJsonFilePath, "utf-8"),
      );
      const names = eimJsonContent.idfInstalled.map((entry) => entry.name);
      expect(
        names,
        `Expected both '${idfVersion}' and '${namedVersion}' to remain in eim_idf.json`,
      ).to.include.members([idfVersion, namedVersion]);
      expect(
        eimJsonContent.idfInstalled.length,
        "eim_idf.json should still hold exactly 2 entries",
      ).to.equal(2);
    });
  });
}
