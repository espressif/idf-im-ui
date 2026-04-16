import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import os from "os";

/**
 * Verifies that EIM can be installed, queried, and uninstalled via a system
 * package manager (apt, dnf, brew, winget, scoop).
 *
 * The test does NOT perform a full ESP-IDF installation — it validates the
 * package-manager lifecycle only:
 *   1. (optional) configure the package repository
 *   2. install the package
 *   3. verify the binary is on PATH and reports the expected version
 *   4. verify help output is sane
 *   5. verify package metadata via the native query tool
 *   6. (optional) uninstall the package
 */
export function runCLIPkgManagerInstallTest({
  id = 0,
  packageManager,
  packageName,
  repoSetupCommands = [],
  expectedVersion = "",
  deleteAfterTest = true,
}) {
  const isWindows = os.platform() === "win32";

  const installCommands = {
    apt: (pkg) => `sudo apt-get update && sudo apt-get install -y ${pkg}`,
    dnf: (pkg) => `sudo dnf install -y ${pkg}`,
    brew: (pkg) => `brew install ${pkg}`,
    winget: (pkg) =>
      `winget install --id ${pkg} --accept-package-agreements --accept-source-agreements`,
    scoop: (pkg) => `scoop install ${pkg}`,
  };

  const uninstallCommands = {
    apt: (pkg) => `sudo apt-get remove -y ${pkg}`,
    dnf: (pkg) => `sudo dnf remove -y ${pkg}`,
    brew: (pkg) => `brew uninstall ${pkg}`,
    winget: (pkg) =>
      `winget uninstall --id ${pkg} --accept-source-agreements`,
    scoop: (pkg) => `scoop uninstall ${pkg}`,
  };

  const metadataCommands = {
    apt: (pkg) => `dpkg -l ${pkg}`,
    dnf: (pkg) => `rpm -qi ${pkg}`,
    brew: (pkg) => `brew list ${pkg}`,
    winget: (pkg) => `winget list --id ${pkg}`,
    scoop: (pkg) => `scoop info ${pkg}`,
  };

  const whichCommand = isWindows ? "where eim.exe" : "which eim";

  describe(`${id}- Package manager install (${packageManager}) |`, function () {
    let testRunner = null;
    let installStepFailed = false;

    before(async function () {
      this.timeout(300000);
      testRunner = new CLITestRunner();
      try {
        await testRunner.start();
      } catch (error) {
        logger.info("Error starting terminal");
        logger.debug(`Error: ${error}`);
        throw error;
      }

      for (const cmd of repoSetupCommands) {
        logger.info(`Running repo setup: ${cmd}`);
        testRunner.output = "";
        testRunner.sendInput(cmd);
        await testRunner.waitForPrompt(30000);
      }
    });

    afterEach(function () {
      if (this.currentTest.state === "failed") {
        logger.info(`Test failed: ${this.currentTest.title}`);
        logger.info(`Terminal output: >>\r ${testRunner.output.slice(-2000)}`);
        logger.debug(`Terminal output on failure: >>\r ${testRunner.output}`);
        installStepFailed = true;
      }
    });

    after(async function () {
      this.timeout(120000);
      logger.info("Package manager test routine completed");

      if (deleteAfterTest && !installStepFailed) {
        const uninstallFn = uninstallCommands[packageManager];
        if (uninstallFn) {
          logger.info(`Uninstalling ${packageName} via ${packageManager}`);
          testRunner.output = "";
          testRunner.sendInput(uninstallFn(packageName));
          await testRunner.waitForPrompt(60000);
        }
      }

      try {
        await testRunner.stop();
      } catch (error) {
        logger.info("Error cleaning up terminal after test");
        logger.info(` Error: ${error}`);
      } finally {
        testRunner = null;
      }
    });

    it("1- Package should install without error", async function () {
      this.timeout(300000);
      logger.info(
        `Installing ${packageName} via ${packageManager}`,
      );
      testRunner.output = "";

      const installFn = installCommands[packageManager];
      expect(installFn, `Unsupported package manager: ${packageManager}`).to
        .not.be.undefined;

      testRunner.sendInput(
        `${installFn(packageName)} ; echo "PKG_INSTALL" ; echo "DONE"`,
      );

      const installComplete = await testRunner.waitForOutput(
        "PKG_INSTALLDONE",
        240000,
      );
      expect(
        installComplete,
        `Package install via ${packageManager} did not complete in time. Output: ${testRunner.output.slice(-500)}`,
      ).to.be.true;
    });

    it("2- EIM binary should be on PATH", async function () {
      this.timeout(30000);
      if (installStepFailed) this.skip();

      logger.info("Verifying eim binary is on PATH");
      testRunner.output = "";
      testRunner.sendInput(
        `${whichCommand} ; echo "WHICH" ; echo "DONE"`,
      );

      const found = await testRunner.waitForOutput("WHICHDONE", 15000);
      expect(found, "which/where command did not complete").to.be.true;
      expect(
        testRunner.output.toLowerCase(),
        "eim binary not found on PATH",
      ).to.include("eim");
    });

    it("3- EIM version should match expected release", async function () {
      this.timeout(30000);
      if (installStepFailed) this.skip();

      logger.info("Verifying eim --version output");
      testRunner.output = "";
      testRunner.callEIM("eim", ["--version"]);

      const versionSeen = await testRunner.waitForOutput("eim", 15000);
      expect(versionSeen, "eim --version did not produce output").to.be.true;

      if (expectedVersion) {
        const versionStr = expectedVersion.replace(/^eim\s*/, "");
        expect(
          testRunner.output,
          `Expected version ${versionStr} not found in output: ${testRunner.output}`,
        ).to.include(versionStr);
      }
    });

    it("4- EIM help output should be functional", async function () {
      this.timeout(30000);
      if (installStepFailed) this.skip();

      logger.info("Verifying eim --help output");
      testRunner.output = "";
      testRunner.callEIM("eim", ["--help"]);

      const helpSeen = await testRunner.waitForOutput("install", 15000);
      expect(
        helpSeen,
        "eim --help did not show 'install' command",
      ).to.be.true;
    });

    it("5- Package metadata should show installed", async function () {
      this.timeout(30000);
      if (installStepFailed) this.skip();

      const metaFn = metadataCommands[packageManager];
      if (!metaFn) {
        logger.info(
          `No metadata check available for ${packageManager}, skipping`,
        );
        this.skip();
      }

      logger.info(`Checking package metadata via ${packageManager}`);
      testRunner.output = "";
      testRunner.sendInput(
        `${metaFn(packageName)} ; echo "META" ; echo "DONE"`,
      );

      const metaDone = await testRunner.waitForOutput("METADONE", 15000);
      expect(metaDone, "Metadata query did not complete").to.be.true;

      const lowerOutput = testRunner.output.toLowerCase();
      const packageBaseName = packageName.split("/").pop().toLowerCase();
      expect(
        lowerOutput,
        `Package name '${packageBaseName}' not found in metadata output`,
      ).to.include(packageBaseName);
    });
  });
}
