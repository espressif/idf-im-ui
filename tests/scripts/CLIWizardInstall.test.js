import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import {
  IDFMIRRORS,
  TOOLSMIRRORS,
  IDFAvailableVersions,
  availableTargets,
  runInDebug,
} from "../config.js";
import TestProxy from "../classes/TestProxy.class.js";
import logger from "../classes/logger.class.js";
import os from "os";

export function runCLIWizardInstallTest({
  id = 0,
  pathToEim,
  testProxyMode = false,
}) {
  describe(`${id}- Run wizard |`, function () {
    let testRunner = null;
    let installationFailed = false;
    let proxy = null;

    before(async function () {
      logger.debug(`Starting installation wizard with default options`);
      this.timeout(5000);
      testRunner = new CLITestRunner();
      if (testProxyMode) {
        try {
          proxy = new TestProxy({ mode: testProxyMode });
          await proxy.start();
        } catch (error) {
          logger.info("Error to start proxy server");
          logger.debug(`Error: ${error}`);
        }
      }
      try {
        await testRunner.start({
          isolatedEnvironment: testProxyMode === false ? false : true,
        });
      } catch (error) {
        logger.info("Error to start terminal");
        logger.debug(`Error: ${error}`);
      }
    });

    afterEach(function () {
      if (this.currentTest.state === "failed") {
        logger.info(`Test failed: ${this.currentTest.title}`);
        logger.info(`Terminal output: >>${testRunner.output.slice(-1000)}`);
        logger.debug(`Terminal output on failure: >>${testRunner.output}`);
        installationFailed = true;
      }
    });

    after(async function () {
      logger.info("Installation wizard test cleanup");
      this.timeout(20000);
      try {
        await testRunner.stop();
      } catch (error) {
        logger.info("Error to clean up terminal after test");
        logger.info(`${error}`);
      } finally {
        testRunner = null;
      }
      try {
        await proxy.stop();
      } catch (error) {
        logger.info("Error stopping proxy server");
        logger.info(`${error}`);
      }
    });

    /** Run install wizard
     *
     * It is expected to have all requirements installed
     * The step to install the prerequisites in windows is not tested
     *
     */

    it("1- Should install IDF using wizard and default values", async function () {
      logger.info(`Starting test - IDF installation wizard`);
      this.timeout(3660000);
      testRunner.sendInput(`${pathToEim} ${runInDebug ? "-vvv " : ""}wizard`);
      const selectTargetQuestion = await testRunner.waitForOutput(
        "Please select all of the target platforms",
        20000
      );
      expect(selectTargetQuestion, "Failed to ask for installation targets").to
        .be.true;

      for (let target of availableTargets) {
        expect(
          testRunner.output,
          `Failed to offer installation for target '${target}'`
        ).to.include(target);
      }

      expect(
        testRunner.output,
        "Failed to offer installation for 'all' targets"
      ).to.include("all");

      logger.info("Select Target Passed");
      testRunner.output = "";
      testRunner.sendInput("");

      const selectIDFVersion = await testRunner.waitForOutput(
        "Please select the desired ESP-IDF version"
      );
      expect(selectIDFVersion, "Failed to ask for IDF version").to.be.true;

      for (let version of IDFAvailableVersions) {
        expect(
          testRunner.output,
          `Failed to offer installation for IDF version '${version}'`
        ).to.include(version);
      }

      logger.info("Select IDF Version passed");
      testRunner.output = "";
      testRunner.sendInput("");

      const selectIDFMirror = await testRunner.waitForOutput(
        "Select the source from which to download esp-idf"
      );
      expect(selectIDFMirror, "Failed to ask for IDF download mirrors").to.be
        .true;

      for (let mirror of Object.values(IDFMIRRORS)) {
        expect(
          testRunner.output,
          `Failed to offer ${mirror} as a download mirror option`
        ).to.include(mirror);
      }

      logger.info("Select IDF mirror passed");
      testRunner.output = "";
      testRunner.sendInput("");

      const selectToolsMirror = await testRunner.waitForOutput(
        "Select a source from which to download tools"
      );
      expect(selectToolsMirror, "Failed to ask for tools download mirror").to.be
        .true;

      for (let mirror of Object.values(TOOLSMIRRORS)) {
        expect(
          testRunner.output,
          `Failed to offer ${mirror} as a tool mirror option`
        ).to.include(mirror);
      }

      logger.info("Select tools mirror passed");
      testRunner.output = "";
      testRunner.sendInput("");

      const selectInstallPath = await testRunner.waitForOutput(
        "Please select the ESP-IDF installation location"
      );
      expect(selectInstallPath, "Failed to ask for installation path").to.be
        .true;

      const defaultPath =
        os.platform() === "win32"
          ? "(C:\\esp)"
          : `(${os.homedir()}/.espressif)`;
      expect(
        testRunner.output,
        "Failed to provide default installation path"
      ).to.include(defaultPath);

      logger.info("Select install path passed");
      testRunner.output = "";
      testRunner.sendInput("");
      await new Promise((resolve) => setTimeout(resolve, 5000));
      const startTime = Date.now();
      while (Date.now() - startTime < 3600000) {
        if (Date.now() - testRunner.lastDataTimestamp >= 600000) {
          logger.info(">>>>>>>Exited due to Idle terminal!!!!!");
          break;
        }
        if (await testRunner.waitForOutput("panicked", 1000)) {
          logger.info(">>>>>>>Rust App failure!!!!");
          break;
        }
        if (
          await testRunner.waitForOutput(
            "Do you want to save the installer configuration",
            1000
          )
        ) {
          logger.info(">>>>>>>Completed!!!");
          break;
        }
        await new Promise((resolve) => setTimeout(resolve, 500));
      }
      if (Date.now() - startTime >= 3600000) {
        logger.info("Installation timed out after 1 hour");
      }

      expect(
        testRunner.output,
        "Failed to ask to save installation configuration - failure to install using wizard parameters"
      ).to.include("Do you want to save the installer configuration");

      expect(
        testRunner.output,
        "Error to download the tools, missing 'Downloading Tools'"
      ).to.include("Downloading tools");

      logger.info("Installation completed");
      testRunner.output = "";
      testRunner.sendInput("");

      const installationSuccessful = await testRunner.waitForOutput(
        "Successfully installed IDF"
      );
      expect(
        installationSuccessful,
        "Failed to complete installation, missing 'Successfully Installed IDF'"
      ).to.be.true;

      logger.info("installation successful");
    });
  });
}
