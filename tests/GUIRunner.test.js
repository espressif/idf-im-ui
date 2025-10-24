/**
 * Test runner to execute test scripts based on entries of given json file.
 * Entries should follow this format:
 *     {
        "id": <number>,                 // an ID to correlate with the test report
        "type": "custom",               // test type is either "startup", "default", "custom" or "offline"
        "name": "<name>",               // A name for the test to correlate to logs and report
        "data": {                       // Only required for custom test type
            "targetList": "esp32s2",    // Which targets to install "esp32|esp32c6"
            "idfList": "v5.3.2",        // Which IDF version to install "v5.4|v.5.3.2"
            "installFolder": "<folder>", // Folder name to install idf (inside USER folder)
            "idfMirror": "github",      // Mirror to download IDF "github" or "jihulab"
            "toolsMirror": "github"     // Mirror to download tools "github", "dl_com" or "dl_cn"
        }
        "deleteAfterTest": true        // Whether to remove IDF installation folder and IDF tools folder after test
        "testProxyMode": "block"            // If the test run with local proxy to log or block internet access during test : "block", "log", false

 */

import logger from "./classes/logger.class.js";
import fs from "fs";
import path from "path";
import os from "os";
import { describe } from "mocha";
import { runGUIStartupTest } from "./scripts/GUIStartup.test.js";
import { runGUISimplifiedInstallTest } from "./scripts/GUISimplifiedInstall.test.js";
import { runGUICustomInstallTest } from "./scripts/GUICustomInstall.test.js";
import { runInstallVerification } from "./scripts/installationVerification.test.js";
import { runGUIAfterInstallTest } from "./scripts/GUIAfterInstall.test.js";
import { runGUIOfflineInstallTest } from "./scripts/GUIOfflineInstall.test.js";
import { runGUIVersionManagementTest } from "./scripts/GUIVersionManagement.test.js";
import { runCleanUp } from "./scripts/cleanUpRunner.test.js";
import {
  IDFDefaultVersion,
  pathToEIMGUI,
  EIMGUIVersion,
  INSTALLFOLDER,
  TOOLSFOLDER,
  pkgName,
} from "./config.js";
logger.debug(`Filename Env variable: ${process.env.JSON_FILENAME}`);
logger.debug(`Execution folder: ${import.meta.dirname}`);

const jsonFilePath = path.join(
  import.meta.dirname,
  "suites",
  `${process.env.JSON_FILENAME}.json`
);
const testScript = JSON.parse(fs.readFileSync(jsonFilePath, "utf-8"));
logger.info(`Running test script: ${jsonFilePath}`);

testRun(testScript);

function testRun(script) {
  script.forEach((test) => {
    if (test.type === "startup") {
      //routine for startup test script
      describe(`Test${test.id}- ${test.name} |`, function () {
        this.timeout(60000);

        runGUIStartupTest({
          id: `${test.id}1`,
          pathToEIM: pathToEIMGUI,
          eimVersion: EIMGUIVersion,
        });
      });
    } else if (test.type === "default") {
      //routine for default simplified installation

      const deleteAfterTest = test.deleteAfterTest ?? true;

      describe(`Test${test.id}- ${test.name} |`, function () {
        runGUISimplifiedInstallTest({
          id: `${test.id}1`,
          pathToEIM: pathToEIMGUI,
        });

        runGUIAfterInstallTest({
          id: `${test.id}2`,
          pathToEIM: pathToEIMGUI,
          idfList: [IDFDefaultVersion],
        });

        runInstallVerification({
          id: `${test.id}3`,
          installFolder: INSTALLFOLDER,
          idfList: [IDFDefaultVersion],
          toolsFolder: TOOLSFOLDER,
        });

        runCleanUp({
          id: `${test.id}4`,
          installFolder: INSTALLFOLDER,
          toolsFolder: TOOLSFOLDER,
          deleteAfterTest,
        });
      });
    } else if (test.type === "custom") {
      //routine for expert install with custom settings

      let installFolder = test.data.installFolder
        ? path.join(os.homedir(), test.data.installFolder)
        : INSTALLFOLDER;

      const targetList = test.data.targetList
        ? test.data.targetList.split("|")
        : ["All"];

      const idfVersionList = test.data.idfList
        ? test.data.idfList.split("|")
        : [IDFDefaultVersion];

      const idfUpdatedList = idfVersionList.map((idf) =>
        idf === "default" ? IDFDefaultVersion : idf
      );

      const toolsMirror = test.data.toolsMirror || "github";

      const idfMirror = test.data.idfMirror || "github";

      const deleteAfterTest = test.deleteAfterTest ?? true;

      describe(`Test${test.id}- ${test.name} |`, function () {
        runGUICustomInstallTest({
          id: `${test.id}1`,
          pathToEIM: pathToEIMGUI,
          installFolder,
          targetList,
          idfVersionList: idfUpdatedList,
          toolsMirror,
          idfMirror,
        });

        runGUIAfterInstallTest({
          id: `${test.id}2`,
          pathToEIM: pathToEIMGUI,
          idfList: idfUpdatedList,
        });

        runInstallVerification({
          id: `${test.id}3`,
          installFolder,
          idfList: idfUpdatedList,
          targetList,
          toolsFolder: TOOLSFOLDER,
        });

        runCleanUp({
          id: `${test.id}4`,
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

      describe(`Test${test.id}- ${test.name} |`, function () {
        this.timeout(60000);

        runGUIVersionManagementTest({
          id: `${test.id}1`,
          pathToEim: pathToEIMGUI,
          idfList: idfUpdatedList,
          installFolder,
          toolsFolder: TOOLSFOLDER,
        });
      });
    } else if (test.type === "offline") {
      //routine for offline installation test

      let installFolder = test.data.installFolder
        ? path.join(os.homedir(), test.data.installFolder)
        : INSTALLFOLDER;

      const deleteAfterTest = test.deleteAfterTest ?? true;

      describe(`Test${test.id}- ${test.name} |`, function () {
        this.timeout(6000000);

        runGUIOfflineInstallTest({
          id: `${test.id}1`,
          pathToEIM: pathToEIMGUI,
          offlineIDFVersion: IDFDefaultVersion,
          offlinePkgName: pkgName,
        });

        runGUIAfterInstallTest({
          id: `${test.id}2`,
          pathToEIM: pathToEIMGUI,
          idfList: [IDFDefaultVersion],
        });

        runInstallVerification({
          id: `${test.id}3`,
          installFolder,
          idfList: [IDFDefaultVersion],
          toolsFolder: TOOLSFOLDER,
        });

        runCleanUp({
          id: `${test.id}4`,
          installFolder,
          toolsFolder: TOOLSFOLDER,
          deleteAfterTest,
        });
      });
    } else {
      logger.error(`Unknown test type: ${test.type}`);
    }
  });
}
