import { expect } from "chai";
import { describe, it, before, after, afterEach } from "mocha";
import CLITestRunner from "../classes/CLITestRunner.class.js";
import {
  IDFMIRRORS,
  TOOLSMIRRORS,
  PYPIMIRRORS,
  IDFAvailableVersions,
  availableTargets,
  runInDebug,
} from "../config.js";
import { getAvailableFeatures, getAvailableTools } from "../helper.js";
import { startTestProxy, stopTestProxy } from "../helpers/testProxy.js";
import {
  startTerminal,
  stopTerminal,
  logFailedTest,
  waitForTerminalOutput,
} from "../helpers/cliTestHelpers.js";
import logger from "../classes/logger.class.js";
import os from "os";

// This function executes the wizard installation functionality of the EIM CLI
export function runCLIWizardInstallTest({
  id = 0,
  pathToEIM,
  testProxyMode = false,
  proxyBlockList = [],
}) {
  describe(`${id}- Run wizard |`, function () {
    let testRunner = null;
    let installationFailed = false;
    let proxy = null;

    // The setup function should start the proxy server if enabled and start the terminal
    before(async function () {
      logger.debug(`Starting installation wizard with default options`);
      this.timeout(5000);
      testRunner = new CLITestRunner();
      if (testProxyMode) {
        proxy = await startTestProxy(testProxyMode, proxyBlockList);
      }
      await startTerminal(testRunner);
    });

    // The afterEach function should log the terminal output on failure
    afterEach(function () {
      if (this.currentTest.state === "failed") {
        logFailedTest(this.currentTest, testRunner, { tail: 1000 });
        installationFailed = true;
      }
    });

    // The tear down function should stop the terminal and proxy server if enabled
    after(async function () {
      logger.info("Installation wizard test cleanup");
      this.timeout(20000);
      await stopTerminal(testRunner);
      testRunner = null;
      if (testProxyMode) {
        await stopTestProxy(proxy);
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
      testRunner.callEIM(
        pathToEIM,
        runInDebug ? ["-vvv", "wizard"] : ["wizard"],
      );
      if (os.platform() === "win32") {
        if(await testRunner.waitForOutput("Do you want to install Python?", 15000)) {
          testRunner.process.write("y");
          logger.error("EIM not supposed to ask for python installation")
        }
      }
      const selectTargetQuestion = await testRunner.waitForOutput(
        "Please select all of the target platforms",
        30000,
      );
      expect(selectTargetQuestion, "Failed to ask for installation targets").to
        .be.true;

      await new Promise((resolve) => setTimeout(resolve, 500));

      for (let target of availableTargets) {
        expect(
          testRunner.output,
          `Failed to offer installation for target '${target}'`,
        ).to.include(target);
      }

      expect(
        testRunner.output,
        "Failed to offer installation for 'all' targets",
      ).to.include("all");

      logger.info("Select Target Passed");
      testRunner.output = "";
      testRunner.sendInput("");
      await new Promise((resolve) => setTimeout(resolve, 500));

      const selectIDFVersion = await testRunner.waitForOutput(
        "Please select the desired ESP-IDF version",
      );
      expect(selectIDFVersion, "Failed to ask for IDF version").to.be.true;

      await new Promise((resolve) => setTimeout(resolve, 500));

      let IDFAvailableVersionsFlat = Object.values(IDFAvailableVersions).flat();
      for (let version of IDFAvailableVersionsFlat) {
        expect(
          testRunner.output,
          `Failed to offer installation for IDF version '${version}'`,
        ).to.include(version);
      }

      logger.info("Select IDF Version passed");
      testRunner.output = "";
      testRunner.sendInput("");
      await new Promise((resolve) => setTimeout(resolve, 500));

      const selectIDFMirror = await testRunner.waitForOutput(
        "Select the source from which to download ESP-IDF",
      );
      expect(selectIDFMirror, "Failed to ask for IDF download mirrors").to.be
        .true;

      await new Promise((resolve) => setTimeout(resolve, 500));

      for (let mirror of Object.values(IDFMIRRORS)) {
        expect(
          testRunner.output,
          `Failed to offer ${mirror} as a download mirror option`,
        ).to.include(mirror);
      }

      logger.info("Select IDF mirror passed");

      testRunner.output = "";
      testRunner.sendInput("");
      await new Promise((resolve) => setTimeout(resolve, 500));

      const selectToolsMirror = await testRunner.waitForOutput(
        "Select a source from which to download tools",
      );
      expect(selectToolsMirror, "Failed to ask for tools download mirror").to.be
        .true;

      await new Promise((resolve) => setTimeout(resolve, 500));

      for (let mirror of Object.values(TOOLSMIRRORS)) {
        expect(
          testRunner.output,
          `Failed to offer ${mirror} as a tool mirror option`,
        ).to.include(mirror);
      }

      logger.info("Select tools mirror passed");
      testRunner.output = "";
      testRunner.sendInput("");
      await new Promise((resolve) => setTimeout(resolve, 500));

      const selectPyPIMirror = await testRunner.waitForOutput(
        "Select a PyPI mirror to download Python packages",
      );
      expect(selectPyPIMirror, "Failed to ask for PyPI download mirror").to.be
        .true;

      await new Promise((resolve) => setTimeout(resolve, 500));

      for (let mirror of Object.values(PYPIMIRRORS)) {
        expect(
          testRunner.output,
          `Failed to offer ${mirror} as a PyPI mirror option`,
        ).to.include(mirror);
      }

      logger.info("Select pypi mirror passed");

      testRunner.output = "";
      testRunner.sendInput("");
      await new Promise((resolve) => setTimeout(resolve, 500));

      const selectInstallPath = await testRunner.waitForOutput(
        "Please select the ESP-IDF installation location",
      );
      expect(selectInstallPath, "Failed to ask for installation path").to.be
        .true;

      await new Promise((resolve) => setTimeout(resolve, 500));

      const defaultPath =
        os.platform() === "win32"
          ? "(C:\\esp)"
          : `(${os.homedir()}/.espressif)`;
      expect(
        testRunner.output,
        "Failed to provide default installation path",
      ).to.include(defaultPath);

      logger.info("Select install path passed");

      testRunner.output = "";
      testRunner.sendInput("");
      await new Promise((resolve) => setTimeout(resolve, 500));

      const selectFeatures = await testRunner.waitForOutput(
        "Select ESP-IDF features to install",
      );
      expect(selectFeatures, "Failed to ask for ESP-IDF features").to.be.true;

      await new Promise((resolve) => setTimeout(resolve, 500));

      const availableFeatures = await getAvailableFeatures();
      for (let feature of availableFeatures) {
        expect(
          testRunner.output,
          `Failed to show ${feature} as available ESP-IDF feature`,
        ).to.include(feature);
      }

      logger.info("Select ESP-IDF feature passed");

      testRunner.output = "";
      testRunner.sendInput("");
      await new Promise((resolve) => setTimeout(resolve, 500));

      const selectTools = await testRunner.waitForOutput(
        "Select additional tools to install",
      );
      expect(selectTools, "Failed to ask for additional tools").to.be.true;

      await new Promise((resolve) => setTimeout(resolve, 500));

      const availableTools = await getAvailableTools();
      for (let tool of availableTools) {
        expect(
          testRunner.output,
          `Failed to show ${tool} as available additional tool`,
        ).to.include(tool);
      }

      logger.info("Select additional tools passed");

      testRunner.output = "";
      testRunner.sendInput("");
      await new Promise((resolve) => setTimeout(resolve, 5000));

      await waitForTerminalOutput(
        testRunner,
        "Do you want to save the installer configuration",
        { pollMs: 500 }
      );

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
