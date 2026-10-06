/**
 * Test runner to execute test scripts based on entries of given json file.
 * Entries should follow this format:
 *     {
        "id": <number>,                 // an ID to correlate with the test report
        "type": "custom",               // test type is either "prerequisites", "arguments", "default", "custom", "deactivation" or "offline"
        "name": "<name>",               // A name for the test to correlate to logs and report
        "data": {                       // Only required for custom test type
            "targetList": "esp32s2",    // Which targets to install "esp32|esp32c6"
            "idfList": "v5.3.2",        // Which IDF version to install "v5.4|v.5.3.2"
            "installFolder": "<folder>" // Folder name to install idf (inside USER folder)
            "idfMirror": "github",      // Mirror to download IDF "github" or "gitmirror"
            "toolsMirror": "github",    // Mirror to download tools "github", "dl_com" or "dl_cn"
            "pypiMirror": "pypi_org",   // Mirror to download python packages "pypi_org", "pypi_aliyun", "pypi_tsinghua", "pypi_ustc"
            "recursive": false,         // Whether to prevent downloading submodules (set to true if omitted)
            "nonInteractive": false     // Whether to prevent running in non-interactive mode (set to true if omitted)
        },
        "deleteAfterTest": true         // Whether to remove IDF installation folder and IDF tools folder after test
        "testProxyMode": "block"        // If the test run with local proxy to log or block internet access during test : "block", "block-list", "log", false
        "proxyBlockList": []            // List of domains to block when testProxyMode is set to "block-list"


 */

import { describe } from "mocha";
import { runCLIPrerequisitesTest } from "./scripts/CLIPrerequisites.test.js";
import { runCLIArgumentsTest } from "./scripts/CLIArguments.test.js";
import { runCLIWizardInstallTest } from "./scripts/CLIWizardInstall.test.js";
import { runCLICustomInstallTest } from "./scripts/CLICustomInstall.test.js";
import { runInstallVerification } from "./scripts/installationVerification.test.js";
import { runVersionManagementTest } from "./scripts/CLIVersionManagement.test.js";
import { runCLIPythonCheckTest } from "./scripts/CLIPythonCheck.test.js";
import { runCleanUp } from "./scripts/cleanUpRunner.test.js";
import { runCLIClonedIDFRepo } from "./scripts/CLICloneIDFRepo.test.js";
import { runCLIDeactivationTest } from "./scripts/CLIDeactivation.test.js";
import { runListToolsTest } from "./scripts/CLIListTools.test.js";
import { runListFeaturesTest } from "./scripts/CLIListFeatures.test.js";
import { runInstallationStatusTest } from "./scripts/CLIInstallationStatus.test.js";
import { runCLINamedVersionInstallTest } from "./scripts/CLINamedVersionInstall.test.js";
import logger from "./classes/logger.class.js";
import {
  IDFDefaultVersion,
  EIMCLIVersion,
  pathToEIMCLI,
  INSTALLFOLDER,
  TOOLSFOLDER,
  runInDebug,
  prerequisites,
  resolveIdfToken,
} from "./config.js";
import {
  loadTestSuite,
  resolveInstallFolder,
  parseTargetList,
  resolveIdfList,
  getCleanupAndProxyOptions,
  getCLIInstallSettings,
} from "./helpers/suiteSettings.js";
import os from "os";

// Read the test script file from the suites folder
const testScript = loadTestSuite();

// Run the tests
testRun(testScript);

// Install with `eim install <args>`, run the given checks as step 2, then clean up as step 3
function describeInstallThen(test, settings, runChecks) {
  const { installFolder, installArgs, testProxyMode, proxyBlockList, deleteAfterTest } =
    settings;
  describe(`Test${test.id}- ${test.name} |`, function () {
    this.timeout(6000000);

    runCLICustomInstallTest({
      id: `${test.id}1`,
      pathToEIM: pathToEIMCLI,
      args: installArgs,
      testProxyMode,
      proxyBlockList,
    });

    runChecks();

    runCleanUp({
      id: `${test.id}3`,
      installFolder,
      toolsFolder: TOOLSFOLDER,
      deleteAfterTest,
    });
  });
}

// Steps 2 and 3 for installs of the default IDF version into the default folder
function runDefaultFolderVerificationAndCleanUp(test, deleteAfterTest) {
  runInstallVerification({
    id: `${test.id}2`,
    installFolder: INSTALLFOLDER,
    idfList: [IDFDefaultVersion],
    toolsFolder: TOOLSFOLDER,
  });

  runCleanUp({
    id: `${test.id}3`,
    installFolder: INSTALLFOLDER,
    toolsFolder: TOOLSFOLDER,
    deleteAfterTest,
  });
}

