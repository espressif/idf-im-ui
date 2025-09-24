/**
 * Test runner to execute test scripts based on entries of given json file.
 * Entries should follow this format:
 *     {
        "id": <number>,                 // an ID to correlate with the test report
        "type": "custom",               // test type is either "startup", "default" or "custom"
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
import { runCleanUp } from "./scripts/cleanUpRunner.test.js";
import {
  IDFDefaultVersion,
  pathToEIMGUI,
  EIMGUIVersion,
  INSTALLFOLDER,
  TOOLSFOLDER,
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
      describe(`Test${test.id} - ${test.name} ->`, function () {
        this.timeout(60000);

        runGUIStartupTest({
          id: test.id,
          pathToEIM: pathToEIMGUI,
          eimVersion: EIMGUIVersion,
        });
      });
    } else if (test.type === "default") {
      //routine for default simplified installation

      const deleteAfterTest = test.deleteAfterTest ?? true;

      describe(`Test${test.id} - ${test.name} ->`, function () {
        runGUISimplifiedInstallTest(test.id, pathToEIMGUI);
        runInstallVerification({
          installFolder: INSTALLFOLDER,
          idfList: [IDFDefaultVersion],
          toolsFolder: TOOLSFOLDER,
        });

        runGUIAfterInstallTest({
          id: test.id,
          pathToEIM: pathToEIMGUI,
          idfList: [IDFDefaultVersion],
        });

        runCleanUp({
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

      const IDFMirror = test.data.idfMirror || "github";

      const deleteAfterTest = test.deleteAfterTest ?? true;

      describe(`Test${test.id} - ${test.name} ->`, function () {
        runGUICustomInstallTest(
          test.id,
          pathToEIMGUI,
          installFolder,
          targetList,
          idfUpdatedList,
          toolsMirror,
          IDFMirror
        );

        runInstallVerification({
          installFolder,
          idfList: idfUpdatedList,
          targetList,
          toolsFolder: TOOLSFOLDER,
        });

        runGUIAfterInstallTest({
          id: test.id,
          pathToEIM: pathToEIMGUI,
          idfList: idfUpdatedList,
        });

        runCleanUp({
          installFolder,
          toolsFolder: TOOLSFOLDER,
          deleteAfterTest,
        });
      });
    }
  });
}
