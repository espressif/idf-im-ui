import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import logger from "../classes/logger.class.js";
import path from "path";
import fs from "fs";
import os from "os";

function getActivationScriptDir(_installFolder) {
  if (os.platform() === "win32") {
    return "C:\\Espressif\\tools";
  }
  return path.join(os.homedir(), ".espressif", "tools");
}

function getScriptPaths(installFolder, idfVersion) {
  const dir = getActivationScriptDir(installFolder);
  if (os.platform() === "win32") {
    return {
      activate: path.join(dir, `Microsoft.${idfVersion}.PowerShell_profile.ps1`),
      deactivate: path.join(dir, `Microsoft.${idfVersion}.PowerShell_deactivate.ps1`),
    };
  }
  return {
    activate: path.join(dir, `activate_idf_${idfVersion}.sh`),
    deactivate: path.join(dir, `deactivate_idf_${idfVersion}.sh`),
  };
}

function extractProbeOutput(buffer, marker) {
  const idx = buffer.indexOf(marker);
  return idx >= 0 ? buffer.slice(idx) : buffer;
}

export function runCLIDeactivationTest({
  id = 0,
  pathToEIM,
  args = [],
  idfVersion,
  installFolder,
}) {
  describe(`${id}- EIM deactivation lifecycle |`, function () {
    this.timeout(3600000);

    let testRunner = null;
    const scriptPaths = getScriptPaths(installFolder, idfVersion);

    before(async function () {
      this.timeout(3600000);
      logger.info(
        `Starting deactivation lifecycle test for IDF ${idfVersion} with args ${args.join(" ")}`
      );
      // Make sure the activate/deactivate scripts don't exist from a
      // previous run before we start the install.
      for (const p of [scriptPaths.activate, scriptPaths.deactivate]) {
        if (fs.existsSync(p)) {
          fs.rmSync(p, { force: true });
        }
      }
      testRunner = new CLITestRunner();
      await testRunner.start();
    });

    afterEach(function () {
      if (this.currentTest.state === "failed") {
        logger.info(`Test failed: ${this.currentTest.title}`);
        if (testRunner) {
          logger.info(
            `Terminal output: >>\r ${testRunner.output.slice(-2000)}`
          );
        }
      }
    });

    after(async function () {
      this.timeout(50000);
      try {
        if (testRunner) await testRunner.stop();
      } catch (error) {
        logger.info("Error cleaning up terminal");
        logger.info(`  ${error}`);
      } finally {
        testRunner = null;
      }
    });

    it("1- Should install IDF using specified parameters", async function () {
      this.timeout(3600000);
      logger.info(`Installing IDF ${idfVersion} for deactivation test`);
      testRunner.callEIM(pathToEIM, ["install", ...args]);
      // Wait for the install to finish the same way CLICustomInstall does.
      const startTime = Date.now();
      let installed = false;
      while (Date.now() - startTime < 3600000) {
        if (Date.now() - testRunner.lastDataTimestamp >= 600000) {
          logger.info("Exited due to idle terminal");
          break;
        }
        if (await testRunner.waitForOutput("panicked", 1000)) {
          logger.info("Rust app failure");
          break;
        }
        if (
          await testRunner.waitForOutput("Now you can start using IDF tools", 1000)
        ) {
          installed = true;
          break;
        }
        await new Promise((resolve) => setTimeout(resolve, 1000));
      }
      expect(installed, "Installation did not complete").to.be.true;
    });

    it("2- Should create both activate and deactivate scripts on disk", async function () {
      logger.info(
        `Verifying deactivation script at ${scriptPaths.deactivate}`
      );
      expect(
        fs.existsSync(scriptPaths.activate),
        `Activate script not found at ${scriptPaths.activate}`
      ).to.be.true;
      expect(
        fs.existsSync(scriptPaths.deactivate),
        `Deactivate script not found at ${scriptPaths.deactivate}`
      ).to.be.true;
      // Deactivate script must not be empty.
      const stat = fs.statSync(scriptPaths.deactivate);
      expect(stat.size, "Deactivate script is empty").to.be.greaterThan(0);
    });

    it("3- Sourcing deactivate should unset IDF env vars", async function () {
      // Use a fresh terminal so the source state is predictable.
      try {
        await testRunner.stop();
      } catch {
        /* already stopped */
      }
      testRunner = new CLITestRunner();
      await testRunner.start();

      const isWin = os.platform() === "win32";
      const envProbeFormat = isWin
        ? null
        : `printf 'IDF_PATH=%s||IDF_TOOLS_PATH=%s||ESP_IDF_VERSION=%s||VIRTUAL_ENV=%s||\\n' "$IDF_PATH" "$IDF_TOOLS_PATH" "$ESP_IDF_VERSION" "$VIRTUAL_ENV"`;
      const envProbeWriteHost = isWin
        ? `Write-Host "IDF_PATH=$env:IDF_PATH||"; Write-Host "IDF_TOOLS_PATH=$env:IDF_TOOLS_PATH||"; Write-Host "ESP_IDF_VERSION=$env:ESP_IDF_VERSION||"; Write-Host "VIRTUAL_ENV=$env:VIRTUAL_ENV||"`
        : null;


      const sourceCmd = isWin
        ? `. "${scriptPaths.activate}"`
        : `source "${scriptPaths.activate}"`;
      const activateCombined = isWin
        ? `${sourceCmd}; ${envProbeWriteHost}`
        : `${sourceCmd} && ${envProbeFormat}`;
      const activateCheck = await testRunner.runAndCapture(
        activateCombined,
        "ACTIVATE_DONE",
        60000
      );

      expect(
        activateCheck.output,
        `Activation script did not produce expected banner. Output: ${activateCheck.output}`
      ).to.match(/Environment setup complete|IDF PowerShell Environment/);

      const activateMarker = isWin
        ? "IDF PowerShell Environment"
        : "Environment setup complete";
      const postActivateOutput = extractProbeOutput(
        activateCheck.output,
        activateMarker
      );
      const idfToolsPathMatch = postActivateOutput.match(
        /IDF_TOOLS_PATH=(?!%s)([^|]*)/
      );
      expect(
        idfToolsPathMatch && idfToolsPathMatch[1].length > 0,
        `IDF_TOOLS_PATH should be non-empty after activation. Output: ${postActivateOutput}`
      ).to.be.true;

      // Now source the deactivate script and re-check the same vars,
      // again in a single command for the same reasons as above.
      const deactCmd = isWin
        ? `. "${scriptPaths.deactivate}"`
        : `source "${scriptPaths.deactivate}"`;
      const deactCombined = isWin
        ? `${deactCmd}; ${envProbeWriteHost}`
        : `${deactCmd} && ${envProbeFormat}`;
      const deactCheck = await testRunner.runAndCapture(
        deactCombined,
        "POST_DONE",
        30000
      );
      // The deactivate script prints "ESP-IDF environment
      // deactivated." to stdout. It appears in the captured buffer
      // alongside the probe output.
      expect(
        deactCheck.output,
        `Deactivation script did not finish. Output: ${deactCheck.output}`
      ).to.include("ESP-IDF environment deactivated.");

      const postDeactOutput = extractProbeOutput(
        deactCheck.output,
        "ESP-IDF environment deactivated."
      );
      for (const varName of [
        "IDF_PATH",
        "IDF_TOOLS_PATH",
        "VIRTUAL_ENV",
      ]) {
        const re = new RegExp(`${varName}=(?!%s)([^|]*)`);
        const m = postDeactOutput.match(re);
        const value = m ? m[1] : null;
        expect(
          !value || value.length === 0,
          `After deactivation, ${varName} should be empty, got '${value}'.\nOutput: ${postDeactOutput}`
        ).to.be.true;
      }
      const espMatch = postDeactOutput.match(
        /ESP_IDF_VERSION=(?!%s)([^|]*)/
      );
      const espValue = espMatch ? espMatch[1] : null;
      expect(
        !espValue || !/^\d/.test(espValue),
        `After deactivation, ESP_IDF_VERSION should be empty, got '${espValue}'.\nOutput: ${postDeactOutput}`
      ).to.be.true;
    });

    it("4- eim remove should delete both activate and deactivate scripts", async function () {
      this.timeout(120000);
      try {
        await testRunner.stop();
      } catch {
        /* already stopped */
      }
      testRunner = new CLITestRunner();
      await testRunner.start();

      testRunner.callEIM(pathToEIM, ["remove", idfVersion]);
      const removed = await testRunner.waitForOutput(
        `Removed version: ${idfVersion}`,
        60000
      );
      expect(removed, "eim remove did not report success").to.be.true;

      // Give the OS a moment to settle on the file deletion.
      await new Promise((resolve) => setTimeout(resolve, 2000));

      expect(
        fs.existsSync(scriptPaths.activate),
        `Activate script should be gone after remove: ${scriptPaths.activate}`
      ).to.be.false;
      expect(
        fs.existsSync(scriptPaths.deactivate),
        `Deactivate script should be gone after remove: ${scriptPaths.deactivate}`
      ).to.be.false;
    });
  });
}
