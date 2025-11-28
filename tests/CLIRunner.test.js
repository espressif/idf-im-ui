/**
 * Test runner to execute test scripts based on entries of given json file.
 * Entries should follow this format:
 *     {
        "id": <number>,                 // an ID to correlate with the test report
        "type": "custom",               // test type is either "prerequisites", "arguments", "default", "custom" or "offline"
        "name": "<name>",               // A name for the test to correlate to logs and report
        "data": {                       // Only required for custom test type
            "targetList": "esp32s2",    // Which targets to install "esp32|esp32c6"
            "idfList": "v5.3.2",        // Which IDF version to install "v5.4|v.5.3.2"
            "installFolder": "<folder>" // Folder name to install idf (inside USER folder)
            "idfMirror": "github",      // Mirror to download IDF "github" or "jihulab"
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
import { runCleanUp } from "./scripts/cleanUpRunner.test.js";
import logger from "./classes/logger.class.js";
import {
  IDFMIRRORS,
  TOOLSMIRRORS,
  PYPIMIRRORS,
  IDFDefaultVersion,
  EIMCLIVersion,
  pathToEIMCLI,
  INSTALLFOLDER,
  TOOLSFOLDER,
  runInDebug,
  pkgName,
} from "./config.js";
import os from "os";
import path from "path";
import fs from "fs";

const jsonFilePath = path.join(
  import.meta.dirname,
  "suites",
  `${process.env.JSON_FILENAME}.json`
);
const testScript = JSON.parse(fs.readFileSync(jsonFilePath, "utf-8"));
logger.info(`Running test script: ${jsonFilePath}`);

testRun(testScript);

function testRun(jsonScript) {
  // Test Runs
  jsonScript.forEach((test) => {
    if (test.type === "prerequisites") {
      //route for prerequisites tests

      describe(`Test${test.id}- ${test.name} |`, function () {
        this.timeout(20000);

        runCLIPrerequisitesTest({ id: `${test.id}1`, pathToEIM: pathToEIMCLI });
      });
    } else if (test.type === "arguments") {
      //routine for arguments tests

      describe(`Test${test.id}- ${test.name} |`, function () {
        this.timeout(20000);

        runCLIArgumentsTest({
          id: `${test.id}1`,
          pathToEIM: pathToEIMCLI,
          eimVersion: EIMCLIVersion,
        });
      });
    } else if (test.type === "default") {
      //routine for default installation tests

      const deleteAfterTest = test.deleteAfterTest ?? true;
      const testProxyMode = test.testProxyMode ?? false;
      const proxyBlockList = test.proxyBlockList ?? [];

      describe(`Test${test.id}- ${test.name} |`, function () {
        this.timeout(6000000);

        runCLIWizardInstallTest({
          id: `${test.id}1`,
          pathToEIM: pathToEIMCLI,
          testProxyMode,
          proxyBlockList,
        });

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
      });
    } else if (test.type === "custom") {
      //routine for custom installation tests
      let installFolder = test.data.installFolder
        ? path.join(os.homedir(), test.data.installFolder)
        : INSTALLFOLDER;

      const targetList = test.data.targetList
        ? test.data.targetList.split("|")
        : ["esp32"];

      const idfVersionList = test.data.idfList
        ? test.data.idfList.split("|")
        : [IDFDefaultVersion];

      const idfUpdatedList = idfVersionList.map((idf) =>
        idf === "default" ? IDFDefaultVersion : idf
      );

      let installArgs = [];

      runInDebug && installArgs.push("-vvv");

      test.data.installFolder && installArgs.push(`-p ${installFolder}`);

      test.data.targetList && installArgs.push(`-t ${targetList.join(",")}`);

      test.data.idfList && installArgs.push(`-i ${idfUpdatedList.join(",")}`);

      test.data.toolsMirror &&
        installArgs.push(
          `-m ${TOOLSMIRRORS[test.data.toolsMirror] || "https://github.com"}`
        );

      test.data.idfMirror &&
        installArgs.push(
          `--idf-mirror ${
            IDFMIRRORS[test.data.idfMirror] || "https://github.com"
          }`
        );

      test.data.pypiMirror &&
        installArgs.push(
          `--pypi-mirror ${
            PYPIMIRRORS[test.data.pypiMirror] || "https://pypi.org/simple"
          }`
        );

      test.data.recursive && installArgs.push(`-r ${test.data.recursive}`);

      test.data.nonInteractive &&
        installArgs.push(`-n ${test.data.nonInteractive}`);

      const deleteAfterTest = test.deleteAfterTest ?? true;
      const testProxyMode = test.testProxyMode ?? false;
      const proxyBlockList = test.proxyBlockList ?? [];

      describe(`Test${test.id}- ${test.name} |`, function () {
        this.timeout(6000000);

        runCLICustomInstallTest({
          id: `${test.id}1`,
          pathToEIM: pathToEIMCLI,
          args: installArgs,
          testProxyMode,
          proxyBlockList,
        });

        runInstallVerification({
          id: `${test.id}2`,
          installFolder,
          idfList: idfUpdatedList,
          targetList,
          toolsFolder: TOOLSFOLDER,
        });

        runCleanUp({
          id: `${test.id}3`,
          installFolder,
          toolsFolder: TOOLSFOLDER,
          deleteAfterTest,
        });
      });
    } else if (test.type === "version-management") {
      //routine for version management tests
      const idfVersionList = test.data.idfList
        ? test.data.idfList.split("|")
        : [IDFDefaultVersion];

      const idfUpdatedList = idfVersionList.map((idf) =>
        idf === "default" ? IDFDefaultVersion : idf
      );

      let installFolder = test.data.installFolder
        ? path.join(os.homedir(), test.data.installFolder)
        : INSTALLFOLDER;

      const deleteAfterTest = test.deleteAfterTest ?? true;

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
    } else if (test.type === "offline") {
      //routine for offline installation test
      const deleteAfterTest = test.deleteAfterTest ?? true;
      const testProxyMode = test.testProxyMode ?? "block";
      const proxyBlockList = test.proxyBlockList ?? [];

      describe(`Test${test.id}- ${test.name} |`, async function () {
        this.timeout(6000000);

        runCLICustomInstallTest({
          id: `${test.id}1`,
          pathToEIM: pathToEIMCLI,
          offlineIDFVersion: IDFDefaultVersion,
          offlinePkgName: pkgName,
          testProxyMode,
          proxyBlockList,
        });

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
      });
    } else {
      logger.error(`Unknown test type: ${test.type}`);
    }
  });
}