function testRun(jsonScript) {
  // Test Runs
  jsonScript.forEach((test) => {
    if (test.type === "prerequisites") {
      //routine for prerequisites tests

      describe(`Test${test.id}- ${test.name} |`, function () {
        this.timeout(250000);

        runCLIPrerequisitesTest({ id: `${test.id}1`, pathToEIM: pathToEIMCLI, prerequisites });
      });

    } else if (test.type === "pythoncheck") {
      //routine for python check tests

      describe(`Test${test.id}- ${test.name} |`, function () {
        this.timeout(100000);

        runCLIPythonCheckTest({ id: `${test.id}1`, pathToEIM: pathToEIMCLI});
      });
    } else if (test.type === "arguments") {
      //routine for arguments tests

      describe(`Test${test.id}- ${test.name} |`, function () {
        this.timeout(500000);

        runCLIArgumentsTest({
          id: `${test.id}1`,
          pathToEIM: pathToEIMCLI,
          eimVersion: EIMCLIVersion,
        });
      });
    } else if (test.type === "default") {
      //routine for default installation tests

      const { deleteAfterTest, testProxyMode, proxyBlockList } =
        getCleanupAndProxyOptions(test);

      describe(`Test${test.id}- ${test.name} |`, function () {
        this.timeout(6000000);

        runCLIWizardInstallTest({
          id: `${test.id}1`,
          pathToEIM: pathToEIMCLI,
          testProxyMode,
          proxyBlockList,
        });

        runDefaultFolderVerificationAndCleanUp(test, deleteAfterTest);
      });
    } else if (test.type === "custom") {
      //routine for custom installation tests

      const settings = getCLIInstallSettings(test);

      describeInstallThen(test, settings, () => {
        runInstallVerification({
          id: `${test.id}2`,
          installFolder: settings.installFolder,
          idfList: settings.idfList,
          targetList: settings.targetList,
          toolsFolder: TOOLSFOLDER,
        });
      });
    } else if (test.type === "version-management") {
      //routine for version management tests

      const deleteAfterTest = test.deleteAfterTest ?? true;
      const idfUpdatedList = resolveIdfList(test.data);
      const installFolder = resolveInstallFolder(test.data);

      describe(`Test${test.id}- ${test.name} |`, function () {
        this.timeout(60000);

        runVersionManagementTest({
          id: `${test.id}1`,
          pathToEIM: pathToEIMCLI,
          idfList: idfUpdatedList,
          installFolder,
        });

        runCleanUp({
          id: `${test.id}3`,
          installFolder,
          toolsFolder: TOOLSFOLDER,
          deleteAfterTest,
        });
      });
    } else if (test.type === "deactivation") {
      //routine for deactivation script lifecycle tests. Mirrors the
      //shape of the "custom" type: builds CLI args from `data`, then
      //runs the deactivation scenario. The scenario does the install,
      //verifies the deactivate script, sources activate+deactivate,
      //and finally `eim remove`s the version. The runner then runs
      //the standard clean-up to delete the install folder.

      const { deleteAfterTest, installFolder, idfList, installArgs } =
        getCLIInstallSettings(test);

      describe(`Test${test.id}- ${test.name} |`, function () {
        this.timeout(6000000);

        runCLIDeactivationTest({
          id: `${test.id}1`,
          pathToEIM: pathToEIMCLI,
          args: installArgs,
          idfVersion: idfList[0],
          installFolder,
        });

        // The deactivation scenario ends with `eim remove` (step 4 of
        // runCLIDeactivationTest), which deletes the IDF entry from
        // eim_idf.json. runInstallVerification reads eim_idf.json and
        // expects the entry to still be there, so it can only run
        // BEFORE the deactivation lifecycle -- and the lifecycle
        // itself already verifies the install layout (script
        // existence, activation, deactivation, removal). Skipping the
        // standalone verification step avoids an unavoidable failure
        // for the deactivation test type.
        //
        // Clean up still runs: runCLIDeactivationTest already removed
        // the esp-idf subfolder via `eim remove`; CleanUp removes
        // the parent directory and the tools folder when
        // deleteAfterTest is true.
        runCleanUp({
          id: `${test.id}2`,
          installFolder,
          toolsFolder: TOOLSFOLDER,
          deleteAfterTest,
        });
      });
    } else if (test.type === "offline") {
      //routine for offline installation test

      const { deleteAfterTest, testProxyMode, proxyBlockList } =
        getCleanupAndProxyOptions(test, "block");

      describe(`Test${test.id}- ${test.name} |`, async function () {
        this.timeout(6000000);

        runCLICustomInstallTest({
          id: `${test.id}1`,
          pathToEIM: pathToEIMCLI,
          offlineIDFVersion: IDFDefaultVersion,
          testProxyMode,
          proxyBlockList,
        });

        runDefaultFolderVerificationAndCleanUp(test, deleteAfterTest);
      });
    } else if (test.type === "existing-git-clone") {
      const gitRepoUrl = test.data.gitRepoUrl ?? "https://github.com/espressif/esp-idf.git";
      const gitRepoBranch = test.data.gitRepoBranch
        ? resolveIdfToken(test.data.gitRepoBranch)
        : IDFDefaultVersion;
      const installFolder = resolveInstallFolder(test.data);
      const targetList = parseTargetList(test.data, ["esp32"]);
      const { deleteAfterTest, testProxyMode, proxyBlockList } =
        getCleanupAndProxyOptions(test);

      const installArgs = [];
      runInDebug && installArgs.push("-vvv");
      installArgs.push(
        os.platform() === "win32" ? `-p "${installFolder}"` : `-p ${installFolder}`
      );

      describe(`Test${test.id}- ${test.name} |`, function () {
        this.timeout(6000000);

        runCLIClonedIDFRepo({
          id: `${test.id}1`,
          path: installFolder,
          gitRepoUrl,
          gitRepoBranch,
        });

        runCLICustomInstallTest({
          id: `${test.id}2`,
          pathToEIM: pathToEIMCLI,
          args: installArgs,
          testProxyMode,
          proxyBlockList,
        });

        runInstallVerification({
          id: `${test.id}3`,
          installFolder,
          idfList: [gitRepoBranch],
          targetList,
          toolsFolder: TOOLSFOLDER,
          existingGitClone: true,
        });

        runCleanUp({
          id: `${test.id}4`,
          installFolder,
          toolsFolder: TOOLSFOLDER,
          deleteAfterTest,
        });
      });
    } else if (test.type === "list-tools") {
      const settings = getCLIInstallSettings(test);

      describeInstallThen(test, settings, () => {
        runListToolsTest({
          id: `${test.id}2`,
          pathToEIM: pathToEIMCLI,
          idfList: settings.idfList,
          installFolder: settings.installFolder,
        });
      });
    } else if (test.type === "list-features") {
      const settings = getCLIInstallSettings(test);

      describeInstallThen(test, settings, () => {
        runListFeaturesTest({
          id: `${test.id}2`,
          pathToEIM: pathToEIMCLI,
          idfList: settings.idfList,
          installFolder: settings.installFolder,
        });
      });
    } else if (test.type === "installation-status") {
      // Tests for eim_idf.json status tracking and interactive dialog status labels

      const settings = getCLIInstallSettings(test);

      describeInstallThen(test, settings, () => {
        runInstallationStatusTest({
          id: `${test.id}2`,
          pathToEIM: pathToEIMCLI,
          idfList: settings.idfList,
          installFolder: settings.installFolder,
          toolsFolder: TOOLSFOLDER,
        });
      });
    } else if (test.type === "named-version-install") {
      // Regression test for installing an additional named IDF entry that
      // points at the same `--idf-path` as an already-installed IDF.
      // Before the fix, the CLI aborted with the "already installed"
      // short-circuit even when `--version-name` was provided. The new
      // branch must register a second `eim_idf.json` entry while the
      // path is reused as-is.

      const {
        deleteAfterTest,
        testProxyMode,
        proxyBlockList,
        installFolder,
        idfList,
        installArgs,
      } = getCLIInstallSettings(test, { quoteInstallPathOnWindows: true });

      const namedVersion =
        test.data["version-name"] || test.data.versionName || "named-idf";

      describe(`Test${test.id}- ${test.name} |`, function () {
        this.timeout(7200000);

        runCLINamedVersionInstallTest({
          id: `${test.id}1`,
          pathToEIM: pathToEIMCLI,
          args: installArgs,
          idfVersion: idfList[0],
          installFolder,
          namedVersion,
          testProxyMode,
          proxyBlockList,
        });

        runCleanUp({
          id: `${test.id}2`,
          installFolder,
          toolsFolder: TOOLSFOLDER,
          deleteAfterTest,
        });
      });
    } else {
      //log an error if the test type is unknown
      logger.error(`Unknown test type: ${test.type}`);
    }
  });
}
